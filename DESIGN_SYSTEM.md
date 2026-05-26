# Design System

The visual language for Headroom. Every screen, icon, and component in the app should be reducible to a decision made on this page.

## Philosophy

**Refined minimal.** Headroom is a utility that sits in your menu bar — it competes for attention with notifications, dock, wallpaper, and the rest of the OS chrome. The right answer is to be unobtrusive when nothing's wrong and unambiguously clear when something is. No gradients, no shadows, no decorative chrome. Whitespace and typography do the work.

**Native vibrancy, not fake glass.** The popover is translucent because Tauri exposes the OS-level vibrancy effect (macOS `NSVisualEffectView`, Windows 11 Mica). We do not simulate this in CSS with backdrop-filter — that's pretending to be native and looking worse than the real thing. On Linux without a compositor that supports blur, we use a near-opaque solid as the honest fallback.

**Typography as hierarchy.** Two font weights, two type sizes per surface, plus muted shades of the foreground color. That's the entire hierarchy — no borders, dividers, or background tints to separate content.

---

## Color tokens

### Light mode

| Token                          | Value             | Used for                                  |
| ------------------------------ | ----------------- | ----------------------------------------- |
| `--bg-window`                  | `rgba(248, 247, 244, 0.82)` | Popover background (translucent)|
| `--bg-window-opaque`           | `#f8f7f4`         | Settings & onboarding windows             |
| `--bg-secondary`               | `rgba(0, 0, 0, 0.04)` | Subtle row backgrounds                |
| `--border-default`             | `rgba(0, 0, 0, 0.08)` | Hairlines between sections            |
| `--border-emphasis`            | `rgba(0, 0, 0, 0.15)` | Buttons, inputs                       |
| `--fg-primary`                 | `rgba(26, 26, 26, 0.92)` | Body text                          |
| `--fg-secondary`               | `rgba(26, 26, 26, 0.72)` | Labels                             |
| `--fg-tertiary`                | `rgba(26, 26, 26, 0.50)` | Meta, timestamps                   |
| `--fg-quaternary`              | `rgba(26, 26, 26, 0.30)` | Disabled, placeholders             |

### Dark mode

| Token                          | Value             | Used for                                  |
| ------------------------------ | ----------------- | ----------------------------------------- |
| `--bg-window`                  | `rgba(22, 24, 30, 0.78)` | Popover background (translucent)   |
| `--bg-window-opaque`           | `rgba(22, 24, 30, 0.96)` | Settings & onboarding windows      |
| `--bg-secondary`               | `rgba(255, 255, 255, 0.05)` | Subtle row backgrounds          |
| `--border-default`             | `rgba(255, 255, 255, 0.08)` | Hairlines between sections      |
| `--border-emphasis`            | `rgba(255, 255, 255, 0.15)` | Buttons, inputs                 |
| `--fg-primary`                 | `rgba(255, 255, 255, 0.92)` | Body text                       |
| `--fg-secondary`               | `rgba(255, 255, 255, 0.78)` | Labels                          |
| `--fg-tertiary`                | `rgba(255, 255, 255, 0.50)` | Meta, timestamps                |
| `--fg-quaternary`              | `rgba(255, 255, 255, 0.30)` | Disabled, placeholders          |

### Status colors

State colors use a two-mode palette. Light values are slightly desaturated terra tones; dark values are brighter pastel-y tones for translucent dark backgrounds.

|         | Light fill | Light text | Dark fill  | Dark text  |
| ------- | ---------- | ---------- | ---------- | ---------- |
| OK      | `#6a8e4a`  | `#5a7d3a`  | `#8ab368`  | `#9bc176`  |
| Warn    | `#c08a2a`  | `#a87420`  | `#d99c52`  | `#e0a85e`  |
| Crit    | `#b03533`  | `#9c2e2c`  | `#d4625d`  | `#e07670`  |
| Unreach | `#888780`  | `#666561`  | `rgba(255,255,255,0.25)` | `rgba(255,255,255,0.45)` |

**Rules:**
- Status colors only appear in three places: progress bars, percentage numbers ≥ warn, advisory text under critical quotas. Never as background fills.
- The "unreachable" state is gray, not red. Red is reserved for "still authenticated but about to expire."
- Headers and labels never take status colors.

---

## Typography

### Fonts

```css
--font-sans: ui-sans-serif, system-ui, -apple-system, "Segoe UI Variable", "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
--font-mono: ui-monospace, SF Mono, Menlo, "Cascadia Code", "Source Code Pro", monospace;
```

We deliberately use system fonts. The popover sits in the menu bar — having it look identical to other native UI is the goal. No web fonts.

### Sizes and weights

| Element                       | Size      | Weight | Notes                                |
| ----------------------------- | --------- | ------ | ------------------------------------ |
| Section labels (UPPERCASE)    | 11 px     | 500    | `letter-spacing: 0.08em`             |
| Body labels                   | 12 px     | 400    |                                      |
| Numbers (monospace)           | 10.5 px   | 500/400| `font-variant-numeric: tabular-nums` |
| Meta / timestamps             | 9.5–10 px | 400    | `--fg-tertiary` or `--fg-quaternary` |
| Settings titles               | 13 px     | 500    |                                      |
| Onboarding heading            | 17–18 px  | 500    | `letter-spacing: -0.01em`            |
| Plan badges                   | 10 px     | 400    | `--fg-tertiary`                      |

**Two weights only: 400 and 500.** Never 600 or 700 — heavy weights look out of place against system UI.

**Sentence case everywhere.** Never Title Case, never ALL CAPS, except for the section labels (intentionally tracked-out small caps).

---

## Layout primitives

### Surface

The base popover container. Translucent, single rounded rectangle, fills its window.

```css
.surface {
  background: var(--bg-window);
  border: 0.5px solid var(--border-default);
  border-radius: 12px;
  padding: 14px 14px 10px;
}
```

The 0.5px border is intentional — it reads as a hairline on retina displays. Don't bump to 1px "for readability"; the goal is the surface looking like a sheet of paper, not a card with a frame.

### Section

A logical group within the surface (one service block, or one settings group).

```css
.section { padding: 8px 0; }
.section + .section { border-top: 0.5px solid var(--border-default); }
```

No background, no margin, no left/right padding. The hairline between sections is the only separator.

### Quota row

The repeated unit inside each service section. Label on the left, numeric on the right, 2 px progress bar underneath, meta line below.

```
┌──────────────────────────────────────────────────┐
│  5h                       142 / 225 · 63%        │  <- 11 px, label + mono right
│  ▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░               │  <- 2 px, status color
│  17:46 · in 2h 14m                               │  <- 9.5 px, --fg-tertiary
└──────────────────────────────────────────────────┘
```

Spacing between quota rows in the same section: `margin-bottom: 9px`. Between sections: the section hairline + `padding: 8px 0` on each.

### Progress bar

Always 2 px tall. Never rounded. Background `var(--bg-secondary)`, fill in the status color. No animation on value change in v1 — the bar snaps to its new width. (Phase 2 may add a 200ms ease.)

---

## Components

### `<TokenCard>` — quota display

A service section. Header with icon + service name + plan badge, body of quota rows.

### `<SegmentedControl>` — settings UI

Two-to-three-option picker. Border on the outer container, hairlines between segments, no spacing. Selected segment gets a slightly raised background (`var(--border-emphasis)` opacity).

### `<Toggle>` — settings UI

Pill-shaped, 26×15 px. Background `var(--border-emphasis)` off, `#5a7d3a` on. Knob is `#fafafa`, 11×11 px.

### `<Button>`

Two variants: default and primary. Default has transparent background, 0.5 px border, hover background `var(--bg-secondary)`. Primary uses the OK status fill as background.

```css
.btn {
  font-size: 11px;
  padding: 5px 12px;
  background: transparent;
  border: 0.5px solid var(--border-emphasis);
  border-radius: 5px;
  color: var(--fg-primary);
}
.btn-primary {
  background: #6a8e4a;
  border-color: #6a8e4a;
  color: #fafafa;
}
```

Buttons never have shadows. Hover state changes background only, not border or transform.

### `<InfoDot>` — contextual notice

A small `ⓘ` icon next to a label, indicating there's a tooltip-level explanation available. Used for things like the Copilot AI Credits migration notice. Triggers a native OS tooltip on hover.

---

## Iconography

### Tray icons

Four states. The geometry is shared across all four — only fill / color / dash style varies. Canvas is 32×32 with a 4 px safe margin on all sides.

- **Ceiling line**: `<rect x="6" y="6" width="20" height="1.6" rx="0.8">`, fixed position in all states.
- **Fill bar**: `<rect x="9" y="..." width="14" height="..." rx="1.5">`, height varies by percentage. At 0%: top edge at y=27, height 0. At 100%: top edge at y=9, height 18.

| State        | Ceiling fill                 | Bar fill                     | Style          |
| ------------ | ---------------------------- | ---------------------------- | -------------- |
| OK           | `#8ab368` @ 0.55 opacity     | `#8ab368`                    | solid          |
| Warn         | `#d99c52` @ 0.55 opacity     | `#d99c52`                    | solid          |
| Crit         | `#d4625d` @ 0.65 opacity     | `#d4625d`                    | solid          |
| Unreachable  | gray @ 0.7, `stroke-dasharray="2 2.2"` | none, only stroked outline of bar | dashed |

Exports: PNG at 16×16, 22×22, 32×32, and 64×64 (the last two are @2x for retina). Source SVG lives at `assets/tray.svg` with the four variants in `<symbol>` elements; a build script in `scripts/build-tray-icons.mjs` rasterizes via `sharp`.

### In-app icons

Tabler outline set. Never `-filled` variants. Sizes 12, 13, 14, 16 px depending on context. Color inherits from text via `currentColor`.

The Claude Code logo is a custom inline SVG (placeholder uses `ti-sparkles` until the official mark is added at `assets/services/claude-code.svg`).

---

## Animation

**Default state: no animation.**

The popover opens with the OS-native menu bar animation; the content inside it does not animate in. Status bars snap to new widths on update. Tray icon swaps are instantaneous.

Exception: when a service transitions to `crit` for the first time in a session, the icon may pulse once (0.5s opacity ramp from 0.6 to 1.0 and back). This is the only place in the app where motion is used to draw attention.

---

## Translucency rules

The popover uses native OS vibrancy. **Do not use CSS `backdrop-filter: blur()`** — it's banned because:

1. It looks worse than native vibrancy on macOS/Windows.
2. It performs poorly on lower-end hardware.
3. It doesn't survive the OS automatically adapting to wallpaper changes.

On Linux without a blur-capable compositor, the popover falls back to a near-opaque solid (`rgba(22, 24, 30, 0.96)` in dark mode). This is honest — pretending to be translucent on a system that doesn't support it produces a flat semitransparent ghost that looks broken.

---

## Mode switching

Headroom respects the OS theme by default. Manual override available in Settings (Auto / Light / Dark). The mode token is exposed to the renderer via a CSS class on `<html>` and the React tree subscribes via a `useColorScheme()` hook.

When transitioning, no animation — both modes are pre-styled and the switch is instantaneous.

---

## Reference mockups

The full popover, settings panel, onboarding, burndown view, and unreachable state are mocked in `docs/mockups/`. Treat those as the source of truth for layout decisions not captured here.
