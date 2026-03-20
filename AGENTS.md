# AGENTS.md — ,services

TUI for managing systemd services. Toggle services on/off, browse by category, apply changes with confirmation.

## Build & Run

```bash
cargo build              # Dev build
cargo build --release    # Release build
cargo run                # Run TUI
cargo test               # Run tests (none currently)
cargo clippy             # Lint
cargo fmt                # Format
```

## Architecture

Rust 2021, async TUI with ratatui + tokio. ~1300 lines total.

### Modules

| Module | Lines | Purpose |
|--------|-------|---------|
| `main.rs` | 74 | Entry point, tokio runtime, event loop |
| `app.rs` | 250 | Central state (services, selections, dirty tracking) |
| `systemd.rs` | 406 | systemctl interaction, curated descriptions |
| `categories.rs` | 98 | Pattern-based service categorization |
| `tui/ui.rs` | 399 | Rendering (ratatui) |
| `tui/handler.rs` | 114 | Input handling, key bindings |

### Data Flow

1. `App::new()` → `systemd::discover_services()` → categorized service list
2. User toggles → `app.dirty` set, no system changes yet
3. Enter → confirmation modal → `apply_changes()` via tokio::spawn
4. Non-blocking: `oneshot::channel` returns results to main loop

### Stack

| Crate | Purpose |
|-------|---------|
| ratatui 0.30 | Terminal UI framework |
| crossterm 0.28 | Terminal input/output |
| tokio | Async runtime for non-blocking systemctl calls |
| anyhow | Error handling with context |

## TDD Workflow

Use RED-GREEN-BLUE cycle:
1. 🔴 Write failing test
2. 🟢 Minimal code to pass
3. 🔵 Refactor

Note: No tests currently exist. When adding features, add tests first.

## Key Bindings

| Key | Action |
|-----|--------|
| j/k or ↑/↓ | Move cursor |
| Space | Toggle service |
| Enter | Review & apply |
| Tab | Switch System/User |
| / | Filter mode |
| h/l or ←/→ | Collapse/expand |
| i | Service info |
| q | Quit |

## Notes

- Requires `pkexec` for system service management
- User services use `systemctl --user`
- Services categorized by name pattern matching
- 50+ curated descriptions for common services
