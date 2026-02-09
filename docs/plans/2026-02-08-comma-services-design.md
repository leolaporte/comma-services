# ,services — Systemd Service Manager TUI

## Overview

A terminal UI for toggling systemd services on/off. Shows all services with checkboxes, supports both system and user services, and applies enable+start / disable+stop changes on confirmation.

## Stack

- **Language:** Rust
- **TUI:** ratatui + crossterm
- **Async:** tokio (for executing systemctl commands without blocking UI)
- **Error handling:** anyhow

## Architecture

```
┌─────────────────────────────────┐
│  TUI Layer (ratatui)            │  Rendering, input handling
├─────────────────────────────────┤
│  App State                      │  Service list, selections, dirty tracking
├─────────────────────────────────┤
│  Systemd Backend                │  Calls systemctl, parses output, applies changes
```

### Module Layout

```
src/
├── main.rs          # Entry, tokio setup
├── app.rs           # Central state
├── systemd.rs       # Backend: query + apply
├── categories.rs    # Service categorization logic
├── tui/
│   ├── ui.rs        # Rendering
│   └── handler.rs   # Key handling
```

## UI Layout

```
╭─ ,services ──────────────────────────────────╮
│ [Tab: System ▼]  [User]          /: search   │
│──────────────────────────────────────────────│
│ ▸ Network (3)                                │
│   [✓] NetworkManager.service                 │
│   [ ] wpa_supplicant.service                 │
│   [✓] systemd-resolved.service               │
│ ▸ Audio (2)                                  │
│   [✓] pipewire.service                       │
│   [ ] pulseaudio.service                     │
│ ▸ Bluetooth (1)                              │
│   [✓] bluetooth.service                      │
│ ▾ Display (2)  ← collapsed                   │
│──────────────────────────────────────────────│
│ 2 pending changes  [Enter] Apply  [q] Quit   │
╰──────────────────────────────────────────────╯
```

## Key Bindings

| Key | Action |
|-----|--------|
| ↑/↓ or j/k | Move cursor |
| Space | Toggle service enabled/disabled |
| Enter | Apply pending changes |
| Tab | Switch between System / User |
| / | Enter filter mode |
| Esc | Clear filter / cancel |
| ←/→ or h/l | Collapse/expand category group |
| q | Quit (warns if pending changes) |

## Service Categories

Static pattern matching on service names:

| Category | Patterns |
|----------|----------|
| Network | NetworkManager, wpa_supplicant, systemd-networkd, systemd-resolved, iwd, dhcpcd |
| Audio | pipewire, pulseaudio, wireplumber |
| Bluetooth | bluetooth |
| Display | gdm, sddm, lightdm, greetd |
| Containers | docker, podman, containerd |
| Security | firewalld, ufw, apparmor, sshd |
| Printing | cups, avahi |
| Systemd Core | systemd-* (remaining) |
| Other | Everything else (sorted last) |

## Applying Changes

### Confirmation Modal

```
╭─ Apply Changes ──────────────────────────────╮
│                                               │
│  The following changes will be applied:       │
│                                               │
│  System (requires authentication):            │
│    ● Enable + Start  wpa_supplicant.service   │
│    ● Disable + Stop  cups.service             │
│                                               │
│  User:                                        │
│    ● Enable + Start  syncthing.service        │
│                                               │
│  [Enter] Confirm    [Esc] Cancel              │
╰──────────────────────────────────────────────╯
```

### Execution Flow

1. Group changes by user vs system
2. Apply user changes first (no auth needed)
3. System changes use pkexec systemctl enable/disable --now SERVICE
4. Results stream via tokio channel — green checkmark or red X per service
5. On completion, refresh full service list to reflect actual state

### Error Handling

- pkexec cancellation skips system changes; user changes already applied remain
- Individual failures don't abort the batch
- Failed services show error in status bar

## Filter/Search

- Activated with /
- Case-insensitive substring match on service name
- List narrows in real-time
- Empty categories hidden during filtering
- Esc clears filter
- Arrow keys and Space still work during filtering

## Visual Feedback

- **Yellow** text for toggled-but-not-applied services (dirty state)
- Status bar shows count of pending changes
- Green/red flash per service after applying shows success/failure

## Binary

- Name: comma-services
- Install to: ~/.local/bin/,services
