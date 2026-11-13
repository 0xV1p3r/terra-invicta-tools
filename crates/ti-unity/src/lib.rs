//! Minimal reader for the Unity asset bundles of Terra Invicta: textures and sprites.
//!
//! Covers what the game's 2D image bundles use (Unity 2020.3): UnityFS archives with
//! LZ4/LZ4HC/LZMA blocks, serialized files with embedded type trees, `Texture2D` (RGBA32, RGB24,
//! Alpha8, DXT1, DXT5, BC7) with `.resS` stream data, and unpacked `Sprite`s.
//!
//! Images come out exactly as UnityPy + Pillow produce them (`Texture2D.image`, `Sprite.image`):
//! the block decoders follow Pillow's `BcnDecode.c` and sprite masks Pillow's polygon filler, so
//! exports match the Python extractor pixel for pixel.

mod bcn;
pub mod bundle;
pub mod image;
mod polygon;
mod read;
pub mod serialized;
pub mod sprite;
pub mod texture;
pub mod typetree;

pub use bundle::Bundle;
pub use image::{Image, Mode};
pub use serialized::{ObjectInfo, SerializedFile};
pub use sprite::Sprite;
pub use texture::Texture2D;
pub use typetree::Value;

/// Unity class ids used here.
pub mod class_id {
    pub const TEXTURE_2D: i32 = 28;
    pub const MONO_BEHAVIOUR: i32 = 114;
    pub const SPRITE: i32 = 213;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Format(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn format_err(msg: impl Into<String>) -> Error {
    Error::Format(msg.into())
}
