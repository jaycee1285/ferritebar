---
id: design-tokens
kind: contract
authority:
  - gtk-theme-css
mutates:
  - generated-css
  - theme-color-struct
observes:
  - gtk-4.0/settings.ini
  - gtk-4.0/gtk.css (user)
  - $theme/gtk-4.0/gtk.css
persists_to: []
depends_on:
  - theme-css-pipeline
staleness_risks:
  - alpha-leaking-into-text-rules
  - new-text-surface-without-contrast-coverage
  - direct-token-substitution-without-flattening
entrypoints:
  - src/theme.rs
---

# Design Tokens

## Purpose
Defines the contract between the GTK4 theme's `@define-color` variables and the colors ferritebar substitutes into its generated CSS. This is the single source of truth for which token serves which surface, what each token must be (solid vs. raw), and how legibility is verified before code ships.

## Tokens

| Token | Source variable order | Solid? | Used for |
|---|---|---|---|
| `bg` | `window_bg_color` → `theme_bg_color` | yes (raw) | bar surface, transparent-popup proxy |
| `fg` | `window_fg_color` → `theme_fg_color` | **flattened over `bg`** | bar text, tray-menu items, tooltip text, all outlines |
| `text` | `view_fg_color` → `theme_text_color` → `window_fg_color` | flattened over `bg` | reserved for view-style surfaces |
| `menu_bg` | scored from `popover_bg_color` / `view_bg_color` / `window_bg_color` | yes (raw) | popovers (toggle, power); tray menu uses `transparent` |
| `menu_fg` | scored — paired with `menu_bg` for AA contrast | flattened over `menu_bg` | popovers' fg |
| `selected_bg` | `accent_bg_color` → `theme_selected_bg_color` | yes (raw) | accent text on bar (icon font) |
| `selected_fg` | `accent_fg_color` → `theme_selected_fg_color` | flattened over `selected_bg` | text on accent fills |
| `success` / `warning` / `error` | `success_color` / `warning_color` / `error_color`+`destructive_color` | raw | status indicators |

## Critical Rule: Flatten Foreground Alpha

libadwaita-derived themes (Adwaita, Orchis, etc.) commonly express foreground colors as `rgba(0, 0, 0, 0.87)` — an alpha-blended near-black designed to render correctly only when GTK composites it against a known opaque surface.

**Substituting these directly into our CSS produces washed-out text** because:
- ferritebar's bar / tray-menu / tooltip surfaces are `background-color: transparent`.
- GTK alpha-blends the rgba text against whatever's actually rendered behind the transparent surface.
- The final visual color is unpredictable, often illegibly faded.

**`extract_colors` flattens every fg token** (`fg`, `text`, `menu_fg`, `selected_fg`) by compositing the rgba over its paired bg before storing the value as a string. After flattening every `*_fg` token is a solid `#rrggbb`. CSS substitution produces opaque text. Alpha-blending no longer applies.

The flatten implementation lives at `theme::flatten_alpha_over` and is invoked once per fg token at the bottom of `extract_colors`.

## Surface → Token Map

| Surface | Background | Text token | Outline token | Threshold |
|---|---|---|---|---|
| Bar workspace / taskbar / clock | bar (transparent → `bg`) | `fg` | — | AA 4.5:1 |
| Bar module label (icon font) | bar (transparent → `bg`) | `selected_bg` | — | AA Large 3:1 |
| `.tray-menu` container | transparent | — | `selected_bg` | non-text 3:1 |
| `.tray-menu button` (live item) | transparent | `selected_bg` | `selected_bg` | AA Large 3:1 |
| `.tray-menu button:disabled` (header) | transparent | `selected_bg` | `selected_bg` | AA Large 3:1 |
| Tooltip popup (`tooltip` / `.ferrite-tooltip`) | transparent | `selected_bg` | `selected_bg` | AA Large 3:1 |
| Power popover | `alpha(menu_bg, 0.96)` | `menu_fg` | `alpha(menu_fg, 0.2)` | AA 4.5:1 |

**Tray menu + tooltip share the bar's `{selected_bg}` token** — same color as `.module label.module-label`. This is deliberate: the bar's primary visible color (the accent applied to module icons) is what the user perceives as "ferritebar's color," and adjacent popups should match it. Using `{fg}` for these surfaces produced visually-different popups (a different theme variable) regardless of CSS specificity, which read as broken even when the rules technically applied.

The shared threshold for these surfaces is **AA Large** (3:1) because the bar's module labels are already that contract — we're matching the floor, not raising it. ferritebar's default `font_size` derives from bar height (≥14px, typically 24px at 96dpi ≈ 18pt) which qualifies the menu items as Large Text. Tooltip text uses `tooltip_font_size = font_size * 0.75` (≈18px / 13.5pt — borderline Normal Text); we still hold it to AA Large because the user's contract is "match the bar exactly," and in practice accent colors on theme bg surfaces almost always pass 3:1 but rarely pass 4.5:1.

## Specificity

GTK4 / Adwaita ship default rules that compete with ours:

| Selector | Specificity | Source |
|---|---|---|
| `tooltip.background` | (0,0,1,1) | bundled GTK4 |
| `button:disabled` | (0,0,1,1) | bundled GTK4 |
| `tooltip > label` (no class) | (0,0,0,2) | bundled GTK4 |
| `.tray-menu button` | (0,0,2,1) | ours — beats `button:disabled` |
| `.tray-menu button > label` | (0,0,2,2) | ours — beats GTK's `button > label` |
| `.tray-menu button:disabled > label` | (0,0,2,3) | ours — beats GTK's `button:disabled` |
| `tooltip > label.ferrite-tooltip` | (0,0,1,2) | ours — beats `tooltip > label` |
| `.ferrite-tooltip.ferrite-tooltip` | (0,0,2,0) | ours — chained class beats theme classes |

**Key rules of thumb:**
- Always restate `color:` in `:hover` and `:disabled` rules — GTK4's default `button:disabled` (0,0,1,1) sets a specific gray; if you only override the base `color:` and not the `:disabled` variant, the disabled state falls back to gray.
- Always target the inner Label child of a Button with `> label`. GTK Buttons render their label as a child widget; rules on the Button affect the surface, but a `button > label` rule from GTK or a theme will override the inherited `color:` if you don't restate it on the same path.
- Chain a class with itself (`.foo.foo`) to bump specificity from (0,0,1,0) to (0,0,2,0) without adding a second class on the widget.

## Verification — How to Trust This

`#[cfg(test)] mod legibility_tests` in `src/theme.rs` ships four tests that run on `cargo test`:

- `light_theme_passes_wcag_aa_everywhere` — Orchis-Light-style raw colors (with the rgba foreground) → flatten → assert AA across every surface in the table above.
- `dark_theme_passes_wcag_aa_everywhere` — same for a dark scheme.
- `orchis_light_window_fg_flattens_to_solid_legible_color` — direct regression: the exact `rgba(0, 0, 0, 0.87)` case that produced the user-visible bug.
- `generated_css_substitutes_solid_fg_only` — runs `generate_css` end-to-end and asserts no `color: rgba(...)` slips into a tooltip / tray-menu rule.

If you add a new text-bearing surface, add a corresponding `measure(...)` line. If you change a token's solid/raw policy, update the table here AND the flatten call site. The tests are the contract.

## Authority Notes
- Theme `@define-color` files are upstream truth.
- Token names in this doc are stable. Source-variable ordering can be tightened freely; flatten policy is governed by the test suite.
- The runtime flatten happens once per config load (or hot-reload), so cost is negligible.

## Links
- [Theme CSS Pipeline](theme-css-pipeline.md)
- [Module Host](module-host.md)
- [Tray Integration](tray-integration.md)
