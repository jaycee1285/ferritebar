---
id: polling-status-modules
kind: module
authority: "unreviewed"
mutates:
  - module-widget-state
  - shell-command-side-effects
observes:
  - /proc/meminfo
  - /sys/class/power_supply/BAT0
  - /sys/class/net
  - nmcli
  - iw
  - wpctl
  - external-script-stdout
  - provider-spend-json
  - provider-http-apis
  - local-clock
  - ipc-bus
persists_to: []
depends_on:
  - module-host
  - mini-bar-widget
  - ferritebar-config
  - ipc-bus
staleness_risks:
  - interval-driven-lag
  - command-availability-differences
  - derived-tooltips
entrypoints:
  - src/modules/api_spend.rs
  - src/modules/audio.rs
  - src/modules/battery.rs
  - src/modules/clock.rs
  - src/modules/memory.rs
  - src/modules/network.rs
  - src/modules/script.rs
  - src/modules/swap.rs
  - src/modules/meminfo.rs
  - src/widgets/mini_bar.rs
---

# Polling Status Modules

## Purpose
Implements the modules that poll local files, shell commands, time, or HTTP-backed APIs and then update GTK widgets on a cadence. This includes clock, battery, audio, network, memory, swap, script, and API spend surfaces, plus the mini progress bar used by memory and swap.

## Scope of Touch
Safe to edit when changing:
- display formatting
- polling intervals
- tooltip content

Risky to edit when changing:
- shell command execution contracts
- external dependency assumptions such as `nmcli`, `iw`, `wpctl`, and `curl`
- IPC-triggered actions for script and network modules

## Authority Notes
These widgets are mostly projections over external system state or command output.
They should be treated as derived views rather than sources of truth.

## Links
- [Ferritebar Config](ferritebar-config.md)
- [Theme CSS Pipeline](theme-css-pipeline.md)
- [Module Host](module-host.md)
- [IPC Bus](ipc-bus.md)
- [Mini Bar Widget](mini-bar-widget.md)
