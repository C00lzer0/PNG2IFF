# Png2Iff

Konvertiert PNG-Bilder in das Amiga-Format **IFF ILBM**. Ein gemeinsamer Rust-Kern, drei Oberflächen:

| Plattform | Variante |
|---|---|
| Windows, macOS, Linux | Desktop-App (GUI, `egui`) und Kommandozeile |
| iOS / iPadOS (sowie Android und jeder moderne Browser) | Web-App (WebAssembly), als PWA auf den Home-Bildschirm installierbar |

## Funktionen

- Palettenbilder mit 1–8 Bitplanes (2–256 Farben), automatische Reduktion der Planes, Median-Cut-Quantisierung, optional Floyd-Steinberg-Dithering
- **EHB** (Extra Half-Brite, 64 Farben), **HAM6** und **HAM8**
- Optionale OCS/ECS-Palette (4 Bit pro Farbkanal)
- Transparenz als Maskenebene, als transparente Farbe (Index 0) oder mit Hintergrundfarbe verrechnet
- ByteRun1-Kompression (abschaltbar)
- Live-Vorschau, wie das Bild auf dem Amiga aussieht
- Unterstützt alle PNG-Varianten (Graustufen, RGB(A), indiziert, 16 Bit)

## Desktop (Windows, macOS, Linux)

```sh
cargo run --release              # GUI starten
cargo run --release -- bild.png  # Kommandozeile -> bild.iff
```

Kommandozeile (`png2iff --help`):

```
png2iff [OPTIONEN] DATEI.png [DATEI2.png ...]
  -o, --output DATEI        Ausgabedatei (nur bei einer Eingabedatei)
  -m, --mode MODUS          indexed (Standard) | ehb | ham6 | ham8
  -p, --planes N            Max. Bitplanes im Modus indexed, 1-8 (Standard 5)
      --fixed-planes        Immer genau N Planes
      --ocs                 4-Bit-Palette (OCS/ECS)
  -d, --dither              Floyd-Steinberg-Dithering
  -t, --transparency ART    mask (Standard) | key | none
  -b, --background RRGGBB   Hintergrundfarbe für Transparenz
      --no-compress         Keine ByteRun1-Kompression
```

Unter Linux werden für die GUI die üblichen X11-/Wayland- und GTK-Entwicklungspakete benötigt
(z. B. `libgtk-3-dev` für den Dateidialog). Ohne GUI: `cargo build --release --no-default-features`.

## Web / iOS

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli     # Version muss zur wasm-bindgen-Version in Cargo.lock passen
web/build.sh                       # oder: web\build.ps1 unter Windows
python3 -m http.server -d web 8080 # lokal testen
```

Den Ordner `web/` auf einen beliebigen HTTPS-Webspace legen. Für GitHub Pages baut und veröffentlicht
`.github/workflows/pages.yml` die App bei jedem Push auf `main` automatisch (einmalig unter
**Settings → Pages → Source: GitHub Actions** aktivieren); Adresse: `https://<benutzer>.github.io/<repo>/`.
Auf dem iPhone/iPad in
Safari öffnen und über **Teilen → Zum Home-Bildschirm** installieren; danach läuft die App offline.
Die Konvertierung findet vollständig lokal im Browser statt, es werden keine Daten hochgeladen.

> Eine native App für den App Store lässt sich aus `web/` mit einem WKWebView-Wrapper (Xcode, macOS erforderlich) erzeugen.

## Tests

```sh
cargo test --lib --no-default-features
```

Die Tests prüfen u. a. Rundreisen (PNG → IFF → Dekodierung durch einen eigenen ILBM-Decoder, inkl. EHB/HAM/Maske).

## Format-Hinweise

- Ausgabe: `FORM ILBM` mit `BMHD`, `CMAP`, `CAMG` (nur EHB/HAM) und `BODY`; Pixel-Seitenverhältnis 1:1.
- HAM-Basispaletten werden per Median-Cut gewählt; HAM8 ersetzt bei Modifikation die oberen 6 Bit des Kanals (wie die AGA-Hardware).
- Bei EHB/HAM wird „transparente Farbe“ wie Maskenebene behandelt.
