# Tasks: Telegram Permission Hook for Claude Code

**Input**: Design documents from `specs/001-telegram-permission-hook/`
**Prerequisites**: plan.md (required), spec.md (required), contracts/hook-io.md, contracts/ipc.md, clarifications.md

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US3)

---

## Phase 1: Setup & Core Infrastructure

**Purpose**: Project scaffolding, config, CLI, and shared types. No user-facing functionality yet.

- [ ] T001 Create project scaffolding: Cargo.toml with all dependencies (teloxide, tokio, clap, serde, serde_json, toml, uuid, dirs, tracing, tracing-subscriber, dashmap), justfile (check, lint, format, test, build), clippy.toml, rustfmt.toml. Follow Cor CLI conventions.
- [ ] T002 [P] CLI argument parsing in `src/main.rs`: clap with 4 subcommands — (default) hook mode, `bot`, `install`, `init`. Each dispatches to its module.
- [ ] T003 [P] Config types and parsing in `src/config.rs`: `Config` struct (telegram_bot_token, allowed_chat_ids, timeout_seconds, socket_path). Parse from `~/.config/vibe-reachout/config.toml`. Platform-specific default socket path (macOS: `/tmp/vibe-reachout-{uid}.sock`, Linux: `$XDG_RUNTIME_DIR/vibe-reachout.sock`).
- [ ] T004 [P] Shared types — Claude Code hook I/O in `src/types/hook_io.rs`: `HookInput` (session_id, transcript_path, cwd, permission_mode, hook_event_name, tool_name, tool_input as Value, permission_suggestions), `HookOutput` (hookSpecificOutput with hookEventName + decision), `PermissionSuggestion` struct.
- [ ] T005 [P] Shared types — IPC protocol in `src/types/ipc.rs`: `IpcRequest` (request_id UUID, tool_name, tool_input Value, cwd, session_id, permission_suggestions), `IpcResponse` (request_id, decision enum allow/deny/timeout, message Option, always_allow_suggestion Option). Note: `session_id` is required for multi-session disambiguation (FR-008).
- [ ] T006 `init` subcommand in `src/init.rs`: Interactive mode — prompt for bot token and chat ID. Non-interactive mode — `--token` and `--chat-id` flags. Writes config.toml. Errors if config already exists (use `--force` to overwrite). Depends on T003 (config module for struct + default paths).
- [ ] T007 [P] Unit tests for config parsing in `tests/config_test.rs`: valid config, missing fields, invalid chat IDs, default socket path per platform.
- [ ] T008 [P] Unit tests for hook I/O serialization in `tests/hook_io_test.rs`: deserialize sample Claude Code JSON for Bash, Write, Edit, Read, Glob, Grep, WebFetch, WebSearch, Task tools. Serialize HookOutput for allow, deny, always-allow decisions.
- [ ] T009 [P] Unit tests for IPC serialization in `tests/ipc_test.rs`: round-trip IpcRequest/IpcResponse through serde_json.

- [ ] T041 [P] Configure tracing-subscriber in `src/main.rs`: hook mode → stderr-only writer (stdout reserved for JSON), bot mode → stdout writer. Support `RUST_LOG` env var for level control. Ensure hook mode NEVER writes non-JSON to stdout.

**Checkpoint**: `just check` passes. All types compile and serialize correctly. `vibe-reachout --help` shows subcommands.

---

## Phase 2: User Story 3 — Start the Bot (Priority: P1) 🎯

**Goal**: Bot process that listens on Unix socket and connects to Telegram.

**Independent Test**: Run `vibe-reachout bot`, verify socket is created and Telegram connection succeeds.

### Tests for US3

- [ ] T010 [P] [US3] Unit test for stale socket detection in `tests/bot_server_test.rs`: mock socket file exists but no listener → detected as stale → removed.
- [ ] T011 [P] [US3] Unit test for message formatting in `tests/formatter_test.rs`: test Bash (command + description), Write (file_path + line count), Edit (file + truncated diff), Read, Glob, Grep, WebFetch (URL), WebSearch (query), Task, MCP tool formatting.
- [ ] T012 [P] [US3] Unit test for callback parsing and chat ID validation in `tests/bot_telegram_test.rs`: authorized callback accepted, unauthorized rejected, malformed callback data handled. Verify callback data for all action types (allow, deny, always) stays under Telegram's 64-byte limit.

### Implementation for US3

- [ ] T013 [US3] Unix socket server in `src/bot/server.rs`: tokio UnixListener at configured path. Accept concurrent connections. Each connection: read IpcRequest, register in pending map, wait for resolution via oneshot channel, send IpcResponse. Stale socket detection on startup (try connect → if fails, delete and rebind; if succeeds, error "bot already running").
- [ ] T014 [US3] Request correlation in `src/bot/pending.rs`: `DashMap<Uuid, oneshot::Sender<IpcResponse>>`. Methods: `register(request_id) → oneshot::Receiver`, `resolve(request_id, response)`, `timeout(request_id)`, `remove(request_id)`.
- [ ] T015 [US3] Telegram bot setup in `src/bot/telegram.rs`: Initialize teloxide bot with token from config. Long polling dispatcher. Callback query handler that extracts `request_id:action` from callback data, validates chat ID, resolves pending request. Configure teloxide retry policy for rate limits and network errors.
- [ ] T016 [US3] Tool-specific message formatting in `src/bot/formatter.rs`: Format IpcRequest into Telegram message text + inline keyboard. Tool-specific detail extraction per plan (Bash: command, Write: file + size, Edit: file + truncated diff, WebFetch: URL, WebSearch: query, Task: agent + description, MCP: tool_name + truncated JSON, etc.). Include cwd and session_id (first 8 chars). Hide "Always Allow" button when permission_suggestions is empty.
- [ ] T017 [US3] Per-request timeout in `src/bot/pending.rs`: Spawn tokio::time::sleep task per request. On timeout: resolve with `decision: "timeout"`, edit Telegram message to "⏱️ Timed out — respond in terminal". Depends on T015 (needs bot instance + stored message_id for editing).
- [ ] T018 [US3] Bot entry point in `src/bot/mod.rs`: Wire together socket server + Telegram bot + pending map. Graceful shutdown on SIGINT/SIGTERM (send timeout to all pending, remove socket file).
- [ ] T019 [US3] Integration test in `tests/integration/bot_test.rs`: Start bot, connect via Unix socket, send IpcRequest, verify Telegram message is sent (mock Telegram API or use teloxide test utilities).

**Checkpoint**: `vibe-reachout bot` starts, binds socket, connects to Telegram. Can receive IPC connections and send messages.

---

## Phase 3: User Stories 1 & 2 — Approve and Deny (Priority: P1) 🎯 MVP

**Goal**: Hook mode that bridges Claude Code stdin/stdout to the bot via Unix socket.

**Independent Test**: Pipe sample HookInput JSON into `vibe-reachout`, tap Allow/Deny on Telegram, verify correct HookOutput JSON on stdout.

### Tests for US1 & US2

- [ ] T020 [P] [US1] Unit test for stdin→IpcRequest conversion in `tests/hook_test.rs`: various tool_name/tool_input combinations correctly mapped to IpcRequest fields.
- [ ] T021 [P] [US2] Unit test for IpcResponse→HookOutput conversion in `tests/hook_test.rs`: allow → `behavior: "allow"`, deny → `behavior: "deny"` with message, timeout → no output (exit 1).

### Implementation for US1 & US2

- [ ] T022 [US1] Stdin JSON reader in `src/hook.rs`: Read all stdin, deserialize as HookInput. On malformed JSON: log to stderr, exit 1.
- [ ] T023 [US1] Unix socket client in `src/hook.rs`: Connect to socket at configured path. Send serialized IpcRequest + newline. Block on response. Handle connection refused → exit 1.
- [ ] T024 [US1] Stdout JSON writer in `src/hook.rs`: Convert IpcResponse to HookOutput JSON. Allow → `behavior: "allow"`. Deny → `behavior: "deny"` + `message`.
- [ ] T025 [US1] Exit code logic in `src/hook.rs`: Success (allow/deny) → exit 0 with JSON. Socket connection failed → exit 1 (fallback to terminal). Timeout → exit 1. SIGTERM handler → exit 1. Only two exit codes: 0 (success with JSON) and 1 (any error, triggers terminal fallback).
- [ ] T026 [US1] Hook timeout in `src/hook.rs`: Client-side safety timeout (slightly less than Claude Code's 600s hook timeout). If IPC response doesn't arrive, exit 1.
- [ ] T027 [US1] Integration test — full hook flow in `tests/integration/full_flow_test.rs`: Start bot in background. Pipe test HookInput JSON into hook mode. Simulate Telegram callback (mock or direct pending map resolution). Verify hook stdout JSON and exit code 0.

**Checkpoint**: Full MVP working. Permission prompts forwarded to Telegram, approve/deny flows back to Claude Code. `just check` green.

---

## Phase 4: User Stories 6 & 8 — Always Allow & Install (Priority: P3)

**Goal**: Always Allow button support and automated hook installation.

### Tests

- [ ] T028 [P] [US6] Unit test for always-allow response in `tests/hook_test.rs`: IpcResponse with always_allow_suggestion → HookOutput includes `updatedPermissions`.
- [ ] T029 [P] [US8] Unit test for settings merge in `tests/install_test.rs`: empty file, existing hooks, existing PermissionRequest (update), nested settings preserved.

### Implementation

- [ ] T030 [US6] Always Allow response in `src/hook.rs` AND `src/bot/telegram.rs` (cross-cutting): When IpcResponse has `always_allow_suggestion`, include `updatedPermissions` in HookOutput decision. In bot: callback data `{request_id}:always` triggers this path, using first matching entry from `permission_suggestions`.
- [ ] T031 [US8] Read existing settings in `src/install.rs`: Read `~/.claude/settings.json`. Handle: file exists, doesn't exist, malformed JSON.
- [ ] T032 [US8] Merge hook configuration in `src/install.rs`: Add PermissionRequest hook entry. Preserve existing settings. Update if hook already exists. Write back with 2-space indent.

**Checkpoint**: Always Allow works end-to-end. `vibe-reachout install` sets up Claude Code correctly.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Reliability edge cases, CI/CD, documentation.

- [ ] T033 [US4] Timeout UX: Bot edits Telegram message to "⏱️ Timed out — respond in terminal" on timeout. Late callbacks answered with "This request has already been handled."
- [ ] T034 [US5] Bot-down edge cases: Verify stderr-only logging in hook mode. Verify clean exit on connection refused.
- [ ] T035 [US7] Auth edge cases: Multiple authorized users — first response wins, second gets "already handled". Unauthorized callback gets error answer.
- [ ] T036 CI workflow in `.github/workflows/ci.yml`: fmt --check, clippy, test matrix (macOS + Linux), tarpaulin coverage, cargo audit. Follow Cor CLI patterns.
- [ ] T037 Release workflow in `.github/workflows/release.yml`: CalVer, git-cliff changelog, cross-compile (macOS aarch64, Linux aarch64 + x86_64), GitHub Release with binaries.
- [ ] T038 README.md: Setup instructions (install binary, create config, run install, start bot, use Claude Code normally).
- [ ] T039 Error message audit: Review all error paths for helpful messages (missing config, bad token, socket not found, unauthorized chat ID, bot already running, stale socket).
- [ ] T040 E2E test — full Claude Code simulation in `tests/integration/e2e_test.rs`: Spawn hook with realistic stdin JSON, verify stdout JSON for all scenarios (allow, deny, always-allow, timeout, bot-down). Include concurrent scenario: spawn 5+ hook processes simultaneously with independent request_ids, verify all receive independent responses (FR-002).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Bot)**: Depends on T001, T003, T004, T005
- **Phase 3 (Hook)**: Depends on T001, T003, T004, T005. Can run in parallel with Phase 2 (different files)
- **Phase 4 (Always Allow + Install)**: Depends on Phase 2 + Phase 3 being functional
- **Phase 5 (Polish)**: Depends on Phase 4

### Within Each Phase

- Tests MUST be written and FAIL before implementation (red-green-refactor)
- Types before services, services before entry points
- [P] tasks can run in parallel
- Story complete before moving to next priority

### Parallel Opportunities

```
Phase 1: T002, T003, T004, T005 all [P] — different files, no deps
Phase 1: T007, T008, T009 all [P] — test files
Phase 2: T010, T011, T012 all [P] — test files
Phase 3: T020, T021 [P] — test files
Phase 4: T028, T029 [P] — test files
Phase 2 + Phase 3 can overlap (bot/server.rs vs hook.rs — different modules)
```

## Implementation Strategy

### MVP First (Phases 1–3)

1. Phase 1: scaffolding + types + config
2. Phase 2: bot mode (socket server + Telegram)
3. Phase 3: hook mode (stdin → socket → stdout)
4. **STOP and VALIDATE**: pipe test JSON, tap buttons, verify end-to-end
5. Commit + tag as v0.1.0

### Incremental Delivery

1. Phases 1–3 → MVP (approve + deny)
2. Phase 4 → always-allow + install convenience
3. Phase 5 → production quality (CI, cross-compile, docs)
