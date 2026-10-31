//! PNG -> Amiga IFF (ILBM) Konverter.
//!
//! Der Kern ist plattformunabhängig und läuft nativ (Windows, macOS, Linux)
//! sowie als WebAssembly im Browser (z. B. iOS Safari).

pub mod convert;
pub mod ilbm;
pub mod quantize;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use convert::{Error, Mode, Options, Output, Rgba, Transparency, convert, convert_rgba, decode_png};

#[cfg(test)]
mod tests;
