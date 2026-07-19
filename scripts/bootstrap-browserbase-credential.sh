#!/usr/bin/env bash
set -euo pipefail

umask 077

credential_name="browserbase-api-key"
credential_dir="/etc/credstore.encrypted/emux-browser-broker"
credential_path="${credential_dir}/${credential_name}.cred"
rotate=false

usage() {
  printf 'Usage: %s [--rotate]\n' "$0"
}

case "${1:-}" in
  "") ;;
  --rotate) rotate=true ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

if [[ ! -t 0 || ! -t 1 ]]; then
  printf 'error: run this script from an interactive terminal\n' >&2
  exit 1
fi

command -v systemd-creds >/dev/null || {
  printf 'error: systemd-creds is required\n' >&2
  exit 1
}

# Prove installation authority before reading the secret. A sudo password
# prompt must never consume input intended for the API key.
if ! sudo -n true; then
  printf 'error: passwordless sudo is required for credential installation\n' >&2
  exit 1
fi

if sudo -n test -e "$credential_path" && [[ "$rotate" != true ]]; then
  printf 'error: credential already exists; use --rotate to replace it atomically\n' >&2
  exit 1
fi

encrypted_tmp="$(mktemp /tmp/surfari-browserbase-credential.XXXXXX)"
browserbase_key=""
cleanup() {
  browserbase_key=""
  unset browserbase_key
  rm -f -- "$encrypted_tmp"
}
trap cleanup EXIT HUP INT TERM

printf 'Browserbase API key (hidden): '
IFS= read -r -s browserbase_key
printf '\n'

if [[ -z "$browserbase_key" ]]; then
  printf 'error: key must not be empty\n' >&2
  exit 1
fi

# Plaintext exists only in this process and its pipe to systemd-creds.
printf '%s' "$browserbase_key" \
  | sudo -n systemd-creds encrypt --name="$credential_name" - "$encrypted_tmp" \
      >/dev/null

browserbase_key=""
unset browserbase_key

# Verify host-local decryption without printing the value.
sudo -n systemd-creds decrypt --name="$credential_name" "$encrypted_tmp" - \
  >/dev/null

sudo -n install -d -o root -g root -m 0700 "$credential_dir"
sudo -n install -o root -g root -m 0600 "$encrypted_tmp" "${credential_path}.new"
sudo -n mv -f -- "${credential_path}.new" "$credential_path"

printf 'Installed encrypted credential: %s\n' "$credential_path"
printf 'Credential name for LoadCredentialEncrypted=: %s\n' "$credential_name"
printf 'No plaintext credential was written to disk.\n'
