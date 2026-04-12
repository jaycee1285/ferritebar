---
id: ferritebar-config
kind: contract
authority:
  - xdg-config-home/ferritebar/config.toml
mutates:
  - config-reload-signal
observes:
  - XDG_CONFIG_HOME
  - HOME
  - xdg-config-home/ferritebar/config.toml
persists_to:
  - xdg-config-home/ferritebar/config.toml
depends_on:
  - src/config/types.rs
  - notify-inotify-watch
staleness_risks:
  - fallback-default-config
  - debounce-delayed-reload
entrypoints:
  - src/config/mod.rs
  - src/config/types.rs
  - src/settings/mod.rs
---

# Ferritebar Config

## Purpose
Defines the TOML schema for bar, theme, modules, and power commands. Loads the file from the XDG config directory, falls back to defaults on read or parse failure, and emits reload signals when the file changes.

## Scope of Touch
Safe to edit when changing:
- config schema defaults
- config path resolution
- reload debounce behavior

Risky to edit when changing:
- backward compatibility of the TOML schema
- file watcher assumptions around atomic saves
- settings editor validation

## Authority Notes
`config.toml` is the source of truth for runtime configuration.
Parsed `Config` values are derived snapshots that get rebuilt on reload.

## Links
- [App Runtime](app-runtime.md)
- [Theme CSS Pipeline](theme-css-pipeline.md)
- [Module Host](module-host.md)
- [Power Menu Surface](power-menu-surface.md)
