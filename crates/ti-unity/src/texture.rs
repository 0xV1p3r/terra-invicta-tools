//! `Texture2D` objects and decoding them (UnityPy's `Texture2DConverter`).

use std::borrow::Cow;

use crate::image::{Image, Mode, check_size};
use crate::read::Reader;
use crate::serialized::{ObjectInfo, SerializedFile};
use crate::typetree::Value;
use crate::{Error, Result, bcn, format_err, zeroed};

/// Bytes read for a [`TextureHeader`]: the fields before the pixel data take a few dozen, but
/// may take more with a long name or platform data.
const HEADER_BYTES: [usize; 2] = [1 << 10, 64 << 10];

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

/// What decoding a `Texture2D` takes, read without its pixel data.
#[derive(Debug, Clone, Copy)]
pub struct TextureHeader {
    pub width: u32,
    pub height: u32,
    pub format: i64,
    /// Bytes of the serialized object (which hold inline pixel data).
    pub object_size: u32,
}

impl TextureHeader {
    /// Peak bytes of reading and decoding the texture with [`Texture2D::read`] and
    /// [`Texture2D::decode_unflipped`]: the object and its parsed copy of the inline data, or the
    /// stream data, then the decoded pixels (padded to whole blocks) and their cropped or
    /// converted copy.
    pub fn decode_bytes(&self) -> u64 {
        use format::*;
        let (w, h) = (u64::from(self.width), u64::from(self.height));
        let object = u64::from(self.object_size);
        let encoded = mip0_size(self.width, self.height, self.format).map_or(object, |n| n as u64);
        // An object smaller than the image has its pixels in a stream file.
        let read = if object >= encoded { 2 * object } else { object + encoded };
        let decoded = match self.format {
            DXT1 | DXT5 | BC5 | BC7 => {
                let (pw, ph) = (w.div_ceil(4) * 4, h.div_ceil(4) * 4);
                let copy = (pw, ph) != (w, h) || self.format == BC5;
                pw * ph * 4 + if copy { w * h * 4 } else { 0 }
            }
            RGB24 => w * h * 3,
            _ => w * h * 4,
        };
        read + decoded
    }

    /// Bytes of the decoded image.
    pub fn image_bytes(&self) -> u64 {
        let channels = if matches!(self.format, format::RGB24 | format::BC5) { 3 } else { 4 };
        u64::from(self.width) * u64::from(self.height) * channels
    }
}

/// Bytes of the full-size image (the first mip level) of `format`, if it is known.
fn mip0_size(width: u32, height: u32, format: i64) -> Option<usize> {
    use format::*;
    let (w, h) = (width as usize, height as usize);
    let blocks = w.div_ceil(4).checked_mul(h.div_ceil(4))?;
    let pixels = w.checked_mul(h)?;
    match format {
        ALPHA8 => Some(pixels),
        RGB24 => pixels.checked_mul(3),
        RGBA32 | ARGB32 | BGRA32 => pixels.checked_mul(4),
        DXT1 => blocks.checked_mul(8),
        DXT5 | BC5 | BC7 => blocks.checked_mul(16),
        _ => None,
    }
}

impl Texture2D {
    pub fn from_value(v: &Value) -> Result<Self> {
        let image_data = match v.field("image data")? {
            Value::Bytes(b) => b.clone(),
            _ => return Err(format_err("image data is not bytes")),
        };
        Self::with_data(v, image_data)
    }

    /// [`Texture2D::from_value`] that moves the pixel data out of `v` instead of copying it.
    pub fn from_owned(mut v: Value) -> Result<Self> {
        let image_data = match v.take("image data") {
            Some(Value::Bytes(b)) => b,
            Some(_) => return Err(format_err("image data is not bytes")),
            None => return Err(format_err("missing field image data")),
        };
        Self::with_data(&v, image_data)
    }

    fn with_data(v: &Value, image_data: Vec<u8>) -> Result<Self> {
        let stream = match v.get("m_StreamData") {
            Some(s) => {
                let path = s.str_field("path")?;
                let size = s.i64_field("size")?;
                let offset = u64::try_from(s.i64_field("offset")?).map_err(|_| format_err("bad stream offset"))?;
                (!path.is_empty() && size > 0)
                    .then(|| usize::try_from(size).map(|size| (path.to_owned(), offset, size)))
                    .transpose()
                    .map_err(|_| format_err("bad stream size"))?
            }
            None => None,
        };
        let (width, height, format) = dims(v)?;
        Ok(Self {
            name: v.str_field("m_Name")?.to_owned(),
            width,
            height,
            format,
            image_data,
            stream,
        })
    }

    pub fn read(sf: &SerializedFile, obj: &ObjectInfo) -> Result<Self> {
        Self::from_owned(sf.read(obj)?)
    }

    /// The size and format of a texture, from the start of its object only.
    pub fn read_header(sf: &SerializedFile, obj: &ObjectInfo) -> Result<TextureHeader> {
        let mut v = Err(format_err("no header"));
        for n in HEADER_BYTES {
            let head = sf.bundle.read(sf.node, obj.offset, (obj.size as usize).min(n))?;
            v = sf.tree(obj).read_until(&mut Reader::new(&head, sf.big_endian), "image data");
            if v.is_ok() || head.len() < n {
                break;
            }
        }
        let v = v.map_err(|e| format_err(format!("object {}: {e}", obj.path_id)))?;
        let (width, height, format) = dims(&v)?;
        Ok(TextureHeader {
            width,
            height,
            format,
            object_size: obj.size,
        })
    }

    /// Bytes of the full-size image (the first mip level), if the format is known.
    pub fn mip0_size(&self) -> Option<usize> {
        mip0_size(self.width, self.height, self.format)
    }

    /// The encoded pixel data of the first mip level (or all of it for unknown formats).
    pub fn data(&self, sf: &SerializedFile) -> Result<Cow<'_, [u8]>> {
        match &self.stream {
            Some((path, offset, size)) => Ok(Cow::Owned(sf.stream_data(path, *offset, self.mip0_size().map_or(*size, |n| n.min(*size)))?)),
            None => Ok(Cow::Borrowed(&self.image_data)),
        }
    }

    /// Decoded pixels in Unity's row order (bottom row first): `get_image_from_texture2d(flip=False)`.
    pub fn decode_unflipped(&self, sf: &SerializedFile) -> Result<Image> {
        if self.width == 0 || self.height == 0 {
            return Image::new(0, 0, Mode::Rgba);
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

fn dims(v: &Value) -> Result<(u32, u32, i64)> {
    let dim = |name| -> Result<u32> { u32::try_from(v.i64_field(name)?).map_err(|_| format_err(format!("bad {name}"))) };
    Ok((dim("m_Width")?, dim("m_Height")?, v.i64_field("m_TextureFormat")?))
}

fn raw(data: &[u8], width: u32, height: u32, bpp: usize, mode: Mode, convert: impl Fn(&[u8]) -> [u8; 4]) -> Result<Image> {
    check_size(width.into(), height.into())?;
    let n = width as usize * height as usize;
    let src = data.get(..n * bpp).ok_or_else(|| format_err("not enough image data"))?;
    let c = mode.channels();
    let mut out = crate::with_capacity(n * c)?;
    for px in src.chunks_exact(bpp) {
        out.extend_from_slice(&convert(px)[..c]);
    }
    Ok(Image {
        width,
        height,
        mode,
        data: out,
    })
}

/// Decode `width` x `height` pixels of `format` (in data order).
pub fn decode(data: &[u8], width: u32, height: u32, format: i64) -> Result<Image> {
    use format::*;
    check_size(width.into(), height.into())?;
    match format {
        ALPHA8 => raw(data, width, height, 1, Mode::Rgba, |p| [0, 0, 0, p[0]]),
        RGB24 => raw(data, width, height, 3, Mode::Rgb, |p| [p[0], p[1], p[2], 255]),
        RGBA32 => raw(data, width, height, 4, Mode::Rgba, |p| [p[0], p[1], p[2], p[3]]),
        ARGB32 => raw(data, width, height, 4, Mode::Rgba, |p| [p[1], p[2], p[3], p[0]]),
        BGRA32 => raw(data, width, height, 4, Mode::Rgba, |p| [p[2], p[1], p[0], p[3]]),
        DXT1 | DXT5 | BC7 | BC5 => {
            let (pw, ph) = (width.div_ceil(4) * 4, height.div_ceil(4) * 4);
            check_size(pw.into(), ph.into())?;
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
                im = im.crop(0, 0, width.into(), height.into())?;
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
    let mut out = zeroed(width * height * 4).ok()?;
    for (k, ch) in [(0usize, 0usize), (8, 1)] {
        let mut blocks = crate::with_capacity(bw * bh * 16).ok()?;
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

    #[test]
    fn oversized_textures_are_errors() {
        assert!(decode(&[], u32::MAX, u32::MAX, format::RGBA32).is_err());
        assert!(decode(&[], 1 << 15, 1 << 15, format::DXT1).is_err());
        assert!(decode(&[], u32::MAX - 1, 1, format::BC7).is_err());
        assert_eq!(mip0_size(u32::MAX, u32::MAX, format::RGBA32), None);
        assert!(decode(&[0; 4], 2, 2, format::RGBA32).is_err());
    }
}
