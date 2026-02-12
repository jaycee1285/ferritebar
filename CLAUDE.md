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

## Build
```sh
nix develop --command cargo build
```

## Config
`~/.config/ferritebar/config.toml` - see `config.example.toml`
