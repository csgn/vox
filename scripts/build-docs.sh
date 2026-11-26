#!/usr/bin/env bash
# Build the docs website: code tour data, mdBook, and the API reference (rustdoc,
# including private items) under docs/book/api/.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/gen-code-tour.py
mdbook build docs
rm -rf target/doc
cargo doc --workspace --no-deps --document-private-items
rm -rf docs/book/api
cp -r target/doc docs/book/api
echo "docs built: docs/book/index.html (serve with: python3 -m http.server -d docs/book 3000)"
