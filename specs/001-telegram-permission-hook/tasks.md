# Tasks: Telegram Permission Hook

**Input**: Design documents from `/specs/001-telegram-permission-hook/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: Not explicitly requested in spec. Integration tests included in Polish phase for validation.

**Organization**: Tasks grouped by user story. US1 & US2 combined (both P1, share 95% infrastructure).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Initialize Rust project with all dependencies and directory structure

- [x] T001 Initialize Cargo project: run `cargo init --name vibe-reachout`, set edition = "2024", rust-version = "1.85" in Cargo.toml
- [x] T002 Add all dependencies to Cargo.toml per specs/001-telegram-permission-hook/research.md section R6 (tokio, teloxide, clap, serde, serde_json, toml, dashmap, uuid, anyhow, thiserror, tracing, tracing-subscriber, dirs, libc, tokio-util)
- [x] T003 Configure release profile in Cargo.toml: strip = true, lto = true, codegen-units = 1, opt-level = "z"
- [x] T004 Create source directory structure: src/telegram/ (mod.rs, handler.rs, keyboard.rs, formatter.rs), src/ipc/ (mod.rs, server.rs, client.rs), and stub files for src/config.rs, src/models.rs, src/hook.rs, src/bot.rs, src/install.rs, src/error.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared types, config, error handling, CLI parsing, and logging that ALL stories depend on

**CRITICAL**: No user story work can begin until this phase is complete

- [x] T005 Define error types using thiserror in src/error.rs: HookError (SocketNotFound, ConnectionRefused, InvalidResponse, Timeout), BotError (AlreadyRunning, StaleSocket, TelegramApi, ConfigInvalid), InstallError (SettingsNotFound, ParseError, WriteError)
- [x] T006 [P] Define all shared types in src/models.rs per specs/001-telegram-permission-hook/data-model.md: Config (with serde Deserialize, validation methods), HookInput (with serde Deserialize for all fields including tool_input as serde_json::Value), HookOutput (with serde Serialize, nested hookSpecificOutput.decision structure), IpcRequest (with Serialize + Deserialize, request_id as Uuid), IpcResponse (with Serialize + Deserialize, Decision enum: Allow/Deny/AlwaysAllow/Reply/Timeout), SentMessage struct (chat_id, message_id), PendingRequest struct (request_id, oneshot::Sender, sent_messages: Vec<SentMessage>, original_text, permission_suggestions, created_at)
- [x] T007 [P] Implement config loading in src/config.rs: load from ~/.config/vibe-reachout/config.toml using dirs crate + toml crate, validate (non-empty token, at least one chat_id, timeout > 0 and <= 3600), default_socket_path() using XDG_RUNTIME_DIR or /tmp/vibe-reachout-{uid}.sock fallback per specs/001-telegram-permission-hook/contracts/ipc.md
- [x] T008 Implement CLI parsing with clap derive in src/main.rs: Cli struct with Optional Commands enum (Bot, Install), None = hook mode. Wire subcommand dispatch stubs that load config and call placeholder functions
- [x] T009 [P] Set up tracing infrastructure in src/main.rs: init_tracing(is_hook_mode: bool) function, stderr writer, EnvFilter from RUST_LOG with default "warn" for hook mode and "info" for bot mode per spec Constraints section

**Checkpoint**: Foundation ready — `cargo build` succeeds, `vibe-reachout --help` shows subcommands

---

## Phase 3: US3 — Start the Bot (Priority: P1)

**Goal**: `vibe-reachout bot` starts Telegram polling and binds Unix socket, ready to accept hook connections

**Independent Test**: Run `vibe-reachout bot`, verify it connects to Telegram API and listens on Unix socket

- [x] T010 [US3] Implement Unix socket server startup in src/ipc/server.rs: async fn run_server(socket_path, cancel_token) that calls UnixListener::bind(), loops on accept() with tokio::select! against cancellation, spawns tokio::spawn per connection with placeholder handler
- [x] T011 [US3] Implement stale socket detection in src/ipc/server.rs: fn detect_and_clean_stale_socket(socket_path) using synchronous std::os::unix::net::UnixStream::connect — if ConnectionRefused remove file and return Ok, if connection succeeds return BotError::AlreadyRunning
- [x] T012 [US3] Implement Telegram bot initialization in src/bot.rs: create Bot::new(token), build dptree dispatcher with empty callback_query and message handler branches, set up long polling via Dispatcher::builder().dispatch()
- [x] T013 [US3] Implement graceful shutdown in src/bot.rs: CancellationToken shared between socket server and Telegram dispatcher, tokio::signal handler for SIGTERM + SIGINT that cancels the token, cleanup socket file on exit, resolve all pending requests with Timeout
- [x] T014 [US3] Wire bot subcommand in src/main.rs: load config, call detect_and_clean_stale_socket, launch socket server + Telegram dispatcher concurrently with tokio::select!, handle startup errors with clear messages

**Checkpoint**: `vibe-reachout bot` starts, binds socket, connects to Telegram, handles Ctrl+C cleanly

---

## Phase 4: US1 & US2 — Approve & Deny Permission from Telegram (Priority: P1) MVP

**Goal**: Full permission round-trip: Claude Code → hook → socket → bot → Telegram message → user taps Allow/Deny → response flows back → Claude Code proceeds or blocks

**Independent Test**: Start bot, trigger permission in Claude Code, tap Allow/Deny on Telegram, verify Claude Code proceeds/blocks

- [x] T015 [P] [US1] Implement message formatter in src/telegram/formatter.rs: format_permission_message(ipc_request) → String with project name header (cwd basename), tool-specific formatting (Bash: command in code block, Write: file_path + size, Edit: file + diff snippet, generic: JSON excerpt), truncation at 500 chars per field and 4000 chars total per specs/001-telegram-permission-hook/contracts/telegram-ui.md
- [x] T016 [P] [US1] Implement inline keyboard builder in src/telegram/keyboard.rs: fn make_keyboard(request_id, has_permission_suggestions) → InlineKeyboardMarkup with Allow and Deny buttons, callback_data format "{uuid}:{action}" per specs/001-telegram-permission-hook/contracts/ipc.md Telegram Callback Data section
- [x] T017 [P] [US1] Implement IPC client in src/ipc/client.rs: async fn send_request(socket_path, ipc_request, timeout) → Result<IpcResponse> that connects UnixStream, writes NDJSON line, shuts down write half, reads response line with tokio::time::timeout, deserializes IpcResponse
- [x] T018 [US1] Implement hook mode in src/hook.rs: async fn run_hook(config) that reads all stdin to String, deserializes HookInput, generates UUID v4 request_id, builds IpcRequest, calls ipc::client::send_request, maps IpcResponse to HookOutput JSON per specs/001-telegram-permission-hook/contracts/hook-io.md (Allow→allow behavior, Deny→deny with message, Timeout→exit 1), writes JSON to stdout, exits 0 or 1
- [x] T019 [US1] Implement Telegram message sending in src/bot.rs: async fn send_permission_to_telegram(bot, config, ipc_request, pending_map) that formats message, builds keyboard, sends to ALL authorized chat_ids (skip failures, proceed if at least one succeeds), stores PendingRequest in DashMap with oneshot sender, sent_messages: Vec<SentMessage> collecting all (chat_id, message_id) pairs, original_text, permission_suggestions, created_at
- [x] T020 [US1] Implement callback handler for Allow in src/telegram/handler.rs: handle_callback(bot, query, pending_map, config) that answers callback query, parses callback_data "{uuid}:allow", removes PendingRequest from DashMap, sends IpcResponse(Allow) via oneshot, edits ALL sent_messages across all chats to append "✅ Approved" and remove keyboard
- [x] T021 [US2] Add Deny callback handling in src/telegram/handler.rs: extend handle_callback to parse "{uuid}:deny", send IpcResponse(Deny, message="Denied by user via Telegram"), edit ALL sent_messages to append "❌ Denied" and remove keyboard
- [x] T022 [US1] Implement IPC server connection handler in src/ipc/server.rs: async fn handle_connection(stream, bot, config, pending_map) that reads IpcRequest NDJSON line, calls send_permission_to_telegram, awaits oneshot receiver, writes IpcResponse NDJSON line back to stream. Note: DashMap keyed by UUID naturally isolates concurrent sessions (US1 AS3 — multiple sessions trigger permissions simultaneously)
- [x] T023 [US1] Wire hook mode in src/main.rs: when no subcommand, call run_hook(config), catch all errors and exit(1) with stderr logging
- [x] T024 [US1] Handle late/duplicate callbacks in src/telegram/handler.rs: if request_id not in DashMap, answer callback with "This request has already been handled" show_alert(true)

**Checkpoint**: Full approve/deny flow works end-to-end. Claude Code → Telegram → Claude Code

---

## Phase 5: US4 — Timeout Fallback to Terminal (Priority: P2)

**Goal**: If user doesn't respond on Telegram within timeout_seconds, hook exits code 1 and Claude Code shows terminal prompt

**Independent Test**: Trigger permission, wait for timeout, verify terminal prompt appears

- [x] T025 [US4] Implement per-request timeout in src/ipc/server.rs: wrap oneshot receiver with tokio::time::timeout(Duration::from_secs(config.timeout_seconds)), on timeout remove PendingRequest from DashMap, write IpcResponse(Timeout) to stream
- [x] T026 [US4] Edit Telegram message on timeout in src/bot.rs: after timeout fires, edit original message to append "⏱️ Timed out" and remove keyboard
- [x] T027 [US4] Verify late callbacks after timeout handled by T024 (request_id not in DashMap → "already handled" alert)

**Checkpoint**: Timeout after configured seconds, clean fallback, Telegram message updated

---

## Phase 6: US5 — Bot Down Fallback (Priority: P2)

**Goal**: If bot is not running, hook exits code 1 immediately and Claude Code shows normal terminal prompt

**Independent Test**: Don't start bot, trigger permission, verify terminal prompt appears

- [x] T028 [US5] Handle socket-not-found in src/ipc/client.rs: if socket path doesn't exist, return HookError::SocketNotFound, log to stderr "Bot not running (socket not found)"
- [x] T029 [US5] Handle connection-refused in src/ipc/client.rs: if UnixStream::connect returns ConnectionRefused, return HookError::ConnectionRefused, log to stderr
- [x] T030 [US5] Ensure all hook errors result in exit code 1 in src/hook.rs: wrap run_hook in catch-all, log error to stderr at warn level, call std::process::exit(1)

**Checkpoint**: Hook fails gracefully when bot is unavailable, Claude Code falls back to terminal

---

## Phase 7: US6 — Reply with Details from Telegram (Priority: P2)

**Goal**: User can tap Reply, type free-text, and it flows back to Claude Code as a deny with user message

**Independent Test**: Trigger permission, tap Reply, type message, verify Claude Code receives it

- [x] T031 [US6] Add Reply button to inline keyboard in src/telegram/keyboard.rs: add "💬 Reply" button between Deny and Always Allow, callback_data "{uuid}:reply"
- [x] T032 [US6] Implement Reply callback handler in src/telegram/handler.rs: on "{uuid}:reply" callback, answer query, send new message with ForceReply markup "Type your reply:", track (chat_id → request_id) in ReplyState DashMap<ChatId, Uuid>
- [x] T033 [US6] Implement Message handler for ForceReply responses in src/telegram/handler.rs: register message handler in dispatcher, check ReplyState for chat_id, extract text, validate non-empty (re-prompt if empty), remove from ReplyState, resolve PendingRequest with IpcResponse(Reply, user_message=text), edit original permission message to "💬 Replied"
- [x] T034 [US6] Map Reply IpcResponse to HookOutput in src/hook.rs: Decision::Reply → HookOutput deny behavior with message = "User replied: {user_message}" per specs/001-telegram-permission-hook/contracts/hook-io.md

**Checkpoint**: Reply flow works end-to-end, Claude receives user's free-text as denial reason

---

## Phase 8: US7 — Always-Allow a Tool from Telegram (Priority: P3)

**Goal**: User taps "Always Allow" and Claude Code stops prompting for that tool type in the session

**Independent Test**: Tap Always Allow for Bash, verify no further Bash prompts in session

- [x] T035 [US7] Add conditional Always Allow button in src/telegram/keyboard.rs: only show "🔓 Always Allow" button when permission_suggestions is non-empty, callback_data "{uuid}:always"
- [x] T036 [US7] Implement AlwaysAllow callback handler in src/telegram/handler.rs: on "{uuid}:always" callback, resolve PendingRequest with IpcResponse(AlwaysAllow, always_allow_suggestion from stored PendingRequest.permission_suggestions[0]), edit message to "🔓 Always Allowed"
- [x] T037 [US7] Map AlwaysAllow IpcResponse to HookOutput in src/hook.rs: Decision::AlwaysAllow → HookOutput allow behavior with updatedPermissions = [always_allow_suggestion] per specs/001-telegram-permission-hook/contracts/hook-io.md

**Checkpoint**: Always-allow flow works, Claude Code applies permission rule for session

---

## Phase 9: US8 — Security: Only Authorized Users Can Respond (Priority: P3)

**Goal**: Only callbacks/messages from authorized chat_ids are processed

**Independent Test**: Send callback from unauthorized chat ID, verify rejection

- [x] T038 [US8] Implement chat_id validation in src/telegram/handler.rs: at top of handle_callback and handle_message, extract chat_id, check config.allowed_chat_ids.contains(), if unauthorized: answer callback with "Unauthorized" show_alert(true), return early
- [x] T039 [US8] Log unauthorized access attempts in src/telegram/handler.rs: tracing::warn! with unauthorized chat_id for audit trail
- [x] T040 [US8] Ensure pending request is unaffected by unauthorized callbacks in src/telegram/handler.rs: unauthorized callback must not remove or modify PendingRequest in DashMap

**Checkpoint**: Unauthorized chat IDs are rejected, authorized ones work, pending requests unaffected

---

## Phase 10: US9 — Install the Hook (Priority: P3)

**Goal**: `vibe-reachout install` registers the PermissionRequest hook in Claude Code settings

**Independent Test**: Run `vibe-reachout install`, verify `~/.claude/settings.json` has the hook entry

- [x] T041 [US9] Implement settings.json read/parse in src/install.rs: read ~/.claude/settings.json (create with {} if missing), parse as serde_json::Value, preserve all existing content
- [x] T042 [US9] Implement hook registration in src/install.rs: navigate to or create hooks.PermissionRequest array, add hook entry per specs/001-telegram-permission-hook/contracts/hook-io.md Hook Configuration section (type: "command", command: "vibe-reachout", timeout: 600)
- [x] T043 [US9] Handle idempotent install in src/install.rs: if hook with command "vibe-reachout" already exists, update it in place; do not create duplicate entries; preserve other hooks for other events
- [x] T044 [US9] Wire install subcommand in src/main.rs: call install::run_install(), print success message with path to modified settings file

**Checkpoint**: `vibe-reachout install` adds hook to settings.json, idempotent, preserves existing config

---

## Phase 11: Polish & Cross-Cutting Concerns

**Purpose**: Build optimization, validation, and cleanup

- [x] T045 [P] Configure cross-compilation: add Cross.toml for Linux targets (aarch64-unknown-linux-gnu, x86_64-unknown-linux-gnu), ensure teloxide uses rustls TLS backend (disable default features, add rustls feature)
- [x] T046 [P] Run cargo clippy --all-targets and fix all warnings
- [x] T047 Validate binary size: cargo build --release, verify < 20MB
- [x] T048 Run end-to-end validation per specs/001-telegram-permission-hook/quickstart.md: build, configure, install hook, start bot, trigger permission, approve/deny/reply from Telegram

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories
- **US3 (Phase 3)**: Depends on Foundational — BLOCKS US1/US2 (bot must run first)
- **US1 & US2 (Phase 4)**: Depends on US3 — core approve/deny flow (MVP)
- **US4 (Phase 5)**: Depends on US1/US2 — adds timeout to existing flow
- **US5 (Phase 6)**: Depends on Foundational only — hook error handling (can parallel with US3)
- **US6 (Phase 7)**: Depends on US1/US2 — extends callback handler with Reply flow
- **US7 (Phase 8)**: Depends on US1/US2 — extends keyboard and callback handler
- **US8 (Phase 9)**: Depends on US1/US2 — adds validation layer to callback handler
- **US9 (Phase 10)**: Depends on Foundational only — independent file (can parallel with US3+)
- **Polish (Phase 11)**: Depends on all desired stories complete

### User Story Dependencies

```
Setup → Foundational → US3 (bot) → US1+US2 (approve/deny) → US4 (timeout)
                                                            → US6 (reply)
                                                            → US7 (always-allow)
                                                            → US8 (security)
                       Foundational → US5 (bot-down fallback)  [parallel with US3]
                       Foundational → US9 (install)             [parallel with US3]
```

### Parallel Opportunities

Within Phase 4 (US1+US2):
- T015, T016, T017 can run in parallel (different files: formatter.rs, keyboard.rs, client.rs)

Across phases after Foundational:
- US5 and US9 can run in parallel with US3 (no dependency on bot running)
- US4, US6, US7, US8 can run in parallel after US1+US2 complete

---

## Parallel Example: US1 & US2

```bash
# Launch these in parallel (different files, no dependencies):
Task: "Implement message formatter in src/telegram/formatter.rs"     # T015
Task: "Implement inline keyboard builder in src/telegram/keyboard.rs" # T016
Task: "Implement IPC client in src/ipc/client.rs"                     # T017

# Then sequentially:
Task: "Implement hook mode in src/hook.rs"                            # T018 (depends on T017)
Task: "Implement Telegram message sending in src/bot.rs"              # T019 (depends on T015, T016)
Task: "Implement callback handler in src/telegram/handler.rs"         # T020 (depends on T019)
```

---

## Implementation Strategy

### MVP First (US3 + US1 + US2)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: US3 — Bot starts and listens
4. Complete Phase 4: US1+US2 — Full approve/deny round-trip
5. **STOP and VALIDATE**: End-to-end test with real Claude Code
6. Ship MVP

### Incremental Delivery

1. Setup + Foundational → project builds
2. US3 → bot starts and connects to Telegram
3. US1+US2 → approve/deny works → **MVP!**
4. US5 → bot-down fallback (resilience)
5. US4 → timeout fallback (resilience)
6. US6 → reply with details (enhanced UX)
7. US7 → always-allow (convenience)
8. US8 → security validation (hardening)
9. US9 → install command (convenience)
10. Polish → cross-compilation, size validation

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story
- US1 and US2 combined in Phase 4 since deny is a small delta on the approve flow
- Each story checkpoint is independently testable
- All hook mode errors → exit code 1 (terminal fallback)
- stdout is ONLY for JSON in hook mode; all logging to stderr
