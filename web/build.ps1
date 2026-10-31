$ErrorActionPreference = 'Stop'
# Voraussetzungen: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli
Set-Location $PSScriptRoot\..
cargo build --lib --release --target wasm32-unknown-unknown --no-default-features
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
wasm-bindgen --target web --out-dir web\pkg --no-typescript target\wasm32-unknown-unknown\release\png2iff_core.wasm
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Fertig. Ordner 'web' per HTTP(S) ausliefern, z. B.: npx serve web"
