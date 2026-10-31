use crate::ilbm::{decode, packbits, unpackbits};
use crate::*;

fn encode_png(w: u32, h: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(color);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(data).unwrap();
    wr.finish().unwrap();
    out
}

fn gradient(w: u32, h: u32) -> Vec<u8> {
    let mut d = Vec::new();
    for y in 0..h {
        for x in 0..w {
            d.extend_from_slice(&[(x * 255 / (w - 1)) as u8, (y * 255 / (h - 1)) as u8, ((x + y) * 255 / (w + h - 2)) as u8, 255]);
        }
    }
    d
}

#[test]
fn packbits_roundtrip() {
    let cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![1],
        vec![1, 1],
        vec![1, 1, 1],
        vec![0; 300],
        (0..=255).collect(),
        vec![1, 2, 2, 3, 3, 3, 4, 4, 5, 5, 5, 5, 6],
        (0..500).map(|i| ((i * 7) % 5) as u8).collect(),
    ];
    for c in cases {
        let mut packed = Vec::new();
        packbits(&c, &mut packed);
        let mut out = Vec::new();
        let used = unpackbits(&packed, c.len(), &mut out).unwrap();
        assert_eq!(out, c);
        assert_eq!(used, packed.len());
    }
}

#[test]
fn indexed_exact_roundtrip() {
    // 4 Farben, ungerade Breite (Zeilenpadding)
    let (w, h) = (37u32, 9u32);
    let pal = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
    let mut data = Vec::new();
    for i in 0..(w * h) as usize {
        let c = pal[(i * 7 / 3) % 4];
        data.extend_from_slice(&[c[0], c[1], c[2], 255]);
    }
    let png = encode_png(w, h, png::ColorType::Rgba, &data);
    for compress in [true, false] {
        let out = convert(&png, &Options { compress, ..Options::default() }).unwrap();
        assert_eq!(out.planes, 2);
        assert_eq!(out.colors, 4);
        let dec = decode(&out.iff).unwrap();
        assert_eq!((dec.width, dec.height), (37, 9));
        assert_eq!(dec.rgba, data);
        assert_eq!(out.preview_rgba, data);
    }
}

#[test]
fn indexed_quantized_matches_preview() {
    let png = encode_png(64, 64, png::ColorType::Rgba, &gradient(64, 64));
    for (planes, dither) in [(3u8, false), (5, true), (8, false)] {
        let out = convert(&png, &Options { planes, dither, auto_planes: false, ..Options::default() }).unwrap();
        assert_eq!(out.planes, planes);
        assert_eq!(decode(&out.iff).unwrap().rgba, out.preview_rgba);
    }
}

#[test]
fn ocs_palette_is_4_bit() {
    let png = encode_png(32, 32, png::ColorType::Rgba, &gradient(32, 32));
    let out = convert(&png, &Options { ocs_palette: true, ..Options::default() }).unwrap();
    assert!(out.preview_rgba.chunks(4).all(|p| p[..3].iter().all(|v| v % 17 == 0)));
}

#[test]
fn ehb_and_ham_match_preview() {
    let png = encode_png(48, 40, png::ColorType::Rgba, &gradient(48, 40));
    for (mode, planes) in [(Mode::Ehb, 6), (Mode::Ham6, 6), (Mode::Ham8, 8)] {
        let out = convert(&png, &Options { mode, dither: true, ..Options::default() }).unwrap();
        assert_eq!(out.planes, planes);
        assert_eq!(decode(&out.iff).unwrap().rgba, out.preview_rgba, "{mode:?}");
    }
}

#[test]
fn ham8_is_close_to_source() {
    let src = gradient(64, 64);
    let png = encode_png(64, 64, png::ColorType::Rgba, &src);
    let out = convert(&png, &Options { mode: Mode::Ham8, ..Options::default() }).unwrap();
    let err: u64 = src.chunks(4).zip(out.preview_rgba.chunks(4)).map(|(a, b)| (0..3).map(|c| (a[c] as i64 - b[c] as i64).unsigned_abs()).sum::<u64>()).sum();
    assert!(err / (64 * 64 * 3) < 3, "mittlerer Fehler zu groß: {}", err / (64 * 64 * 3));
}

#[test]
fn transparency_modes() {
    let (w, h) = (20u32, 4u32);
    let mut data = Vec::new();
    for i in 0..(w * h) {
        data.extend_from_slice(&if i % 2 == 0 { [10, 200, 30, 255] } else { [99, 99, 99, 0] });
    }
    let png = encode_png(w, h, png::ColorType::Rgba, &data);
    let alpha = |rgba: &[u8]| rgba.chunks(4).map(|p| p[3]).collect::<Vec<_>>();
    let expect: Vec<u8> = data.chunks(4).map(|p| p[3]).collect();

    for (mode, transparency) in [
        (Mode::Indexed, Transparency::Mask),
        (Mode::Indexed, Transparency::ColorKey),
        (Mode::Ham6, Transparency::Mask),
        (Mode::Ehb, Transparency::ColorKey),
    ] {
        let out = convert(&png, &Options { mode, transparency, ..Options::default() }).unwrap();
        let dec = decode(&out.iff).unwrap();
        assert_eq!(alpha(&dec.rgba), expect, "{mode:?} {transparency:?}");
    }

    let out = convert(&png, &Options { transparency: Transparency::None, background: [1, 2, 3], ..Options::default() }).unwrap();
    let dec = decode(&out.iff).unwrap();
    assert!(alpha(&dec.rgba).iter().all(|&a| a == 255));
    assert_eq!(&dec.rgba[4..7], &[1, 2, 3]);
}

#[test]
fn gray_and_indexed_png_inputs() {
    let gray = encode_png(4, 1, png::ColorType::Grayscale, &[0, 85, 170, 255]);
    let out = convert(&gray, &Options::default()).unwrap();
    assert_eq!(out.colors, 4);
    assert_eq!(&out.preview_rgba[4..8], &[85, 85, 85, 255]);

    let mut buf = Vec::new();
    let mut enc = png::Encoder::new(&mut buf, 2, 1);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(vec![255, 0, 0, 0, 0, 255]);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(&[1, 0]).unwrap();
    wr.finish().unwrap();
    let out = convert(&buf, &Options::default()).unwrap();
    assert_eq!(out.preview_rgba, [0, 0, 255, 255, 255, 0, 0, 255]);
}

#[test]
fn iff_header_layout() {
    let png = encode_png(3, 2, png::ColorType::Rgba, &gradient(3, 2));
    let out = convert(&png, &Options { transparency: Transparency::None, ..Options::default() }).unwrap();
    assert_eq!(&out.iff[0..4], b"FORM");
    assert_eq!(u32::from_be_bytes(out.iff[4..8].try_into().unwrap()) as usize, out.iff.len() - 8);
    assert_eq!(&out.iff[8..12], b"ILBM");
    assert_eq!(&out.iff[12..16], b"BMHD");
    assert_eq!(out.iff.len() % 2, 0);
}

#[test]
fn invalid_input() {
    assert!(convert(b"not a png", &Options::default()).is_err());
    let png = encode_png(2, 2, png::ColorType::Rgba, &gradient(2, 2));
    assert!(convert(&png, &Options { planes: 9, ..Options::default() }).is_err());
}
