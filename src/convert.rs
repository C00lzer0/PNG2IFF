//! Konvertierungspipeline PNG -> IFF ILBM.

use crate::ilbm::{self, IlbmImage};
use crate::quantize::{self, Rgb, dist2, map_to_palette, median_cut, round4};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Png(String),
    InvalidOptions(String),
    TooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Png(m) => write!(f, "PNG-Fehler: {m}"),
            Error::InvalidOptions(m) => write!(f, "Ungültige Optionen: {m}"),
            Error::TooLarge => write!(f, "Bild zu groß (maximal 65535x65535 Pixel)"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Palettenbild mit 1-8 Bitplanes (2-256 Farben).
    Indexed,
    /// Extra Half-Brite: 6 Planes, 32 Farben + 32 halbhelle Varianten.
    Ehb,
    /// Hold-And-Modify mit 6 Planes (16 Basisfarben, 4 Bit pro Kanal).
    Ham6,
    /// Hold-And-Modify mit 8 Planes (64 Basisfarben, 6 Bit Modifikation).
    Ham8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transparency {
    /// Transparenz wird mit der Hintergrundfarbe verrechnet.
    None,
    /// Zusätzliche Maskenebene (masking = 1).
    Mask,
    /// Palettenindex 0 ist die transparente Farbe (masking = 2). Nur im Modus `Indexed`,
    /// sonst wird wie `Mask` verfahren.
    ColorKey,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub mode: Mode,
    /// Maximale Anzahl Bitplanes im Modus `Indexed` (1-8).
    pub planes: u8,
    /// Bei wenigen Farben automatisch weniger Planes verwenden.
    pub auto_planes: bool,
    /// Palette auf 4 Bit pro Kanal (OCS/ECS) beschränken.
    pub ocs_palette: bool,
    /// Floyd-Steinberg-Dithering (Indexed, EHB).
    pub dither: bool,
    /// ByteRun1-Kompression.
    pub compress: bool,
    pub transparency: Transparency,
    pub background: Rgb,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::Indexed,
            planes: 5,
            auto_planes: true,
            ocs_palette: false,
            dither: false,
            compress: true,
            transparency: Transparency::Mask,
            background: [0, 0, 0],
        }
    }
}

/// RGBA8-Bild.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub struct Output {
    pub iff: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub planes: u8,
    /// Anzahl verwendeter Palettenfarben.
    pub colors: usize,
    /// So wird das Ergebnis auf dem Amiga aussehen (RGBA8).
    pub preview_rgba: Vec<u8>,
}

pub fn decode_png(bytes: &[u8]) -> Result<Rgba, Error> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| Error::Png(e.to_string()))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or(Error::TooLarge)?];
    let info = reader.next_frame(&mut buf).map_err(|e| Error::Png(e.to_string()))?;
    let src = &buf[..info.buffer_size()];
    let n = (info.width as usize) * (info.height as usize);
    let mut data = Vec::with_capacity(n * 4);
    match info.color_type {
        png::ColorType::Rgba => data.extend_from_slice(src),
        png::ColorType::Rgb => src.chunks_exact(3).for_each(|p| data.extend_from_slice(&[p[0], p[1], p[2], 255])),
        png::ColorType::Grayscale => src.iter().for_each(|&g| data.extend_from_slice(&[g, g, g, 255])),
        png::ColorType::GrayscaleAlpha => {
            src.chunks_exact(2).for_each(|p| data.extend_from_slice(&[p[0], p[0], p[0], p[1]]))
        }
        png::ColorType::Indexed => return Err(Error::Png("Indizierte Daten wurden nicht expandiert".into())),
    }
    Ok(Rgba { width: info.width, height: info.height, data })
}

pub fn convert(png_bytes: &[u8], opts: &Options) -> Result<Output, Error> {
    convert_rgba(&decode_png(png_bytes)?, opts)
}

fn histogram(rgb: &[Rgb], opaque: &[bool]) -> Vec<(Rgb, u32)> {
    let mut index: HashMap<Rgb, usize> = HashMap::new();
    let mut hist: Vec<(Rgb, u32)> = Vec::new();
    for (c, _) in rgb.iter().zip(opaque).filter(|(_, o)| **o) {
        match index.get(c) {
            Some(&i) => hist[i].1 += 1,
            None => {
                index.insert(*c, hist.len());
                hist.push((*c, 1));
            }
        }
    }
    hist
}

fn build_palette(hist: &[(Rgb, u32)], max: usize, four_bit: bool) -> Vec<Rgb> {
    if hist.len() <= max {
        return hist.iter().map(|h| h.0).collect();
    }
    let mut pal = median_cut(hist, max);
    if four_bit {
        pal.iter_mut().for_each(|c| *c = round4(*c));
    }
    pal
}

fn pad_palette(pal: &mut Vec<Rgb>, len: usize) {
    pal.resize(len, [0, 0, 0]);
}

pub fn convert_rgba(img: &Rgba, opts: &Options) -> Result<Output, Error> {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 {
        return Err(Error::InvalidOptions("leeres Bild".into()));
    }
    if w > 65535 || h > 65535 {
        return Err(Error::TooLarge);
    }
    if opts.mode == Mode::Indexed && !(1..=8).contains(&opts.planes) {
        return Err(Error::InvalidOptions("Bitplanes müssen zwischen 1 und 8 liegen".into()));
    }

    let four_bit = match opts.mode {
        Mode::Ham6 => true,
        Mode::Ham8 => false,
        _ => opts.ocs_palette,
    };
    let fix = |c: Rgb| if four_bit { round4(c) } else { c };
    let bg = fix(opts.background);

    let mut transparency = opts.transparency;
    if opts.mode != Mode::Indexed && transparency == Transparency::ColorKey {
        transparency = Transparency::Mask;
    }

    let n = w * h;
    let mut rgb = vec![[0u8; 3]; n];
    let mut opaque = vec![true; n];
    for (i, px) in img.data.chunks_exact(4).enumerate() {
        let a = px[3] as u32;
        if transparency == Transparency::None {
            rgb[i] = fix(std::array::from_fn(|ch| {
                ((px[ch] as u32 * a + opts.background[ch] as u32 * (255 - a) + 127) / 255) as u8
            }));
        } else if a < 128 {
            opaque[i] = false;
            rgb[i] = bg;
        } else {
            rgb[i] = fix([px[0], px[1], px[2]]);
        }
    }

    let hist = histogram(&rgb, &opaque);
    let mut palette: Vec<Rgb>;
    let pens: Vec<u8>;
    let mut shown: Vec<Rgb> = vec![bg; n];
    let planes: u8;
    let colors: usize;
    let mut camg = None;

    match opts.mode {
        Mode::Indexed => {
            let reserve = (transparency == Transparency::ColorKey) as usize;
            planes = if opts.auto_planes {
                (1..=opts.planes)
                    .find(|p| (1usize << p) - reserve >= hist.len().max(1))
                    .unwrap_or(opts.planes)
            } else {
                opts.planes
            };
            let max = (1usize << planes) - reserve;
            let mut pal = build_palette(&hist, max, four_bit);
            if pal.is_empty() {
                pal.push(bg);
            }
            let exact = hist.len() <= max;
            let idx = map_to_palette(&rgb, &opaque, w, &pal, opts.dither && !exact);
            colors = pal.len() + reserve;
            palette = Vec::with_capacity(1 << planes);
            if reserve == 1 {
                palette.push(bg);
            }
            palette.extend_from_slice(&pal);
            pens = idx.iter().zip(&opaque).map(|(&i, &o)| if o { i + reserve as u8 } else { 0 }).collect();
            pad_palette(&mut palette, 1 << planes);
            for i in 0..n {
                if opaque[i] {
                    shown[i] = palette[pens[i] as usize];
                }
            }
        }
        Mode::Ehb => {
            planes = 6;
            camg = Some(ilbm::CAMG_EHB);
            let mut pal = build_palette(&hist, 32, four_bit);
            colors = pal.len();
            pad_palette(&mut pal, 32);
            let mut cands = pal.clone();
            cands.extend(pal.iter().map(|c| c.map(|v| v >> 1)));
            pens = map_to_palette(&rgb, &opaque, w, &cands, opts.dither);
            for i in 0..n {
                if opaque[i] {
                    shown[i] = cands[pens[i] as usize];
                }
            }
            palette = pal;
        }
        Mode::Ham6 | Mode::Ham8 => {
            let bits = if opts.mode == Mode::Ham6 { 4 } else { 6 };
            planes = bits as u8 + 2;
            camg = Some(ilbm::CAMG_HAM);
            let mut pal = build_palette(&hist, 1 << bits, four_bit);
            if pal.is_empty() {
                pal.push(bg);
            }
            colors = pal.len();
            pad_palette(&mut pal, 1 << bits);
            let mut p = vec![0u8; n];
            encode_ham(&rgb, &opaque, w, &pal, bits, &mut p, &mut shown);
            pens = p;
            palette = pal;
        }
    }

    let masking = match transparency {
        Transparency::None => ilbm::MASK_NONE,
        Transparency::Mask => ilbm::MASK_PLANE,
        Transparency::ColorKey => ilbm::MASK_COLOR_KEY,
    };
    let iff = ilbm::write(&IlbmImage {
        width: w as u16,
        height: h as u16,
        planes,
        palette: &palette,
        pens: &pens,
        mask: Some(&opaque),
        masking,
        transparent_color: 0,
        camg,
        compress: opts.compress,
    });

    let mut preview_rgba = Vec::with_capacity(n * 4);
    for i in 0..n {
        preview_rgba.extend_from_slice(&shown[i]);
        preview_rgba.push(if opaque[i] { 255 } else { 0 });
    }
    Ok(Output { iff, width: w as u32, height: h as u32, planes, colors, preview_rgba })
}

/// HAM-Encoder: pro Pixel wird die Palettenfarbe oder die Änderung eines
/// einzelnen Kanals des linken Nachbarn gewählt, je nachdem was am nächsten liegt.
fn encode_ham(rgb: &[Rgb], opaque: &[bool], w: usize, pal: &[Rgb], bits: u32, pens: &mut [u8], shown: &mut [Rgb]) {
    for row in 0..rgb.len() / w {
        let mut prev = pal[0];
        for x in 0..w {
            let i = row * w + x;
            if !opaque[i] {
                prev = pal[0];
                continue;
            }
            let t = rgb[i];
            let k = quantize::nearest(pal, t);
            let mut best = (dist2(t, pal[k]), k as u8, pal[k]);
            for ch in 0..3 {
                let (val, v) = if bits == 4 {
                    let v = ((t[ch] as i32 + 8) / 17).min(15);
                    ((v * 17) as u8, v)
                } else {
                    let low = (prev[ch] & 3) as i32;
                    let v = (t[ch] as i32 - low + 2).div_euclid(4).clamp(0, 63);
                    (((v << 2) | low) as u8, v)
                };
                let mut c = prev;
                c[ch] = val;
                let e = dist2(t, c);
                if e < best.0 {
                    let ctrl = match ch {
                        2 => 1u8,
                        0 => 2,
                        _ => 3,
                    };
                    best = (e, (ctrl << bits) | v as u8, c);
                }
            }
            pens[i] = best.1;
            shown[i] = best.2;
            prev = best.2;
        }
    }
}
