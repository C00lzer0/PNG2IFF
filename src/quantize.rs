//! Farbquantisierung (Median Cut) und Zuordnung zu einer Palette (optional mit Floyd-Steinberg-Dithering).

pub type Rgb = [u8; 3];

pub fn dist2(a: Rgb, b: Rgb) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

/// Rundet auf 4 Bit pro Kanal (Amiga OCS/ECS) und expandiert wieder auf 8 Bit (n * 17).
pub fn round4(c: Rgb) -> Rgb {
    c.map(|v| ((v as u32 + 8) / 17).min(15) as u8 * 17)
}

pub fn nearest(cands: &[Rgb], c: Rgb) -> usize {
    let mut best = 0;
    let mut best_d = i32::MAX;
    for (i, &p) in cands.iter().enumerate() {
        let d = dist2(c, p);
        if d < best_d {
            best_d = d;
            best = i;
            if d == 0 {
                break;
            }
        }
    }
    best
}

/// Median-Cut über ein gewichtetes Histogramm. Liefert höchstens `n` Farben.
pub fn median_cut(hist: &[(Rgb, u32)], n: usize) -> Vec<Rgb> {
    if hist.is_empty() || n == 0 {
        return Vec::new();
    }
    let mut boxes: Vec<Vec<(Rgb, u32)>> = vec![hist.to_vec()];
    while boxes.len() < n {
        let mut pick: Option<(usize, u64, usize)> = None; // (box, score, channel)
        for (i, b) in boxes.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            let (ch, range) = widest_channel(b);
            if range == 0 {
                continue;
            }
            let pop: u64 = b.iter().map(|e| e.1 as u64).sum();
            let score = pop * range as u64;
            if pick.is_none_or(|p| score > p.1) {
                pick = Some((i, score, ch));
            }
        }
        let Some((i, _, ch)) = pick else { break };
        let mut b = boxes.swap_remove(i);
        b.sort_by_key(|e| e.0[ch]);
        let total: u64 = b.iter().map(|e| e.1 as u64).sum();
        let mut acc = 0u64;
        let mut cut = 1;
        for (k, e) in b.iter().enumerate() {
            acc += e.1 as u64;
            if acc * 2 >= total {
                cut = (k + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let right = b.split_off(cut);
        boxes.push(b);
        boxes.push(right);
    }
    let mut pal: Vec<(Rgb, u64)> = boxes.iter().map(|b| (average(b), pop(b))).collect();
    pal.sort_by(|a, b| b.1.cmp(&a.1));
    pal.into_iter().map(|p| p.0).collect()
}

fn pop(b: &[(Rgb, u32)]) -> u64 {
    b.iter().map(|e| e.1 as u64).sum()
}

fn widest_channel(b: &[(Rgb, u32)]) -> (usize, u8) {
    let mut best = (0, 0u8);
    for ch in 0..3 {
        let min = b.iter().map(|e| e.0[ch]).min().unwrap();
        let max = b.iter().map(|e| e.0[ch]).max().unwrap();
        if max - min > best.1 {
            best = (ch, max - min);
        }
    }
    best
}

fn average(b: &[(Rgb, u32)]) -> Rgb {
    let total = pop(b).max(1);
    let mut sum = [0u64; 3];
    for (c, w) in b {
        for ch in 0..3 {
            sum[ch] += c[ch] as u64 * *w as u64;
        }
    }
    sum.map(|s| ((s + total / 2) / total) as u8)
}

/// Ordnet jedem deckenden Pixel den Index der nächsten Kandidatenfarbe zu.
/// Nicht deckende Pixel erhalten Index 0 und beeinflussen das Dithering nicht.
pub fn map_to_palette(
    rgb: &[Rgb],
    opaque: &[bool],
    width: usize,
    cands: &[Rgb],
    dither: bool,
) -> Vec<u8> {
    let mut out = vec![0u8; rgb.len()];
    if cands.is_empty() {
        return out;
    }
    if !dither {
        let mut cache = std::collections::HashMap::new();
        for i in 0..rgb.len() {
            if opaque[i] {
                out[i] = *cache.entry(rgb[i]).or_insert_with(|| nearest(cands, rgb[i]) as u8);
            }
        }
        return out;
    }
    let height = rgb.len() / width;
    let mut cur = vec![[0f32; 3]; width + 2];
    let mut next = vec![[0f32; 3]; width + 2];
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            if !opaque[i] {
                continue;
            }
            let e = cur[x + 1];
            let want: Rgb = std::array::from_fn(|ch| (rgb[i][ch] as f32 + e[ch]).round().clamp(0.0, 255.0) as u8);
            let k = nearest(cands, want);
            out[i] = k as u8;
            let err: [f32; 3] = std::array::from_fn(|ch| want[ch] as f32 - cands[k][ch] as f32);
            for ch in 0..3 {
                cur[x + 2][ch] += err[ch] * 7.0 / 16.0;
                next[x][ch] += err[ch] * 3.0 / 16.0;
                next[x + 1][ch] += err[ch] * 5.0 / 16.0;
                next[x + 2][ch] += err[ch] / 16.0;
            }
        }
        std::mem::swap(&mut cur, &mut next);
        next.iter_mut().for_each(|e| *e = [0.0; 3]);
    }
    out
}
