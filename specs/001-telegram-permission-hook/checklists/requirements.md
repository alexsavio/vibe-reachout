# Requirements Quality Checklist: Telegram Permission Hook

**Purpose**: Validate completeness, clarity, and consistency of spec.md, plan.md, and tasks.md requirements
**Created**: 2026-02-22
**Feature**: [spec.md](../spec.md)

## Requirement Completeness

- [ ] CHK001 Are requirements defined for ALL tools that can trigger PermissionRequest? The spec lists Bash, Write, Edit, Read, Glob, Grep, Task, MCP, WebFetch, WebSearch — are there others Claude Code may add in the future? [Completeness, Spec §FR-006]
- [ ] CHK002 Are requirements defined for what happens when `tool_input` contains unexpected/unknown fields? [Completeness, Gap]
- [ ] CHK003 Are requirements defined for the bot's behavior when Telegram API is completely unreachable (not just rate-limited)? [Completeness, Spec §Edge Cases]
- [ ] CHK004 Are requirements defined for config file permissions/ownership validation? A world-readable config.toml exposes the bot token. [Completeness, Gap]
- [ ] CHK005 Are requirements specified for what the hook outputs when `tool_name` is unrecognized by the formatter? [Completeness, Plan §Formatting]
- [ ] CHK006 Are graceful shutdown requirements defined for bot mode — specifically the order of operations (drain pending → close socket → exit)? [Completeness, Plan §Signal Handling]
- [ ] CHK007 Are requirements defined for bot startup validation (e.g., verify token works via `getMe` before binding socket)? [Completeness, Gap]

## Requirement Clarity

- [ ] CHK008 Is "within 2 seconds" (FR-001) measured from stdin read to Telegram API call, or to message delivery on user's device? [Clarity, Spec §FR-001]
- [ ] CHK009 Is "up to 10 concurrent permission requests" (FR-002) a hard limit or a design target? What happens at request 11? [Clarity, Spec §FR-002]
- [ ] CHK010 Is "fall back to terminal prompt on any error condition" specific enough? Does "any error" include partial errors like Telegram message sent but callback never received? [Clarity, Spec §FR-003]
- [ ] CHK011 Is the timeout hierarchy clearly specified? Hook has client-side timeout, bot has per-request timeout, Claude Code has 600s hook timeout — which fires first and what's the cascade? [Clarity, Plan §Architecture]
- [ ] CHK012 Is "stale socket" precisely defined? The plan says "try connect, if fails → stale" but what if the connect hangs? Is there a connection timeout for the staleness check? [Clarity, Plan §IPC]
- [ ] CHK013 Is the "first matching entry" from `permission_suggestions` for Always Allow clearly defined? What constitutes a "match" — tool name, type, or both? [Clarity, Clarifications §C003]

## Requirement Consistency

- [ ] CHK014 Are exit code semantics consistent between spec.md ("exit code 1"), plan.md ("exit 0 or 1"), and contracts/hook-io.md? The task analysis removed exit code 2 — is this reflected everywhere? [Consistency, Spec §FR-003 vs Plan §Data Flow]
- [ ] CHK015 Are timeout values consistent? Plan says "default 300s", Claude Code hook timeout is 600s, hook client-side timeout is "slightly less than 600s" — are these documented in one place? [Consistency, Plan §Configuration vs §Hook Timeout]
- [ ] CHK016 Does the IPC contract's `session_id` field align with how the hook extracts it from HookInput? HookInput has `session_id` at the top level — is the mapping documented? [Consistency, contracts/hook-io.md vs contracts/ipc.md]
- [ ] CHK017 Are `allowed_chat_ids` semantics consistent? Config uses an array, but the plan says "send to all allowed_chat_ids" while callbacks validate against the list. Is the send-to-all behavior explicitly required? [Consistency, Plan §Data Flow]
- [ ] CHK018 Is the message format template in plan.md consistent with the formatter table? The template shows `Project:` but the formatter table doesn't include `cwd` as a tool-specific field. [Consistency, Plan §Formatting]

## Acceptance Criteria Quality

- [ ] CHK019 Are US1 acceptance scenarios measurable? "Telegram message appears within 2s" — is this testable in CI without a real Telegram API? [Measurability, Spec §US1]
- [ ] CHK020 Are US3 acceptance scenarios specific enough for "bot starts, binds socket, connects to Telegram"? What constitutes a successful Telegram connection — `getMe` response, first long poll, or something else? [Measurability, Spec §US3]
- [ ] CHK021 Are US7 acceptance scenarios complete? "Attacker sees error" — what error exactly? Is the error message defined? [Measurability, Spec §US7]
- [ ] CHK022 Is SC-004 ("Binary size under 20MB") achievable given teloxide + tokio + serde dependencies? Has this been validated against similar Rust projects? [Measurability, Spec §SC-004]

## Scenario Coverage

- [ ] CHK023 Are requirements defined for the scenario where the user has multiple Telegram devices and taps on different devices? [Coverage, Gap]
- [ ] CHK024 Are requirements defined for bot restart while hooks are connected and waiting? (Socket connections would be dropped mid-flight) [Coverage, Gap]
- [ ] CHK025 Are requirements defined for config file changes while the bot is running? (Hot-reload vs restart-required) [Coverage, Gap]
- [ ] CHK026 Are requirements defined for the `install` command when Claude Code settings.json is malformed/corrupt JSON? [Coverage, Spec §US8]
- [ ] CHK027 Are requirements defined for what happens when the bot sends a Telegram message but the message ID is not returned (API partial failure)? [Coverage, Gap]
- [ ] CHK028 Are requirements defined for hook behavior when stdin is empty or not valid JSON but not completely malformed (e.g., truncated JSON)? [Coverage, Plan §Hook Mode]

## Edge Case Coverage

- [ ] CHK029 Is the behavior specified when `allowed_chat_ids` is empty in the config? Should the bot refuse to start, or accept all? [Edge Case, Gap]
- [ ] CHK030 Is the behavior specified when the socket path directory doesn't exist? (e.g., `$XDG_RUNTIME_DIR` not set on Linux) [Edge Case, Plan §IPC]
- [ ] CHK031 Is the behavior specified for very long `tool_input` values? The formatter truncates to 100/200 chars, but is there a max message size for Telegram (4096 chars)? [Edge Case, Plan §Formatting]
- [ ] CHK032 Is the behavior specified when the config file exists but has incorrect permissions (e.g., 777)? [Edge Case, Gap]
- [ ] CHK033 Are requirements defined for Unicode/emoji in tool_input fields (e.g., file paths with special characters)? [Edge Case, Gap]

## Non-Functional Requirements

- [ ] CHK034 Are memory requirements under load specified beyond "under 50MB idle, under 100MB with 10 pending"? What about memory growth over time (potential leaks from resolved requests)? [NFR, Spec §SC-005]
- [ ] CHK035 Are log format requirements specified? Should logs be structured (JSON) or human-readable? [NFR, Gap]
- [ ] CHK036 Are requirements specified for the bot's CPU usage during long-polling idle periods? [NFR, Gap]
- [ ] CHK037 Is the minimum supported Rust version (MSRV) documented? Plan says "Rust 1.75+" but teloxide may require newer. [NFR, Plan §Technical Context]

## Dependencies & Assumptions

- [ ] CHK038 Is the assumption that teloxide handles Telegram API retries internally validated? The task analysis flagged this (I004) — is the behavior documented? [Assumption, Task Analysis §I004]
- [ ] CHK039 Is the assumption that Unix domain sockets work identically on macOS and Linux validated? Are there platform-specific socket path length limits (108 chars on Linux)? [Assumption, Plan §IPC]
- [ ] CHK040 Is the dependency on `dirs` crate for XDG paths documented with fallback behavior when XDG vars are unset? [Dependency, Plan §Configuration]
- [ ] CHK041 Is the assumption that Claude Code's `PermissionRequest` hook API is stable documented? What happens if Anthropic changes the hook I/O schema? [Assumption, contracts/hook-io.md]

## Notes

- Check items off as completed: `[x]`
- Add inline comments with findings or resolutions
- Items reference spec sections with `[Spec §X]` notation
- `[Gap]` markers indicate missing requirements that should be added
