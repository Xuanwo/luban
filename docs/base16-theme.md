# Base16 Theme Foundation

Luban's UI theme system is **Base16-first**: the canonical input is a Base16 palette (`base00`..`base0f`),
and all semantic UI tokens (backgrounds, borders, status colors, charts, terminal palette, etc.) are derived from it.

This document describes the current state and the conventions to follow when adding new UI.

## Source Of Truth

- Web UI tokens: `web/app/globals.css`

It defines:

- `--base00`..`--base0f`: Base16 palette for the active theme scope
- Semantic tokens: `--background`, `--foreground`, `--primary`, `--status-*`, `--terminal-*`, ...
- Tailwind color bindings via `@theme inline`:
  - Semantic: `bg-background`, `text-muted-foreground`, `border-border`, `text-status-warning`, ...
  - Base16: `text-base0d`, `bg-base01`, `border-base02`, ...

## Base16 Palette

The palette is provided as CSS custom properties:

- `--base00`..`--base07`: neutral ramp
- `--base08`..`--base0f`: accent colors

Luban currently provides two palettes:

- `:root`: light palette
- `.dark`: dark palette

Future multi-theme support should add additional scope classes that override the Base16 variables, for example:

```css
.theme-nord {
  --base00: ...;
  /* ... */
  --base0f: ...;
}
```

## Semantic Mapping (Current)

All semantic tokens are derived only from:

- `var(--base00)`..`var(--base0f)`
- `color-mix(...)` between Base16 colors

No additional hex/oklch colors should be introduced for semantic tokens.

The important mapping rules today are:

- `--primary` derives from `--base0d`
- `--destructive` derives from `--base08`
- `--status-success` derives from `--base0b`
- `--status-warning` derives from `--base09` (light) / `--base0a` (dark)
- `--status-info` derives from `--base0e`

When adding new tokens, derive them from Base16 (or Base16 + `color-mix`) by default.

## Component Guidelines

- Prefer semantic Tailwind classes (`bg-background`, `text-foreground`, `text-status-error`) over Tailwind's built-in palette.
- If a component needs an accent color that is not semantic, prefer Base16 classes (`text-base0d`, `text-base0e`, ...).
- Avoid hard-coded hex colors in React components.
- For theme previews (showing light/dark regardless of the current theme), wrap a subtree with `.dark` and reuse the same semantic markup.
