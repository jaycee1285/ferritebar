---
id: thread-boundary-bridge
kind: constraint
authority:
  - gtk-single-threaded-invariant
  - tokio-multi-threaded-runtime
mutates: []
observes:
  - every-module-that-crosses-async-to-gtk
persists_to: []
depends_on:
  - module-host
  - app-runtime
staleness_risks:
  - silent-widget-drop-if-channel-closed
  - panic-on-cross-thread-widget-access
  - lingering-tokio-tasks-after-bar-rebuild
entrypoints:
  - src/modules/mod.rs (recv_on_main_thread)
---

# Thread Boundary Bridge

## The Problem

Ferritebar has two runtimes that cannot touch each other's state:

1. **Tokio** (multi-threaded) — owns all async work: polling `/proc/`, running shell commands, HTTP requests, Wayland protocol listeners, IPC socket handling.
2. **GTK main thread** (single-threaded) — owns all widget state. Any widget mutation from another thread is undefined behavior: silent corruption, garbled rendering, or a hard panic.

There is no safe way to call `label.set_label()` from a Tokio task. There is no safe way to `await` inside a GTK callback.

## The Bridge

Every module crosses this boundary the same way:

```
Tokio task  -->  mpsc::channel<T>  -->  recv_on_main_thread  -->  GTK callback
```

1. A Tokio task produces data (`T`) on its own schedule (interval timer, event stream, IPC message).
2. It sends `T` through a `tokio::sync::mpsc::channel`.
3. `recv_on_main_thread` (defined in `src/modules/mod.rs`) wraps the receiver in `glib::spawn_future_local`, which runs on the GTK main loop.
4. The closure inside `recv_on_main_thread` receives `T` and mutates widgets safely — it's guaranteed to be on the GTK thread.

## Why This Is a Constraint, Not a Convention

If a module skips the bridge:
- **Direct widget access from Tokio** → panic or silent corruption. GTK4's thread safety model does not permit this. There is no "usually works" — it is always wrong.
- **Blocking the GTK thread with async work** → frozen bar. The compositor will eventually kill the surface.
- **Using `glib::idle_add` instead of the channel** → works but loses backpressure. A fast producer (e.g., workspace events during rapid switching) can queue unbounded GTK callbacks.

The mpsc channel provides bounded backpressure (capacity is typically 4-8). If the GTK thread falls behind, the Tokio sender blocks rather than flooding the main loop.

## Where It Shows Up

Every module in the codebase follows this pattern:
- `clock`, `battery`, `audio`, `network`, `memory`, `swap`, `api_spend`, `script`, `weather` — interval-based Tokio tasks sending through mpsc.
- `workspaces`, `taskbar` — Wayland protocol listeners sending through mpsc.
- `tray` — StatusNotifier event stream sending through mpsc.
- `power_menu`, `toggle_menu` — IPC broadcast receivers sending through a second mpsc to trigger GTK visibility changes.

## Staleness Risks

- **Lingering Tokio tasks after rebuild.** When the bar reloads config, `bar.clear()` drops all widgets, but the Tokio tasks that were sending to those widgets' channels keep running. The channel's receiver is dropped, so `tx.send()` returns `Err` and the task breaks out of its loop. This works — but there's a window where the old task is still alive alongside the new one. If a module holds external resources (Wayland connections, file watchers), the old instance may briefly compete with the new one.
- **Silent channel close.** If the GTK side drops (widget removed, window destroyed), the Tokio sender sees `Err` on next send. The task must exit cleanly on this error. Ignoring it means a zombie task consuming resources with nowhere to send.

## Links
- [Module Host](module-host.md)
- [App Runtime](app-runtime.md)
- [Polling Status Modules](polling-status-modules.md)
- [Workspace Observer](workspace-observer.md)
- [Taskbar Toplevel Observer](taskbar-toplevel-observer.md)
- [IPC Bus](ipc-bus.md)
