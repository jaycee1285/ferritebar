---
id: taskbar-toplevel-observer
kind: module
authority:
  - wlr-foreign-toplevel-wayland-protocol
mutates:
  - taskbar-button-list
  - toplevel-activation-requests
  - toplevel-close-requests
observes:
  - wlr-foreign-toplevel-manager
  - desktop-files
  - ferritebar-config
  - ipc-bus
persists_to: []
depends_on:
  - module-host
  - wayland-client-thread
staleness_risks:
  - desktop-icon-cache
  - compositor-protocol-availability
entrypoints:
  - src/modules/taskbar.rs
---

# Taskbar Toplevel Observer

## Purpose
Tracks foreign toplevel windows from the compositor, resolves icon names from desktop files, and exposes GTK buttons that can activate or close windows.

## Scope of Touch
Safe to edit when changing:
- taskbar display modes
- title truncation
- icon lookup behavior

Risky to edit when changing:
- Wayland request handling
- cached app-id to icon mappings
- IPC or click actions that affect real windows

## Authority Notes
The Wayland foreign toplevel protocol is authoritative for window lifetime and focus.
Desktop file scanning is only a lookup aid for icon presentation.

## Links
- [Module Host](module-host.md)
- [Ferritebar Config](ferritebar-config.md)
- [IPC Bus](ipc-bus.md)
