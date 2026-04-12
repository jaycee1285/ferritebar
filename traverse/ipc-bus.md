---
id: ipc-bus
kind: service
authority:
  - xdg-runtime-dir/ferritebar.sock
mutates:
  - in-process-broadcast-channel
observes:
  - XDG_RUNTIME_DIR
  - unix-socket-clients
persists_to:
  - xdg-runtime-dir/ferritebar.sock
depends_on:
  - tokio-unix-listener
staleness_risks:
  - dropped-messages-for-lagging-subscribers
  - stale-socket-file-on-startup
entrypoints:
  - src/ipc.rs
  - src/main.rs
---

# IPC Bus

## Purpose
Owns the Unix socket used by `ferritebar msg <command>`, accepts one-shot commands from clients, and republishes them over an in-process broadcast channel consumed by modules and the power menu.

## Scope of Touch
Safe to edit when changing:
- socket path logic
- client message parsing
- local subscriber usage

Risky to edit when changing:
- socket lifecycle
- message fanout guarantees
- command naming contracts already used by modules

## Authority Notes
The socket message string is the canonical command payload inside the process.
Every module subscriber treats that payload as an external trigger, not durable state.

## Links
- [App Runtime](app-runtime.md)
- [Module Host](module-host.md)
- [Polling Status Modules](polling-status-modules.md)
- [Workspace Observer](workspace-observer.md)
- [Taskbar Toplevel Observer](taskbar-toplevel-observer.md)
- [Power Menu Surface](power-menu-surface.md)
