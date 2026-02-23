# Implementation Plan: Telegram Permission Hook

**Branch**: `001-telegram-permission-hook` | **Date**: 2026-02-23 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-telegram-permission-hook/spec.md`

## Summary

Build a Rust CLI (`vibe-reachout`) that hooks into Claude Code's `PermissionRequest` event, forwards permission prompts to a Telegram bot via Unix domain socket IPC, and returns the user's approve/deny/reply decision. Uses teloxide for Telegram, tokio for async I/O, and clap for CLI. Two runtime modes: long-running bot process and short-lived hook process.

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024)
**Primary Dependencies**: teloxide 0.13 (Telegram), tokio 1 (async runtime), clap 4 (CLI), serde/serde_json (serialization), toml 0.8 (config), dashmap 6 (concurrent map), tracing 0.1 (logging)
**Storage**: TOML config file at `~/.config/vibe-reachout/config.toml`, Unix domain socket for IPC, no database
**Testing**: `cargo test` (unit + integration), mock Telegram API for bot tests, Unix socket test harness for IPC tests
**Target Platform**: macOS (aarch64), Linux (aarch64 + x86_64)
**Project Type**: CLI tool (two modes: hook + bot)
**Performance Goals**: Hook startup + socket connect <100ms, permission round-trip <5s, binary <20MB
**Constraints**: Single static binary, <50MB idle memory, <100MB with 10 pending requests, stdout reserved for JSON in hook mode
**Scale/Scope**: Single user, up to 10 concurrent permission requests

## Constitution Check

*No constitution file found. Skipping gates.*

## Project Structure

### Documentation (this feature)

```text
specs/001-telegram-permission-hook/
├── plan.md              # This file
├── research.md          # Phase 0: technology research
├── data-model.md        # Phase 1: entities and state machines
├── quickstart.md        # Phase 1: setup and usage guide
├── contracts/
│   ├── hook-io.md       # Claude Code stdin/stdout JSON contract
│   ├── ipc.md           # Unix socket IPC protocol contract
│   └── telegram-ui.md   # Telegram message format and button contract
└── tasks.md             # Phase 2 output (via /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── main.rs              # Entry point, CLI parsing (clap), mode dispatch
├── config.rs            # Config loading from TOML, validation, defaults
├── models.rs            # Shared types: HookInput, HookOutput, IpcRequest, IpcResponse, Decision
├── hook.rs              # Hook mode: read stdin, connect socket, send IPC, write stdout
├── bot.rs               # Bot mode: Telegram polling, socket server, request lifecycle
├── telegram/
│   ├── mod.rs           # Telegram module root
│   ├── handler.rs       # Callback query + message handlers (dptree dispatcher)
│   ├── keyboard.rs      # Inline keyboard builder (Allow/Deny/Reply/Always Allow)
│   └── formatter.rs     # Permission message formatting (tool-specific, truncation)
├── ipc/
│   ├── mod.rs           # IPC module root
│   ├── server.rs        # Unix socket server (bot side): accept, read, respond
│   └── client.rs        # Unix socket client (hook side): connect, send, receive
├── install.rs           # Install command: modify ~/.claude/settings.json
└── error.rs             # Error types (thiserror)

tests/
├── hook_integration.rs  # End-to-end hook mode tests (stdin → socket → stdout)
├── ipc_test.rs          # Socket server/client round-trip tests
├── formatter_test.rs    # Message formatting + truncation tests
└── install_test.rs      # Settings.json modification tests
```

**Structure Decision**: Single Rust project (cargo workspace not needed). Flat `src/` with two submodules (`telegram/`, `ipc/`) to group related functionality. Test files at repository root `tests/` for integration tests; unit tests inline via `#[cfg(test)]` modules.

## Complexity Tracking

No constitution violations to track.
