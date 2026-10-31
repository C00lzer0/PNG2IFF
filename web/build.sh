#!/usr/bin/env sh
# Voraussetzungen: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli
set -e
cd "$(dirname "$0")/.."
cargo build --lib --release --target wasm32-unknown-unknown --no-default-features
wasm-bindgen --target web --out-dir web/pkg --no-typescript target/wasm32-unknown-unknown/release/png2iff_core.wasm
echo "Fertig. Ordner 'web' per HTTP(S) ausliefern, z. B.: python3 -m http.server -d web 8080"
