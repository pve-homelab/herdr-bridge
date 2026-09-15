#!/usr/bin/env sh
# Print install paths and env hints for herdr-http-plugin.
set -eu

root="${HERDR_PLUGIN_ROOT:-.}"
config="${HERDR_PLUGIN_CONFIG_DIR:-"(unset)"}"
state="${HERDR_PLUGIN_STATE_DIR:-"(unset)"}"
bin="$root/target/release/herdr-http-plugin"

echo "plugin id:     ${HERDR_PLUGIN_ID:-pve-homelab.herdr-http-plugin}"
echo "plugin root:   $root"
echo "config dir:    $config"
echo "state dir:     $state"
echo "binary:        $bin"

if [ -x "$bin" ]; then
  echo "binary status: ok"
else
  echo "binary status: missing — run: (cd \"$root\" && cargo build --release)"
fi

echo
echo "Required env for the model bridge:"
echo "  MODEL_BASE_URL   e.g. http://localhost:8000/v1"
echo "  MODEL_API_KEY    optional Bearer token"
echo
echo "Put user config under the config dir above (do not write into the managed plugin root)."
echo "Docs: https://github.com/pve-homelab/herdr-bridge#readme"
