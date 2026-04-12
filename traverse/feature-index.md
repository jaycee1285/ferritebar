---
id: feature-index
kind: index
authority: []
mutates: []
observes:
  - traverse-node-docs
persists_to:
  - traverse/feature-index.md
depends_on:
  - app-runtime
  - ferritebar-config
  - theme-css-pipeline
  - bar-shell-surface
  - module-host
  - polling-status-modules
  - workspace-observer
  - taskbar-toplevel-observer
  - tray-integration
  - power-menu-surface
  - ipc-bus
  - thread-boundary-bridge
staleness_risks: []
entrypoints:
  - traverse/feature-index.md
---

# Feature Index

## Startup And Reload
- [App Runtime](app-runtime.md)
- [Ferritebar Config](ferritebar-config.md)
- [Theme CSS Pipeline](theme-css-pipeline.md)
- [IPC Bus](ipc-bus.md)

## Shell Surfaces
- [Bar Shell Surface](bar-shell-surface.md)
- [Power Menu Surface](power-menu-surface.md)
- [Tray Integration](tray-integration.md)

## Constraints
- [Thread Boundary Bridge](thread-boundary-bridge.md)

## Module Neighborhoods
- [Module Host](module-host.md)
- [Polling Status Modules](polling-status-modules.md)
- [Mini Bar Widget](mini-bar-widget.md)
- [Workspace Observer](workspace-observer.md)
- [Taskbar Toplevel Observer](taskbar-toplevel-observer.md)
