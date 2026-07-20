# Task Slayer

A small, fast CLI task-slaying tool. Track your quests, slay your tasks.

## Install

### From GitHub Releases (recommended)

```bash
curl -Lo ~/.local/bin/tslay https://github.com/umago/task-slayer/releases/latest/download/tslay
chmod +x ~/.local/bin/tslay
```

### From source

```bash
cargo install --path .
```

Or build manually and move the binary to your `$PATH`:

```bash
cargo build --release
cp target/release/tslay ~/.local/bin/
```

## Usage

### Add a task

```bash
tslay add "Buy milk"
```

```
Created task 1.
```

### Edit a task

```bash
tslay edit 1 "Buy oat milk"
```

```
Updated task 1.
```

### List pending tasks (default)

```bash
tslay
```

```
ID Description
-- -----------
1  Buy oat milk
2  Buy eggs

2 tasks.
```

### List all tasks (including completed)

```bash
tslay all
```

```
ID ✓ Description
-- - -----------
1  ✓ Buy oat milk
2    Buy eggs

2 tasks.
```

### Mark tasks as done

```bash
tslay done 3            # single
tslay done 1 4 7        # multiple
tslay done 1-5          # range
tslay done 1 3-5 8      # mixed
```

### Undo completed tasks

```bash
tslay undo 3            # single
tslay undo 1 4 7        # multiple
tslay undo 1-5          # range
tslay undo 1 3-5 8      # mixed
```

### Remove tasks

```bash
tslay rm 3              # single
tslay rm 1 4 7          # multiple
tslay rm 1-5            # range
tslay rm 1 3-5 8        # mixed
```

Duplicate task IDs across selectors are ignored.

## Storage

Tasks are stored as JSON at:

```
~/.local/share/tslay/tasks.json
```

Or `$XDG_DATA_HOME/tslay/tasks.json` if that variable is set. The directory and file are created automatically on first use.

The storage layer is abstracted behind a `Storage` trait, so the JSON backend can be replaced (e.g. with SQLite) without changing command behavior.

### Task IDs

- IDs are unique and never reused, even after deletion.
- New IDs always increment from the previous maximum.

## License

MIT
