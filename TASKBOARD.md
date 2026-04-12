# TASKBOARD

## Done

- Reworked menu styling to prefer measured readability over accent-color guesswork.
- Added GTK menu color fallback scoring for tray, toggle menu, power menu, and tooltips.
- Added `theme.menu_bg_color` and `theme.menu_fg_color` as explicit escape hatches.
- Added repo-local WCAG report generation behind `ferritebar --wcag`.
- Verified by human smoke against one light theme and one dark theme.

## Next

- Add lightweight user-facing docs for `--wcag` and menu overrides.
- Decide whether runtime-generated `theme-contrast-report.md` should be ignored, retained, or exported elsewhere.
- Expand parsing support if real GTK themes surface unsupported color syntaxes.

## Watch

- Themes that resolve GTK variables through syntax outside `#hex`, `rgb()`, or `rgba()` may still need parser work.
- If a theme offers only poor menu candidate pairs, Ferritebar will still choose the best available pair but does not yet raise a stronger warning in the UI.
