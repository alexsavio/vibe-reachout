# vibe-reachout

[![CI](https://github.com/alexsavio/vibe-reachout/actions/workflows/ci.yml/badge.svg)](https://github.com/alexsavio/vibe-reachout/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/vibe-reachout.svg)](https://crates.io/crates/vibe-reachout)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Approve Claude Code permission prompts from your phone via Telegram -- so you can walk away from the terminal and let it keep coding.

## The Problem

Claude Code's autonomous "vibe coding" sessions are powerful: kick off a task, walk away, come back to working code. Except they don't actually work that way. Claude Code blocks on permission prompts every time it needs to run a shell command, write a file, or edit code. You have to sit at the terminal and babysit it, which defeats the entire purpose.

**vibe-reachout** fixes this. It intercepts permission requests, forwards them to a Telegram bot, and lets you approve or deny from your phone. Claude Code keeps working; you keep living your life.

## How It Works

```text
                         +-----------------+
                         |   Claude Code   |
                         |   (terminal)    |
                         +--------+--------+
                                  |
                          spawns hook process
                          (stdin: JSON)
                                  |
                                  v
                         +--------+--------+
                         |  vibe-reachout  |
                         |   (hook mode)   |
                         +--------+--------+
                                  |
                          connects via
                          Unix domain socket
                          (NDJSON protocol)
                                  |
                                  v
                         +--------+--------+
                         |  vibe-reachout  |
                         |   (bot mode)    |
                         +--------+--------+
                                  |
                          Telegram Bot API
                          (HTTPS)
                                  |
                                  v
                         +--------+--------+
                         |    Telegram     |
                         |  (your phone)   |
                         +--------+--------+
                                  |
                          user taps button
                                  |
                                  v
                         response flows back:
                    Telegram -> bot -> socket -> hook
                                  |
                                  v
                         +--------+--------+
                         |   Claude Code   |
                         |   continues...  |
                         +-----------------+
```

The system runs as two processes:

- **Bot process** (`vibe-reachout bot`) -- long-running daemon that listens on a Unix socket and communicates with Telegram. Start it once and leave it running.
- **Hook process** (`vibe-reachout` with no subcommand) -- short-lived, spawned by Claude Code for each permission request. Reads JSON from stdin, forwards it to the bot over the socket, waits for a response, and writes the decision to stdout.

Communication between the two processes uses newline-delimited JSON (NDJSON) over a Unix domain socket.

## Features

- **Approve or deny** tool calls from Telegram with a single tap
- **Reply with free text** when Claude Code needs more than a yes/no (API keys, clarifications, design choices)
- **Always Allow** a tool type for the rest of the session (when Claude Code provides permission suggestions)
- **Timeout fallback** -- if you don't respond within the configured timeout (default: 300s), the hook exits and Claude Code falls back to the terminal prompt
- **Bot-down fallback** -- if the bot process isn't running, the hook exits immediately and Claude Code shows the normal terminal prompt. The tool never breaks your workflow
- **Security** -- only your authorized Telegram chat IDs can respond to permission requests. Unauthorized users are ignored
- **Multi-device support** -- configure multiple chat IDs to receive prompts on your phone and desktop Telegram simultaneously. First response wins
- **Rich formatting** -- tool-specific message formatting: Bash commands in code blocks, file paths and sizes for Write, diffs for Edit
- **One-command install** -- `vibe-reachout install` registers the hook in Claude Code settings automatically
- **Single binary** -- no runtime dependencies, compiles to one static executable

## Prerequisites

- **Rust 1.93+** -- install or update via [rustup](https://rustup.rs/): `rustup update stable`
- **Telegram bot token** -- create a bot by talking to [@BotFather](https://t.me/BotFather) on Telegram. Save the token it gives you.
- **Your Telegram chat ID** -- get it by messaging [@userinfobot](https://t.me/userinfobot) on Telegram. It returns a numeric ID like `123456789`.

## Installation

### From crates.io

```bash
cargo install vibe-reachout
```

### Build from source

```bash
git clone https://github.com/alexsavio/vibe-reachout.git
cd vibe-reachout
cargo install --path .
```

## Configuration

Create the configuration file at `~/.config/vibe-reachout/config.toml`:

```bash
mkdir -p ~/.config/vibe-reachout
```

```toml
# Required: your Telegram bot token from @BotFather
telegram_bot_token = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"

# Required: list of authorized Telegram chat IDs
# Get yours from @userinfobot. Multiple IDs let you respond from
# different devices (phone, desktop, etc.) -- first response wins.
allowed_chat_ids = [123456789]

# Optional: seconds to wait for a Telegram response before falling
# back to the terminal prompt. Must be between 1 and 3600.
# Default: 300 (5 minutes)
timeout_seconds = 300

# Optional: override the Unix socket path.
# Default: $XDG_RUNTIME_DIR/vibe-reachout.sock (Linux)
#      or  /tmp/vibe-reachout-{uid}.sock (macOS)
# socket_path = "/tmp/vibe-reachout.sock"
```

### Configuration fields reference

| Field                | Type       | Required | Default | Description                                                        |
|----------------------|------------|----------|---------|--------------------------------------------------------------------|
| `telegram_bot_token` | string     | yes      | --      | Bot token from @BotFather                                          |
| `allowed_chat_ids`   | list[int]  | yes      | --      | Telegram chat IDs authorized to respond (at least one)             |
| `timeout_seconds`    | integer    | no       | 300     | Seconds to wait before falling back to terminal (1--3600)          |
| `socket_path`        | string     | no       | (auto)  | Unix socket path; auto-detected from XDG_RUNTIME_DIR or /tmp      |

## Usage

### 1. Install the hook

Register vibe-reachout as a Claude Code permission hook:

```bash
vibe-reachout install
```

This adds a `PermissionRequest` hook entry to `~/.claude/settings.json`. The command is idempotent -- running it again updates the existing entry without creating duplicates. Existing hooks for other events are preserved.

The installed hook configuration looks like this:

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

The 600-second timeout in `settings.json` is a safety net. The bot's own timeout (`timeout_seconds` in `config.toml`, default 300s) fires first under normal operation.

### 2. Start the bot

```bash
vibe-reachout bot
```

Keep this running in a dedicated terminal, a tmux session, or background it. The bot binds a Unix socket and connects to the Telegram API. It will:

- Detect and clean up stale socket files from previous crashed sessions
- Refuse to start if another bot instance is already running
- Handle concurrent permission requests from multiple Claude Code sessions

To see detailed logs:

```bash
RUST_LOG=debug vibe-reachout bot
```

### 3. Use Claude Code normally

```bash
claude
```

When Claude Code needs a permission, instead of showing a terminal prompt, it spawns the hook process. The hook connects to the bot over the Unix socket, the bot sends you a Telegram message, and you respond from your phone. Claude Code continues working.

If the bot is not running or anything goes wrong, Claude Code falls back to the normal terminal prompt. The hook is designed to never break your workflow.

### Hook mode (advanced)

When invoked without a subcommand, vibe-reachout runs in hook mode. This is what Claude Code calls -- you don't need to run it manually. It:

1. Reads Claude Code's JSON from stdin
2. Connects to the bot via Unix socket
3. Sends the permission request
4. Waits for the user's decision
5. Writes the response JSON to stdout

Hook mode logs to stderr at `warn` level by default. Override with `RUST_LOG`:

```bash
echo '{"session_id":"abc","tool_name":"Bash",...}' | RUST_LOG=debug vibe-reachout
```

## Telegram UI

When Claude Code triggers a permission prompt, you receive a Telegram message like this:

```text
📋 my-project

🔧 Bash
  cargo test --all

📁 /home/user/projects/my-project
🆔 Session: a1b2c3d4
```

With inline buttons:

```text
[ ✅ Allow ]  [ ❌ Deny ]  [ 💬 Reply ]  [ 🔓 Always Allow ]
```

The "Always Allow" button only appears when Claude Code provides permission suggestions for the tool.

### Button actions

| Button | Effect |
|--------|--------|
| ✅ Allow | Approves the tool call. Claude Code proceeds. |
| ❌ Deny | Blocks the tool call. Claude Code sees the denial and adjusts. |
| 💬 Reply | Prompts you for free-text input. Your message is sent back to Claude Code as context. |
| 🔓 Always Allow | Approves and adds a permission rule so this tool type is auto-approved for the session. |

After you respond, the message is edited to show the final status (e.g., "Approved", "Denied", "Replied", "Timed out") and the buttons are disabled. All messages across all authorized chats are updated, not just the one you tapped.

### Tool-specific formatting

The message body adapts to the tool type:

- **Bash**: command shown in a code block
- **Write**: file path and content size (e.g., `src/main.rs (2.4 KB)`)
- **Edit**: file path with old/new text diff
- **Other tools**: JSON excerpt of tool input

Long content is truncated to keep messages readable (500 chars per field, 4000 chars total).

## Troubleshooting

### No Telegram message appears

1. Verify the bot is running: `vibe-reachout bot` should be active in a terminal
2. Check your config file exists at `~/.config/vibe-reachout/config.toml`
3. Verify the bot token is correct by checking bot startup logs
4. Make sure you've messaged your bot at least once on Telegram (Telegram requires this before a bot can send you messages)

### "Bot already running" error

Another instance of the bot is already bound to the socket. Either:

- Kill the existing process
- Remove the stale socket file manually (see socket path in config or default location)

If the previous bot process crashed without cleaning up, the new instance should detect the stale socket automatically. If it doesn't, remove the socket file:

```bash
rm /tmp/vibe-reachout-$(id -u).sock    # macOS default
rm $XDG_RUNTIME_DIR/vibe-reachout.sock  # Linux default
```

### Permission prompt still appears in the terminal

The hook falls back to the terminal on any error. Run the bot with debug logging to diagnose:

```bash
RUST_LOG=debug vibe-reachout bot
```

Common causes:

- Bot process not running
- Config file missing or invalid
- Socket path mismatch between bot and hook (both read from the same config)
- Network issues reaching the Telegram API

### Unauthorized user errors

If someone other than you tries to tap the buttons, their callback is rejected. This is by design. Only chat IDs listed in `allowed_chat_ids` can respond to permission requests.

### Timeout behavior

If you don't respond within `timeout_seconds` (default: 300s), the bot sends a Timeout response to the hook, the hook exits with code 1, and Claude Code shows the terminal prompt. If you tap a button after the timeout, the bot shows "This request has already been handled" and edits the message to reflect the timeout.

## Development

### Running tests

```bash
cargo test
```

### Linting

```bash
cargo clippy -- -D warnings
```

### Formatting

```bash
cargo fmt
```

### Building for release

```bash
cargo build --release
```

The release profile is configured for minimal binary size:

- LTO (link-time optimization) enabled
- Single codegen unit
- Symbols stripped
- Optimized for speed (`opt-level = 2`)

### Cross-compilation targets

The project targets:

- **macOS aarch64** (`aarch64-apple-darwin`)
- **Linux aarch64** (`aarch64-unknown-linux-gnu`)
- **Linux x86_64** (`x86_64-unknown-linux-gnu`)

Cross-compile with:

```bash
rustup target add aarch64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

### Project structure

```text
src/
  main.rs          # CLI entry point (clap), dispatches to bot/install/hook
  config.rs        # Config loading and validation (~/.config/vibe-reachout/config.toml)
  bot.rs           # Bot process: socket server + Telegram bot loop
  hook.rs          # Hook process: stdin -> socket -> stdout
  install.rs       # Registers hook in ~/.claude/settings.json
  models.rs        # Shared types: HookInput, HookOutput, IpcRequest, IpcResponse
  error.rs         # Error types
  ipc/
    mod.rs         # IPC module
    server.rs      # Unix socket server (bot side)
    client.rs      # Unix socket client (hook side)
  telegram/
    mod.rs         # Telegram module
    formatter.rs   # Tool-specific message formatting
    keyboard.rs    # Inline keyboard button generation
    callback_data.rs # Typed callback data parsing
    handler.rs     # Callback query and message handling
```

## License

[MIT](LICENSE)
