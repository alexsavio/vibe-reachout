# Research: Telegram Permission Hook

## R1: Claude Code Hook API (PermissionRequest)

**Decision**: Use the PermissionRequest hook event with JSON stdin/stdout protocol.

**Rationale**: Claude Code's hook system provides exactly the lifecycle point needed. PermissionRequest fires when the terminal would show a permission dialog, making it the ideal interception point.

**Key findings**:
- **HookInput** (stdin): JSON with `session_id`, `transcript_path`, `cwd`, `permission_mode`, `hook_event_name`, `tool_name`, `tool_input`, `permission_suggestions`
- **HookOutput** (stdout): JSON with `hookSpecificOutput.decision` containing `behavior` ("allow"/"deny"), optional `message`, `updatedPermissions`, `updatedInput`
- **Exit codes**: 0 = success (parse stdout), 1 = non-blocking error (fallback to terminal), 2 = blocking error (hard deny from stderr)
- **Registration**: `~/.claude/settings.json` under `hooks.PermissionRequest[]` with matcher regex on tool_name
- **Hook timeout**: Configurable per handler, default 600s for command type

**Alternatives considered**:
- PreToolUse hook: fires on every tool call (not just permission-gated ones), would add unnecessary overhead
- Custom Claude Code fork: not maintainable

## R2: Telegram Bot Framework

**Decision**: Use `teloxide 0.13` with long polling.

**Rationale**: Most mature Rust Telegram framework, built on tokio (same runtime as our socket server), provides high-level dispatcher with dptree routing for callbacks and messages.

**Key findings**:
- Inline keyboard buttons with callback_data (max 64 bytes — our `{uuid}:{action}` is ~43 bytes, safe)
- `ForceReply` markup for collecting free-text replies
- `answer_callback_query` MUST be called within 10s to dismiss spinner
- `edit_message_text` to update resolved messages (fails if text unchanged)
- Message text limit: 4096 chars (must truncate tool_input)
- Rate limits: ~30 msg/s across chats (not a concern for single-user)
- Long polling preferred (no server infrastructure needed)
- Use `Throttle` adapter for safety

**Alternatives considered**:
- `frankenstein`: lower-level, no dispatcher framework
- `tbot`: less maintained
- Webhooks: requires public server + TLS, unnecessary for local tool

## R3: Unix Domain Socket IPC

**Decision**: tokio `UnixListener`/`UnixStream` with newline-delimited JSON and `DashMap<Uuid, oneshot::Sender>` for request correlation.

**Rationale**: tokio is required by teloxide; DashMap + oneshot provides lockless concurrent request tracking with type-safe single-use resolution.

**Key findings**:
- Server: `UnixListener::bind()` + `tokio::spawn` per connection
- Client: `UnixStream::connect()` with timeout
- Stale socket detection: synchronous `std::os::unix::net::UnixStream::connect` test at startup
- NDJSON: `BufReader::read_line` + `serde_json::from_str`, never use `to_string_pretty`
- Stdin reading (hook mode): `read_to_string` (Claude Code sends single JSON blob, not NDJSON)
- Graceful shutdown: `CancellationToken` + `tokio::signal` for SIGTERM/SIGINT
- Socket path: `$XDG_RUNTIME_DIR/vibe-reachout.sock` (Linux), `/tmp/vibe-reachout-{uid}.sock` (macOS/fallback)

**Alternatives considered**:
- Length-prefixed binary: harder to debug with socat
- MessagePack: unnecessary complexity, JSON is fast enough
- async-std: incompatible with teloxide

## R4: CLI Framework & Configuration

**Decision**: `clap 4` with derive for CLI, `toml 0.8` for config at `~/.config/vibe-reachout/config.toml`.

**Rationale**: clap derive is idiomatic Rust CLI standard. TOML is native to the Rust ecosystem (Cargo uses it). XDG config path is standard.

**Key findings**:
- Subcommands: `bot` (long-running), `install` (register hook), default (no subcommand) = hook mode (reads stdin)
- Config fields: `telegram_bot_token`, `allowed_chat_ids: Vec<i64>`, `timeout_seconds` (default 300), `socket_path` (optional override)
- `dirs` crate for `~/.config` resolution

## R5: Cross-Compilation & Binary Size

**Decision**: Use `rustls` TLS backend, `cross` for Linux targets, release profile with `opt-level = "z"` + `strip` + `lto`.

**Rationale**: rustls avoids native OpenSSL dependency for cross-compilation. Size optimizations should keep binary under 20MB target.

**Key findings**:
- Targets: `aarch64-apple-darwin` (native), `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`
- musl targets for fully static Linux binaries (zero runtime deps)
- Expected binary size: 10-15MB with optimizations
- teloxide needs `default-features = false` + explicit rustls feature for cross-compilation

## R6: Dependency Matrix

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "io-util", "signal", "time", "fs"] }
tokio-util = { version = "0.7", features = ["rt"] }
teloxide = { version = "0.13", features = ["macros"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
dashmap = "6"
uuid = { version = "1", features = ["v4", "serde"] }
anyhow = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dirs = "5"
libc = "0.2"
```
