# Headroom UI Mockups

Static HTML mockups of every Headroom screen. They render exactly as the React UI should look once implemented — same colors, same typography, same spacing, same hairlines.

**Drop this whole folder under `docs/mockups/` in the repo.** It's already referenced from `README.md` and `SPEC.md`.

## How to view

Open any `.html` file directly in a browser. No build step, no dependencies, no JavaScript — just static HTML + CSS using the project's design tokens.

## How Claude Code should use these

When implementing or modifying a React component, **open the corresponding mockup file first** and use it as the visual reference. The CSS in each mockup uses the same custom properties (`--bg-window`, `--fg-primary`, etc.) that `src/index.css` declares — values stay in sync between mockups and runtime.

If a component's appearance drifts from the mockup, the mockup is the source of truth, not the React implementation. Update the React code to match.

## File index

| File                       | Screen                                | Maps to React component               |
| -------------------------- | ------------------------------------- | ------------------------------------- |
| `00-design-tokens.html`    | Color, typography, component samples  | `src/index.css` + Tailwind config     |
| `01-popover.html`          | Main popover (light + dark)           | `App.tsx` + `TokenCard.tsx`           |
| `02-onboarding.html`       | Auth method picker                    | `OnboardingFlow.tsx` (to build)       |
| `03-settings.html`         | Settings panel                        | `SettingsPanel.tsx` (to build)        |
| `04-states.html`           | Unreachable / auth-required / critical | `TokenCard.tsx` (state variants)      |
| `05-tray-icons.html`       | The 4 tray icon states at all sizes   | `src-tauri/icons/tray-*.png`          |
| `06-burndown.html`         | 7-day burndown chart with projection  | `BurndownChart.tsx` (Phase 2)         |

## Design rules (the short version — see `DESIGN_SYSTEM.md` for the full set)

- **Two font weights only**: 400 and 500. Never 600 or 700.
- **Sentence case** everywhere except section labels (which are UPPERCASE with `letter-spacing: 0.08em`).
- **Hairlines at 0.5px** for all borders. Never 1px.
- **No shadows, no gradients, no rounded corners on progress bars.**
- **No CSS `backdrop-filter`** — translucency is native OS vibrancy, faked in these mockups with a near-solid background.
- **Status colors only in three places**: progress bar fills, percentage numbers ≥ warn, advisory text under critical quotas. Never as background fills.
- **`currentColor` for the brand mark** so it inherits the surrounding text color.

## Modifying these mockups

If a design decision changes:

1. Update the relevant mockup file
2. Update `DESIGN_SYSTEM.md` if the change touches the token system
3. Update the React implementation to match
4. All three in the same PR.
