---
id: bar-shell-surface
kind: ui-surface
authority:
  - ferritebar-config
mutates:
  - gtk-layer-shell-window
  - module-containers
observes:
  - ferritebar-config
  - gtk-application-state
persists_to: []
depends_on:
  - theme-css-pipeline
  - module-host
staleness_risks:
  - container-contents-after-reload
entrypoints:
  - src/bar.rs
  - src/app.rs
---

# Bar Shell Surface

## Purpose
Creates the top or bottom layer-shell window, anchors it to screen edges, applies margins, and exposes the left, center, and right GTK containers that modules are appended into.

## Scope of Touch
Safe to edit when changing:
- bar geometry
- container layout
- layer-shell anchoring

Risky to edit when changing:
- exclusive zone behavior
- popup keyboard interaction
- assumptions made by tray and power overlays

## Authority Notes
The bar config is authoritative for position, height, and margins.
Container contents are delegated to the module host.

## Links
- [App Runtime](app-runtime.md)
- [Ferritebar Config](ferritebar-config.md)
- [Module Host](module-host.md)
- [Power Menu Surface](power-menu-surface.md)
- [Tray Integration](tray-integration.md)
