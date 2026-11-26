#!/bin/sh
# Install vox from source with cargo.
#
#   curl -fsSL https://raw.githubusercontent.com/csgn/vox/master/install.sh | sh
#
# Environment:
#   VOX_VERSION       tag to install (default: latest v* tag)
#   VOX_REPO          git URL (default: https://github.com/csgn/vox.git)
#   VOX_INSTALL_ROOT  install into ROOT/bin (default: cargo's, usually ~/.cargo/bin)
#
# Needs git and cargo (https://rustup.rs).

set -eu

REPO="${VOX_REPO:-https://github.com/csgn/vox.git}"
VERSION="${VOX_VERSION:-}"

say() { printf 'vox-install: %s\n' "$*"; }
die() { say "error: $*" >&2; exit 1; }

command -v git >/dev/null 2>&1 || die "git not found"
command -v cargo >/dev/null 2>&1 ||
    die "cargo not found; install Rust from https://rustup.rs, then run this again"

if [ -z "$VERSION" ]; then
    VERSION=$(git ls-remote --tags --refs "$REPO" 'v*' |
        sed 's|.*refs/tags/||' |
        sort -t. -k1.2,1n -k2,2n -k3,3n |
        tail -n 1) || die "cannot read tags from $REPO"
    [ -n "$VERSION" ] || die "no version tags found in $REPO"
fi

say "installing vox $VERSION from $REPO"
if [ -n "${VOX_INSTALL_ROOT:-}" ]; then
    cargo install --locked --force --git "$REPO" --tag "$VERSION" --root "$VOX_INSTALL_ROOT" vox
    BIN="$VOX_INSTALL_ROOT/bin"
else
    cargo install --locked --force --git "$REPO" --tag "$VERSION" vox
    BIN="${CARGO_HOME:-$HOME/.cargo}/bin"
fi

say "installed $BIN/vox"
case ":$PATH:" in
    *":$BIN:"*) ;;
    *) say "add $BIN to your PATH" ;;
esac
say "plugins are built for wasm32-wasip2: rustup target add wasm32-wasip2"
