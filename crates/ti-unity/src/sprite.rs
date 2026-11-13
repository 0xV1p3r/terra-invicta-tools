//! `Sprite` objects and cutting them out of their texture (UnityPy's `SpriteHelper`).

use crate::image::{Image, Mode};
use crate::polygon::Mask;
use crate::serialized::{ObjectInfo, SerializedFile};
use crate::texture::Texture2D;
use crate::typetree::Value;
use crate::{Error, Result, format_err};

const SPRITE_ATLAS: i32 = 687078895;
/// Vertex streams of a mesh (Unity has at most 4).
const MAX_STREAMS: i64 = 4;
/// Mesh coordinates in pixels, far past any texture yet safe for the polygon filler's `i32` maths.
const MAX_MESH_COORD: f64 = (1 << 24) as f64;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    fn from_value(v: &Value) -> Result<Self> {
        Ok(Self {
            x: v.f64_field("x")?,
            y: v.f64_field("y")?,
            width: v.f64_field("width")?,
            height: v.f64_field("height")?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Sprite {
    pub name: String,
    pub pixels_to_units: f64,
    /// `m_RD.texture` path id (same file).
    pub texture: i64,
    /// `m_RD.alphaTexture` path id, 0 if none.
    pub alpha_texture: i64,
    pub texture_rect: Rect,
    pub settings_raw: u32,
    /// Mesh vertex positions (x, y).
    pub vertices: Vec<(f64, f64)>,
    /// Whether the mesh has a non-zero UV0 channel (UnityPy then renders the mesh instead of masking).
    pub has_uv: bool,
    pub triangles: Vec<[u16; 3]>,
}

fn component_size(format: i64) -> Result<usize> {
    // VertexFormat (Unity 2019+).
    Ok(match format {
        0 | 10 | 11 => 4,
        1 | 4 | 5 | 8 | 9 => 2,
        2 | 3 | 6 | 7 => 1,
        f => return Err(format_err(format!("vertex format {f}"))),
    })
}

struct Channel {
    stream: i64,
    offset: usize,
    format: i64,
    dimension: usize,
}

/// Read `chn` of the vertex data as f32 tuples (`get_streams` + `read_vertex_data` of UnityPy's `MeshHandler`).
fn channel_floats(vertex_data: &Value, chn: usize) -> Result<Option<Vec<Vec<f32>>>> {
    let bad = |what: &str| format_err(format!("vertex data: bad {what}"));
    let count = usize::try_from(vertex_data.i64_field("m_VertexCount")?).map_err(|_| bad("vertex count"))?;
    let data = vertex_data
        .field("m_DataSize")?
        .as_bytes()
        .ok_or_else(|| format_err("m_DataSize is not bytes"))?;
    if count == 0 || data.is_empty() {
        return Ok(None);
    }
    let channels = vertex_data
        .array_field("m_Channels")?
        .iter()
        .map(|c| {
            let channel = Channel {
                stream: c.i64_field("stream")?,
                offset: usize::try_from(c.i64_field("offset")?).map_err(|_| bad("channel offset"))?,
                format: c.i64_field("format")?,
                dimension: usize::try_from(c.i64_field("dimension")?).map_err(|_| bad("channel dimension"))?,
            };
            if !(0..MAX_STREAMS).contains(&channel.stream) {
                return Err(bad("channel stream"));
            }
            Ok(channel)
        })
        .collect::<Result<Vec<_>>>()?;
    let Some(ch) = channels.get(chn).filter(|c| c.dimension > 0) else {
        return Ok(None);
    };
    let stream_count = 1 + channels.iter().map(|c| c.stream).max().unwrap_or(0);
    let (mut offset, mut stream_offset, mut stride) = (0usize, 0, 0);
    for s in 0..stream_count {
        let mut st = 0;
        for c in channels.iter().filter(|c| c.stream == s && c.dimension > 0) {
            st += (c.dimension & 0xf) * component_size(c.format)?;
        }
        if s == ch.stream {
            (stream_offset, stride) = (offset, st);
        }
        // Every vertex takes `st` bytes of the data, so the streams fit in it.
        offset = count.checked_mul(st).and_then(|n| n.checked_add(offset)).ok_or_else(|| bad("vertex count"))?;
        if offset > data.len() {
            return Err(format_err("vertex data too short"));
        }
        offset = offset.div_ceil(16) * 16;
    }
    if ch.format != 0 {
        return Err(Error::Unsupported(format!("vertex channel {chn} format {}", ch.format)));
    }
    let dim = ch.dimension & 0xf;
    let mut out = Vec::with_capacity(count);
    for v in 0..count {
        // `stream_offset + v * stride` is within the data, checked above.
        let at = (stream_offset + v * stride).checked_add(ch.offset);
        let bytes = at
            .and_then(|at| data.get(at..)?.get(..dim * 4))
            .ok_or_else(|| format_err("vertex data too short"))?;
        out.push(bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect());
    }
    Ok(Some(out))
}

/// The (x, y) of the position channel.
fn positions(vertex_data: &Value) -> Result<Vec<(f64, f64)>> {
    channel_floats(vertex_data, 0)?
        .unwrap_or_default()
        .iter()
        .map(|p| match p[..] {
            [x, y, ..] => Ok((f64::from(x), f64::from(y))),
            _ => Err(format_err("vertex positions with fewer than 2 components")),
        })
        .collect()
}

impl Sprite {
    pub fn from_value(v: &Value, sf: &SerializedFile) -> Result<Self> {
        let (atlas_file, atlas) = v.pptr_field("m_SpriteAtlas")?;
        let tags = v.get("m_AtlasTags").and_then(Value::as_array).is_some_and(|t| !t.is_empty());
        if atlas != 0 || atlas_file != 0 || (tags && sf.objects.iter().any(|o| o.class_id == SPRITE_ATLAS)) {
            return Err(Error::Unsupported("sprite atlases".into()));
        }
        let rd = v.field("m_RD")?;
        let (tex_file, texture) = rd.pptr_field("texture")?;
        let (alpha_file, alpha_texture) = rd.pptr_field("alphaTexture")?;
        if tex_file != 0 || (alpha_texture != 0 && alpha_file != 0) {
            return Err(Error::Unsupported("sprite texture in another file".into()));
        }
        let vertex_data = rd.field("m_VertexData")?;
        let vertices = positions(vertex_data)?;
        let has_uv = channel_floats(vertex_data, 4)?.is_some_and(|uv| uv.iter().any(|p| p.iter().take(2).any(|&c| c != 0.0)));
        let index = rd.field("m_IndexBuffer")?.as_bytes().ok_or_else(|| format_err("m_IndexBuffer is not bytes"))?;
        let indices: Vec<u16> = index.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        let mut triangles = Vec::new();
        for sub in rd.array_field("m_SubMeshes")? {
            let short = || format_err("index buffer too short");
            let first = usize::try_from(sub.i64_field("firstByte")?).map_err(|_| short())? / 2;
            let count = usize::try_from(sub.i64_field("indexCount")?).map_err(|_| short())?;
            if sub.i64_field("topology")? != 0 {
                return Err(Error::Unsupported("sprite mesh topology".into()));
            }
            let idx = first.checked_add(count).and_then(|end| indices.get(first..end)).ok_or_else(short)?;
            triangles.extend(idx.chunks(3).filter(|t| t.len() == 3).map(|t| [t[0], t[1], t[2]]));
        }
        Ok(Self {
            name: v.str_field("m_Name")?.to_owned(),
            pixels_to_units: v.f64_field("m_PixelsToUnits")?,
            texture,
            alpha_texture,
            texture_rect: Rect::from_value(rd.field("textureRect")?)?,
            settings_raw: rd.i64_field("settingsRaw")? as u32,
            vertices,
            has_uv,
            triangles,
        })
    }

    /// The objects of the texture and the alpha texture (if any).
    pub fn texture_objects<'f>(&self, sf: &'f SerializedFile) -> Result<(&'f ObjectInfo, Option<&'f ObjectInfo>)> {
        let obj = |id: i64| sf.object(id).ok_or_else(|| format_err(format!("{}: texture {id} not found", self.name)));
        Ok((obj(self.texture)?, (self.alpha_texture != 0).then(|| obj(self.alpha_texture)).transpose()?))
    }

    /// The source image of `get_image`: the texture unflipped, with the alpha texture's first
    /// channel as alpha when there is one.
    pub fn source_image(&self, sf: &SerializedFile) -> Result<Image> {
        let (tex, alpha) = self.texture_objects(sf)?;
        let alpha = alpha.map(|a| Texture2D::read(sf, a)).transpose()?;
        self.source_from(sf, &Texture2D::read(sf, tex)?, alpha.as_ref())
    }

    /// [`Sprite::source_image`] from textures already read (see [`Sprite::texture_objects`]).
    pub fn source_from(&self, sf: &SerializedFile, texture: &Texture2D, alpha: Option<&Texture2D>) -> Result<Image> {
        let im = texture.decode_unflipped(sf)?;
        let Some(alpha) = alpha else { return Ok(im) };
        let alpha = alpha.decode_unflipped(sf)?;
        if (alpha.width, alpha.height) != (im.width, im.height) {
            return Err(format_err(format!("{}: alpha texture size differs", self.name)));
        }
        let rgba = im.to_rgba();
        let c = alpha.mode.channels();
        let data = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(alpha.data.chunks_exact(c))
            .flat_map(|(p, a)| [p[0], p[1], p[2], a[0]])
            .collect();
        Ok(Image {
            width: im.width,
            height: im.height,
            mode: Mode::Rgba,
            data,
        })
    }

    /// The box `image` cuts from the source image.
    pub fn crop_box(&self) -> Result<(i64, i64, i64, i64)> {
        let r = self.texture_rect;
        let edges = [r.x, r.y, r.x + r.width, r.y + r.height];
        if edges.iter().any(|v| !v.is_finite() || v.abs() > MAX_MESH_COORD) {
            return Err(format_err(format!("{}: bad texture rect {r:?}", self.name)));
        }
        let [x0, y0, x1, y1] = edges.map(|v| v.round_ties_even() as i64);
        Image::crop_pixels(x0, y0, x1, y1)?;
        Ok((x0, y0, x1, y1))
    }

    /// `Sprite.image`, cut from `source` (from [`Sprite::source_image`]; clone it to share it
    /// between the sprites of one texture).
    pub fn image(&self, source: Image) -> Result<Image> {
        let (x0, y0, x1, y1) = self.crop_box()?;
        let mut im = source.into_crop(x0, y0, x1, y1)?;
        let packed = self.settings_raw & 1 != 0;
        let packing_mode = (self.settings_raw >> 1) & 1;
        if packed {
            match (self.settings_raw >> 2) & 0xf {
                1 => im.flip_horizontal(),
                2 => im.flip_vertical(),
                3 => {
                    im.flip_horizontal();
                    im.flip_vertical();
                }
                4 => im = im.rotate_270()?,
                _ => {}
            }
        }
        if packing_mode == 0 {
            // Tight packing: keep only the mesh polygon.
            if self.has_uv {
                return Err(Error::Unsupported(format!("{}: tight sprite with UVs (mesh rendering)", self.name)));
            }
            if self.vertices.is_empty() {
                return Err(format_err(format!("{}: no vertices found in sprite mesh", self.name)));
            }
            im = self.mask(im)?;
        }
        im.flip_vertical();
        Ok(im)
    }

    /// UnityPy's `mask_sprite`.
    fn mask(&self, im: Image) -> Result<Image> {
        let min_x = self.vertices.iter().map(|v| v.0).fold(f64::INFINITY, f64::min);
        let min_y = self.vertices.iter().map(|v| v.1).fold(f64::INFINITY, f64::min);
        let f = self.pixels_to_units;
        let pos: Vec<(f64, f64)> = self.vertices.iter().map(|&(x, y)| ((x - min_x) * f, (y - min_y) * f)).collect();
        if pos.iter().any(|&(x, y)| !(x.abs() <= MAX_MESH_COORD && y.abs() <= MAX_MESH_COORD)) {
            return Err(format_err(format!("{}: sprite mesh out of range", self.name)));
        }
        let mut mask = Mask::new(im.width, im.height)?;
        for t in &self.triangles {
            let tri: Option<Vec<(f64, f64)>> = t.iter().map(|&i| pos.get(usize::from(i)).copied()).collect();
            if let Some(tri) = tri {
                mask.fill_polygon(&tri);
            }
        }
        match im.mode {
            // Image.composite(image, empty, mask)
            Mode::Rgba => {
                let mut im = im;
                for (p, &m) in im.data.as_chunks_mut::<4>().0.iter_mut().zip(&mask.data) {
                    if m == 0 {
                        p.fill(0);
                    }
                }
                Ok(im)
            }
            // putalpha(mask)
            Mode::Rgb => {
                let data = im
                    .data
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .zip(&mask.data)
                    .flat_map(|(p, &m)| [p[0], p[1], p[2], if m != 0 { 255 } else { 0 }])
                    .collect();
                Ok(Image {
                    width: im.width,
                    height: im.height,
                    mode: Mode::Rgba,
                    data,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(fields: &[(&str, Value)]) -> Value {
        Value::Object(fields.iter().map(|(n, v)| ((*n).to_owned(), v.clone())).collect())
    }

    fn vertex_data(count: i64, channels: &[[i64; 4]], data: Vec<u8>) -> Value {
        let channels = channels
            .iter()
            .map(|&[stream, offset, format, dimension]| {
                obj(&[
                    ("stream", Value::Int(stream)),
                    ("offset", Value::Int(offset)),
                    ("format", Value::Int(format)),
                    ("dimension", Value::Int(dimension)),
                ])
            })
            .collect();
        obj(&[
            ("m_VertexCount", Value::Int(count)),
            ("m_Channels", Value::Array(channels)),
            ("m_DataSize", Value::Bytes(data)),
        ])
    }

    #[test]
    fn reads_positions() {
        let data: Vec<u8> = [1.0f32, 2.0, 0.0, 3.0, 4.0, 0.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        let vd = vertex_data(2, &[[0, 0, 0, 3]], data);
        assert_eq!(positions(&vd).unwrap(), [(1.0, 2.0), (3.0, 4.0)]);
    }

    #[test]
    fn malformed_vertex_data_is_an_error() {
        let data = vec![0; 64];
        // A one-component position channel.
        assert!(positions(&vertex_data(2, &[[0, 0, 0, 1]], data.clone())).is_err());
        // Counts, offsets and streams that do not fit the data.
        assert!(positions(&vertex_data(-1, &[[0, 0, 0, 3]], data.clone())).is_err());
        assert!(positions(&vertex_data(i64::MAX, &[[0, 0, 0, 3]], data.clone())).is_err());
        assert!(positions(&vertex_data(1 << 40, &[[0, 0, 0, 3]], data.clone())).is_err());
        assert!(positions(&vertex_data(2, &[[0, i64::MAX, 0, 3]], data.clone())).is_err());
        assert!(positions(&vertex_data(2, &[[0, -4, 0, 3]], data.clone())).is_err());
        assert!(positions(&vertex_data(2, &[[i64::MAX, 0, 0, 3]], data.clone())).is_err());
        assert!(positions(&vertex_data(2, &[[-1, 0, 0, 3]], data.clone())).is_err());
        assert!(positions(&vertex_data(2, &[[0, 0, 0, -3]], data)).is_err());
    }

    fn sprite(rect: Rect, vertices: Vec<(f64, f64)>) -> Sprite {
        Sprite {
            name: "test".into(),
            pixels_to_units: 1.0,
            texture: 1,
            alpha_texture: 0,
            texture_rect: rect,
            settings_raw: 0,
            vertices,
            has_uv: false,
            triangles: vec![[0, 1, 2]],
        }
    }

    #[test]
    fn bad_rects_and_meshes_are_errors() {
        let source = || Image::new(4, 4, Mode::Rgba).unwrap();
        let tri = vec![(0.0, 0.0), (4.0, 0.0), (0.0, 4.0)];
        let rect = |x, y, width, height| Rect { x, y, width, height };
        assert_eq!(sprite(rect(0.0, 0.0, 4.0, 4.0), tri.clone()).image(source()).unwrap().data.len(), 64);
        for r in [
            rect(f64::NAN, 0.0, 4.0, 4.0),
            rect(0.0, 0.0, f64::INFINITY, 4.0),
            rect(-1e300, 0.0, 2e300, 4.0),
            rect(0.0, 0.0, 1e6, 1e6),
        ] {
            assert!(sprite(r, tri.clone()).image(source()).is_err(), "{r:?}");
        }
        let far = vec![(0.0, 0.0), (1e30, 0.0), (0.0, f64::NAN)];
        assert!(sprite(rect(0.0, 0.0, 4.0, 4.0), far).image(source()).is_err());
    }
}
