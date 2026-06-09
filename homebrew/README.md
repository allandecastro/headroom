# Homebrew Cask for Headroom

This directory holds the Homebrew Cask used to install Headroom on macOS:

```bash
brew install --cask allandecastro/headroom/headroom
```

Homebrew is the recommended macOS install path because the cask strips the
`com.apple.quarantine` flag after install, so the app launches without the
misleading **"Headroom is damaged and can't be opened"** Gatekeeper dialog that
appears with a direct `.dmg` download. (Headroom ships ad-hoc signed, not yet
notarized through the Apple Developer Program — see [FAQ.md](../FAQ.md#in-app-auto-update-code-signing).)

## One-time tap setup (maintainer)

Homebrew resolves `allandecastro/headroom/headroom` to a repository named
`homebrew-headroom` under the `allandecastro` account. Create it once:

1. Create a new public repo **`allandecastro/homebrew-headroom`**.
2. Add the cask at `Casks/headroom.rb` (copy it from this directory):

   ```
   homebrew-headroom/
   └── Casks/
       └── headroom.rb
   ```

3. Commit and push. Users can now run the `brew install --cask` command above.

> `homebrew/Casks/headroom.rb` in this repo is the **template / source of truth**
> for the cask's structure; the tap repo holds the published copy Homebrew reads,
> with `version` + `sha256` filled in for the latest release.

## Updating on each release — automated

The `update-tap` job in [`.github/workflows/release.yml`](../.github/workflows/release.yml)
does this for you on every `v*` tag: once the release is published, it downloads
the built `aarch64` DMG, computes its `sha256`, renders the cask from the template
above, and pushes it to `allandecastro/homebrew-headroom`. **No manual step.**

### One-time secret setup (required for the automation)

The built-in `GITHUB_TOKEN` can't push to a second repo, so the job needs a
token with write access to the tap:

1. Create a token — either:
   - a **classic** [Personal Access Token](https://github.com/settings/tokens/new) with the **`repo`** scope, or
   - a **fine-grained** token scoped to **`allandecastro/homebrew-headroom`** with **Contents: Read and write**.
2. Add it to the **app repo** at **Settings → Secrets and variables → Actions → New repository secret**, named **`TAP_GITHUB_TOKEN`**.

Until that secret exists the job skips itself with a warning — releases are never
blocked — and you can fall back to the manual path below.

### Manual fallback

```bash
# From the headroom repo root, after the GitHub release is live:
scripts/update-cask.sh 1.5.2          # downloads the DMG, recomputes sha256
cp homebrew/Casks/headroom.rb ../homebrew-headroom/Casks/headroom.rb
cd ../homebrew-headroom && git commit -am "headroom 1.5.2" && git push
```

Verify the cask before publishing:

```bash
brew style --online homebrew/Casks/headroom.rb
brew audit --cask --online homebrew/Casks/headroom.rb
```

## When macOS gets notarized

Once the macOS build is Developer ID signed and notarized (the commented Apple
secrets in `.github/workflows/release.yml`), delete the `postflight` quarantine
strip from the cask — Gatekeeper will accept the notarized app on its own.
