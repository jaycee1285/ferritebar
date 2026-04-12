---
id: tray-integration
kind: module
authority:
  - status-notifier-tray-items
mutates:
  - tray-icon-row
  - tray-popup-surface
  - tray-item-activation-requests
observes:
  - system-tray-client-events
  - icon-theme-paths
  - tray-item-menus
  - bar-shell-surface
persists_to: []
depends_on:
  - module-host
  - bar-shell-surface
  - gtk-layer-shell-popup
staleness_risks:
  - cached-tray-item-map
  - menu-diff-application
entrypoints:
  - src/modules/tray.rs
---

# Tray Integration

## Purpose
Bridges StatusNotifier tray items into a GTK row, renders their icons, and hosts a reusable popup layer-shell menu surface for tray menus and activation requests. The popup surface is styled by Ferritebar's generated GTK CSS rather than delegated to an external host menu theme.

## Scope of Touch
Safe to edit when changing:
- tray icon presentation
- popup placement
- menu rendering

Risky to edit when changing:
- tray activation semantics
- menu diff application
- interaction between popup visibility and the main bar surface

## Authority Notes
Remote tray items and their menus are the source of truth.
GTK widgets and popup contents are projections over the latest client events.
The tray popup uses Ferritebar's theme CSS pipeline for readability, including contrast-scored GTK menu color selection and optional menu overrides from config.

## Links
- [Bar Shell Surface](bar-shell-surface.md)
- [Module Host](module-host.md)
