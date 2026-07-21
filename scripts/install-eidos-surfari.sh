#!/usr/bin/env bash
# install-eidos-surfari.sh — Eidos Surfari web-release installer (not npm)
#
# Docs: docs/EIDOS_UPGRADE.md  |  docs site: /eidos-upgrade
#
# Usage (new machine / future installer):
#   curl -fsSL https://raw.githubusercontent.com/eidos-agi/surfari/main/scripts/install-eidos-surfari.sh | bash
#   bash scripts/install-eidos-surfari.sh
#   bash scripts/install-eidos-surfari.sh v0.32.2
#   bash scripts/install-eidos-surfari.sh --prefix /opt/eidos-surfari
#   bash scripts/install-eidos-surfari.sh --self-test
#
# Requires: curl, bash, uname. Optional: gh (fallback only).
# Does NOT use npm.
set -euo pipefail

REPO="${EIDOS_SURFARI_REPO:-eidos-agi/surfari}"
PREFIX="${EIDOS_SURFARI_PREFIX:-${HOME}/.local/share/eidos/surfari}"
BIN_LINK_DIR="${EIDOS_SURFARI_BINDIR:-${HOME}/.local/bin}"
TAG=""
SELF_TEST=0

usage() {
  sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage 0 ;;
    --prefix) PREFIX="$2"; shift 2 ;;
    --bindir) BIN_LINK_DIR="$2"; shift 2 ;;
    --self-test) SELF_TEST=1; shift ;;
    --repo) REPO="$2"; shift 2 ;;
    v*|V*) TAG="$1"; shift ;;
    *)
      echo "Unknown arg: $1" >&2
      usage 1
      ;;
  esac
done

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"
case "${os}-${arch}" in
  darwin-arm64|darwin-aarch64) ASSET="agent-browser-darwin-arm64" ;;
  darwin-x86_64)               ASSET="agent-browser-darwin-x64" ;;
  linux-x86_64)                ASSET="agent-browser-linux-x64" ;;
  linux-aarch64|linux-arm64)   ASSET="agent-browser-linux-arm64" ;;
  *)
    echo "Unsupported platform: ${os}-${arch}" >&2
    exit 1
    ;;
esac

BINDIR="${PREFIX}/bin"
WRAPPER="${PREFIX}/surfari-wrapper.sh"
UPDATER="${PREFIX}/update-eidos-surfari.sh"
PRODUCT="surfari"

download_asset() {
  local dest_dir="$1"
  mkdir -p "$dest_dir"
  local tmp
  tmp="$(mktemp -d)"
  (
    cd "$tmp"
    if [[ -n "$TAG" ]]; then
      URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
    else
      URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"
    fi
    echo "Downloading from web: ${URL}"
    if ! curl -fsSL --retry 3 --retry-delay 1 -o "$ASSET" "$URL"; then
      echo "curl failed; trying gh..." >&2
      if [[ -n "$TAG" ]]; then
        gh release download "$TAG" -R "$REPO" -p "$ASSET" --clobber
      else
        gh release download -R "$REPO" -p "$ASSET" --clobber
      fi
    fi
    [[ -s "$ASSET" ]] || { echo "download empty" >&2; exit 1; }
    install -m 755 "$ASSET" "${dest_dir}/${PRODUCT}"
    cp -f "$ASSET" "${dest_dir}/${ASSET}"
    chmod +x "${dest_dir}/${ASSET}"
  )
  rm -rf "$tmp"
}

write_wrapper() {
  # Wrapper must resolve through PATH symlinks back to PREFIX (not bindir).
  cat > "$WRAPPER" <<'WRAP'
#!/usr/bin/env bash
set -euo pipefail
# Resolve this script even when invoked via ~/.local/bin/surfari → .../surfari-wrapper.sh
_src="${BASH_SOURCE[0]:-$0}"
while [[ -L "$_src" ]]; do
  _dir="$(cd -P "$(dirname "$_src")" && pwd)"
  _src="$(readlink "$_src")"
  [[ "$_src" != /* ]] && _src="${_dir}/${_src}"
done
HERE="$(cd -P "$(dirname "$_src")" && pwd)"
REAL="${HERE}/bin/surfari"
UPDATER="${HERE}/update-eidos-surfari.sh"
if [[ ! -x "$REAL" ]]; then
  echo "surfari binary missing at $REAL" >&2
  exit 127
fi
case "${1:-}" in
  upgrade|update)
    shift
    if [[ -x "$UPDATER" ]]; then
      exec "$UPDATER" "$@"
    fi
    echo "updater missing at $UPDATER" >&2
    exit 1
    ;;
  *)
    exec "$REAL" "$@"
    ;;
esac
WRAP
  chmod +x "$WRAPPER"
}

write_updater() {
  # Updater re-invokes this installer's logic by embedding a thin re-exec.
  # Prefer: colocated install.sh; else re-fetch installer from GitHub (web).
  cat > "$UPDATER" <<UP
#!/usr/bin/env bash
set -euo pipefail
export EIDOS_SURFARI_REPO="${REPO}"
export EIDOS_SURFARI_PREFIX="${PREFIX}"
export EIDOS_SURFARI_BINDIR="${BIN_LINK_DIR}"
INSTALLER="${PREFIX}/install.sh"
INSTALLER_URL="https://raw.githubusercontent.com/${REPO}/main/scripts/install-eidos-surfari.sh"
if [[ ! -x "\$INSTALLER" ]]; then
  mkdir -p "${PREFIX}"
  curl -fsSL "\$INSTALLER_URL" -o "\$INSTALLER"
  chmod +x "\$INSTALLER"
fi
exec "\$INSTALLER" "\$@"
UP
  chmod +x "$UPDATER"
}

link_path_names() {
  mkdir -p "$BIN_LINK_DIR"
  ln -sfn "$WRAPPER" "${BIN_LINK_DIR}/surfari"
  ln -sfn "$WRAPPER" "${BIN_LINK_DIR}/surfari-browser"
  ln -sfn "$WRAPPER" "${BIN_LINK_DIR}/agent-browser"
}


install_licenses() {
  # Apache-2.0 §4: ship License + NOTICE with Object form redistribution
  local licdir="${PREFIX}/licenses"
  mkdir -p "$licdir"
  local here base ref
  here="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
  base="https://raw.githubusercontent.com/${REPO}"
  if [[ -n "${TAG:-}" ]]; then
    ref="$TAG"
  else
    ref="main"
  fi
  for f in LICENSE NOTICE; do
    # Prefer files from a git checkout of this repo (works before remote merge)
    if [[ -f "${here}/../${f}" ]]; then
      cp -f "${here}/../${f}" "${licdir}/${f}"
    elif [[ -f "${here}/${f}" ]]; then
      cp -f "${here}/${f}" "${licdir}/${f}"
    elif curl -fsSL "${base}/${ref}/${f}" -o "${licdir}/${f}" 2>/dev/null; then
      :
    fi
  done
  if [[ ! -s "${licdir}/LICENSE" ]]; then
    echo "WARNING: LICENSE missing under ${licdir} (Apache-2.0 redistribution)" >&2
  fi
  if [[ ! -s "${licdir}/NOTICE" ]]; then
    echo "WARNING: NOTICE missing under ${licdir} (attribution)" >&2
  fi
}

install_main() {
  mkdir -p "$BINDIR" "$PREFIX"
  download_asset "$BINDIR"
  install_licenses
  write_wrapper
  # Persist this installer so future upgrades on this machine are local.
  # If we were piped from curl, $0 may be bash / fd — copy from BASH_SOURCE when possible.
  # Skip no-op copy (macOS cp errors when src==dest).
  _persist_installer() {
    local src="$1"
    local dest="${PREFIX}/install.sh"
    [[ -f "$src" && -r "$src" ]] || return 0
    local src_abs dest_abs
    src_abs="$(cd "$(dirname "$src")" && pwd)/$(basename "$src")"
    mkdir -p "$PREFIX"
    dest_abs="$(cd "$PREFIX" && pwd)/install.sh"
    if [[ "$src_abs" != "$dest_abs" ]]; then
      cp -f "$src" "$dest"
    fi
    chmod +x "$dest"
  }
  if [[ -f "${BASH_SOURCE[0]:-}" ]]; then
    _persist_installer "${BASH_SOURCE[0]}"
  elif [[ -f "$0" && -r "$0" ]]; then
    _persist_installer "$0"
  fi
  # curl|bash path: fetch a durable copy of this installer into the prefix
  if [[ ! -x "${PREFIX}/install.sh" ]]; then
    curl -fsSL "https://raw.githubusercontent.com/${REPO}/main/scripts/install-eidos-surfari.sh" \
      -o "${PREFIX}/install.sh" || true
    chmod +x "${PREFIX}/install.sh" 2>/dev/null || true
  fi
  write_updater
  link_path_names

  export PATH="${BIN_LINK_DIR}:${PATH}"
  echo "Installed from web (renamed ${ASSET} → ${PRODUCT})"
  echo "  prefix:  ${PREFIX}"
  echo "  bindir:  ${BIN_LINK_DIR}"
  echo "  version: $("${BIN_LINK_DIR}/surfari" --version)"
  echo "  license: Apache-2.0 (see ${PREFIX}/licenses/LICENSE)"
  echo "  notice:  ${PREFIX}/licenses/NOTICE"
  echo "  upgrade: surfari upgrade"
}

self_test() {
  local work
  work="$(mktemp -d /tmp/eidos-surfari-selftest.XXXXXX)"
  echo "=== SELF-TEST (clean room) workdir=${work} ==="
  local fake_home="${work}/home"
  local fake_prefix="${fake_home}/.local/share/eidos/surfari"
  local fake_bin="${fake_home}/.local/bin"
  mkdir -p "$fake_home" "$fake_bin"

  # Install as a brand-new user would, isolated HOME
  env -i \
    HOME="$fake_home" \
    PATH="/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/usr/local/bin" \
    USER="${USER:-installer}" \
    TMPDIR="${work}/tmp" \
    EIDOS_SURFARI_REPO="$REPO" \
    bash "${BASH_SOURCE[0]}" --prefix "$fake_prefix" --bindir "$fake_bin" ${TAG:+"$TAG"}

  # Prove PATH names
  local s a
  s="$(env -i HOME="$fake_home" PATH="$fake_bin:/usr/bin:/bin" command -v surfari)"
  a="$(env -i HOME="$fake_home" PATH="$fake_bin:/usr/bin:/bin" command -v agent-browser)"
  [[ "$s" == "${fake_bin}/surfari" ]] || { echo "FAIL surfari path: $s"; exit 1; }
  [[ "$a" == "${fake_bin}/agent-browser" ]] || { echo "FAIL agent-browser path: $a"; exit 1; }
  echo "OK paths: surfari=$s agent-browser=$a"

  # Version
  local ver
  ver="$(env -i HOME="$fake_home" PATH="$fake_bin:/usr/bin:/bin" surfari --version)"
  echo "OK version: $ver"
  [[ -n "$ver" ]] || { echo "FAIL empty version"; exit 1; }

  # Upgrade from web (second install cycle — what future users run months later)
  mkdir -p "${work}/tmp"
  local up
  up="$(
    env HOME="$fake_home" \
      PATH="${fake_bin}:/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin" \
      TMPDIR="${work}/tmp" \
      "${fake_bin}/surfari" upgrade 2>&1
  )"
  echo "$up"
  echo "$up" | grep -q "Downloading from web" || { echo "FAIL upgrade not from web"; exit 1; }
  if echo "$up" | grep -qi "npm install"; then
    echo "FAIL upgrade used npm"
    exit 1
  fi
  echo "OK upgrade from web (no npm)"

  # Hash rename integrity
  local h1 h2
  h1="$(shasum -a 256 "${fake_prefix}/bin/surfari" | awk '{print $1}')"
  h2="$(shasum -a 256 "${fake_prefix}/bin/${ASSET}" | awk '{print $1}')"
  [[ "$h1" == "$h2" ]] || { echo "FAIL hash mismatch"; exit 1; }
  echo "OK rename integrity $h1"

  # Binary is real Mach-O/ELF
  file "${fake_prefix}/bin/surfari" | tee /dev/stderr | grep -Eqi 'Mach-O|ELF|executable' \
    || { echo "FAIL not a native binary"; exit 1; }
  echo "OK native binary"

  # Apache-2.0 redistribution artifacts
  [[ -s "${fake_prefix}/licenses/LICENSE" ]] || { echo "FAIL missing LICENSE"; exit 1; }
  [[ -s "${fake_prefix}/licenses/NOTICE" ]] || { echo "FAIL missing NOTICE"; exit 1; }
  grep -q "Apache" "${fake_prefix}/licenses/LICENSE" || { echo "FAIL LICENSE not Apache"; exit 1; }
  grep -qi "Vercel" "${fake_prefix}/licenses/NOTICE" || { echo "FAIL NOTICE missing Vercel attribution"; exit 1; }
  echo "OK LICENSE + NOTICE installed"

  # Browserbase subcommand present (no credentials required for --help)
  env HOME="$fake_home" PATH="$fake_bin:/usr/bin:/bin" "${fake_bin}/surfari" browserbase --help 2>&1 | head -5 \
    | grep -qi browserbase || { echo "FAIL browserbase missing"; exit 1; }
  echo "OK browserbase subcommand"

  # Live browser only if Chrome already installed in real home (reuse) or skip with note
  if [[ -d "${HOME}/.agent-browser/browsers" ]]; then
    # Point clean home at shared browser cache via symlink for proof without re-download
    mkdir -p "${fake_home}/.agent-browser"
    ln -sfn "${HOME}/.agent-browser/browsers" "${fake_home}/.agent-browser/browsers"
    local shot="${work}/proof.png"
    env HOME="$fake_home" PATH="$fake_bin:/usr/bin:/bin:/opt/homebrew/bin" \
      "${fake_bin}/surfari" open "https://example.com" >/dev/null
    env HOME="$fake_home" PATH="$fake_bin:/usr/bin:/bin:/opt/homebrew/bin" \
      "${fake_bin}/surfari" screenshot "$shot" >/dev/null
    env HOME="$fake_home" PATH="$fake_bin:/usr/bin:/bin:/opt/homebrew/bin" \
      "${fake_bin}/surfari" close >/dev/null || true
    [[ -s "$shot" ]] || { echo "FAIL screenshot empty"; exit 1; }
    file "$shot"
    echo "OK live browser screenshot from clean-room install"
  else
    echo "SKIP live browser (no chrome cache); install+upgrade+binary OK"
  fi

  # Write proof artifact into the clean prefix and to real share if present
  {
    echo "self_test_ok=1"
    echo "timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "repo=${REPO}"
    echo "version=${ver}"
    echo "hash=${h1}"
    echo "workdir=${work}"
  } | tee "${fake_prefix}/SELFTEST.ok"

  echo "=== SELF-TEST PASSED ==="
  echo "Clean-room prefix can be removed: rm -rf ${work}"
}

if [[ "$SELF_TEST" -eq 1 ]]; then
  self_test
else
  install_main
fi
