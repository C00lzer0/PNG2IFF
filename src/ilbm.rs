//! Schreiben und Lesen von IFF-ILBM-Dateien (Amiga Interleaved Bitmap).

use crate::quantize::Rgb;

pub const CAMG_EHB: u32 = 0x0080;
pub const CAMG_HAM: u32 = 0x0800;

pub const MASK_NONE: u8 = 0;
pub const MASK_PLANE: u8 = 1;
pub const MASK_COLOR_KEY: u8 = 2;

pub struct IlbmImage<'a> {
    pub width: u16,
    pub height: u16,
    pub planes: u8,
    pub palette: &'a [Rgb],
    /// Ein Pen-Wert (Chunky) pro Pixel.
    pub pens: &'a [u8],
    /// Eine Maskenebene schreiben (true = deckend). Nur mit `masking == MASK_PLANE`.
    pub mask: Option<&'a [bool]>,
    pub masking: u8,
    pub transparent_color: u16,
    pub camg: Option<u32>,
    pub compress: bool,
}

/// ByteRun1 (PackBits) Kompression einer einzelnen Zeile.
pub fn packbits(src: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < src.len() {
        let mut run = 1;
        while i + run < src.len() && src[i + run] == src[i] && run < 128 {
            run += 1;
        }
        if run >= 3 {
            out.push((257 - run) as u8);
            out.push(src[i]);
            i += run;
        } else {
            let start = i;
            while i < src.len()
                && i - start < 128
                && !(i + 2 < src.len() && src[i] == src[i + 1] && src[i] == src[i + 2])
            {
                i += 1;
            }
            out.push((i - start - 1) as u8);
            out.extend_from_slice(&src[start..i]);
        }
    }
}

/// ByteRun1 Dekompression von genau `len` Bytes. Gibt die Anzahl gelesener Eingabebytes zurück.
pub fn unpackbits(src: &[u8], len: usize, out: &mut Vec<u8>) -> Result<usize, String> {
    let target = out.len() + len;
    let mut i = 0;
    while out.len() < target {
        let n = *src.get(i).ok_or("BODY zu kurz")? as i8;
        i += 1;
        if n >= 0 {
            let cnt = n as usize + 1;
            let data = src.get(i..i + cnt).ok_or("BODY zu kurz")?;
            out.extend_from_slice(data);
            i += cnt;
        } else if n != -128 {
            let cnt = (1 - n as i32) as usize;
            let b = *src.get(i).ok_or("BODY zu kurz")?;
            i += 1;
            out.extend(std::iter::repeat_n(b, cnt));
        }
    }
    if out.len() != target {
        return Err("Zeilenlänge im BODY inkonsistent".into());
    }
    Ok(i)
}

fn chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
}

pub fn write(img: &IlbmImage) -> Vec<u8> {
    let w = img.width as usize;
    let h = img.height as usize;
    let row_bytes = w.div_ceil(16) * 2;

    let mut bmhd = Vec::with_capacity(20);
    bmhd.extend_from_slice(&img.width.to_be_bytes());
    bmhd.extend_from_slice(&img.height.to_be_bytes());
    bmhd.extend_from_slice(&0i16.to_be_bytes());
    bmhd.extend_from_slice(&0i16.to_be_bytes());
    bmhd.push(img.planes);
    bmhd.push(img.masking);
    bmhd.push(img.compress as u8);
    bmhd.push(0);
    bmhd.extend_from_slice(&img.transparent_color.to_be_bytes());
    bmhd.push(1);
    bmhd.push(1);
    bmhd.extend_from_slice(&(img.width as i16).to_be_bytes());
    bmhd.extend_from_slice(&(img.height as i16).to_be_bytes());

    let cmap: Vec<u8> = img.palette.iter().flatten().copied().collect();

    let mut body = Vec::new();
    let mut line = vec![0u8; row_bytes];
    let emit = |line: &[u8], body: &mut Vec<u8>| {
        if img.compress {
            packbits(line, body);
        } else {
            body.extend_from_slice(line);
        }
    };
    for y in 0..h {
        let row = &img.pens[y * w..(y + 1) * w];
        for p in 0..img.planes {
            line.fill(0);
            for (x, &pen) in row.iter().enumerate() {
                if (pen >> p) & 1 != 0 {
                    line[x / 8] |= 0x80 >> (x % 8);
                }
            }
            emit(&line, &mut body);
        }
        if img.masking == MASK_PLANE {
            line.fill(0);
            if let Some(mask) = img.mask {
                for x in 0..w {
                    if mask[y * w + x] {
                        line[x / 8] |= 0x80 >> (x % 8);
                    }
                }
            }
            emit(&line, &mut body);
        }
    }

    let mut form = Vec::new();
    form.extend_from_slice(b"ILBM");
    chunk(&mut form, b"BMHD", &bmhd);
    chunk(&mut form, b"CMAP", &cmap);
    if let Some(camg) = img.camg {
        chunk(&mut form, b"CAMG", &camg.to_be_bytes());
    }
    chunk(&mut form, b"BODY", &body);

    let mut out = Vec::with_capacity(form.len() + 8);
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&(form.len() as u32).to_be_bytes());
    out.extend_from_slice(&form);
    out
}

/// Ergebnis von [`decode`]: RGBA8-Bild.
pub struct Decoded {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Liest eine ILBM-Datei (inkl. ByteRun1, Maske, EHB, HAM6/HAM8) in ein RGBA-Bild.
pub fn decode(data: &[u8]) -> Result<Decoded, String> {
    if data.len() < 12 || &data[0..4] != b"FORM" || &data[8..12] != b"ILBM" {
        return Err("Keine IFF-ILBM-Datei".into());
    }
    let (mut bmhd, mut cmap, mut camg, mut body): (Option<&[u8]>, &[u8], u32, Option<&[u8]>) =
        (None, &[], 0, None);
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let len = u32::from_be_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let end = (pos + 8 + len).min(data.len());
        let payload = &data[pos + 8..end];
        match id {
            b"BMHD" => bmhd = Some(payload),
            b"CMAP" => cmap = payload,
            b"CAMG" if payload.len() >= 4 => camg = u32::from_be_bytes(payload[..4].try_into().unwrap()),
            b"BODY" => body = Some(payload),
            _ => {}
        }
        pos += 8 + len + (len & 1);
    }
    let bmhd = bmhd.filter(|b| b.len() >= 20).ok_or("BMHD fehlt")?;
    let body = body.ok_or("BODY fehlt")?;
    let w = u16::from_be_bytes([bmhd[0], bmhd[1]]) as usize;
    let h = u16::from_be_bytes([bmhd[2], bmhd[3]]) as usize;
    let planes = bmhd[8] as usize;
    let masking = bmhd[9];
    let compression = bmhd[10];
    let transparent = u16::from_be_bytes([bmhd[12], bmhd[13]]) as usize;
    if planes == 0 || planes > 8 {
        return Err("Nicht unterstützte Ebenenanzahl".into());
    }
    let palette: Vec<Rgb> = cmap.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    let row_bytes = w.div_ceil(16) * 2;
    let total_planes = planes + (masking == MASK_PLANE) as usize;

    let mut raw = Vec::with_capacity(row_bytes * total_planes * h);
    if compression == 1 {
        let mut i = 0;
        for _ in 0..h * total_planes {
            i += unpackbits(&body[i..], row_bytes, &mut raw)?;
        }
    } else {
        raw.extend_from_slice(body.get(..row_bytes * total_planes * h).ok_or("BODY zu kurz")?);
    }

    let ham = camg & CAMG_HAM != 0 && (planes == 6 || planes == 8);
    let ehb = camg & CAMG_EHB != 0 && planes == 6;
    let ham_bits = planes.saturating_sub(2);
    let pal = |i: usize| palette.get(i).copied().unwrap_or([0, 0, 0]);
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let mut prev = pal(0);
        for x in 0..w {
            let mut pen = 0usize;
            for p in 0..planes {
                let off = (y * total_planes + p) * row_bytes + x / 8;
                pen |= (((raw[off] >> (7 - x % 8)) & 1) as usize) << p;
            }
            let mut alpha = 255;
            if masking == MASK_PLANE {
                let off = (y * total_planes + planes) * row_bytes + x / 8;
                if (raw[off] >> (7 - x % 8)) & 1 == 0 {
                    alpha = 0;
                }
            } else if masking == MASK_COLOR_KEY && pen == transparent {
                alpha = 0;
            }
            let c = if ham {
                let v = pen & ((1 << ham_bits) - 1);
                let mut c = prev;
                let val = |prev: u8| -> u8 {
                    if ham_bits == 4 {
                        (v * 17) as u8
                    } else {
                        ((v << 2) as u8) | (prev & 3)
                    }
                };
                match pen >> ham_bits {
                    0 => c = pal(v),
                    1 => c[2] = val(prev[2]),
                    2 => c[0] = val(prev[0]),
                    _ => c[1] = val(prev[1]),
                }
                prev = c;
                c
            } else if ehb && pen >= 32 {
                pal(pen - 32).map(|v| v >> 1)
            } else {
                pal(pen)
            };
            let o = (y * w + x) * 4;
            rgba[o..o + 3].copy_from_slice(&c);
            rgba[o + 3] = alpha;
        }
    }
    Ok(Decoded { width: w, height: h, rgba })
}
