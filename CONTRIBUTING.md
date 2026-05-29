# Contributing

Thanks for considering a contribution. Headroom is small enough that the contribution surface is mostly bug reports, design tweaks, and adding new quota sources. Here's how to do each.

## Quick setup

```bash
git clone https://github.com/USER/headroom.git
cd headroom
npm install
npm run tauri dev
```

Prerequisites: Rust 1.78+, Node.js 20+, and the platform-specific Tauri prerequisites listed at <https://tauri.app/start/prerequisites/>.

## Branching and commits

- `main` is always green and always shippable. No direct pushes — PRs only.
- Feature branches: `feat/<short-name>`. Bug fixes: `fix/<short-name>`. Docs: `docs/<short-name>`.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/):
  - `feat: add Cursor source adapter`
  - `fix(tray): icon stays stale after auth_required`
  - `docs: clarify keychain service name in SPEC`
  - `chore: bump tauri to 2.3.1`

Squash-merge to `main`. The PR title becomes the commit message.

## Code style

### Rust

- Format with `cargo fmt`. CI checks this.
- Lint with `cargo clippy --all-targets --all-features -- -D warnings`. CI checks this.
- Prefer `?` over explicit `match` for error propagation.
- Avoid `unwrap()` / `expect()` outside of `main` setup and tests. Use `?` with a typed error or `anyhow::Context` with a descriptive `.context("...")` message.
- Tests live next to the code they test (`#[cfg(test)] mod tests { ... }`). Integration tests in `src-tauri/tests/`.

### TypeScript / React

- Format with Prettier (config in `package.json`). CI checks this.
- Lint with `eslint --ext .ts,.tsx src/`. CI checks this.
- Functional components only. Hooks for state. No class components.
- Tailwind utility classes for styling. Avoid inline `style={}` except for dynamic values (progress bar widths, etc.).
- File names: `PascalCase.tsx` for components, `camelCase.ts` for utilities.
- Import order: React → external libs → `@tauri-apps/*` → local components → local libs → CSS. ESLint enforces this.

### General

- No commented-out code in PRs. If you're not using it, delete it; Git remembers.
- No TODO comments without an associated issue number: `// TODO(#42): handle Cloudflare 503`.
- Lines longer than 100 chars get a side-eye unless there's a good reason.

## Pull requests

A good PR:

1. **Solves one thing.** Not "fix the bug and also clean up the file." Split it.
2. **Has a description that says why, not just what.** The diff already shows what.
3. **Updates tests.** If you change behavior, prove it.
4. **Updates docs if it changes the spec.** SPEC.md is the source of truth for architecture; if you changed how a source adapter works, update the spec in the same PR.
5. **Passes CI.** Lint, typecheck, test, build — all green before requesting review.

The PR template prompts for these. Use it.

## Adding a new quota source

The architecture is designed to make this a self-contained change.

1. Create `src-tauri/src/sources/<service>.rs` implementing the `QuotaSource` trait.
2. Register the source in the orchestrator (`src-tauri/src/lib.rs`).
3. Add a row to the onboarding flow in `src/components/OnboardingFlow.tsx`.
4. Add the service's icon to `assets/services/<service>.svg`.
5. Document the data source in [SPEC.md](SPEC.md#data-sources).
6. Add a [CHANGELOG.md](CHANGELOG.md) entry under `[Unreleased] → Added`.

Tests should cover at minimum: a successful fetch with a recorded response, an auth failure, and a network timeout.

## Reporting bugs

GitHub Issues. Include:

- OS and version (`uname -a` / `winver` / `cat /etc/os-release`)
- Headroom version (Settings → About, or `headroom --version` on CLI builds)
- What you expected
- What you saw instead
- Logs from `~/.local/share/headroom/headroom.log` (or platform equivalent), specifically the last 50 lines around the issue

Security issues: do **not** open a public issue. Email the maintainer (address in `CODEOWNERS`).

## Reviewing PRs

If you're added as a reviewer, you get one approval round and one revision round before a re-review. We don't expect five rounds of nits — the standard is "would I be happy maintaining this code in a year?"

Reviewers focus on:

- **Correctness** — does the code do what the PR says it does?
- **Maintainability** — will someone understand this in six months?
- **Test coverage** — is the new behavior verified?
- **Spec alignment** — does this change the architecture? If so, is SPEC.md updated?

Style nits (formatting, naming) are handled by CI. Don't waste review cycles on them.

## License

By contributing, you agree your contribution is licensed under the project's MIT license.
