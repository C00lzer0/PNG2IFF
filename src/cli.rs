use png2iff_core::{Mode, Options, Transparency, convert};
use std::path::{Path, PathBuf};

pub const USAGE: &str = "\
png2iff - PNG nach Amiga IFF (ILBM) konvertieren

Aufruf:
  png2iff                      Startet die grafische Oberfläche
  png2iff [OPTIONEN] DATEI.png [DATEI2.png ...]

Optionen:
  -o, --output DATEI           Ausgabedatei (nur bei einer Eingabedatei)
  -m, --mode MODUS             indexed (Standard) | ehb | ham6 | ham8
  -p, --planes N               Max. Bitplanes im Modus indexed, 1-8 (Standard 5)
      --fixed-planes           Immer genau N Planes statt automatisch zu reduzieren
      --ocs                    Palette auf 4 Bit pro Farbkanal beschränken (OCS/ECS)
  -d, --dither                 Floyd-Steinberg-Dithering (indexed, ehb)
  -t, --transparency ART       mask (Standard) | key | none
  -b, --background RRGGBB      Hintergrundfarbe für Transparenz (Standard 000000)
      --no-compress            Keine ByteRun1-Kompression
  -h, --help                   Diese Hilfe
";

pub fn run(args: Vec<String>) -> Result<(), String> {
    let mut opts = Options::default();
    let mut inputs: Vec<PathBuf> = Vec::new();
    let mut output: Option<PathBuf> = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} erwartet einen Wert"));
        match a.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "-o" | "--output" => output = Some(value(&a)?.into()),
            "-m" | "--mode" => {
                opts.mode = match value(&a)?.to_lowercase().as_str() {
                    "indexed" => Mode::Indexed,
                    "ehb" => Mode::Ehb,
                    "ham6" => Mode::Ham6,
                    "ham8" => Mode::Ham8,
                    other => return Err(format!("Unbekannter Modus: {other}")),
                }
            }
            "-p" | "--planes" => {
                opts.planes = value(&a)?.parse().map_err(|_| "Ungültige Planes-Anzahl".to_string())?;
            }
            "--fixed-planes" => opts.auto_planes = false,
            "--ocs" => opts.ocs_palette = true,
            "-d" | "--dither" => opts.dither = true,
            "--no-compress" => opts.compress = false,
            "-t" | "--transparency" => {
                opts.transparency = match value(&a)?.to_lowercase().as_str() {
                    "mask" => Transparency::Mask,
                    "key" => Transparency::ColorKey,
                    "none" => Transparency::None,
                    other => return Err(format!("Unbekannte Transparenzart: {other}")),
                }
            }
            "-b" | "--background" => opts.background = parse_hex(&value(&a)?)?,
            s if s.starts_with('-') => return Err(format!("Unbekannte Option: {s}\n\n{USAGE}")),
            _ => inputs.push(a.into()),
        }
    }
    if inputs.is_empty() {
        return Err(format!("Keine Eingabedatei angegeben\n\n{USAGE}"));
    }
    if output.is_some() && inputs.len() > 1 {
        return Err("--output kann nur mit einer Eingabedatei verwendet werden".into());
    }
    for input in inputs {
        let out_path = output.clone().unwrap_or_else(|| input.with_extension("iff"));
        convert_file(&input, &out_path, &opts)?;
    }
    Ok(())
}

fn parse_hex(s: &str) -> Result<[u8; 3], String> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 || !s.is_ascii() {
        return Err("Hintergrundfarbe muss als RRGGBB angegeben werden".into());
    }
    let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "Ungültige Hintergrundfarbe".to_string());
    Ok([p(0)?, p(2)?, p(4)?])
}

fn convert_file(input: &Path, output: &Path, opts: &Options) -> Result<(), String> {
    let bytes = std::fs::read(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let out = convert(&bytes, opts).map_err(|e| format!("{}: {e}", input.display()))?;
    std::fs::write(output, &out.iff).map_err(|e| format!("{}: {e}", output.display()))?;
    println!(
        "{} -> {} ({}x{}, {} Planes, {} Farben, {} Bytes)",
        input.display(),
        output.display(),
        out.width,
        out.height,
        out.planes,
        out.colors,
        out.iff.len()
    );
    Ok(())
}
