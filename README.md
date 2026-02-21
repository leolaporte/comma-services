# ,services

A terminal UI for managing systemd services. Toggle services on/off, browse by category, and apply changes with a single confirmation — no more memorizing `systemctl enable --now` incantations.

![Rust](https://img.shields.io/badge/Rust-2021-orange) ![License](https://img.shields.io/badge/license-MIT-blue)

```
 System   User              Tab: switch  /: search  q: quit
────────────────────────────────────────────────────────────
▾ Network (3)
   [✓] NetworkManager.service       Manages network connections
   [ ] wpa_supplicant.service       WPA/WPA2 wireless auth
   [✓] systemd-resolved.service     DNS resolution
▸ Audio (2)
▸ Bluetooth (1)
▾ Security (2)
   [✓] firewalld.service            Dynamic firewall manager
   [ ] sshd.service                 OpenSSH server
────────────────────────────────────────────────────────────
 2 pending changes  [Enter] Apply    Space: toggle  i: info
```

## Features

- **Two tabs** — System services (requires `pkexec` authentication) and User services
- **Categorized view** — Services grouped into Network, Audio, Bluetooth, Display, Containers, Security, Printing, Systemd Core, and Other
- **Collapsible categories** — Expand/collapse with arrow keys or `h`/`l`
- **Real-time filter** — Press `/` and type to narrow the list instantly
- **Toggle + confirm workflow** — Space to toggle, Enter to review changes in a confirmation modal, Enter again to apply
- **Curated descriptions** — 50+ common services have human-written explanations (shown via `i` info modal)
- **Non-blocking apply** — Changes run in the background via tokio; the UI stays responsive
- **Active state detection** — Shows `(running)` for services that are active but not enabled (e.g., socket-activated)

## Requirements

- Linux with systemd
- Rust toolchain (1.70+)
- `pkexec` (from polkit) for managing system services

## Install

```bash
cargo install --path . --root ~/.local
```

Then run with:

```bash
comma-services
```

## Key Bindings

| Key | Action |
|-----|--------|
| `j` / `k` or `↑` / `↓` | Move cursor |
| `Space` | Toggle service on/off |
| `Enter` | Review & apply pending changes |
| `Tab` | Switch System / User tab |
| `/` | Enter filter mode |
| `Esc` | Clear filter or cancel |
| `h` / `l` or `←` / `→` | Collapse / expand category |
| `i` | Show service info |
| `q` | Quit |

## How It Works

1. On startup, queries `systemctl list-unit-files` to discover toggleable services (enabled, disabled, or linked — skipping static/generated/masked units)
2. Services are categorized by pattern matching on their names and grouped into collapsible sections
3. Toggling a service marks it as dirty (shown in yellow). No system changes happen yet
4. Pressing Enter opens a confirmation modal listing all pending changes
5. On confirm, changes are applied asynchronously:
   - **User services**: `systemctl --user enable --now` / `disable --now`
   - **System services**: `pkexec systemctl enable --now` / `disable --now`
6. Individual failures are reported in the status bar but don't abort the batch
7. After apply, the full service list refreshes to reflect actual state

## Architecture

```
src/
├── main.rs          # Entry point, tokio runtime, event loop
├── app.rs           # Central state (services, selections, dirty tracking)
├── systemd.rs       # systemctl interaction, curated descriptions
├── categories.rs    # Pattern-based service categorization
└── tui/
    ├── ui.rs        # Rendering (ratatui)
    └── handler.rs   # Input handling, key bindings
```

## Stack

| Crate | Purpose |
|-------|---------|
| [ratatui](https://ratatui.rs) | Terminal UI framework |
| [crossterm](https://github.com/crossterm-rs/crossterm) | Terminal input/output |
| [tokio](https://tokio.rs) | Async runtime for non-blocking systemctl calls |
| [anyhow](https://github.com/dtolnay/anyhow) | Error handling with context |

## License

MIT
