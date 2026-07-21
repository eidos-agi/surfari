# Eidos Surfari — install & upgrade (web release channel)

Canonical docs for the **eidos-agi/surfari** release channel (not npm).

**Site page:** [docs: Eidos install & upgrade](./src/app/eidos-upgrade/page.mdx)  
**Installer:** [`scripts/install-eidos-surfari.sh`](../scripts/install-eidos-surfari.sh)

## Install (new machine)

```bash
curl -fsSL https://raw.githubusercontent.com/eidos-agi/surfari/main/scripts/install-eidos-surfari.sh | bash
surfari install   # Chrome for Testing, first time
```

## Upgrade

```bash
surfari upgrade              # latest GitHub release asset
surfari upgrade v0.32.2      # pin
```

Downloads from:

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
| `surfari upgrade` via the Eidos wrapper | Trust raw binary `upgrade` without the wrapper |

There is no npm package `surfari` / `@eidos-agi/surfari`. The release asset name remains `agent-browser-*` (upstream cargo name); product PATH name is `surfari`.
