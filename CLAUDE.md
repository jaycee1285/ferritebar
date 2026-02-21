# Ferritebar

GTK4 Wayland status bar replacing Waybar. Single TOML config, auto-themed from GTK4, native modules with gradient visualizations.

## Stack
- Rust + GTK4 + gtk4-layer-shell
- Tokio multi-thread runtime (OnceLock pattern from ironbar)
- wayland-protocols-wlr for taskbar (wlr-foreign-toplevel)
- system-tray crate for SNI
- libpulse-binding for audio
- Direct /proc/ and /sys/ reads (no sysinfo crate)

## Architecture
- `src/main.rs` - Static Tokio runtime, GTK Application bootstrap
- `src/bar.rs` - Layer-shell CenterBox (start/center/end)
- `src/modules/` - Each module: async controller (tokio) -> mpsc -> GTK widget via `recv_on_main_thread`
- `src/theme.rs` - Parses `@define-color` from GTK4 theme CSS, generates programmatic CSS
- `src/widgets/mini_bar.rs` - Cairo DrawingArea gradient progress bar

## Module Visibility Patterns
Modules use `container.set_visible(bool)` to hide without occupying bar space. Three patterns:

1. **IPC toggle** — User-triggered via compositor keybinding (`ferritebar msg <name>`). Module subscribes to `crate::ipc::subscribe()`, filters for its message, toggles visibility. Used by: memory, swap (`memory-toggle`), taskbar focus mode (`taskbar-focus`).

2. **Data-driven auto-hide** — Module hides/shows based on its own data. No IPC. Used by: workspaces (hides when <=1 workspace), taskbar (hides when 0 windows), tray (hides when 0 items).

3. **Script visibility** — Not yet implemented. Scripts could control visibility by returning empty output. Deferred because scripts may legitimately return empty/null states.

IPC pattern template (add before the final `debug!` + `container.upcast()`):
```rust
let (ipc_tx, ipc_rx) = mpsc::channel::<()>(4);
let mut ipc_sub = crate::ipc::subscribe();
crate::spawn(async move {
    loop {
        match ipc_sub.recv().await {
            Ok(msg) if msg == "MESSAGE_NAME" => { let _ = ipc_tx.send(()).await; }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
});
let container_ipc = container.clone();
super::recv_on_main_thread(ipc_rx, move |_| {
    container_ipc.set_visible(!container_ipc.is_visible());
});
```

## Build
```sh
nix develop --command cargo build
```

## Config
`~/.config/ferritebar/config.toml` - see `config.example.toml`
