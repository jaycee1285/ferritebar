# Theme Contrast Smoke Test 03/29

## What changed

- Ferritebar now scores GTK menu color candidates by WCAG contrast instead of trusting a single fixed variable order.
- Menu report writing is gated behind `--wcag` so daily-driver launches stay quiet.

## Human smoke

Flow John exercised:

- Switch Ferritebar across one light GTK theme and one dark GTK theme.
- Confirm the tray menu remains readable in both.
- Confirm the contrast logic still meets standards even when the themes are parsed slightly differently from Base16 YAML inputs.

## Observations

- Tray menu contrast passed in both the light-theme and dark-theme checks.
- The fallback ladder across GTK menu-related colors held up instead of producing unreadable menu text/background combinations.
- The `--wcag` report mode is the preferred debugging path; routine launches should not write contrast artifacts.

## Current gaps

- Smoke covered two real themes, not the full universe of GTK theme syntaxes.
- Themes that expose non-RGB/non-hex color expressions may still fall back to the direct priority path until parser support expands.
