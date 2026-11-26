#!/usr/bin/env bash
# Build a plugin project and package it in the standard installed layout:
#   DIR/dist/NAME/{plugin.toml, plugin.wasm, README.md}
# With --install DEST, also copy the package to DEST/NAME (e.g. ~/.config/vox/plugins).
# Needs: rustup target add wasm32-wasip2
set -euo pipefail
usage() { echo "usage: $0 PLUGIN_DIR [--install DEST]" >&2; exit 2; }
[ $# -ge 1 ] || usage
dir=$(cd "$1" && pwd)
dest=""
if [ $# -ge 2 ]; then
  [ "$2" = "--install" ] && [ $# -eq 3 ] || usage
  dest=$3
fi
[ -f "$dir/plugin.toml" ] || { echo "error: $dir/plugin.toml not found" >&2; exit 1; }
name=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$dir/plugin.toml" | head -1)
[ -n "$name" ] || { echo "error: no name in plugin.toml" >&2; exit 1; }
crate=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$dir/Cargo.toml" | head -1)
target_dir=${CARGO_TARGET_DIR:-$dir/target}
cargo build --release --target wasm32-wasip2 --manifest-path "$dir/Cargo.toml" --target-dir "$target_dir"
wasm="$target_dir/wasm32-wasip2/release/${crate//-/_}.wasm"
out="$dir/dist/$name"
rm -rf "$out" && mkdir -p "$out"
cp "$dir/plugin.toml" "$out/"
cp "$wasm" "$out/plugin.wasm"
[ -f "$dir/README.md" ] && cp "$dir/README.md" "$out/"
echo "packaged $out"
if [ -n "$dest" ]; then
  mkdir -p "$dest"
  rm -rf "${dest:?}/$name"
  cp -r "$out" "$dest/$name"
  echo "installed $dest/$name"
fi
