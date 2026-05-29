# Security Policy

## Reporting a vulnerability

If you find a security issue in Headroom, **please don't open a public issue.** Instead:

1. Open a **private security advisory** at [github.com/allandecastro/headroom/security/advisories/new](https://github.com/allandecastro/headroom/security/advisories/new).
2. Include: what the issue is, how to reproduce it, what an attacker could do, and (if you have one) a suggested fix.
3. You'll get an acknowledgement within ~72 hours.

If GitHub's private advisory flow isn't available to you, fall back to emailing the address listed on [@allandecastro](https://github.com/allandecastro)'s GitHub profile.

## Supported versions

Headroom is in active development and only the latest tagged release is supported. Fixes will land in a new release, not as a back-port.

| Version    | Supported |
| ---------- | --------- |
| latest tag | ✅        |
| older tags | ❌        |

## Threat model

Headroom is a personal desktop app that polls two AI vendor APIs and stores nothing beyond your local machine. The realistic risks:

- **Stolen API token / cookie.** Mitigated by storing credentials only in the OS keychain (Windows Credential Manager / macOS Keychain / Linux Secret Service). Never logged, never written elsewhere, never sent off-machine.
- **Compromised binary.** Same risk as any desktop app: if you install a malicious build, it can do whatever your user account can do. Always install from [github.com/allandecastro/headroom/releases](https://github.com/allandecastro/headroom/releases) (the only official source). Code signing is on the roadmap (Phase 5) but not yet shipped — verify checksums if you're cautious.
- **Other user-level malware on the same machine** can read OS keychain items requested by name. This is an OS limitation, not specific to Headroom; the entire desktop app ecosystem shares it.

## What Headroom doesn't do (by design)

- ❌ Send any telemetry, analytics, or usage data off-machine.
- ❌ Log credential values (verified — only error objects are logged, never tokens or cookies).
- ❌ Store credentials anywhere except the OS keychain.
- ❌ Bundle a client secret (the device-flow / PAT paths require none).
- ❌ Accept inbound network connections.

See also: [FAQ.md § Privacy & data](FAQ.md#privacy--data).
