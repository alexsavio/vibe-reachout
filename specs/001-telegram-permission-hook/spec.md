# Spec: Telegram Permission Hook for Claude Code

**Status**: Draft
**Created**: 2026-02-22

## Problem

When using Claude Code in "vibe coding" mode (kick off a task, walk away), the terminal blocks on permission prompts. The user must stay at the terminal to approve or deny tool calls, which defeats the purpose of autonomous coding sessions.

## Solution

**vibe-reachout** — a Rust CLI that hooks into Claude Code's `PermissionRequest` event, forwards permission prompts to a Telegram bot, and returns the user's approve/deny decision back to Claude Code. The user approves from their phone; Claude Code continues working.

## User Scenarios & Testing

### US1: Approve a permission from Telegram (Priority: P1)

As a developer using Claude Code, I want to receive permission prompts on Telegram so I can approve or deny tool calls from my phone without being at the terminal.

**Why this priority**: Core functionality — without approve, the tool has no purpose.

**Independent Test**: Start bot, trigger a permission in Claude Code, tap Allow on Telegram, verify tool executes.

**Acceptance Scenarios:**
1. **Given** Claude Code triggers a Bash permission prompt, **When** bot is running, **Then** a Telegram message appears within 2s with tool name, command, and project path
2. **Given** user taps Allow on Telegram, **When** hook receives the response, **Then** Claude Code proceeds with the tool call
3. **Given** multiple sessions trigger permissions simultaneously, **When** user taps Allow on each, **Then** each session proceeds independently

### US2: Deny a permission from Telegram (Priority: P1)

As a developer, I want to deny dangerous tool calls from Telegram so Claude Code stops before executing them.

**Why this priority**: Core functionality — deny is as critical as approve.

**Independent Test**: Trigger a permission, tap Deny, verify tool is blocked and Claude sees the reason.

**Acceptance Scenarios:**
1. **Given** a permission request on Telegram, **When** user taps Deny, **Then** Claude Code blocks the tool and shows denial reason
2. **Given** a denied permission, **When** Claude sees the denial, **Then** Claude adjusts its approach (no retry of the same command)

### US3: Start the bot (Priority: P1)

As a developer, I want a simple command to start the Telegram bot process.

**Why this priority**: Bot must run for anything else to work.

**Independent Test**: Run `vibe-reachout bot`, verify it connects to Telegram API and listens on Unix socket.

**Acceptance Scenarios:**
1. **Given** valid config file, **When** user runs `vibe-reachout bot`, **Then** bot starts, binds socket, connects to Telegram
2. **Given** bot is already running, **When** user runs `vibe-reachout bot` again, **Then** error message "Bot already running" (detects existing socket)
3. **Given** stale socket file from crashed bot, **When** user runs `vibe-reachout bot`, **Then** bot detects stale socket, removes it, and starts normally

### US4: Timeout fallback to terminal (Priority: P2)

As a developer, I want the system to fall back to the terminal prompt if I don't respond on Telegram within the timeout period.

**Why this priority**: Essential for reliability — prevents hung sessions.

**Independent Test**: Trigger a permission, wait for timeout, verify Claude Code shows terminal prompt.

**Acceptance Scenarios:**
1. **Given** no Telegram response within configured timeout (default: 300s), **When** timeout fires, **Then** hook exits code 1 and Claude Code shows terminal prompt
2. **Given** a timeout has occurred, **When** user later taps a button on the expired Telegram message, **Then** bot answers "This request has already been handled" and edits the message to show "⏱️ Timed out"

### US5: Bot down fallback (Priority: P2)

As a developer, I want Claude Code to still work normally if the Telegram bot process isn't running.

**Why this priority**: Essential for reliability — tool must not break Claude Code.

**Independent Test**: Don't start the bot, trigger a permission in Claude Code, verify terminal prompt appears.

**Acceptance Scenarios:**
1. **Given** bot is not running (socket doesn't exist), **When** hook tries to connect, **Then** exits code 1 immediately, Claude Code shows terminal prompt
2. **Given** hook fails, **When** error occurs, **Then** errors go to stderr only (visible in Claude Code verbose mode, not in normal output)

### US6: Always-allow a tool from Telegram (Priority: P3)

As a developer, I want to tap "Always Allow" so I'm not prompted again for the same tool type during the session.

**Why this priority**: Convenience — reduces repeated approvals but not blocking for MVP.

**Independent Test**: Tap Always Allow for Bash, verify no further Bash prompts in the session.

**Acceptance Scenarios:**
1. **Given** a permission request with `permission_suggestions`, **When** user taps Always Allow, **Then** Claude Code applies the permission rule
2. **Given** empty `permission_suggestions`, **When** Telegram message is shown, **Then** Always Allow button is hidden

### US7: Security — only authorized users can respond (Priority: P3)

As a developer, I want only my Telegram account to be able to approve/deny permissions.

**Why this priority**: Important for security but single-user self-hosted tool limits risk.

**Independent Test**: Send a callback from an unauthorized chat ID, verify it's rejected.

**Acceptance Scenarios:**
1. **Given** callback from authorized chat ID, **When** user taps a button, **Then** response is accepted
2. **Given** callback from unauthorized chat ID, **When** attacker taps a button, **Then** callback is ignored, attacker sees error, pending request unaffected
3. **Given** two authorized users both tap, **When** first response arrives, **Then** first wins, second sees "already handled"

### US8: Install the hook (Priority: P3)

As a developer, I want a simple command to register the hook in Claude Code settings.

**Why this priority**: Convenience — manual config editing works but install is nicer.

**Independent Test**: Run `vibe-reachout install`, verify `~/.claude/settings.json` has the hook entry.

**Acceptance Scenarios:**
1. **Given** no existing hooks in settings, **When** user runs `vibe-reachout install`, **Then** PermissionRequest hook is added
2. **Given** existing hooks for other events, **When** user runs install, **Then** existing hooks preserved, PermissionRequest added
3. **Given** hook already installed, **When** user runs install again, **Then** hook is updated (idempotent), no duplicates

### Edge Cases

- What if Telegram API is rate-limited? Bot retries with exponential backoff; hook keeps waiting until timeout.
- What if config file has an invalid bot token? Bot fails to start with clear error message.
- What if socket file exists but bot process died? Bot detects stale socket (connection test fails), removes it, re-binds.
- What if two authorized users both tap different buttons on the same request? First response wins; second gets "already handled."
- What if the user taps after the hook timed out? Bot answers "This request has already been handled" and edits the original message.
- What if Claude Code sends SIGTERM to the hook? Hook handles signal, exits code 1 (clean fallback).

## Requirements

### Functional Requirements

- **FR-001**: System MUST forward `PermissionRequest` hook input to Telegram within 2 seconds of receiving it
- **FR-002**: System MUST support at least 10 concurrent permission requests (separate Claude Code sessions)
- **FR-003**: System MUST fall back to terminal prompt on any error condition (exit code 1)
- **FR-004**: System MUST validate Telegram chat ID before accepting callback responses
- **FR-005**: System MUST preserve existing Claude Code settings during install (no data loss)
- **FR-006**: System MUST format Telegram messages with tool-specific details (Bash: command, Write: file path + size, Edit: file + diff, etc.)
- **FR-007**: System MUST handle stale socket files from crashed bot processes
- **FR-008**: System MUST include session context (`cwd`, `session_id`) in Telegram messages to distinguish multiple sessions

### Key Entities

- **IpcRequest**: Permission details sent from hook to bot over Unix socket (request_id, tool_name, tool_input, cwd, permission_suggestions)
- **IpcResponse**: Decision sent from bot to hook over Unix socket (request_id, decision, message, always_allow_suggestion)
- **HookInput**: Claude Code's JSON sent to hook via stdin (session_id, tool_name, tool_input, permission_suggestions, etc.)
- **HookOutput**: JSON written to stdout for Claude Code (hookSpecificOutput with decision behavior)
- **Config**: User configuration (telegram_bot_token, allowed_chat_ids, timeout_seconds, socket_path)

## Success Criteria

### Measurable Outcomes

- **SC-001**: Permission approve/deny round-trip (Claude Code → Telegram → Claude Code) under 5 seconds on normal network
- **SC-002**: Hook mode startup and socket connection under 100ms
- **SC-003**: Zero data loss on timeout — Claude Code falls back cleanly every time
- **SC-004**: Binary size under 20MB (single static binary)
- **SC-005**: Bot process uses under 50MB memory idle, under 100MB with 10 pending requests

## Non-Goals

- Slack, Discord, or other messaging platforms (Telegram only for MVP)
- Web UI or desktop app
- Modifying tool input before approval (supported by the hook API but deferred)
- Multiple users / team approval workflows
- Notification-only mode (async pings without blocking — nice-to-have, not MVP)
- Auto-start bot via launchd/systemd (deferred to polish phase)

## Constraints

- Must be a single compiled Rust binary (no runtime dependencies)
- Must work on macOS (aarch64) and Linux (aarch64 + x86_64)
- Must not break Claude Code's normal flow if the bot is unavailable
- Must handle concurrent permission requests (multiple hooks may fire in parallel)
- Hook mode must write ONLY JSON to stdout (logging to stderr only)
