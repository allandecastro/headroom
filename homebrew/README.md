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

> The canonical copy of the cask lives here in the app repo (`homebrew/Casks/headroom.rb`)
> so it's versioned alongside the code; the tap repo is just the published mirror
> Homebrew reads from.

## Updating on each release

After a release is published, bump the pinned `version` + `sha256` and mirror it
to the tap:

```bash
# From the headroom repo root, after the GitHub release is live:
scripts/update-cask.sh 1.5.2          # downloads the DMG, recomputes sha256

# Then copy the updated cask into the tap repo and push:
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
