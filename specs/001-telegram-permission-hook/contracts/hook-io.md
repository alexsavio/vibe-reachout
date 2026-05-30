# Contract: Claude Code Hook I/O

Source: [Claude Code Hooks Reference](https://code.claude.com/docs/en/hooks)

_Retrieved: 2026-05-30._ Re-verify against the live docs if the hook stops firing or decisions stop applying; the protocol is an external moving spec (the docs host already moved `docs.claude.com` → `code.claude.com`, and new events/modes are added over time).

## Hook Event: `PermissionRequest`

Fires when Claude Code is about to show a permission dialog to the user.

### Input (stdin JSON)

```json
{
  "session_id": "string",
  "transcript_path": "string (absolute path)",
  "cwd": "string (absolute path)",
  "permission_mode": "default" | "plan" | "acceptEdits" | "auto" | "dontAsk" | "bypassPermissions",
  "hook_event_name": "PermissionRequest",
  "tool_name": "string",
  "tool_input": { ... },
  "permission_suggestions": [
    { "type": "string", "tool": "string", ... }
  ]
}
```

#### `tool_name` values

| Tool | Description |
| --- | --- |
| `Bash` | Shell command execution |
| `Write` | File creation/overwrite |
| `Edit` | String replacement in file |
| `Read` | File reading |
| `Glob` | File pattern matching |
| `Grep` | Content search |
| `WebFetch` | URL fetching |
| `WebSearch` | Web search |
| `Task` | Subagent spawning |
| `mcp__<server>__<tool>` | MCP server tools |

#### `tool_input` schemas by tool

**Bash:**
```json
{
  "command": "string",
  "description": "string (optional)",
  "timeout": "number (optional, ms)",
  "run_in_background": "boolean (optional)"
}
```

**Write:**
```json
{
  "file_path": "string (absolute path)",
  "content": "string"
}
```

**Edit:**
```json
{
  "file_path": "string (absolute path)",
  "old_string": "string",
  "new_string": "string",
  "replace_all": "boolean (optional)"
}
```

**Read:**
```json
{
  "file_path": "string (absolute path)",
  "offset": "number (optional)",
  "limit": "number (optional)"
}
```

**Task:**
```json
{
  "prompt": "string",
  "description": "string",
  "subagent_type": "string",
  "model": "string (optional)"
}
```

#### `permission_suggestions` examples

```json
[
  { "type": "toolAlwaysAllow", "tool": "Bash" },
  { "type": "toolAlwaysAllow", "tool": "Write" }
]
```

These are the "always allow" options the user would see in the terminal dialog.

### Output (stdout JSON)

#### Allow

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PermissionRequest",
    "decision": {
      "behavior": "allow"
    }
  }
}
```

#### Deny

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PermissionRequest",
    "decision": {
      "behavior": "deny",
      "message": "string (reason shown to Claude)"
    }
  }
}
```

#### Allow with Always-Allow permission

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PermissionRequest",
    "decision": {
      "behavior": "allow",
      "updatedPermissions": [
        { "type": "toolAlwaysAllow", "tool": "Bash" }
      ]
    }
  }
}
```

#### Deny with user reply (free-text)

When the user taps "Reply" on Telegram and sends free-text, the hook denies the tool call but includes the user's message so Claude can read it as feedback.

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PermissionRequest",
    "decision": {
      "behavior": "deny",
      "message": "User replied: Use yarn instead of npm for this project"
    }
  }
}
```

#### Allow with modified input (optional, not in MVP)

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PermissionRequest",
    "decision": {
      "behavior": "allow",
      "updatedInput": {
        "command": "modified command"
      }
    }
  }
}
```

### Exit Codes

| Code | Meaning | Claude Code behavior |
| --- | --- | --- |
| 0 | Success — read JSON from stdout | Applies the decision from JSON |
| 2 | Blocking error — read stderr | Denies the permission, shows stderr to user |
| Other | Non-blocking error | Falls back to normal terminal permission dialog |

### Hook Configuration

In `~/.claude/settings.json`:

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

- No `matcher` field — matches all permission requests
- `timeout: 600` — 10 minutes for user to respond on phone
- No `async` — hook must block (Claude Code waits for the decision)

### Unverified against live docs

`decision.message` (on `behavior: "deny"`) is how the denial reason and the user's free-text Reply reach Claude. The published reference does not explicitly enumerate a `message`/`reason` field inside the PermissionRequest `decision` object, so its delivery to Claude is **not confirmed from docs alone**. Verify with a live run (see `quickstart.md` → "Verify the Reply path"). If a future docs revision renames it (e.g. to `reason`), update `HookOutput` in `src/models.rs`.

### Related events not yet used

`PermissionDenied` (`hookSpecificOutput.retry: true`) fires when an auto-mode denial occurs and lets a hook request a retry. Candidate future enhancement: route auto-mode denials to the phone for a retry decision. Out of scope for the current hook.
