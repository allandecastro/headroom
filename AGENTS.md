# AGENTS.md

Guidelines for AI coding assistants (Claude Code, GitHub Copilot, Cursor, etc.) working on this project. Read this before making changes.

## What Headroom is

A native menu bar app, written in Tauri 2 (Rust + React). It polls Claude Code and GitHub Copilot quota endpoints every 30 seconds and shows the worst remaining quota in the tray, with a click-through popover for details. See [README.md](README.md) for product context and [SPEC.md](SPEC.md) for architecture.

## Architecture invariants

These are not negotiable in PRs:

1. **The renderer never makes network calls.** All HTTP traffic, credential handling, and polling lives in the Rust backend (`src-tauri/`). The renderer subscribes to `tokens-updated` events and renders snapshots. Violating this means credentials leak into the renderer process, which is a security hole.

2. **Credentials live in the OS keychain.** Never in `localStorage`, never in plain files, never in environment variables passed to the renderer. The `credentials` module wraps the `keyring` crate — use it.

3. **`QuotaSource` is the only extension point for new services.** Don't add bespoke code paths for one service. If the trait doesn't fit a new source, change the trait — but be prepared to defend the change.

4. **The orchestrator owns the polling loop.** Sources are pure functions of credentials. They don't schedule themselves, don't retain state across calls, don't write to disk.

5. **The tray icon is a function of state, not the other way around.** Changes to which icon shows come from snapshot changes propagating through the state machine. Don't reach for `tray.set_icon()` from inside a source adapter.

## Code style

### Rust

- Run `cargo fmt` before committing. CI rejects unformatted code.
- Run `cargo clippy --all-targets --all-features -- -D warnings`. CI enforces.
- Prefer `?` for error propagation. Use `anyhow::Context` for descriptive errors at boundary points; use thiserror-derived enums for the source layer where callers need to discriminate error kinds.
- No `unwrap()` outside `main()` setup, tests, or in contexts where the invariant is documented in a comment immediately above.
- Public functions get doc comments. `cargo doc` should produce useful output.
- Tests live in `#[cfg(test)] mod tests { ... }` blocks next to the code under test. Cross-module integration tests in `src-tauri/tests/`.

### TypeScript

- Functional components only. Class components are out.
- Tailwind utility classes. Inline `style` is allowed only for dynamic values (progress bar width, computed positions). Static styles → Tailwind.
- Imports ordered: React → external → `@tauri-apps/*` → local components → local libs → CSS. ESLint enforces.
- `any` is banned. If a Tauri IPC payload's type isn't known, fix the type on the Rust side and re-emit, don't paper over with `any`.

### Markdown

- Sentence case headings.
- Tables for structured comparisons, prose for flowing explanation. Don't bullet-list every other line.
- Internal links use the relative path: `[SPEC.md](SPEC.md)` not absolute URLs.

## Workflow expectations

When making a change:

1. Read the relevant section of [SPEC.md](SPEC.md) first. If the change contradicts the spec, decide whether the spec is wrong (update both in the same PR) or whether the change is wrong (rethink).
2. Run `npm run tauri dev` and verify the change works end-to-end on at least one platform (macOS preferred during dev).
3. Run `npm run lint`, `npm run typecheck`, `cd src-tauri && cargo clippy && cargo test`. Don't push if any fails.
4. Write a commit message that says **why** the change is needed, not just what changed. The diff already shows what.
5. Update [SPEC.md](SPEC.md), [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md), or [ROADMAP.md](ROADMAP.md) in the same PR if the change touches their concerns.

## Things to avoid

- **Don't add localStorage / IndexedDB calls in the renderer.** State is either ephemeral (React state) or backend-persisted (Tauri IPC → Rust → disk).
- **Don't introduce new dependencies casually.** The dependency surface is intentionally small. New crate or npm package = explain why in the PR.
- **Don't add CSS `backdrop-filter`** — translucency is OS-native (vibrancy/Mica), not faked in CSS. See [DESIGN_SYSTEM.md § Translucency rules](DESIGN_SYSTEM.md#translucency-rules).
- **Don't use emoji in the UI** unless it's an explicit design call. The visual language is restrained; emoji break it.
- **Don't refactor existing code "while you're in there."** Mixed-purpose PRs are hard to review. Open a separate PR for the cleanup.
- **Don't suppress lint warnings with `#[allow(...)]` or `// eslint-disable`** without a comment explaining why. CI will flag silent suppressions.
- **Don't change `tauri.conf.json` window settings** (transparency, vibrancy, decorations) without testing on all three platforms. These settings interact in non-obvious ways across OSes.
- **Don't rasterize the brand mark into the renderer bundle.** PNG copies exist for tray icons and the macOS/Windows app bundle only — never for UI use.

## Patterns to follow

### Adding a quota source

See [CONTRIBUTING.md § Adding a new quota source](CONTRIBUTING.md#adding-a-new-quota-source). The trait is in `src-tauri/src/sources/mod.rs`. Implement it, register in `lib.rs`, add an icon, document in SPEC.

### Modifying a UI component

The renderer reads snapshots and dispatches user actions. To change what's displayed:

1. If the data already exists in the snapshot → render-only change in `src/components/`.
2. If new data is needed → extend the snapshot shape in `src-tauri/src/sources/mod.rs::ServiceStatus`, update each source adapter that produces this data, and propagate to TypeScript types in `src/lib/api.ts`.

### Handling a new error case

Errors flow source → orchestrator → renderer. The state machine in `src-tauri/src/sources/mod.rs::SourceError` is the discrimination layer. Add a new variant if needed, handle it in the orchestrator's state transitions, and surface a corresponding UI state in `TokenCard.tsx`.

## Project context for AI assistants

- The author is a French developer (Allan De Castro, MVP Business Applications). Code and docs are in English; in-line comments may occasionally be in French — that's fine.
- The project prioritizes refinement over feature breadth. A new dropdown or hover state usually gets rejected; a new source adapter or a fix for a real bug gets merged fast.
- The visual design (mockups in `docs/mockups/`) is settled. Resist the urge to "improve" the popover — design changes should come from explicit design discussions, not from PR side effects.
- The data acquisition strategy is fragile by nature (we depend on undocumented Anthropic endpoints and the official-but-evolving GitHub billing API). Resilience matters more than DRY. Two source adapters that share 80% of their HTTP boilerplate are fine; abstracting that boilerplate into a `BaseSource` is not.
- The brand mark source of truth is `assets/headroom-mark.svg`. In the React UI, never import the SVG file directly — use the `<BrandMark />` component from `src/components/BrandMark.tsx`. The component renders inline SVG with `fill="currentColor"`, which lets the mark inherit color from its container via Tailwind classes (e.g. `<BrandMark className="text-fg-primary" />`).

## When in doubt

Ask. Open a draft PR with a `[discussion]` prefix in the title and describe what you're trying to do. It's faster than guessing and getting reverted.
