#!/bin/sh
# Optional convenience wrapper for the workspace-local toolchain used in this session.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
if [ -x "$root/.cargo/bin/cargo" ] && [ -d "$root/.rustup" ]; then
  export CARGO_HOME="$root/.cargo"
  export RUSTUP_HOME="$root/.rustup"
  exec "$root/.cargo/bin/cargo" "$@"
fi
exec cargo "$@"
