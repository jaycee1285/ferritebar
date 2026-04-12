---
id: power-menu-surface
kind: ui-surface
authority:
  - ferritebar-config
  - ipc-bus
mutates:
  - overlay-layer-shell-window
  - shell-command-side-effects
  - bar-shell-surface-visibility
observes:
  - ferritebar-config
  - ipc-bus
persists_to: []
depends_on:
  - bar-shell-surface
  - ipc-bus
staleness_risks:
  - hidden-bar-while-overlay-open
entrypoints:
  - src/power_menu.rs
---

# Power Menu Surface

## Purpose
Creates an overlay layer-shell window that opens when the IPC bus receives `power`, hides the main bar while active, and runs configured shell commands for lock, suspend, reboot, shutdown, and optional logout actions.

## Scope of Touch
Safe to edit when changing:
- button labels and ordering
- keyboard navigation
- overlay styling

Risky to edit when changing:
- destructive command wiring
- bar visibility restoration
- IPC trigger semantics

## Authority Notes
Configured power commands are the source of truth for what actions execute.
The overlay only presents and dispatches those commands.

## Links
- [Ferritebar Config](ferritebar-config.md)
- [Bar Shell Surface](bar-shell-surface.md)
- [IPC Bus](ipc-bus.md)
