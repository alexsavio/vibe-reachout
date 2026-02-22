# Plan Clarifications: Telegram Permission Hook

**Date**: 2026-02-22
**Input**: spec.md, plan.md, contracts/hook-io.md, contracts/ipc.md
**Purpose**: Identify ambiguities, gaps, and decisions that need resolution before implementation.

## Resolved Clarifications

### C001: Hook mode needs async runtime too

**Issue**: The plan says hook mode is "short-lived" and bot mode uses tokio. But hook mode also needs tokio — it uses Unix domain sockets (tokio::net::UnixStream) and needs async I/O to handle timeout properly.

**Resolution**: Hook mode runs a minimal tokio runtime (`#[tokio::main]` or `Runtime::new()`) for the socket client + timeout. This is fine — the runtime starts, does one operation, and exits. No long-running tasks.

### C002: Config file needed in hook mode too

**Issue**: Hook mode needs to know the socket path to connect. It must read the config file.

**Resolution**: Both modes read `~/.config/vibe-reachout/config.toml`. Hook mode only needs `socket_path` (and optionally `timeout_seconds` as a client-side safety). Bot mode needs everything.

### C003: Which `permission_suggestions` to use for Always Allow

**Issue**: The `permission_suggestions` array can contain multiple options (e.g., `toolAlwaysAllow` for Bash, `toolAlwaysAllow` for a prompt pattern). Which one does "Always Allow" apply?

**Resolution**: Use the first `permission_suggestions` entry that matches the current tool. If the array is empty, the "Always Allow" button is hidden from the Telegram keyboard.

### C004: Socket path on Linux vs macOS

**Issue**: `/tmp/vibe-reachout.sock` works on both platforms, but `/tmp` is world-writable. Any user could connect to the socket.

**Resolution**: Use `$XDG_RUNTIME_DIR/vibe-reachout.sock` on Linux (falls back to `/tmp/vibe-reachout-{uid}.sock`). On macOS, use `/tmp/vibe-reachout-{uid}.sock`. The `{uid}` suffix prevents other users from connecting. Document in config that `socket_path` can override this.

### C005: Bot mode — long polling vs webhook

**Issue**: teloxide supports both long polling and webhooks for receiving Telegram updates. Which one?

**Resolution**: Long polling. Simpler, no need for a public URL or HTTPS certificate. The bot is running locally — not on a server. Long polling is the standard for local/CLI bots.

## Open Questions (Need User Decision)

### Q001: Bot lifecycle management

**Options:**
1. **Manual**: User starts `vibe-reachout bot` in a separate terminal / tmux / background process. User manages it.
2. **Launchd/systemd**: `vibe-reachout install` also installs a launchd plist (macOS) or systemd unit (Linux) to auto-start the bot.
3. **Self-fork**: Hook mode auto-starts the bot if it detects the socket is missing (forks a background process).

**Recommendation**: Option 1 for MVP. Option 2 as Phase 5 polish. Option 3 is fragile and hard to debug.

### Q002: Multiple Claude Code sessions

**Scenario**: User has 3 Claude Code sessions running in different terminals. All fire `PermissionRequest` hooks. All connect to the same bot process.

**Current design handles this**: Each hook is a separate socket connection with a unique `request_id`. The bot shows 3 separate Telegram messages. User taps each independently. No conflict.

**But**: The Telegram messages don't indicate WHICH session the permission is from. The `cwd` field helps (different projects = different directories), but two sessions in the same project would be ambiguous.

**Proposed fix**: Include `session_id` (from hook stdin) in the Telegram message. Format: `Session: abc123...` (truncated).

### Q003: Telegram message for non-Bash tools

**Issue**: The Telegram message format in the plan shows `Command: rm -rf node_modules` for Bash. But for `Write`, the relevant info is `file_path` + `content` (which could be huge). For `Edit`, it's `file_path` + `old_string` + `new_string`.

**Proposed solution**: Tool-specific formatters:
- **Bash**: Show `command` and `description`
- **Write**: Show `file_path` and content length (`Writing 150 lines to src/main.rs`)
- **Edit**: Show `file_path`, `old_string` (truncated to 100 chars), `new_string` (truncated)
- **Read/Glob/Grep**: Show the path/pattern
- **Task**: Show `description` and `subagent_type`
- **MCP tools**: Show tool name and raw `tool_input` as JSON (truncated)

### Q004: What if the user taps a button after the hook has already timed out?

**Scenario**: Hook times out after 300s, exits with code 1 (fallback to terminal). User taps "Allow" on Telegram 30 seconds later.

**Issue**: The socket connection is closed. The bot has no pending request for that `request_id`.

**Resolution**: Bot should edit the Telegram message to show "⏱️ Timed out — responded in terminal" when the timeout fires. If a late callback arrives, answer with "This request has already been handled."

### Q005: Config file creation / first-run experience

**Issue**: If `~/.config/vibe-reachout/config.toml` doesn't exist, what happens?

**Proposed flow**:
1. `vibe-reachout bot` without config → error with helpful message: "Config not found. Run `vibe-reachout init` to create one."
2. `vibe-reachout init` → interactive prompts: paste bot token, enter chat ID. Writes config file.
3. Or: `vibe-reachout init --token=xxx --chat-id=123` for non-interactive setup.

This adds a 4th subcommand (`init`). Worth it for UX.

### Q006: Logging in hook mode

**Issue**: Hook mode must not write anything to stdout except the JSON response (Claude Code parses stdout). Logging to stderr is shown in verbose mode only.

**Resolution**: Hook mode uses `tracing` with stderr writer. Bot mode uses `tracing` with stdout/file writer. This is a config difference at startup, not a code difference.

## Spec Gaps

### G001: Spec missing priorities on user stories

The spec has US1–US8 but no priority labels (P1, P2, etc.). Per spec-kit convention, user stories should be prioritized.

**Proposed priorities:**
- **P1 (MVP)**: US1 (approve), US2 (deny), US7 (start bot) — core functionality
- **P2 (Essential)**: US4 (timeout fallback), US5 (bot down fallback) — reliability
- **P3 (Important)**: US3 (always allow), US6 (auth), US8 (install) — convenience + security

### G002: Spec missing edge cases section

Per spec-kit template, edge cases should be documented:
- What if Telegram API is rate-limited? (Bot should retry with backoff, hook keeps waiting)
- What if the config file has an invalid bot token? (Bot fails to start with clear error)
- What if the socket file exists but the bot process died? (Stale socket — hook gets connection refused, falls back)
- What if two people are in `allowed_chat_ids` and both tap different buttons? (First response wins, second gets "already handled")

### G003: Spec missing functional requirements section

Per spec-kit template, requirements should use FR-xxx format:
- **FR-001**: System MUST forward `PermissionRequest` hook input to Telegram within 2 seconds
- **FR-002**: System MUST support concurrent permission requests (up to 10 simultaneous)
- **FR-003**: System MUST fall back to terminal on any error condition
- **FR-004**: System MUST validate Telegram chat ID before accepting responses
- **FR-005**: System MUST preserve existing Claude Code settings during install

### G004: Spec missing success criteria

Per spec-kit template:
- **SC-001**: Permission approve/deny round-trip (Claude Code → Telegram → Claude Code) under 5 seconds on normal network
- **SC-002**: Hook mode startup and socket connection under 100ms
- **SC-003**: Zero data loss on timeout (Claude Code falls back cleanly)
- **SC-004**: Binary size under 20MB (single static binary)

### G005: Plan missing data-model.md

No persistent data, but the IPC protocol types should be documented as a data model (request/response structs). Currently in `contracts/ipc.md` which is fine, but the plan template expects a `data-model.md`.

**Resolution**: Not needed — this project has no database or persistent state. The IPC types in `contracts/ipc.md` serve this purpose.

## Plan Gaps

### P001: Plan doesn't address stale socket file

If the bot crashes, the socket file remains on disk. Next bot start succeeds (it can re-bind). But if the bot is already running and you start another, it fails because the socket is in use.

**Resolution**: On startup, bot checks if socket file exists. If it does, try to connect as a client. If connection succeeds → another bot is running → error "Bot already running". If connection fails → stale socket → delete and re-bind.

### P002: Plan doesn't address signal handling in hook mode

The hook process blocks waiting for a response. If the user hits Ctrl+C in Claude Code, Claude Code sends SIGTERM to child processes. The hook should handle this gracefully.

**Resolution**: Hook mode installs a SIGTERM handler that exits with code 1 (clean fallback). No special cleanup needed — the socket connection closes automatically.

### P003: Plan doesn't specify the `init` subcommand

The plan mentions `bot`, `install`, and default (hook) modes. But the first-run experience (Q005) suggests an `init` command for config file creation.

**Resolution**: Add `init` as a 4th subcommand. Phase 1 task.
