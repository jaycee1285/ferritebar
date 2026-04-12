---
id: mini-bar-widget
kind: component
authority: []
mutates:
  - cairo-drawing-area
observes:
  - theme-css-pipeline
  - fraction-props-from-memory-or-swap
persists_to: []
depends_on:
  - theme-css-pipeline
staleness_risks: []
entrypoints:
  - src/widgets/mini_bar.rs
  - src/widgets/mod.rs
---

# Mini Bar Widget

## Purpose
Renders a compact Cairo-backed progress bar with theme-derived colors. It is reused by the memory and swap modules to visualize utilization without depending on GTK progress widgets.

## Scope of Touch
Safe to edit when changing:
- bar geometry
- gradient styling
- redraw behavior

Risky to edit when changing:
- color expectations of memory and swap modules
- Cairo drawing assumptions for very small sizes

## Authority Notes
This widget has no durable state.
Its displayed fraction is fully derived from parent modules.

## Links
- [Theme CSS Pipeline](theme-css-pipeline.md)
- [Polling Status Modules](polling-status-modules.md)
