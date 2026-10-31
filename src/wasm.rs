//! WebAssembly-Schnittstelle für die Web-App (läuft u. a. in iOS Safari).

use crate::{Mode, Options, Output, Transparency, convert};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Converted(Output);

#[wasm_bindgen]
impl Converted {
    /// Die fertige IFF-Datei.
    pub fn iff(&self) -> Vec<u8> {
        self.0.iff.clone()
    }

    /// Vorschau (RGBA8), wie das Bild auf dem Amiga aussieht.
    pub fn preview_rgba(&self) -> Vec<u8> {
        self.0.preview_rgba.clone()
    }

    pub fn width(&self) -> u32 {
        self.0.width
    }

    pub fn height(&self) -> u32 {
        self.0.height
    }

    pub fn planes(&self) -> u8 {
        self.0.planes
    }

    pub fn colors(&self) -> usize {
        self.0.colors
    }
}

/// `mode`: indexed | ehb | ham6 | ham8, `transparency`: mask | key | none,
/// `background`: 0xRRGGBB.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn convert_png(
    bytes: &[u8],
    mode: &str,
    planes: u8,
    auto_planes: bool,
    ocs_palette: bool,
    dither: bool,
    compress: bool,
    transparency: &str,
    background: u32,
) -> Result<Converted, JsError> {
    let mode = match mode {
        "indexed" => Mode::Indexed,
        "ehb" => Mode::Ehb,
        "ham6" => Mode::Ham6,
        "ham8" => Mode::Ham8,
        other => return Err(JsError::new(&format!("Unbekannter Modus: {other}"))),
    };
    let transparency = match transparency {
        "mask" => Transparency::Mask,
        "key" => Transparency::ColorKey,
        "none" => Transparency::None,
        other => return Err(JsError::new(&format!("Unbekannte Transparenzart: {other}"))),
    };
    let opts = Options {
        mode,
        planes,
        auto_planes,
        ocs_palette,
        dither,
        compress,
        transparency,
        background: [(background >> 16) as u8, (background >> 8) as u8, background as u8],
    };
    convert(bytes, &opts).map(Converted).map_err(|e| JsError::new(&e.to_string()))
}
