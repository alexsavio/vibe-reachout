# Quickstart: vibe-reachout

## Prerequisites

- Rust 1.85+ (`rustup update stable`)
- A Telegram bot token (talk to [@BotFather](https://t.me/BotFather))
- Your Telegram chat ID (talk to [@userinfobot](https://t.me/userinfobot))

## Setup

### 1. Build

```bash
cargo build --release
```

Binary at `target/release/vibe-reachout`.

### 2. Configure

Create `~/.config/vibe-reachout/config.toml`:

```toml
telegram_bot_token = "123456:ABC-DEF..."
allowed_chat_ids = [123456789]
timeout_seconds = 300
# socket_path = "/tmp/vibe-reachout.sock"  # optional override
```

### 3. Install hook

```bash
vibe-reachout install
```

This adds the PermissionRequest hook to `~/.claude/settings.json`.

### 4. Start the bot

```bash
vibe-reachout bot
```

Keep this running in a separate terminal (or background it).

### 5. Use Claude Code

Start Claude Code normally. When it needs permission, you'll get a Telegram message with Allow/Deny/Reply buttons instead of a terminal prompt.

## Commands

| Command | Description |
|---------|-------------|
| `vibe-reachout bot` | Start the Telegram bot (long-running) |
| `vibe-reachout install` | Register hook in Claude Code settings |
| (no subcommand) | Hook mode — reads stdin, used by Claude Code |

## How it works

```
Claude Code → spawns hook (stdin JSON) → connects to Unix socket → bot sends Telegram message
                                                                          ↓
Claude Code ← hook writes stdout JSON  ← bot sends socket response ← user taps button
```

## Troubleshooting

- **No Telegram message?** Check `vibe-reachout bot` is running and config is valid
- **"Bot already running"?** Kill the existing process or remove stale socket file
- **Permission still shows in terminal?** The hook falls back to terminal on any error — check bot logs (`RUST_LOG=debug vibe-reachout bot`)
