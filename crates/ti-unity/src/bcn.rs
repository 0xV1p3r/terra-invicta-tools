//! BC1/BC3/BC7 block decoding, ported from Pillow's `BcnDecode.c` (public domain, CC0) so the
//! pixels match what UnityPy produces through Pillow. Its integer rounding differs from other
//! decoders (e.g. texture2ddecoder), so do not swap this for a crate.

type Rgba = [u8; 4];

fn decode_565(x: u16) -> Rgba {
    let mut r = ((x & 0xf800) >> 8) as u32;
    r |= r >> 5;
    let mut g = ((x & 0x7e0) >> 3) as u32;
    g |= g >> 6;
    let mut b = ((x & 0x1f) << 3) as u32;
    b |= b >> 5;
    [r as u8, g as u8, b as u8, 0xff]
}

fn decode_bc1_color(dst: &mut [Rgba; 16], src: &[u8], separate_alpha: bool) {
    let c0 = u16::from_le_bytes([src[0], src[1]]);
    let c1 = u16::from_le_bytes([src[2], src[3]]);
    let mut p = [decode_565(c0), decode_565(c1), [0; 4], [0; 4]];
    let (r0, g0, b0) = (u32::from(p[0][0]), u32::from(p[0][1]), u32::from(p[0][2]));
    let (r1, g1, b1) = (u32::from(p[1][0]), u32::from(p[1][1]), u32::from(p[1][2]));
    if c0 > c1 || separate_alpha {
        p[2] = [((2 * r0 + r1) / 3) as u8, ((2 * g0 + g1) / 3) as u8, ((2 * b0 + b1) / 3) as u8, 0xff];
        p[3] = [((r0 + 2 * r1) / 3) as u8, ((g0 + 2 * g1) / 3) as u8, ((b0 + 2 * b1) / 3) as u8, 0xff];
    } else {
        p[2] = [((r0 + r1) / 2) as u8, ((g0 + g1) / 2) as u8, ((b0 + b1) / 2) as u8, 0xff];
        p[3] = [0, 0, 0, 0];
    }
    for n in 0..4 {
        for o in 0..4 {
            let cw = 3 & (src[4 + n] >> (2 * o));
            dst[n * 4 + o] = p[cw as usize];
        }
    }
}

fn decode_bc3_alpha(dst: &mut [Rgba; 16], src: &[u8], channel: usize) {
    let (a0, a1) = (u32::from(src[0]), u32::from(src[1]));
    let lut1 = u32::from(src[2]) | (u32::from(src[3]) << 8) | (u32::from(src[4]) << 16);
    let lut2 = u32::from(src[5]) | (u32::from(src[6]) << 8) | (u32::from(src[7]) << 16);
    let mut a = [a0 as u8, a1 as u8, 0, 0, 0, 0, 0, 0];
    if a0 > a1 {
        for (k, v) in a[2..].iter_mut().enumerate() {
            let k = k as u32;
            *v = (((6 - k) * a0 + (1 + k) * a1) / 7) as u8;
        }
    } else {
        for (k, v) in a[2..6].iter_mut().enumerate() {
            let k = k as u32;
            *v = (((4 - k) * a0 + (1 + k) * a1) / 5) as u8;
        }
        a[6] = 0;
        a[7] = 0xff;
    }
    for n in 0..8 {
        dst[n][channel] = a[(7 & (lut1 >> (3 * n))) as usize];
        dst[8 + n][channel] = a[(7 & (lut2 >> (3 * n))) as usize];
    }
}

fn get_bits(src: &[u8], bit: usize, count: usize) -> u8 {
    if count == 0 {
        return 0;
    }
    let by = bit >> 3;
    let bit = bit & 7;
    if bit + count <= 8 {
        (src[by] >> bit) & ((1u16 << count) - 1) as u8
    } else {
        let x = u32::from(src[by]) | (u32::from(src.get(by + 1).copied().unwrap_or(0)) << 8);
        ((x >> bit) & ((1 << count) - 1)) as u8
    }
}

struct Bc7Mode {
    ns: usize,
    pb: usize,
    rb: usize,
    isb: usize,
    cb: usize,
    ab: usize,
    epb: bool,
    spb: bool,
    ib: usize,
    ib2: usize,
}

#[allow(clippy::too_many_arguments)]
const fn m(ns: usize, pb: usize, rb: usize, isb: usize, cb: usize, ab: usize, epb: bool, spb: bool, ib: usize, ib2: usize) -> Bc7Mode {
    Bc7Mode {
        ns,
        pb,
        rb,
        isb,
        cb,
        ab,
        epb,
        spb,
        ib,
        ib2,
    }
}

const BC7_MODES: [Bc7Mode; 8] = [
    m(3, 4, 0, 0, 4, 0, true, false, 3, 0),
    m(2, 6, 0, 0, 6, 0, false, true, 3, 0),
    m(3, 6, 0, 0, 5, 0, false, false, 2, 0),
    m(2, 6, 0, 0, 7, 0, true, false, 2, 0),
    m(1, 0, 2, 1, 5, 6, false, false, 2, 3),
    m(1, 0, 2, 0, 7, 8, false, false, 2, 2),
    m(1, 0, 0, 0, 7, 7, true, false, 4, 0),
    m(2, 6, 0, 0, 5, 5, true, false, 2, 0),
];

const BC7_SI2: [u16; 64] = [
    0xcccc, 0x8888, 0xeeee, 0xecc8, 0xc880, 0xfeec, 0xfec8, 0xec80, 0xc800, 0xffec, 0xfe80, 0xe800, 0xffe8, 0xff00, 0xfff0, 0xf000, 0xf710, 0x008e, 0x7100,
    0x08ce, 0x008c, 0x7310, 0x3100, 0x8cce, 0x088c, 0x3110, 0x6666, 0x366c, 0x17e8, 0x0ff0, 0x718e, 0x399c, 0xaaaa, 0xf0f0, 0x5a5a, 0x33cc, 0x3c3c, 0x55aa,
    0x9696, 0xa55a, 0x73ce, 0x13c8, 0x324c, 0x3bdc, 0x6996, 0xc33c, 0x9966, 0x0660, 0x0272, 0x04e4, 0x4e40, 0x2720, 0xc936, 0x936c, 0x39c6, 0x639c, 0x9336,
    0x9cc6, 0x817e, 0xe718, 0xccf0, 0x0fcc, 0x7744, 0xee22,
];

const BC7_SI3: [u32; 64] = [
    0xaa685050, 0x6a5a5040, 0x5a5a4200, 0x5450a0a8, 0xa5a50000, 0xa0a05050, 0x5555a0a0, 0x5a5a5050, 0xaa550000, 0xaa555500, 0xaaaa5500, 0x90909090, 0x94949494,
    0xa4a4a4a4, 0xa9a59450, 0x2a0a4250, 0xa5945040, 0x0a425054, 0xa5a5a500, 0x55a0a0a0, 0xa8a85454, 0x6a6a4040, 0xa4a45000, 0x1a1a0500, 0x0050a4a4, 0xaaa59090,
    0x14696914, 0x69691400, 0xa08585a0, 0xaa821414, 0x50a4a450, 0x6a5a0200, 0xa9a58000, 0x5090a0a8, 0xa8a09050, 0x24242424, 0x00aa5500, 0x24924924, 0x24499224,
    0x50a50a50, 0x500aa550, 0xaaaa4444, 0x66660000, 0xa5a0a5a0, 0x50a050a0, 0x69286928, 0x44aaaa44, 0x66666600, 0xaa444444, 0x54a854a8, 0x95809580, 0x96969600,
    0xa85454a8, 0x80959580, 0xaa141414, 0x96960000, 0xaaaa1414, 0xa05050a0, 0xa0a5a5a0, 0x96000000, 0x40804080, 0xa9a8a9a8, 0xaaaaaa44, 0x2a4a5254,
];

const BC7_AI0: [u8; 64] = [
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 2, 8, 2, 2, 8, 8, 15, 2, 8, 2, 2, 8, 8, 2, 2, 15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2,
    2, 15, 15, 6, 6, 2, 6, 8, 15, 15, 2, 2, 15, 15, 15, 15, 15, 2, 2, 15,
];

const BC7_AI1: [u8; 64] = [
    3, 3, 15, 15, 8, 3, 15, 15, 8, 8, 6, 6, 6, 5, 3, 3, 3, 3, 8, 15, 3, 3, 6, 10, 5, 8, 8, 6, 8, 5, 15, 15, 8, 15, 3, 5, 6, 10, 8, 15, 15, 3, 15, 5, 15, 15,
    15, 15, 3, 15, 5, 5, 5, 8, 5, 10, 5, 10, 8, 13, 15, 12, 3, 3,
];

const BC7_AI2: [u8; 64] = [
    15, 8, 8, 3, 15, 15, 3, 8, 15, 15, 15, 15, 15, 15, 15, 8, 15, 8, 15, 3, 15, 8, 15, 8, 3, 15, 6, 10, 15, 15, 10, 8, 15, 3, 15, 10, 10, 8, 9, 10, 6, 15, 8,
    15, 3, 6, 6, 8, 15, 3, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 3, 15, 15, 8,
];

const W2: [u8; 4] = [0, 21, 43, 64];
const W3: [u8; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const W4: [u8; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

fn weights(n: usize) -> &'static [u8] {
    match n {
        2 => &W2,
        3 => &W3,
        _ => &W4,
    }
}

fn subset(ns: usize, partition: usize, n: usize) -> usize {
    match ns {
        2 => usize::from(1 & (BC7_SI2[partition] >> n)),
        3 => (3 & (BC7_SI3[partition] >> (2 * n))) as usize,
        _ => 0,
    }
}

fn expand_quantized(v: u8, bits: usize) -> u8 {
    let v = u32::from(v << (8 - bits));
    (v | (v >> bits)) as u8
}

fn lerp(e: &[Rgba], s0: u8, s1: u8) -> Rgba {
    let (s0, s1) = (u32::from(s0), u32::from(s1));
    let (t0, t1) = (64 - s0, 64 - s1);
    let l = |a: u8, b: u8, t: u32, s: u32| ((t * u32::from(a) + s * u32::from(b) + 32) >> 6) as u8;
    [
        l(e[0][0], e[1][0], t0, s0),
        l(e[0][1], e[1][1], t0, s0),
        l(e[0][2], e[1][2], t0, s0),
        l(e[0][3], e[1][3], t1, s1),
    ]
}

fn decode_bc7_block(col: &mut [Rgba; 16], src: &[u8]) {
    let first = src[0];
    if first == 0 {
        col.fill([0, 0, 0, 255]);
        return;
    }
    let mode = first.trailing_zeros() as usize;
    let mut bit = mode + 1;
    let info = &BC7_MODES[mode];
    let (mut cb, mut ab) = (info.cb, info.ab);
    let cw = weights(info.ib);
    let aw = weights(if ab != 0 && info.ib2 != 0 { info.ib2 } else { info.ib });
    let mut load = |n: usize| {
        let v = get_bits(src, bit, n);
        bit += n;
        v
    };
    let partition = load(info.pb) as usize;
    let rotation = load(info.rb);
    let index_sel = load(info.isb);
    let numep = info.ns << 1;
    let mut ep = [[0u8; 4]; 6];
    for c in 0..3 {
        for e in ep.iter_mut().take(numep) {
            e[c] = load(cb);
        }
    }
    for e in ep.iter_mut().take(numep) {
        e[3] = if ab != 0 { load(ab) } else { 255 };
    }
    let assign = |e: &mut Rgba, val: u8, ab: usize| {
        for v in &mut e[..3] {
            *v = (*v << 1) | val;
        }
        if ab != 0 {
            e[3] = (e[3] << 1) | val;
        }
    };
    if info.epb {
        cb += 1;
        if ab != 0 {
            ab += 1;
        }
        for e in ep.iter_mut().take(numep) {
            let val = load(1);
            assign(e, val, ab);
        }
    }
    if info.spb {
        cb += 1;
        if ab != 0 {
            ab += 1;
        }
        for pair in ep[..numep].as_chunks_mut::<2>().0.iter_mut() {
            let val = load(1);
            for e in pair {
                assign(e, val, ab);
            }
        }
    }
    for e in ep.iter_mut().take(numep) {
        for v in &mut e[..3] {
            *v = expand_quantized(*v, cb);
        }
        if ab != 0 {
            e[3] = expand_quantized(e[3], ab);
        }
    }
    let mut cibit = bit;
    let mut aibit = cibit + 16 * info.ib - info.ns;
    for (i, out) in col.iter_mut().enumerate() {
        let s = subset(info.ns, partition, i) << 1;
        let mut ib = info.ib;
        if i == 0 {
            ib -= 1;
        } else if info.ns == 2 {
            if i == usize::from(BC7_AI0[partition]) {
                ib -= 1;
            }
        } else if info.ns == 3 && (i == usize::from(BC7_AI1[partition]) || i == usize::from(BC7_AI2[partition])) {
            ib -= 1;
        }
        let i0 = get_bits(src, cibit, ib) as usize;
        cibit += ib;
        let e = &ep[s..s + 2];
        let mut c = if ab != 0 && info.ib2 != 0 {
            let mut ib2 = info.ib2;
            if i == 0 {
                ib2 -= 1;
            }
            let i1 = get_bits(src, aibit, ib2) as usize;
            aibit += ib2;
            if index_sel != 0 { lerp(e, aw[i1], cw[i0]) } else { lerp(e, cw[i0], aw[i1]) }
        } else {
            lerp(e, cw[i0], cw[i0])
        };
        match rotation {
            1 => c.swap(0, 3),
            2 => c.swap(1, 3),
            3 => c.swap(2, 3),
            _ => {}
        }
        *out = c;
    }
}

/// Decode `width` x `height` (multiples of 4) of BC`n` blocks to RGBA rows in data order.
pub(crate) fn decode(data: &[u8], width: usize, height: usize, n: u8) -> Option<Vec<u8>> {
    let block_size = if n == 1 { 8 } else { 16 };
    let (bw, bh) = (width / 4, height / 4);
    if data.len() < bw * bh * block_size {
        return None;
    }
    let mut out = crate::zeroed(width * height * 4).ok()?;
    for by in 0..bh {
        for bx in 0..bw {
            let src = &data[(by * bw + bx) * block_size..][..block_size];
            let mut col = [[0u8; 4]; 16];
            match n {
                1 => decode_bc1_color(&mut col, src, false),
                3 => {
                    decode_bc1_color(&mut col, &src[8..], true);
                    decode_bc3_alpha(&mut col, src, 3);
                }
                7 => decode_bc7_block(&mut col, src),
                _ => return None,
            }
            for j in 0..4 {
                let row = (by * 4 + j) * width + bx * 4;
                for i in 0..4 {
                    out[(row + i) * 4..][..4].copy_from_slice(&col[j * 4 + i]);
                }
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bc1_uses_pillow_rounding() {
        // c0 = pure red (0xf800), c1 = black; indices 0,1,2,3 in the first row.
        let block = [0x00, 0xf8, 0x00, 0x00, 0b11_10_01_00, 0, 0, 0];
        let px = decode(&block, 4, 4, 1).unwrap();
        let reds: Vec<u8> = px.as_chunks::<4>().0.iter().take(4).map(|p| p[0]).collect();
        assert_eq!(reds, [255, 0, 170, 85]);
    }

    #[test]
    fn bc3_alpha_six_step_table() {
        let mut block = [0u8; 16];
        block[0] = 0; // a0 <= a1: 6-value table with 0 and 255
        block[1] = 255;
        block[2] = 0b10_111_110; // pixel 0 -> 6 (0), pixel 1 -> 7 (255), pixel 2 -> 2
        block[3] = 0b0000_0000;
        let px = decode(&block, 4, 4, 3).unwrap();
        let alphas: Vec<u8> = px.as_chunks::<4>().0.iter().take(3).map(|p| p[3]).collect();
        assert_eq!(alphas, [0, 255, 51]);
    }

    #[test]
    fn bc7_mode6_zero_endpoints() {
        // Mode 6, endpoints all zero except alpha max: every pixel (0, 0, 0, 255) at index 0.
        let mut block = [0u8; 16];
        block[0] = 0x40;
        let px = decode(&block, 4, 4, 7).unwrap();
        assert!(px.as_chunks::<4>().0.iter().all(|p| *p == [0, 0, 0, 0]));
    }
}
