# Implementation Plan: Telegram Permission Hook

**Date**: 2026-02-22 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/001-telegram-permission-hook/spec.md`

## Summary

Build a Rust CLI (`vibe-reachout`) that intercepts Claude Code's `PermissionRequest` hook events, forwards them to a Telegram bot, and returns the user's approve/deny decision. Two-mode binary: hook mode (short-lived, invoked by Claude Code) and bot mode (long-running Telegram daemon). IPC via Unix domain socket.

## Technical Context

**Language/Version**: Rust 1.75+
**Primary Dependencies**: teloxide (Telegram), tokio (async), clap (CLI), serde/serde_json (serialization)
**Storage**: None (stateless — pending requests held in memory only)
**Testing**: cargo test (unit + integration)
**Target Platform**: macOS aarch64, Linux aarch64 + x86_64
**Project Type**: Single binary CLI
**Performance Goals**: <100ms hook startup, <5s full round-trip, <50MB idle memory
**Constraints**: Single binary, no runtime deps, must not break Claude Code on failure
**Scale/Scope**: Single user, up to 10 concurrent permission requests

## Architecture

### Single Binary, Two Modes

`vibe-reachout` is a single Rust binary with four subcommands:

1. **(default / no subcommand)** — Hook mode. Invoked by Claude Code. Reads JSON stdin → Unix socket → waits → JSON stdout. Short-lived, needs minimal tokio runtime for async socket I/O + timeout.

2. **`bot`** — Long-running daemon. Telegram bot (teloxide, long polling) + Unix socket server. Receives requests from hook processes, sends Telegram messages with inline keyboards, routes callbacks back.

3. **`install`** — Adds `PermissionRequest` hook to `~/.claude/settings.json`. Idempotent.

4. **`init`** — Creates config file at `~/.config/vibe-reachout/config.toml`. Interactive or `--token`/`--chat-id` flags.

### Component Diagram

```
┌─────────────┐     stdin JSON      ┌──────────────┐    Unix socket     ┌──────────────┐
│ Claude Code  │ ──────────────────→ │ vibe-reachout │ ─────────────────→ │ vibe-reachout │
│              │                     │ (hook mode)   │ ←───────────────── │ (bot mode)    │
│              │ ←────────────────── │               │    response        │               │
│              │     stdout JSON     └──────────────┘                    └───────┬───────┘
└─────────────┘                                                                  │
                                                                          Telegram API
                                                                          (long polling)
                                                                                 │
                                                                         ┌───────▼───────┐
                                                                         │  User's phone  │
                                                                         │  [Allow] [Deny]│
                                                                         │ [Always Allow] │
                                                                         └────────────────┘
```

### IPC: Unix Domain Socket

Bot listens on a Unix socket. Default path: platform-specific with UID suffix for security.
- **Linux**: `$XDG_RUNTIME_DIR/vibe-reachout.sock` (fallback: `/tmp/vibe-reachout-{uid}.sock`)
- **macOS**: `/tmp/vibe-reachout-{uid}.sock`
- **Override**: `socket_path` in config.toml

Protocol (newline-delimited JSON):
1. Hook connects to socket
2. Hook sends `IpcRequest\n`
3. Bot sends Telegram message, stores `request_id → oneshot::Sender<IpcResponse>`
4. User taps button → Telegram callback → bot resolves via oneshot channel
5. Bot sends `IpcResponse\n` over socket
6. Hook reads response, disconnects

Stale socket handling: On startup, bot checks if socket file exists. Tries client connection — if succeeds, another bot is running (error). If fails, stale socket — delete and re-bind.

### Configuration

File: `~/.config/vibe-reachout/config.toml`

```toml
telegram_bot_token = "123456:ABC-DEF..."
allowed_chat_ids = [123456789]
timeout_seconds = 300
socket_path = "/tmp/vibe-reachout-501.sock"  # optional override
```

Both modes read the config. Hook mode only needs `socket_path` and `timeout_seconds`. Bot mode needs everything.

### Claude Code Hook Configuration

Added by `vibe-reachout install` to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PermissionRequest": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "vibe-reachout",
            "timeout": 600
          }
        ]
      }
    ]
  }
}
```

No matcher — intercepts all permission prompts. Timeout 600s (10 min) in Claude Code; the bot's own `timeout_seconds` (default 300s) fires first.

### Telegram Message Formatting

Tool-specific formatters for readable messages:

| Tool | Format |
| --- | --- |
| Bash | `Command: {command}\n{description}` |
| Write | `File: {file_path}\nContent: {line_count} lines` |
| Edit | `File: {file_path}\nReplace: {old_string (truncated 100ch)}\nWith: {new_string (truncated 100ch)}` |
| Read | `File: {file_path}` |
| Glob | `Pattern: {pattern}\nPath: {path}` |
| Grep | `Pattern: {pattern}\nPath: {path}` |
| Task | `Agent: {subagent_type}\nTask: {description}` |
| MCP | `Tool: {tool_name}\n{tool_input as JSON (truncated 200ch)}` |

Full message template:
```
🔐 Permission Request

Tool: {tool_name}
{tool-specific details}

Project: {cwd}
Session: {session_id (first 8 chars)}

[✅ Allow]  [❌ Deny]  [✅ Always Allow]
```

"Always Allow" button hidden when `permission_suggestions` is empty.

### Logging

- **Hook mode**: `tracing` with stderr writer only. stdout reserved for JSON output.
- **Bot mode**: `tracing` with stdout writer. `RUST_LOG` env var for level control.

### Signal Handling

- **Hook mode**: SIGTERM handler exits with code 1 (clean fallback to terminal).
- **Bot mode**: SIGINT/SIGTERM triggers graceful shutdown — send timeout responses to all pending requests, remove socket file, exit.

## Data Flow

### Happy Path (Allow)

1. Claude Code fires `PermissionRequest` → spawns `vibe-reachout`, pipes JSON to stdin
2. Hook reads stdin, connects to Unix socket, sends `IpcRequest` with UUID
3. Bot receives request, formats and sends Telegram message to all `allowed_chat_ids`
4. User taps "Allow" → Telegram callback
5. Bot validates chat ID → resolves pending request via oneshot channel
6. Bot sends `IpcResponse{decision: "allow"}` over socket
7. Hook writes `{hookSpecificOutput: {decision: {behavior: "allow"}}}` to stdout, exits 0
8. Claude Code reads stdout, proceeds with tool execution

### Deny / Always Allow / Timeout / Bot Down

See contracts/hook-io.md and contracts/ipc.md for complete response schemas. Key behaviors:
- **Deny**: Same flow, `behavior: "deny"` with message
- **Always Allow**: Same flow, `behavior: "allow"` with `updatedPermissions`
- **Timeout**: Bot sends `decision: "timeout"`, hook exits 1, Claude Code shows terminal prompt. Bot edits Telegram message to "⏱️ Timed out"
- **Bot down**: Hook can't connect to socket, exits 1 immediately, Claude Code shows terminal prompt

## Crate Dependencies

| Crate | Purpose |
| --- | --- |
| `teloxide` | Telegram Bot API (async, callback queries, inline keyboards) |
| `tokio` | Async runtime (Unix socket server + client, timers, signals) |
| `serde` / `serde_json` | JSON serialization for Claude Code hook I/O and IPC |
| `toml` | Config file parsing |
| `uuid` | Unique request IDs for IPC correlation |
| `clap` | CLI argument parsing (subcommands: default, bot, install, init) |
| `dirs` | XDG config directory resolution |
| `tracing` / `tracing-subscriber` | Structured logging |
| `dashmap` | Concurrent map for pending requests (request_id → oneshot sender) |

## Project Structure

### Documentation (this feature)

```text
specs/001-telegram-permission-hook/
├── spec.md
├── plan.md              # This file
├── tasks.md
├── clarifications.md
└── contracts/
    ├── hook-io.md       # Claude Code PermissionRequest I/O schemas
    └── ipc.md           # Unix socket IPC protocol
```

### Source Code

```text
src/
├── main.rs              # CLI entry point (clap subcommands)
├── config.rs            # Config file parsing (toml)
├── types/
│   ├── mod.rs
│   ├── hook_io.rs       # Claude Code HookInput / HookOutput structs
│   └── ipc.rs           # IpcRequest / IpcResponse structs
├── hook.rs              # Hook mode: stdin → socket → stdout
├── bot/
│   ├── mod.rs
│   ├── server.rs        # Unix socket server
│   ├── telegram.rs      # Telegram bot setup + callback handler
│   ├── formatter.rs     # Tool-specific message formatting
│   └── pending.rs       # Request correlation (DashMap + oneshot)
├── install.rs           # Claude Code settings.json merger
└── init.rs              # Config file creation

tests/
├── hook_io_test.rs      # Serialization tests
├── ipc_test.rs          # IPC protocol tests
├── config_test.rs       # Config parsing tests
├── formatter_test.rs    # Message formatting tests
├── install_test.rs      # Settings merge tests
└── integration/
    └── full_flow_test.rs  # Hook + bot E2E test
```

**Structure Decision**: Single project (`src/` + `tests/`). No need for workspace or multiple crates — the binary is small and focused.

## Phases

### Phase 1: Core Infrastructure
- Project scaffolding (Cargo.toml, justfile, CI config, clippy.toml, rustfmt.toml)
- Config file parsing + init subcommand
- CLI argument parsing (4 subcommands)
- Shared types (HookInput, HookOutput, IpcRequest, IpcResponse)
- Unit tests for config + serialization

### Phase 2: Bot Mode (US3 — P1)
- Unix socket server (tokio)
- Telegram bot setup (teloxide, long polling)
- Tool-specific message formatting
- Callback query handling + chat ID authorization
- Request correlation (DashMap + oneshot channels)
- Timeout handling (per-request timer)
- Stale socket detection + graceful shutdown
- Unit + integration tests

### Phase 3: Hook Mode (US1, US2 — P1)
- Stdin JSON reading
- Unix socket client
- Stdout JSON writing (allow/deny)
- Exit code handling (0/1/2)
- Timeout + signal handling
- Unit + integration tests

### Phase 4: Always Allow + Install (US6, US8 — P3)
- Always Allow button + updatedPermissions response
- `vibe-reachout install` settings merger
- Unit tests

### Phase 5: Polish
- Remaining user stories (US4 timeout UX, US5 edge cases, US7 auth edge cases)
- Cross-compile targets (macOS aarch64, Linux aarch64 + x86_64)
- CI/CD (follow Cor CLI patterns)
- README
- Error message audit
- E2E test (simulated Claude Code flow)
