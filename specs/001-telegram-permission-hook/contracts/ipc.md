# Contract: IPC Protocol (Unix Domain Socket)

## Overview

Communication between hook mode and bot mode processes uses a Unix domain socket. The bot process is the server; each hook invocation is a client.

Socket path: configurable, default `/tmp/vibe-reachout.sock`.

## Protocol

1. Hook connects to Unix socket
2. Hook sends newline-delimited JSON (`IpcRequest` + `\n`)
3. Bot processes request (sends Telegram message, waits for callback)
4. Bot sends newline-delimited JSON response (`IpcResponse` + `\n`)
5. Hook reads response, closes connection

Each connection handles exactly one request-response pair.

## IpcRequest

Sent from hook mode → bot mode.

```json
{
  "request_id": "uuid-v4",
  "tool_name": "Bash",
  "tool_input": {
    "command": "rm -rf node_modules",
    "description": "Remove node_modules directory"
  },
  "cwd": "/Users/alex/my-project",
  "session_id": "abc12345-def6-7890-ghij-klmnopqrstuv",
  "permission_suggestions": [
    { "type": "toolAlwaysAllow", "tool": "Bash" }
  ]
}
```

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `request_id` | UUID v4 string | yes | Unique ID for correlating response |
| `tool_name` | string | yes | Claude Code tool name |
| `tool_input` | object | yes | Tool parameters (varies by tool) |
| `cwd` | string | yes | Working directory of the Claude Code session |
| `session_id` | string | yes | Claude Code session ID (for multi-session disambiguation, FR-008) |
| `permission_suggestions` | array | no | Available always-allow options |

## IpcResponse

Sent from bot mode → hook mode.

```json
{
  "request_id": "uuid-v4",
  "decision": "allow",
  "message": null,
  "always_allow_suggestion": null
}
```

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `request_id` | UUID v4 string | yes | Matches the request |
| `decision` | `"allow"` \| `"deny"` \| `"timeout"` | yes | User's decision |
| `message` | string \| null | no | Reason (used for deny) |
| `always_allow_suggestion` | object \| null | no | Permission suggestion to apply (from `permission_suggestions`) |

### Decision values

| Value | Hook behavior |
| --- | --- |
| `"allow"` | Exit 0, output allow JSON |
| `"deny"` | Exit 0, output deny JSON with message |
| `"timeout"` | Exit 1, no output (Claude Code falls back to terminal) |

When `always_allow_suggestion` is present alongside `"allow"`, the hook includes `updatedPermissions` in the Claude Code output.

## Telegram Callback Data

Encoded in inline keyboard button callback data. Format: `{request_id}:{action}`

| Action | Meaning |
| --- | --- |
| `allow` | Approve this tool call |
| `deny` | Deny this tool call |
| `always` | Approve and always allow this tool |

Example: `550e8400-e29b-41d4-a716-446655440000:allow`

**Size constraint**: Telegram limits callback data to 64 bytes. UUID v4 (36 chars) + `:` (1 char) + action (max 6 chars = `always`) = 43 bytes max. Safe, but keep future action names short.

## Concurrency

- The bot handles multiple simultaneous socket connections
- Each connection maps to one pending Telegram message
- Pending requests stored in `DashMap<Uuid, oneshot::Sender<IpcResponse>>`
- Telegram callbacks resolve the pending request via the oneshot channel
- The hook process blocks on the oneshot receiver until resolved or timed out

## Error Handling

| Scenario | Bot behavior | Hook behavior |
| --- | --- | --- |
| Socket connection refused | N/A | Exit 1 (fallback) |
| Malformed IPC request | Close connection | Exit 1 (fallback) |
| Telegram API error | Send timeout response | Exit 1 (fallback) |
| Request timeout | Send `decision: "timeout"` | Exit 1 (fallback) |
| Unauthorized chat ID | Ignore callback, answer with error | No effect (still waiting) |
