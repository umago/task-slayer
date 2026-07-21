# AGENTS.md

## Project

Task Slayer (`tslay`) is a small, fast, native Linux CLI todo manager written in Rust.

## Architecture

```
CLI (clap) → Commands → TaskRepository → Storage (JSON)
```

Each layer only talks to the one below it. Commands never touch the JSON file directly.

### Modules

| File | Responsibility |
|------|---------------|
| `src/main.rs` | Entry point |
| `src/cli.rs` | Clap parsing, selector expansion, run() |
| `src/commands.rs` | Command dispatch and output formatting |
| `src/model.rs` | `Task` and `Store` structs |
| `src/repository.rs` | `TaskRepository` — domain operations |
| `src/storage.rs` | `Storage` trait + `JsonStorage` impl |

### Key design decisions

- **Storage format** is an object `{ "tasks": [...], "next_id": N }`, not a bare array. This preserves the ID high-water mark even when all tasks are deleted, so IDs are never reused. Do not change this to a bare array.
- **IDs start at 1** and increment monotonically via `next_id`.
- **`compact` is the only exception to ID stability**: it is an opt-in command that renumbers all tasks sequentially from 1 and resets `next_id`. Normal operations (`add`, `done`, `rm`, `edit`) never reuse or renumber IDs. `compact` exists for users who want gap-free IDs after deletions.
- **Atomic writes**: write to temp file → fsync → rename over original. Never write directly to `tasks.json`.
- **Storage is a trait** so the backend can be swapped (e.g. SQLite) without touching commands.
- **Selectors** (`done 1 3-5 8`) are parsed in `cli.rs`, not by clap's value_parser, because one token can expand to multiple IDs.

## Build

```bash
cargo build              # debug
cargo build --release    # optimized (LTO, strip)
cargo clippy             # lint (should be clean)
```

## Testing

Tests are in-module `#[cfg(test)] mod tests` blocks (unit tests only, no integration test directory). Run with:

```bash
cargo test
```

Also smoke test manually:

```bash
export HOME=/tmp/tslay-test
target/debug/tslay add "Buy milk"
target/debug/tslay
target/debug/tslay done 1
target/debug/tslay all
```

## Conventions

- Rust edition 2024.
- Dependencies are minimal: clap, serde, serde_json, chrono, anyhow.
- No feature flags or conditional compilation.
- Error messages are printed to stderr as `tslay: <message>`.
- Exit codes: `0` success, `1` runtime error, `2` CLI parse error.

## Storage location

`~/.local/share/tslay/tasks.json` or `$XDG_DATA_HOME/tslay/tasks.json`.
