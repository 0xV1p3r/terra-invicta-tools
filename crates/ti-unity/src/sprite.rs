//! `Sprite` objects and cutting them out of their texture (UnityPy's `SpriteHelper`).

use crate::image::{Image, Mode};
use crate::polygon::Mask;
use crate::serialized::SerializedFile;
use crate::texture::Texture2D;
use crate::typetree::Value;
use crate::{Error, Result, format_err};

const SPRITE_ATLAS: i32 = 687078895;

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
    let count = vertex_data.i64_field("m_VertexCount")? as usize;
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
            Ok(Channel {
                stream: c.i64_field("stream")?,
                offset: c.i64_field("offset")? as usize,
                format: c.i64_field("format")?,
                dimension: c.i64_field("dimension")? as usize,
            })
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
        offset += count * st;
        offset = offset.div_ceil(16) * 16;
    }
    if ch.format != 0 {
        return Err(Error::Unsupported(format!("vertex channel {chn} format {}", ch.format)));
    }
    let dim = ch.dimension & 0xf;
    let mut out = Vec::with_capacity(count);
    for v in 0..count {
        let at = stream_offset + v * stride + ch.offset;
        let bytes = data.get(at..at + dim * 4).ok_or_else(|| format_err("vertex data too short"))?;
        out.push(bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect());
    }
    Ok(Some(out))
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
        let vertices = channel_floats(vertex_data, 0)?
            .unwrap_or_default()
            .iter()
            .map(|p| (f64::from(p[0]), f64::from(p[1])))
            .collect();
        let has_uv = channel_floats(vertex_data, 4)?.is_some_and(|uv| uv.iter().any(|p| p.iter().take(2).any(|&c| c != 0.0)));
        let index = rd.field("m_IndexBuffer")?.as_bytes().ok_or_else(|| format_err("m_IndexBuffer is not bytes"))?;
        let indices: Vec<u16> = index.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        let mut triangles = Vec::new();
        for sub in rd.array_field("m_SubMeshes")? {
            let first = sub.i64_field("firstByte")? as usize / 2;
            let count = sub.i64_field("indexCount")? as usize;
            if sub.i64_field("topology")? != 0 {
                return Err(Error::Unsupported("sprite mesh topology".into()));
            }
            let idx = indices.get(first..first + count).ok_or_else(|| format_err("index buffer too short"))?;
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

    /// The source image of `get_image`: the texture unflipped, with the alpha texture's first
    /// channel as alpha when there is one.
    pub fn source_image(&self, sf: &SerializedFile) -> Result<Image> {
        let tex = |id: i64| -> Result<Image> {
            let obj = sf.object(id).ok_or_else(|| format_err(format!("{}: texture {id} not found", self.name)))?;
            Texture2D::read(sf, obj)?.decode_unflipped(sf)
        };
        let im = tex(self.texture)?;
        if self.alpha_texture == 0 {
            return Ok(im);
        }
        let alpha = tex(self.alpha_texture)?;
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

    /// `Sprite.image`, cut from `source` (from [`Sprite::source_image`]; clone it to share it
    /// between the sprites of one texture).
    pub fn image(&self, source: Image) -> Result<Image> {
        let r = self.texture_rect;
        let round = |v: f64| v.round_ties_even() as i64;
        let mut im = source.into_crop(round(r.x), round(r.y), round(r.x + r.width), round(r.y + r.height));
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
                4 => im = im.rotate_270(),
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
            im = self.mask(im);
        }
        im.flip_vertical();
        Ok(im)
    }

    /// UnityPy's `mask_sprite`.
    fn mask(&self, im: Image) -> Image {
        let min_x = self.vertices.iter().map(|v| v.0).fold(f64::INFINITY, f64::min);
        let min_y = self.vertices.iter().map(|v| v.1).fold(f64::INFINITY, f64::min);
        let f = self.pixels_to_units;
        let pos: Vec<(f64, f64)> = self.vertices.iter().map(|&(x, y)| ((x - min_x) * f, (y - min_y) * f)).collect();
        let mut mask = Mask::new(im.width, im.height);
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
                im
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
                Image {
                    width: im.width,
                    height: im.height,
                    mode: Mode::Rgba,
                    data,
                }
            }
        }
    }
}
