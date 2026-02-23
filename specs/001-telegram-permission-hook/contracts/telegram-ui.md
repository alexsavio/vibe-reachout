# Contract: Telegram Message UI

## Overview

The bot sends formatted permission request messages to authorized Telegram chats with inline keyboard buttons for user interaction.

## Message Format

### Template

```
📋 {project_name}

🔧 {tool_name}
{tool_details}

📁 {cwd}
🆔 Session: {session_id_short}
```

### Tool-specific formatting

**Bash**:
```
📋 my-project

🔧 Bash
```
npm install
```

📁 /Users/dev/my-project
🆔 Session: abc123
```

**Write**:
```
📋 my-project

🔧 Write
📄 src/main.rs (1.2 KB)

📁 /Users/dev/my-project
🆔 Session: abc123
```

**Edit**:
```
📋 my-project

🔧 Edit
📄 src/main.rs
- old text snippet...
+ new text snippet...

📁 /Users/dev/my-project
🆔 Session: abc123
```

### Truncation rules

- Command/content: max 500 chars, truncated with `... (truncated)`
- Total message: max 4000 chars (Telegram limit is 4096; leave margin for status suffix)
- Project name: derived from last path component of `cwd`

## Inline Keyboard Buttons

### Standard layout (with permission_suggestions)

```
[ ✅ Allow ] [ ❌ Deny ] [ 💬 Reply ] [ 🔓 Always Allow ]
```

### Without permission_suggestions

```
[ ✅ Allow ] [ ❌ Deny ] [ 💬 Reply ]
```

### Callback data format

`{request_id}:{action}`

| Action | Callback data example |
|--------|----------------------|
| Allow | `550e8400-e29b-41d4-a716-446655440000:allow` |
| Deny | `550e8400-e29b-41d4-a716-446655440000:deny` |
| Reply | `550e8400-e29b-41d4-a716-446655440000:reply` |
| Always Allow | `550e8400-e29b-41d4-a716-446655440000:always` |

Max callback_data size: 43 bytes (UUID 36 + colon 1 + action 6). Within Telegram's 64-byte limit.

## Post-Resolution Message Edits

After the user responds, the original message is edited to append a status line and remove the keyboard:

| Resolution | Status appended | Buttons |
|------------|----------------|---------|
| Allow | `\n\n✅ Approved` | Removed |
| Deny | `\n\n❌ Denied` | Removed |
| Always Allow | `\n\n🔓 Always Allowed` | Removed |
| Reply | `\n\n💬 Replied` | Removed |
| Timeout | `\n\n⏱️ Timed out` | Removed |

## Reply Flow

1. User taps "💬 Reply" button
2. Bot answers callback query (dismiss spinner)
3. Bot sends a new message with `ForceReply` markup: "Type your reply:"
4. Bot tracks `(chat_id → request_id)` in reply state map
5. User types and sends text
6. Bot receives Message update, matches via reply state map
7. Bot resolves pending request with Reply decision + user_message
8. Bot edits original permission message to show "💬 Replied"
9. Bot deletes or acknowledges the ForceReply prompt message

### Empty reply handling

If user sends empty text, bot re-prompts with ForceReply. Reply state remains active.

## Late Interaction Handling

If a user taps any button after the request has been resolved or timed out:
- `answer_callback_query` with `text: "This request has already been handled"` and `show_alert: true`
- No message edit (already showing final status)

## Authorization

Before processing any callback_query or message:
1. Extract `chat_id` from the update
2. Check against `config.allowed_chat_ids`
3. If unauthorized: answer callback with error toast, ignore message, log warning to stderr
