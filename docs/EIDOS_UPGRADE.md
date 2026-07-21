# Eidos Surfari — install & upgrade (web release channel)

Canonical docs for the **eidos-agi/surfari** release channel (not npm).

**Site page:** [docs: Eidos install & upgrade](./src/app/eidos-upgrade/page.mdx)  
**Installer:** [`scripts/install-eidos-surfari.sh`](../scripts/install-eidos-surfari.sh)  
**Homebrew:** [`eidos-agi/homebrew-tap`](https://github.com/eidos-agi/homebrew-tap) formula `surfari`

## Install (macOS — Homebrew, preferred when available)

```bash
brew install eidos-agi/tap/surfari
surfari install              # Chrome for Testing, first time
brew upgrade surfari         # brew-native upgrade
```

This formula installs GitHub release assets and renames them to **`surfari`**. It is **not** Homebrew core `agent-browser` (vercel-labs).

## Install (any machine — curl installer)

```bash
curl -fsSL https://raw.githubusercontent.com/eidos-agi/surfari/main/scripts/install-eidos-surfari.sh | bash
surfari install   # Chrome for Testing, first time
```

## Upgrade

| Install method | Upgrade |
|----------------|---------|
| Homebrew | `brew upgrade surfari` |
| curl / install script | `surfari upgrade` or `surfari upgrade v0.32.2` |

Both paths download from:

```text
https://github.com/eidos-agi/surfari/releases/latest/download/agent-browser-<platform>
```

Renames the asset to **`surfari`**. Does **not** use npm.

## Self-test (prove future installers)

```bash
bash scripts/install-eidos-surfari.sh --self-test
```

## Rules

| Do | Don’t |
|----|--------|
| Web release install + rename to `surfari` | `npm install -g agent-browser` for this fork |
| `brew install eidos-agi/tap/surfari` on macOS | Homebrew core `agent-browser` for the Eidos channel |
| `surfari upgrade` via the Eidos wrapper (curl install) | Trust raw binary `upgrade` without the wrapper |

There is no npm package `surfari` / `@eidos-agi/surfari`. The release asset name remains `agent-browser-*` (upstream cargo name); product PATH name is `surfari`.

## Licensing (Apache-2.0 redistribution)

Upstream **agent-browser** is licensed under the **Apache License, Version 2.0** (Copyright 2025 Vercel Inc.). See [`LICENSE`](../LICENSE) and [`NOTICE`](../NOTICE).

Eidos packaging (rename to `surfari`, installers, Homebrew formula) redistributes the **Object form** of that work. Apache-2.0 requires that redistributions include a copy of the License and retain attribution. Packaging does that as follows:

| Channel | Where LICENSE / NOTICE land |
|---------|-----------------------------|
| curl installer | `~/.local/share/eidos/surfari/licenses/{LICENSE,NOTICE}` |
| Homebrew | `$(brew --prefix surfari)/share/doc/surfari/{LICENSE,NOTICE}` |
| Source checkout | repo root `LICENSE` + `NOTICE` |

Rules of the road:

- Keep **Apache-2.0** on all redistributed binaries and installers.
- Do **not** strip or omit `LICENSE` / `NOTICE` from install prefixes.
- Attribute **Vercel Inc.** / **agent-browser** as upstream; “Surfari” is the Eidos product/PATH name only.
- Do **not** imply Vercel endorsement of the Eidos packaging or Browserbase integrations.
- Installer and docs changes are Derivative Works; state material packaging changes in `NOTICE` / docs when they matter.

Full SPDX: `Apache-2.0`.
## Canonical channel (this machine / Eidos)

| Preferred | Command |
|-----------|---------|
| macOS | `brew install eidos-agi/tap/surfari` then `brew upgrade surfari` |
| any OS | curl installer / `surfari upgrade` (web) |

**Not** Homebrew core `agent-browser` (vercel-labs). Core stays upstream; Eidos packaging is **`eidos-agi/tap/surfari` only**.

### PATH / which binary

If both Homebrew and the curl prefix are installed, the wrapper on `~/.local/bin/surfari` **prefers the Homebrew binary** and routes `surfari upgrade` to `brew upgrade eidos-agi/tap/surfari`.

```bash
surfari channel    # channel=homebrew|local-web, browserbase readiness
```

### Browserbase readiness (macOS)

1. Prefer provider flag with API key (local):

```bash
# ~/.agent-browser/browserbase.env  (chmod 600; never commit)
BROWSERBASE_API_KEY=bb_...
```

```bash
surfari -p browserbase open https://example.com
```

The wrapper auto-sources `~/.agent-browser/browserbase.env` when the key is not already in the environment.

2. Lifecycle subcommands (`surfari browserbase create|status|release`) use the **encrypted broker** (Eidos service / systemd credentials). On a plain Mac mini without broker injection they report `Browserbase credential unavailable from encrypted broker` until Knox/service credentials are configured.

3. Check without secrets:

```bash
surfari channel
surfari doctor --offline --quick
```

### Homebrew formula bumps after a release

- Automatic daily cron on `eidos-agi/homebrew-tap` (`Bump surfari formula`)
- On GitHub Release publish: workflow `Notify Homebrew tap` (needs secret `HOMEBREW_TAP_TOKEN`)
- Manual: `gh workflow run 'Bump surfari formula' -R eidos-agi/homebrew-tap -f tag=vX.Y.Z`
