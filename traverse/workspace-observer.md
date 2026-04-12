---
id: workspace-observer
kind: module
authority:
  - ext-workspace-wayland-protocol
mutates:
  - workspace-button-list
  - focused-workspace
  - optional-sync-command-side-effects
observes:
  - wayland-ext-workspace-manager
  - ferritebar-config
persists_to: []
depends_on:
  - module-host
  - gtk-main-thread-bridge
  - ipc-bus
staleness_risks:
  - compositor-protocol-availability
  - sync-command-last-synced-cache
entrypoints:
  - src/modules/workspaces.rs
---

# Workspace Observer

## Purpose
Runs a blocking Wayland workspace watcher, projects the compositor workspace snapshot into GTK buttons, and optionally fires a sync command when the active workspace changes.

## Scope of Touch
Safe to edit when changing:
- workspace label formatting
- scroll or click navigation
- hidden workspace filtering

Risky to edit when changing:
- Wayland protocol handling
- workspace activation requests
- side effects triggered by sync commands

## Authority Notes
The compositor workspace protocol is authoritative for workspace identity and active state.
Rendered buttons and sync-command invocations are derived reactions.

## Links
- [Module Host](module-host.md)
- [Ferritebar Config](ferritebar-config.md)
- [IPC Bus](ipc-bus.md)
