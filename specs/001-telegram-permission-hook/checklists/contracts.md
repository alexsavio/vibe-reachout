# Contracts Quality Checklist: Telegram Permission Hook

**Purpose**: Validate completeness, clarity, and consistency of contracts/hook-io.md and contracts/ipc.md
**Created**: 2026-02-22
**Feature**: [hook-io.md](../contracts/hook-io.md), [ipc.md](../contracts/ipc.md)

## Requirement Completeness

- [ ] CHK042 Are all Claude Code HookInput fields documented? The contract lists session_id, transcript_path, cwd, permission_mode, hook_event_name, tool_name, tool_input, permission_suggestions — does the real Claude Code API send additional fields? [Completeness, contracts/hook-io.md]
- [ ] CHK043 Are `tool_input` schemas documented for ALL tools that trigger PermissionRequest? WebFetch and WebSearch were added late (I007) — are any others missing? [Completeness, contracts/hook-io.md]
- [ ] CHK044 Is the HookOutput schema for `updatedInput` (tool input modification) documented even though it's a non-goal? The hook API supports it — should the contract note it as unsupported? [Completeness, contracts/hook-io.md]
- [ ] CHK045 Are error response formats defined for the IPC protocol? What JSON (if any) does the bot send when it receives a malformed IpcRequest? [Completeness, contracts/ipc.md §Error Handling]
- [ ] CHK046 Is the IPC protocol version or handshake defined? If the binary is updated on one side but not the other, how is incompatibility detected? [Completeness, Gap]

## Requirement Clarity

- [ ] CHK047 Is "newline-delimited JSON" precisely defined? Is it `\n` (LF) or `\r\n` (CRLF)? What about trailing whitespace? [Clarity, contracts/ipc.md §Protocol]
- [ ] CHK048 Is the maximum size of an IpcRequest/IpcResponse defined? Could a very large `tool_input` (e.g., Write with 10000-line content) cause issues? [Clarity, contracts/ipc.md]
- [ ] CHK049 Is the `permission_suggestions` array structure fully specified? The contract shows one example (`toolAlwaysAllow`) — are there other types? What fields does each type have? [Clarity, contracts/hook-io.md]
- [ ] CHK050 Is the `always_allow_suggestion` field in IpcResponse clearly specified as echoing one entry from the request's `permission_suggestions`? [Clarity, contracts/ipc.md §IpcResponse]
- [ ] CHK051 Is the behavior of `message` field in IpcResponse for `allow` decisions defined? Is it always null, or can it carry information? [Clarity, contracts/ipc.md]

## Requirement Consistency

- [ ] CHK052 Is the `decision` field naming consistent? IpcResponse uses `"allow"/"deny"/"timeout"`, HookOutput uses `behavior: "allow"/"deny"` — is the mapping between them documented in one place? [Consistency, contracts/hook-io.md vs contracts/ipc.md]
- [ ] CHK053 Are the `permission_suggestions` field names consistent between HookInput (from Claude Code) and IpcRequest (forwarded to bot)? Are they passed through unchanged? [Consistency, contracts/hook-io.md vs contracts/ipc.md]
- [ ] CHK054 Is the callback data format `{request_id}:{action}` consistent with the IpcResponse `decision` values? Callback uses "always" but IpcResponse decision is "allow" with `always_allow_suggestion` — is this mapping documented? [Consistency, contracts/ipc.md §Callback Data vs §IpcResponse]

## Schema Validation

- [ ] CHK055 Are JSON schema types explicitly specified for all fields? `tool_input` is "object" but its shape varies by tool — is this variability documented with examples for each tool? [Clarity, contracts/hook-io.md]
- [ ] CHK056 Are nullable fields clearly marked? `message` and `always_allow_suggestion` are `null | T` — is this JSON `null` or field-absent? [Clarity, contracts/ipc.md]
- [ ] CHK057 Are required vs optional fields consistently marked across both contracts? [Consistency, contracts/hook-io.md vs contracts/ipc.md]
- [ ] CHK058 Is the UUID format specified precisely (v4 random, lowercase hex, with hyphens)? [Clarity, contracts/ipc.md §IpcRequest]

## Protocol Robustness

- [ ] CHK059 Are requirements defined for partial read/write on the socket? (e.g., hook crashes mid-write, bot reads incomplete JSON) [Coverage, contracts/ipc.md §Error Handling]
- [ ] CHK060 Are requirements defined for connection keepalive or socket timeout? If the bot is slow to respond, does the socket connection stay open indefinitely? [Coverage, Gap]
- [ ] CHK061 Is the behavior defined when multiple `\n`-separated JSON objects are sent on the same connection? The protocol says "exactly one request-response pair" — is this enforced? [Clarity, contracts/ipc.md §Protocol]
- [ ] CHK062 Are requirements defined for the maximum number of concurrent socket connections the bot should accept? FR-002 says 10 requests, but is this enforced at the socket level? [Coverage, Spec §FR-002]

## Notes

- Check items off as completed: `[x]`
- Add inline comments with findings or resolutions
- Items numbered continuing from requirements.md (CHK042+)
- `[Gap]` markers indicate missing contract specifications
