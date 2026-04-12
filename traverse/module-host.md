---
id: module-host
kind: module
authority:
  - ferritebar-config
mutates:
  - module-containers
  - widget-tooltips
observes:
  - ferritebar-config
  - theme-css-pipeline
  - ipc-bus
persists_to: []
depends_on:
  - polling-status-modules
  - workspace-observer
  - taskbar-toplevel-observer
  - tray-integration
staleness_risks:
  - lingering-module-background-tasks-after-rebuild
  - tooltip-state-attached-to-widgets
entrypoints:
  - src/modules/mod.rs
---

# Module Host

## Purpose
Maps module config variants to concrete GTK widgets, appends them into the bar containers, and provides shared helpers for crossing async Tokio receivers back onto the GTK main thread and for custom tooltip rendering.

## Scope of Touch
Safe to edit when changing:
- module placement
- module factory routing
- shared tooltip behavior

Risky to edit when changing:
- GTK thread handoff rules
- rebuild semantics during config reload
- shared helper contracts used by every module

## Authority Notes
This node is authoritative for how configured module lists become widgets in the bar.
It is not authoritative for the data each module displays.

## Links
- [App Runtime](app-runtime.md)
- [Bar Shell Surface](bar-shell-surface.md)
- [Polling Status Modules](polling-status-modules.md)
- [Workspace Observer](workspace-observer.md)
- [Taskbar Toplevel Observer](taskbar-toplevel-observer.md)
- [Tray Integration](tray-integration.md)
- [IPC Bus](ipc-bus.md)
