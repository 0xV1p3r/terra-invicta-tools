//! `Texture2D` objects and decoding them (UnityPy's `Texture2DConverter`).

use crate::image::{Image, Mode};
use crate::serialized::{ObjectInfo, SerializedFile};
use crate::typetree::Value;
use crate::{Error, Result, bcn, format_err};

/// Unity's `TextureFormat` values that are decoded here.
pub mod format {
    pub const ALPHA8: i64 = 1;
    pub const RGB24: i64 = 3;
    pub const RGBA32: i64 = 4;
    pub const ARGB32: i64 = 5;
    pub const DXT1: i64 = 10;
    pub const DXT5: i64 = 12;
    pub const BGRA32: i64 = 14;
    pub const BC5: i64 = 27;
    pub const BC7: i64 = 25;
}

#[derive(Debug, Clone)]
pub struct Texture2D {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: i64,
    /// Inline `image data`; empty when the pixels live in a stream file.
    pub image_data: Vec<u8>,
    /// `m_StreamData`: (path, offset, size).
    pub stream: Option<(String, u64, usize)>,
}

impl Texture2D {
    pub fn from_value(v: &Value) -> Result<Self> {
        let dim = |name| -> Result<u32> { u32::try_from(v.i64_field(name)?).map_err(|_| format_err(format!("bad {name}"))) };
        let stream = match v.get("m_StreamData") {
            Some(s) => {
                let path = s.str_field("path")?;
                let size = s.i64_field("size")?;
                (!path.is_empty() && size > 0)
                    .then(|| Ok::<_, Error>((path.to_owned(), s.i64_field("offset")? as u64, size as usize)))
                    .transpose()?
            }
            None => None,
        };
        let image_data = match v.field("image data")? {
            Value::Bytes(b) => b.clone(),
            _ => return Err(format_err("image data is not bytes")),
        };
        Ok(Self {
            name: v.str_field("m_Name")?.to_owned(),
            width: dim("m_Width")?,
            height: dim("m_Height")?,
            format: v.i64_field("m_TextureFormat")?,
            image_data,
            stream,
        })
    }

    pub fn read(sf: &SerializedFile, obj: &ObjectInfo) -> Result<Self> {
        Self::from_value(&sf.read(obj)?)
    }

    /// Bytes of the full-size image (the first mip level), if the format is known.
    pub fn mip0_size(&self) -> Option<usize> {
        use format::*;
        let (w, h) = (self.width as usize, self.height as usize);
        let blocks = w.div_ceil(4) * h.div_ceil(4);
        Some(match self.format {
            ALPHA8 => w * h,
            RGB24 => w * h * 3,
            RGBA32 | ARGB32 | BGRA32 => w * h * 4,
            DXT1 => blocks * 8,
            DXT5 | BC5 | BC7 => blocks * 16,
            _ => return None,
        })
    }

    /// The encoded pixel data of the first mip level (or all of it for unknown formats).
    pub fn data(&self, sf: &SerializedFile) -> Result<Vec<u8>> {
        match &self.stream {
            Some((path, offset, size)) => sf.stream_data(path, *offset, self.mip0_size().map_or(*size, |n| n.min(*size))),
            None => Ok(self.image_data.clone()),
        }
    }

    /// Decoded pixels in Unity's row order (bottom row first): `get_image_from_texture2d(flip=False)`.
    pub fn decode_unflipped(&self, sf: &SerializedFile) -> Result<Image> {
        if self.width == 0 || self.height == 0 {
            return Ok(Image::new(0, 0, Mode::Rgba));
        }
        let data = self.data(sf)?;
        if data.is_empty() {
            return Err(format_err(format!("{}: Texture2D has no image data", self.name)));
        }
        decode(&data, self.width, self.height, self.format).map_err(|e| format_err(format!("{}: {e}", self.name)))
    }

    /// The image as UnityPy's `Texture2D.image` returns it (top row first).
    pub fn image(&self, sf: &SerializedFile) -> Result<Image> {
        let mut im = self.decode_unflipped(sf)?;
        im.flip_vertical();
        Ok(im)
    }
}

fn raw(data: &[u8], width: u32, height: u32, bpp: usize, mode: Mode, convert: impl Fn(&[u8]) -> [u8; 4]) -> Result<Image> {
    let n = width as usize * height as usize;
    let src = data.get(..n * bpp).ok_or_else(|| format_err("not enough image data"))?;
    let c = mode.channels();
    let mut out = Image::new(width, height, mode);
    for (dst, px) in out.data.chunks_exact_mut(c).zip(src.chunks_exact(bpp)) {
        dst.copy_from_slice(&convert(px)[..c]);
    }
    Ok(out)
}

/// Decode `width` x `height` pixels of `format` (in data order).
pub fn decode(data: &[u8], width: u32, height: u32, format: i64) -> Result<Image> {
    use format::*;
    match format {
        ALPHA8 => raw(data, width, height, 1, Mode::Rgba, |p| [0, 0, 0, p[0]]),
        RGB24 => raw(data, width, height, 3, Mode::Rgb, |p| [p[0], p[1], p[2], 255]),
        RGBA32 => raw(data, width, height, 4, Mode::Rgba, |p| [p[0], p[1], p[2], p[3]]),
        ARGB32 => raw(data, width, height, 4, Mode::Rgba, |p| [p[1], p[2], p[3], p[0]]),
        BGRA32 => raw(data, width, height, 4, Mode::Rgba, |p| [p[2], p[1], p[0], p[3]]),
        DXT1 | DXT5 | BC7 | BC5 => {
            let (pw, ph) = (width.div_ceil(4) * 4, height.div_ceil(4) * 4);
            let n = match format {
                DXT1 => 1,
                DXT5 => 3,
                BC5 => 5,
                _ => 7,
            };
            let px = if n == 5 {
                bc5(data, pw as usize, ph as usize)
            } else {
                bcn::decode(data, pw as usize, ph as usize, n)
            };
            let px = px.ok_or_else(|| format_err("not enough image data"))?;
            let mode = if n == 5 { Mode::Rgb } else { Mode::Rgba };
            let mut im = Image {
                width: pw,
                height: ph,
                mode: Mode::Rgba,
                data: px,
            };
            if (pw, ph) != (width, height) {
                im = im.crop(0, 0, width.into(), height.into());
            }
            if mode == Mode::Rgb {
                im = Image {
                    width: im.width,
                    height: im.height,
                    mode,
                    data: im.data.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect(),
                };
            }
            Ok(im)
        }
        f => Err(Error::Unsupported(format!("texture format {f}"))),
    }
}

/// BC5 as Pillow decodes it into an RGB image: red and green from the two alpha-style blocks.
fn bc5(data: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let (bw, bh) = (width / 4, height / 4);
    if data.len() < bw * bh * 16 {
        return None;
    }
    // Reuse the BC3 alpha decoder through two DXT5-shaped blocks: alpha lands in channel 3.
    let mut out = vec![0u8; width * height * 4];
    for (k, ch) in [(0usize, 0usize), (8, 1)] {
        let mut blocks = Vec::with_capacity(bw * bh * 16);
        for b in data[..bw * bh * 16].as_chunks::<16>().0.iter() {
            blocks.extend_from_slice(&b[k..k + 8]);
            blocks.extend_from_slice(&[0; 8]);
        }
        let px = bcn::decode(&blocks, width, height, 3)?;
        for (o, p) in out.as_chunks_mut::<4>().0.iter_mut().zip(px.as_chunks::<4>().0.iter()) {
            o[ch] = p[3];
        }
    }
    for o in out.as_chunks_mut::<4>().0.iter_mut() {
        o[3] = 255;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha8_is_black_with_alpha() {
        let im = decode(&[0x10, 0x80], 2, 1, format::ALPHA8).unwrap();
        assert_eq!(im.data, [0, 0, 0, 0x10, 0, 0, 0, 0x80]);
    }

    #[test]
    fn block_formats_crop_padding() {
        let im = decode(&[0; 8 * 4], 5, 6, format::DXT1).unwrap();
        assert_eq!((im.width, im.height, im.data.len()), (5, 6, 5 * 6 * 4));
    }
}
