---
id: app-runtime
kind: module
authority:
  - ferritebar-process-lifecycle
mutates:
  - gtk-application-state
  - ipc-bus
  - bar-shell-surface
  - module-host
  - power-menu-surface
observes:
  - cli-args
  - ferritebar-config
persists_to: []
depends_on:
  - ferritebar-config
  - theme-css-pipeline
  - bar-shell-surface
  - module-host
  - power-menu-surface
  - ipc-bus
staleness_risks:
  - runtime-resident-css-provider
  - long-lived-background-tasks
entrypoints:
  - src/main.rs
  - src/app.rs
---

# App Runtime

## Purpose
Bootstraps the shared Tokio runtime, chooses between the bar UI and settings editor from CLI args, loads config, applies CSS, starts IPC, and rebuilds the bar on config reload.

## Scope of Touch
Safe to edit when changing:
- startup sequencing
- CLI subcommands
- config reload orchestration

Risky to edit when changing:
- GTK activation lifetime
- runtime ownership
- global rebuild behavior

## Authority Notes
This node is authoritative for process startup order and which top-level GTK surface is opened.
It is not authoritative for configuration values or per-module data sources.

## Links
- [Ferritebar Config](ferritebar-config.md)
- [Theme CSS Pipeline](theme-css-pipeline.md)
- [Bar Shell Surface](bar-shell-surface.md)
- [Module Host](module-host.md)
- [Power Menu Surface](power-menu-surface.md)
- [IPC Bus](ipc-bus.md)
