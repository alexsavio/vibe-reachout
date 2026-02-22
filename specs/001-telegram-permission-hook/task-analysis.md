# Task Analysis: Telegram Permission Hook

**Date**: 2026-02-22
**Input**: spec.md, plan.md, tasks.md, contracts/hook-io.md, contracts/ipc.md, clarifications.md

## 1. Coverage Matrix: User Stories → Tasks

Cross-reference every acceptance scenario against tasks to ensure nothing is missed.

### US1: Approve a permission (P1)

| Acceptance Scenario | Covered by | Status |
| --- | --- | --- |
| Telegram message appears within 2s with tool details | T016 (formatter), T015 (telegram send) | ✅ Covered |
| Tapping Allow → Claude Code proceeds | T024 (stdout writer), T015 (callback handler) | ✅ Covered |
| Multiple sessions → independent processing | T014 (DashMap + oneshot per request_id) | ✅ Covered |

### US2: Deny a permission (P1)

| Acceptance Scenario | Covered by | Status |
| --- | --- | --- |
| Tapping Deny → Claude Code blocks tool + shows reason | T024 (deny output), T015 (callback) | ✅ Covered |
| Claude adjusts approach after denial | Automatic (Claude Code behavior, not our code) | ✅ N/A |

### US3: Start the bot (P1)

| Acceptance Scenario | Covered by | Status |
| --- | --- | --- |
| `vibe-reachout bot` starts, binds socket, connects to Telegram | T013, T015, T018 | ✅ Covered |
| Bot already running → error message | T013 (stale socket detection) | ✅ Covered |
| Stale socket → remove and start normally | T013 (stale socket detection), T010 (test) | ✅ Covered |

### US4: Timeout fallback (P2)

| Acceptance Scenario | Covered by | Status |
| --- | --- | --- |
| No response within timeout → hook exits 1, terminal fallback | T017 (bot timeout), T026 (hook timeout), T025 (exit code) | ✅ Covered |
| Late button tap → "already handled" + message edit | T033 (Phase 5 polish) | ✅ Covered |

### US5: Bot down fallback (P2)

| Acceptance Scenario | Covered by | Status |
| --- | --- | --- |
| Socket doesn't exist → exits 1 immediately | T023 (connection refused handling), T025 (exit code) | ✅ Covered |
| Errors to stderr only | T034 (Phase 5), but also implicit in T022-T025 (tracing to stderr) | ⚠️ Partial — logging setup not a separate task |

### US6: Always-allow (P3)

| Acceptance Scenario | Covered by | Status |
| --- | --- | --- |
| Always Allow → `updatedPermissions` applied | T030 (always allow response) | ✅ Covered |
| Empty `permission_suggestions` → button hidden | T016 (formatter), T030 | ✅ Covered |

### US7: Security — authorized users only (P3)

| Acceptance Scenario | Covered by | Status |
| --- | --- | --- |
| Authorized callback → accepted | T015 (callback handler), T012 (test) | ✅ Covered |
| Unauthorized callback → ignored with error | T015, T012, T035 (edge cases) | ✅ Covered |
| Two users tap → first wins | T035 (Phase 5 edge case) | ✅ Covered |

### US8: Install the hook (P3)

| Acceptance Scenario | Covered by | Status |
| --- | --- | --- |
| No existing hooks → hook added | T031, T032, T029 (test) | ✅ Covered |
| Existing hooks → preserved, hook added | T031, T032, T029 (test) | ✅ Covered |
| Already installed → idempotent update | T032, T029 (test) | ✅ Covered |

### Coverage Summary

- **8/8 user stories**: All covered
- **19/19 acceptance scenarios**: 18 fully covered, 1 partial (US5 logging setup)
- **6/6 edge cases**: All addressed (timeout late tap, stale socket, rate limit, bad token, concurrent taps, SIGTERM)

---

## 2. Coverage Matrix: Functional Requirements → Tasks

| Requirement | Covered by | Status |
| --- | --- | --- |
| FR-001: Forward to Telegram within 2s | T013 (socket server), T015 (Telegram send), T016 (formatter) | ✅ Covered |
| FR-002: 10 concurrent requests | T014 (DashMap), T013 (concurrent connections) | ⚠️ No explicit concurrency stress test |
| FR-003: Fall back on any error | T025 (exit code logic), T023 (connection refused) | ✅ Covered |
| FR-004: Validate chat ID | T015 (callback handler), T012 (test) | ✅ Covered |
| FR-005: Preserve settings on install | T031, T032, T029 (test) | ✅ Covered |
| FR-006: Tool-specific message formatting | T016 (formatter), T011 (test) | ✅ Covered |
| FR-007: Handle stale sockets | T013 (detection logic), T010 (test) | ✅ Covered |
| FR-008: Include session context in messages | T016 (includes cwd + session_id) | ✅ Covered |

---

## 3. Coverage Matrix: Contracts → Tasks

### hook-io.md → Tasks

| Contract element | Covered by | Status |
| --- | --- | --- |
| HookInput deserialization (all fields) | T004 (types), T008 (tests), T022 (stdin reader) | ✅ |
| HookOutput serialization (allow) | T004, T008, T024 | ✅ |
| HookOutput serialization (deny + message) | T004, T008, T024 | ✅ |
| HookOutput serialization (always-allow + updatedPermissions) | T004, T008, T030 | ✅ |
| Exit code 0 (success) | T025 | ✅ |
| Exit code 1 (fallback) | T025 | ✅ |
| Exit code 2 (explicit deny) | T025 | ⚠️ Task mentions exit 2 but no user story uses it — see Issue I003 |
| tool_input schemas (Bash, Write, Edit, Read, Task) | T008 (deserialization tests per tool) | ✅ |
| tool_input schemas (Glob, Grep, WebFetch, WebSearch) | T008 | ⚠️ Not explicitly listed in T008 description |
| permission_suggestions parsing | T004 (PermissionSuggestion struct), T008 | ✅ |

### ipc.md → Tasks

| Contract element | Covered by | Status |
| --- | --- | --- |
| IpcRequest serialization | T005 (types), T009 (tests) | ✅ |
| IpcResponse serialization | T005, T009 | ✅ |
| Newline-delimited protocol | T013 (server read), T023 (client write) | ⚠️ Newline delimiter not explicitly mentioned in task descriptions |
| Callback data format `{uuid}:{action}` | T015 (callback handler), T012 (test) | ✅ |
| Concurrent connections | T013, T014 | ✅ |
| Error scenarios (all 5) | T025, T017, T034, T035 | ✅ |

---

## 4. Dependency Analysis

### Internal Task Dependencies

```
T001 ──→ T002, T003, T004, T005, T006 (scaffolding first)
T003 ──→ T007 (config test needs config module)
T004 ──→ T008 (hook_io test needs types)
T005 ──→ T009 (ipc test needs types)
T004, T005 ──→ T013, T014, T015, T016 (bot needs shared types)
T004, T005 ──→ T022, T023, T024 (hook needs shared types)
T013 ──→ T014 (server depends on pending map)
T014 ──→ T015, T017 (Telegram + timeout depend on pending)
T015, T016 ──→ T018 (bot entry point wires everything)
T022 ──→ T023 ──→ T024 ──→ T025 (hook is sequential: read → connect → write → exit)
T018, T027 ──→ T040 (E2E needs both bot and hook working)
```

### Issues Found

**D001: T006 (init) depends on T003 (config) but isn't marked**
The `init` subcommand writes a config file — it needs to know the Config struct and default paths from `config.rs`. T006 should depend on T003.

**D002: T017 (timeout) depends on T015 (Telegram) for message editing**
The timeout handler needs to edit the Telegram message to show "⏱️ Timed out". This requires access to the Telegram bot instance and the `message_id` stored alongside the pending request. T017 should depend on T015.

**D003: T030 (always allow) spans both hook.rs AND bot/telegram.rs**
The task description says "In bot: callback data `{request_id}:always` triggers this path." This means T030 touches two modules. Should be split or at least acknowledged as cross-cutting.

---

## 5. Issues Found

### I001: Missing task — tracing/logging setup

**Severity**: Medium
**Description**: No task covers setting up the `tracing-subscriber` configuration. Hook mode needs stderr-only output; bot mode needs stdout output. This is a cross-cutting concern that affects both modes. The plan documents this (Logging section) but no task implements it.
**Recommendation**: Add T041 in Phase 1: "Configure tracing-subscriber: hook mode → stderr writer, bot mode → stdout writer. RUST_LOG env var support. Ensure hook mode never writes non-JSON to stdout."

### I002: Missing task — `types/mod.rs`

**Severity**: Low
**Description**: The plan shows `src/types/mod.rs` as the module root, but no task creates it. T004 and T005 create `hook_io.rs` and `ipc.rs` but the `mod.rs` that re-exports them is implied.
**Recommendation**: Add to T004 description: "Also create `src/types/mod.rs` re-exporting types from hook_io and ipc."

### I003: Exit code 2 is mentioned but no user story uses it

**Severity**: Low
**Description**: T025 mentions exit code 2 for "explicit deny" but no user story or data flow in the plan uses exit code 2. The hook always exits 0 (with allow/deny JSON) or 1 (fallback). Exit code 2 would mean "deny without JSON" — but the deny path already uses exit 0 with deny JSON.
**Recommendation**: Remove exit code 2 from T025. The hook should only ever exit 0 (success with JSON) or 1 (any error, triggers terminal fallback). If future use cases need explicit deny-via-exit-code, it can be added then.

### I004: No task for Telegram API error handling / retry

**Severity**: Medium
**Description**: Edge case "What if Telegram API is rate-limited?" says "Bot retries with exponential backoff; hook keeps waiting." But no task implements retry logic for Telegram API calls (sending messages, answering callbacks). teloxide handles some retries internally, but we should verify and configure it.
**Recommendation**: Add to T015 description: "Configure teloxide retry policy for Telegram API errors (rate limits, network issues). Verify default retry behavior or add exponential backoff."

### I005: FR-002 (10 concurrent requests) has no stress/load test

**Severity**: Low
**Description**: The spec requires supporting 10 concurrent requests, but no test validates this. The integration tests (T019, T027, T040) test single-request flows.
**Recommendation**: Add to T040 (E2E test): "Include a concurrent scenario: spawn 5+ hook processes simultaneously, each with different request_ids. Verify all receive independent responses."

### I006: `IpcRequest` missing `session_id` field in contracts/ipc.md

**Severity**: Medium
**Description**: The plan says Telegram messages should include `session_id (first 8 chars)` for multi-session disambiguation (FR-008). The task T005 includes `session_id` in IpcRequest. But `contracts/ipc.md` does NOT list `session_id` as a field in the IpcRequest schema. The contract and the task disagree with the contract doc.
**Recommendation**: Update `contracts/ipc.md` IpcRequest to include `session_id` field. Mark it as required.

### I007: T011 (formatter test) doesn't test WebFetch/WebSearch tools

**Severity**: Low
**Description**: The plan lists 8 tool formatters (Bash, Write, Edit, Read, Glob, Grep, Task, MCP). T011 lists "Bash, Write, Edit, Read, Glob, Grep, Task, MCP" but the contract also lists WebFetch and WebSearch as tools that can trigger permission prompts. No formatter specified for them.
**Recommendation**: Add WebFetch (`URL: {url}`) and WebSearch (`Query: {query}`) to the formatter table in the plan and to T011/T016.

### I008: T006 (init) — interactive mode needs a Telegram API dependency

**Severity**: Low
**Description**: The `init` command could optionally validate the bot token by calling `getMe` on the Telegram API. This isn't in the task description but would be a nice UX (immediate feedback: "Connected to bot @YourBot"). However, this adds complexity to what should be a simple config writer.
**Recommendation**: Keep T006 simple — write config, no API validation. Add a `vibe-reachout check` or `vibe-reachout bot --dry-run` later if needed.

### I009: Callback data size limit

**Severity**: Medium
**Description**: Telegram callback data has a 64-byte limit. A UUID v4 is 36 chars + `:` + action (5-6 chars) = ~43 bytes. This fits, but barely. If future actions are added (e.g., `allow_with_edit`), it could overflow.
**Recommendation**: Document the 64-byte constraint in contracts/ipc.md under "Telegram Callback Data". Consider using a shorter request ID format (e.g., 8-char hex from UUID) if needed. Add a unit test in T012 verifying callback data stays under 64 bytes for all action types.

### I010: No `uninstall` subcommand

**Severity**: Low
**Description**: `vibe-reachout install` adds the hook. There's no `vibe-reachout uninstall` to remove it. Not critical but would be nice for cleanup.
**Recommendation**: Not needed for MVP. Add to a future "Nice to Have" section if desired.

---

## 6. Sizing & Risk Assessment

### Task Effort Estimates

| Phase | Tasks | Estimated Effort | Risk Level |
| --- | --- | --- | --- |
| Phase 1: Setup | T001–T009 (9 tasks) | Small — standard Rust scaffolding + serde types | Low |
| Phase 2: Bot | T010–T019 (10 tasks) | **Large** — socket server + Telegram integration + concurrency | **High** — teloxide API surface, async concurrency, message editing |
| Phase 3: Hook | T020–T027 (8 tasks) | Medium — stdin/stdout bridge + socket client | Low — straightforward pipe |
| Phase 4: Install | T028–T032 (5 tasks) | Small — JSON merge + simple serde | Low |
| Phase 5: Polish | T033–T040 (8 tasks) | Medium — CI/CD setup + edge case handling | Medium — cross-compile matrix |

### Risk Areas

**R001: teloxide callback query API complexity** — Phase 2, T015
teloxide's dispatcher and callback query handling is well-documented but has a specific pattern. Risk: learning curve and potential API misuse. Mitigation: study teloxide examples/docs for callback queries before starting T015.

**R002: Concurrent socket server + Telegram bot in same process** — Phase 2, T018
Running both a Unix socket listener and a Telegram long-polling bot in the same tokio runtime requires careful task management. Risk: one blocking the other, or shutdown ordering issues. Mitigation: use `tokio::select!` in the bot entry point, with proper cancellation tokens.

**R003: Stale socket detection race condition** — Phase 2, T013
Between checking if the socket is stale and deleting it, another bot process could start. Risk: two bots binding to the same socket. Mitigation: use file locking (`flock`) or a PID file alongside the socket. Low probability in practice (single-user tool).

**R004: Integration test reliability** — Phase 3, T027
The full-flow integration test (start bot → pipe stdin → simulate callback → verify stdout) requires careful orchestration. Risk: flaky test due to timing. Mitigation: use channels/barriers for synchronization rather than sleeps.

---

## 7. Recommended Changes to tasks.md

### Add Tasks

1. **T041** (Phase 1): Configure tracing-subscriber for both modes (hook → stderr, bot → stdout). RUST_LOG support. [Addresses I001]

### Modify Tasks

2. **T005**: Add `session_id: String` to IpcRequest struct. [Addresses I006]
3. **T006**: Add dependency note: "Depends on T003 (config module)." [Addresses D001]
4. **T008**: Expand description: "...for Bash, Write, Edit, Read, Glob, Grep, WebFetch, WebSearch, Task tools." [Addresses I007]
5. **T011/T016**: Add WebFetch (`URL: {url}`) and WebSearch (`Query: {query}`) formatters. [Addresses I007]
6. **T012**: Add test case: "Verify callback data for all action types stays under 64 bytes." [Addresses I009]
7. **T015**: Add: "Configure teloxide retry policy for rate limits and network errors." [Addresses I004]
8. **T017**: Note dependency on T015 (needs message_id for editing). [Addresses D002]
9. **T025**: Remove exit code 2 — hook only exits 0 or 1. [Addresses I003]
10. **T030**: Note this task is cross-cutting (touches both hook.rs and bot/telegram.rs). [Addresses D003]
11. **T040**: Add concurrent scenario: "5+ simultaneous hook processes with independent responses." [Addresses I005]

### Update Contracts

12. **contracts/ipc.md**: Add `session_id` to IpcRequest schema (required field). Add 64-byte callback data constraint note. [Addresses I006, I009]
