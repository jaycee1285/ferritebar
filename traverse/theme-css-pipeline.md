---
id: theme-css-pipeline
kind: service
authority:
  - gtk-theme-css
  - ferritebar-config
mutates:
  - gtk-display-style-provider
  - theme-contrast-report
observes:
  - gtk-4.0/settings.ini
  - gtk-4.0/gtk.css
  - /usr/share/themes/*/gtk-4.0/gtk.css
  - ~/.themes/*/gtk-4.0/gtk.css
  - ferritebar-config
persists_to:
  - theme-contrast-report.md
depends_on:
  - ferritebar-config
  - gtk-theme-files
staleness_risks:
  - theme-file-lookup-misses
  - css-provider-still-loaded-after-reload
  - unsupported-css-color-syntax
  - low-contrast-candidate-sets
entrypoints:
  - src/theme.rs
  - src/app.rs
---

# Theme CSS Pipeline

## Purpose
Extracts colors from the active GTK theme, scores menu foreground/background candidates for WCAG readability, overlays configured status/menu colors, and generates CSS that styles the bar and module widgets. The resulting CSS provider is registered globally on the active display and reused during config reloads. When launched with `--wcag`, it also writes a repo-local contrast report.

## Scope of Touch
Safe to edit when changing:
- color extraction heuristics
- contrast scoring and candidate selection
- generated CSS selectors
- font and icon theme handling

Risky to edit when changing:
- GTK theme compatibility
- CSS class contracts used by modules
- global display styling priority
- parsing of GTK color syntaxes

## Authority Notes
GTK theme files and `theme` config are authoritative inputs.
The generated CSS string is a derived projection for the current process only.
Menu color pairs are chosen from GTK-defined variables, not treated as a separate source of truth.
`theme-contrast-report.md` is diagnostic output only and is only written when `--wcag` is enabled.

## Links
- [App Runtime](app-runtime.md)
- [Ferritebar Config](ferritebar-config.md)
- [Bar Shell Surface](bar-shell-surface.md)
- [Polling Status Modules](polling-status-modules.md)
