# Refactoring Plan: Code Quality & Rust Best Practices

**Branch**: `002-code-quality-refactoring` | **Date**: 2026-02-23
**Scope**: Type safety, correctness, idiomatic Rust, DRY, robustness

## Summary

Address 20 issues identified in a principal Rust engineer review. Grouped into 4 phases by dependency and risk. Each phase is independently shippable — tests must pass after every phase.

---

## Phase 1: Correctness & Safety (Low effort, high impact)

These fix bugs or undefined behavior. No API changes.

### 1.1 Add `HookError::ConnectionFailed` variant

**File**: `src/error.rs`
**Problem**: Socket connection errors in `ipc/client.rs:21` are mapped to `HookError::StdinRead` — semantically wrong.
**Fix**:

```rust
// src/error.rs — add new variant
#[error("IPC connection failed: {0}")]
ConnectionFailed(#[source] std::io::Error),
```

Remove the `#[from] std::io::Error` on `StdinRead` (it auto-converts all io::Error into StdinRead today). Instead, map explicitly at each call site.

**Files touched**: `src/error.rs`, `src/ipc/client.rs`

### 1.2 Remove `process::exit(1)` from async code

**File**: `src/hook.rs:68-71`
**Problem**: `std::process::exit(1)` inside `run_hook()` skips all destructors and tokio shutdown.
**Fix**: Return an error, let `main()` handle exit code (it already does).

```rust
// src/hook.rs
let output = map_decision_to_output(&response)
    .ok_or_else(|| anyhow::anyhow!("Request timed out — falling back to terminal"))?;
```

**Files touched**: `src/hook.rs`

### 1.3 Remove competing SIGINT handlers

**File**: `src/bot.rs:70`
**Problem**: `.enable_ctrlc_handler()` on the teloxide dispatcher races with the custom `spawn_signal_handler()`. The teloxide handler calls `process::exit`, the custom one cancels a token. Nondeterministic.
**Fix**: Remove `.enable_ctrlc_handler()`. The custom signal handler already handles SIGINT/SIGTERM correctly.

**Files touched**: `src/bot.rs`

### 1.4 Eliminate TOCTOU race in IPC client

**File**: `src/ipc/client.rs:13-15`
**Problem**: `socket_path.exists()` check before `connect()` is racy — socket can vanish between check and connect.
**Fix**: Remove the existence check. Attempt connect directly and map error kinds:

```rust
let stream = UnixStream::connect(socket_path).await.map_err(|e| {
    match e.kind() {
        std::io::ErrorKind::NotFound => {
            HookError::SocketNotFound(socket_path.display().to_string())
        }
        std::io::ErrorKind::ConnectionRefused => HookError::ConnectionRefused,
        _ => HookError::ConnectionFailed(e),
    }
})?;
```

**Files touched**: `src/ipc/client.rs`

**Verification**: `cargo test`, `cargo clippy`

---

## Phase 2: Type Safety (Medium effort, high impact)

Replace stringly-typed protocol fields with enums. Requires updating tests.

### 2.1 Introduce `HookBehavior` enum

**File**: `src/models.rs`
**Problem**: `HookDecision.behavior` is a `String` accepting `"allow"` or `"deny"`. A typo compiles silently.
**Fix**:

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HookBehavior {
    Allow,
    Deny,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookDecision {
    pub behavior: HookBehavior,
    // ... rest unchanged
}
```

**Files touched**: `src/models.rs` (struct + constructors + tests)

### 2.2 Introduce `HookEventName` enum

**File**: `src/models.rs`
**Problem**: `HookSpecificOutput.hook_event_name` is hardcoded `"PermissionRequest".to_string()` in 3 places.
**Fix**:

```rust
#[derive(Debug, Serialize)]
pub enum HookEventName {
    PermissionRequest,
}
```

Use `HookEventName::PermissionRequest` in the constructors. No more string allocation.

**Files touched**: `src/models.rs`

### 2.3 Typed `CallbackData` for Telegram callbacks

**File**: `src/telegram/handler.rs:37-45`
**Problem**: `splitn(2, ':')` with silent parse failures. Action compared as raw strings.
**Fix**: Introduce a `CallbackData` struct:

```rust
// src/telegram/callback_data.rs (new file)
pub struct CallbackData {
    pub request_id: Uuid,
    pub action: CallbackAction,
}

pub enum CallbackAction {
    Allow,
    Deny,
    Reply,
    Always,
}

impl CallbackData {
    pub fn parse(data: &str) -> Option<Self> {
        let (id_str, action_str) = data.split_once(':')?;
        let request_id = Uuid::parse_str(id_str).ok()?;
        let action = match action_str {
            "allow" => CallbackAction::Allow,
            "deny" => CallbackAction::Deny,
            "reply" => CallbackAction::Reply,
            "always" => CallbackAction::Always,
            _ => return None,
        };
        Some(Self { request_id, action })
    }
}
```

Update `handler.rs` to use `CallbackData::parse()` and match on `CallbackAction`.

**Files touched**: `src/telegram/callback_data.rs` (new), `src/telegram/mod.rs`, `src/telegram/handler.rs`

### 2.4 Add `IpcResponse` constructors

**File**: `src/models.rs`, `src/ipc/server.rs`, `src/bot.rs`
**Problem**: `IpcResponse` timeout is built identically in 4 places.
**Fix**:

```rust
impl IpcResponse {
    pub fn timeout(request_id: Uuid) -> Self {
        Self {
            request_id,
            decision: Decision::Timeout,
            message: None,
            user_message: None,
            always_allow_suggestion: None,
        }
    }

    pub fn allow(request_id: Uuid) -> Self { /* ... */ }
    pub fn deny(request_id: Uuid, message: String) -> Self { /* ... */ }
    pub fn always_allow(request_id: Uuid, suggestion: Option<serde_json::Value>) -> Self { /* ... */ }
    pub fn reply(request_id: Uuid, user_message: String) -> Self { /* ... */ }
}
```

Replace all inline construction with these constructors.

**Files touched**: `src/models.rs`, `src/ipc/server.rs`, `src/bot.rs`, `src/telegram/handler.rs`

**Verification**: `cargo test`, `cargo clippy`, verify JSON output hasn't changed (existing serialization tests cover this)

---

## Phase 3: Robustness & Cleanup (Mixed effort)

### 3.1 Fix `ChatId(0)` magic sentinel

**File**: `src/telegram/handler.rs:19`
**Problem**: If `query.message` is `None`, `ChatId(0)` silently fails the auth check.
**Fix**:

```rust
let Some(msg) = query.message.as_ref() else {
    tracing::warn!("Callback query with no associated message");
    return Ok(());
};
let chat_id = msg.chat().id;
```

**Files touched**: `src/telegram/handler.rs`

### 3.2 Remove broad `#[allow(dead_code)]`

**Files**: `src/error.rs:25`, `src/models.rs:9,129`
**Fix**:
- `HookInput`: all fields are used for deserialization — remove `#[allow(dead_code)]`, add `#[allow(dead_code)]` only on individually unused fields or use `_` prefix.
- `BotError`: if variants are genuinely unused, remove them. If they're part of a planned API, add targeted `#[allow(dead_code)]` with a `// Used by: <planned feature>` comment.
- `PendingRequest`: same treatment.

**Files touched**: `src/error.rs`, `src/models.rs`

### 3.3 Collapse duplicate `truncate_field`/`truncate_message`

**File**: `src/telegram/formatter.rs:80-96`
**Problem**: Two identical functions.
**Fix**: Remove `truncate_message`, rename `truncate_field` to `truncate`, update callers.

**Files touched**: `src/telegram/formatter.rs`

### 3.4 Reduce `query.id.clone()` calls

**File**: `src/telegram/handler.rs`
**Problem**: `query.id.clone()` called 4 times.
**Fix**: Clone once at the top: `let query_id = query.id;` (move, not clone), then use `&query_id` where needed. Clone only for the final `answer_callback_query` call that takes ownership.

**Files touched**: `src/telegram/handler.rs`

### 3.5 Clean up `lib.rs` dual module tree

**File**: `src/lib.rs`
**Problem**: `lib.rs` and `main.rs` both declare `error` and `models` — confusing.
**Fix**: Make `lib.rs` re-export from the canonical modules. In `main.rs`, use `use vibe_reachout::{error, models}` instead of re-declaring them. This unifies the module tree.

Alternative (simpler): if only integration tests use `lib.rs`, document this clearly with a module-level doc comment explaining the purpose.

**Files touched**: `src/lib.rs`, potentially `src/main.rs`

### 3.6 Add `#[must_use]` on pure constructors

**File**: `src/models.rs`
**Fix**: Add `#[must_use]` to `HookOutput::allow()`, `HookOutput::deny()`, `HookOutput::allow_always()`, and the new `IpcResponse` constructors.

**Files touched**: `src/models.rs`

**Verification**: `cargo test`, `cargo clippy`

---

## Phase 4: Hardening & Performance (Optional, lower priority)

### 4.1 Add connection semaphore to IPC server

**File**: `src/ipc/server.rs`
**Problem**: Unbounded `tokio::spawn` per connection — local DoS vector.
**Fix**:

```rust
use tokio::sync::Semaphore;

let semaphore = Arc::new(Semaphore::new(50));

// In accept loop:
let permit = semaphore.clone().acquire_owned().await.expect("semaphore closed");
tokio::spawn(async move {
    let _permit = permit;
    if let Err(e) = handle_connection(stream, bot, config, pending_map, cancel).await {
        tracing::error!("Connection handler error: {e}");
    }
});
```

**Files touched**: `src/ipc/server.rs`

### 4.2 Use `HashSet<i64>` for `allowed_chat_ids`

**File**: `src/config.rs`
**Problem**: `Vec<i64>` with `.contains()` — O(n) lookup on every request.
**Fix**: Deserialize as `Vec<i64>`, then convert to `HashSet<i64>` post-deserialization. Or use a custom deserializer.

```rust
#[derive(Debug, Clone)]
pub struct Config {
    pub telegram_bot_token: String,
    pub allowed_chat_ids: HashSet<i64>,
    // ...
}
```

**Files touched**: `src/config.rs`, all files calling `.contains()`

### 4.3 Change `opt-level = "z"` to `opt-level = 2`

**File**: `Cargo.toml`
**Problem**: `"z"` optimizes for binary size, hurting latency. The hook path is latency-sensitive.
**Fix**: Change to `opt-level = 2` (or `3`). Use `strip = true` and `lto = true` for size instead.

**Files touched**: `Cargo.toml`

### 4.4 Add callback data length assertion

**File**: `src/telegram/keyboard.rs`
**Problem**: Telegram has a 64-byte limit on callback data. Currently 43 bytes max — safe but unvalidated.
**Fix**:

```rust
debug_assert!(
    id.len() + ":always".len() <= 64,
    "callback data exceeds Telegram 64-byte limit"
);
```

**Files touched**: `src/telegram/keyboard.rs`

### 4.5 Safer `unwrap()` in install.rs

**File**: `src/install.rs:27`
**Problem**: `as_object_mut().unwrap()` after `settings["hooks"] = json!({})` is logically safe but fragile.
**Fix**: Use `entry` API:

```rust
let hooks = settings
    .as_object_mut()
    .expect("settings must be an object")
    .entry("hooks")
    .or_insert_with(|| serde_json::json!({}))
    .as_object_mut()
    .expect("hooks must be an object");
```

**Files touched**: `src/install.rs`

**Verification**: `cargo test`, `cargo clippy`

---

## Out of Scope (Future Work)

These are important but warrant their own spec/plan:

- **Handler module test coverage** — the most complex business logic (`src/telegram/handler.rs`) has zero tests. Requires mocking teloxide types or extracting pure business logic into testable functions. This is a separate effort (spec `003-handler-tests`).
- **`libc` dependency removal** — could use `nix` or `std::os::unix` instead of `unsafe { libc::getuid() }`.
- **Rate limiting on IPC server** — beyond the semaphore in 4.1, a proper token bucket per-second rate limiter for production hardening.

---

## Execution Order

```
Phase 1 (1.1 → 1.2 → 1.3 → 1.4) → cargo test
Phase 2 (2.1 → 2.2 → 2.3 → 2.4) → cargo test
Phase 3 (3.1 → 3.2 → 3.3 → 3.4 → 3.5 → 3.6) → cargo test
Phase 4 (4.1 → 4.2 → 4.3 → 4.4 → 4.5) → cargo test
```

Each phase is a separate commit following Conventional Commits:
- Phase 1: `fix: correct error variants, remove process::exit from async, fix signal handler race`
- Phase 2: `refactor: replace stringly-typed fields with enums, add typed callback parsing`
- Phase 3: `refactor: remove dead code annotations, collapse duplicate functions, clean up module tree`
- Phase 4: `perf: add connection semaphore, use HashSet for auth, optimize release profile`

## Risk Assessment

| Phase | Risk | Mitigation |
|-------|------|------------|
| 1 | Error variant change breaks integration tests | Update test assertions in same commit |
| 2 | Serde rename changes JSON output | Existing serialization tests catch regressions |
| 3 | `lib.rs` changes break integration tests | Run full test suite, including `tests/` dir |
| 4 | HashSet changes deserialization | Custom deserializer or post-load conversion |
