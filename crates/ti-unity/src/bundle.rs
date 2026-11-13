//! UnityFS archives. Blocks are decompressed on demand, so a 1.6 GB bundle is never held in memory.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::read::Reader;
use crate::{Error, Result, format_err};

/// A file stored in the archive (a serialized file, or `.resS`/`.resource` stream data).
#[derive(Debug, Clone)]
pub struct Node {
    pub path: String,
    /// Offset in the concatenated uncompressed block data.
    pub offset: u64,
    pub size: u64,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy)]
struct Block {
    /// Start in the uncompressed stream.
    offset: u64,
    uncompressed: u32,
    /// Start in the bundle file.
    file_offset: u64,
    compressed: u32,
    flags: u16,
}

const BLOCK_CACHE: usize = 32;

pub struct Bundle {
    path: PathBuf,
    file: File,
    pub unity_version: String,
    pub nodes: Vec<Node>,
    blocks: Vec<Block>,
    cache: Mutex<Vec<(usize, Arc<Vec<u8>>)>>,
}

impl std::fmt::Debug for Bundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bundle")
            .field("path", &self.path)
            .field("nodes", &self.nodes)
            .finish_non_exhaustive()
    }
}

fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        context: path.display().to_string(),
        source,
    }
}

#[cfg(unix)]
fn read_at(file: &File, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
}

#[cfg(windows)]
fn read_at(file: &File, mut offset: u64, mut buf: &mut [u8]) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_read(buf, offset) {
            Ok(0) => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut buf[n..];
                offset += n as u64;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub(crate) fn decompress(data: &[u8], uncompressed: usize, compression: u32) -> Result<Vec<u8>> {
    match compression {
        0 => Ok(data.to_vec()),
        1 => {
            // Unity writes the 5-byte LZMA properties without the size field lzma-rs expects.
            if data.len() < 5 {
                return Err(format_err("truncated LZMA block"));
            }
            let mut header = Vec::with_capacity(data.len() + 8);
            header.extend_from_slice(&data[..5]);
            header.extend_from_slice(&(uncompressed as u64).to_le_bytes());
            header.extend_from_slice(&data[5..]);
            let mut out = Vec::with_capacity(uncompressed);
            lzma_rs::lzma_decompress(&mut &header[..], &mut out).map_err(|e| format_err(format!("LZMA: {e}")))?;
            Ok(out)
        }
        2 | 3 => {
            let mut out = vec![0; uncompressed];
            let n = lz4_flex::block::decompress_into(data, &mut out).map_err(|e| format_err(format!("LZ4: {e}")))?;
            if n != uncompressed {
                return Err(format_err(format!("LZ4 block: {n} of {uncompressed} bytes")));
            }
            Ok(out)
        }
        c => Err(Error::Unsupported(format!("compression type {c}"))),
    }
}

impl Bundle {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| io_err(path, e))?;
        let len = file.metadata().map_err(|e| io_err(path, e))?.len();
        let mut head = vec![0; len.min(4096) as usize];
        read_at(&file, 0, &mut head).map_err(|e| io_err(path, e))?;
        let mut r = Reader::new(&head, true);
        let signature = r.cstring()?;
        if signature != "UnityFS" {
            return Err(Error::Unsupported(format!("{}: {signature} archive", path.display())));
        }
        let version = r.u32()?;
        let _player_version = r.cstring()?;
        let unity_version = r.cstring()?;
        let _size = r.i64()?;
        let compressed_info = r.u32()? as usize;
        let uncompressed_info = r.u32()? as usize;
        let flags = r.u32()?;
        if version >= 7 {
            r.align(16);
        }
        let at_end = flags & 0x80 != 0;
        let info_offset = if at_end { len - compressed_info as u64 } else { r.pos as u64 };
        let mut info = vec![0; compressed_info];
        read_at(&file, info_offset, &mut info).map_err(|e| io_err(path, e))?;
        let info = decompress(&info, uncompressed_info, flags & 0x3f)?;
        let mut data_start = if at_end { r.pos as u64 } else { info_offset + compressed_info as u64 };
        if flags & 0x200 != 0 {
            data_start = data_start.div_ceil(16) * 16;
        }

        let mut r = Reader::new(&info, true);
        r.take(16)?; // hash
        let block_count = r.len()?;
        let mut blocks = Vec::with_capacity(block_count);
        let (mut offset, mut file_offset) = (0u64, data_start);
        for _ in 0..block_count {
            let uncompressed = r.u32()?;
            let compressed = r.u32()?;
            let flags = r.u16()?;
            blocks.push(Block {
                offset,
                uncompressed,
                file_offset,
                compressed,
                flags,
            });
            offset += u64::from(uncompressed);
            file_offset += u64::from(compressed);
        }
        let node_count = r.len()?;
        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let offset = r.u64()?;
            let size = r.u64()?;
            let flags = r.u32()?;
            let name = r.cstring()?;
            if offset + size > offset_end(&blocks) {
                return Err(format_err(format!("{}: node {name} past the block data", path.display())));
            }
            nodes.push(Node {
                path: name,
                offset,
                size,
                flags,
            });
        }
        Ok(Self {
            path: path.to_path_buf(),
            file,
            unity_version,
            nodes,
            blocks,
            cache: Mutex::new(Vec::new()),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Index of the node whose path (or file name) is `name`.
    pub fn node_index(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.path == name || n.path.rsplit('/').next() == Some(name))
    }

    fn block(&self, index: usize) -> Result<Arc<Vec<u8>>> {
        if let Some((_, data)) = self.cache.lock().unwrap().iter().find(|(i, _)| *i == index) {
            return Ok(data.clone());
        }
        let b = self.blocks[index];
        let mut raw = vec![0; b.compressed as usize];
        read_at(&self.file, b.file_offset, &mut raw).map_err(|e| io_err(&self.path, e))?;
        let data = Arc::new(decompress(&raw, b.uncompressed as usize, u32::from(b.flags & 0x3f))?);
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= BLOCK_CACHE {
            cache.remove(0);
        }
        cache.push((index, data.clone()));
        Ok(data)
    }

    /// `len` bytes at `offset` within node `node`.
    pub fn read(&self, node: usize, offset: u64, len: usize) -> Result<Vec<u8>> {
        let n = &self.nodes[node];
        if offset + len as u64 > n.size {
            return Err(format_err(format!(
                "{}: read of {len} bytes at {offset} past the end of {}",
                self.path.display(),
                n.path
            )));
        }
        let start = n.offset + offset;
        let end = start + len as u64;
        let mut out = Vec::with_capacity(len);
        let first = self.blocks.partition_point(|b| b.offset + u64::from(b.uncompressed) <= start);
        for i in first..self.blocks.len() {
            let b = self.blocks[i];
            if b.offset >= end {
                break;
            }
            let data = self.block(i)?;
            let from = start.saturating_sub(b.offset) as usize;
            let to = ((end - b.offset) as usize).min(data.len());
            out.extend_from_slice(&data[from..to]);
        }
        if out.len() != len {
            return Err(format_err(format!("{}: short read in {}", self.path.display(), n.path)));
        }
        Ok(out)
    }

    /// The whole node `node`.
    pub fn read_node(&self, node: usize) -> Result<Vec<u8>> {
        self.read(node, 0, self.nodes[node].size as usize)
    }
}

fn offset_end(blocks: &[Block]) -> u64 {
    blocks.last().map_or(0, |b| b.offset + u64::from(b.uncompressed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn be32(v: u32) -> [u8; 4] {
        v.to_be_bytes()
    }

    /// A UnityFS file with two LZ4 blocks holding one node, laid out as Unity 2020.3 writes it.
    fn synthetic_bundle(payload: &[u8]) -> Vec<u8> {
        let (a, b) = payload.split_at(payload.len() / 2);
        let blocks = [lz4_flex::block::compress(a), lz4_flex::block::compress(b)];
        let mut info = vec![0; 16];
        info.extend(be32(2));
        for (raw, packed) in [a, b].iter().zip(&blocks) {
            info.extend(be32(raw.len() as u32));
            info.extend(be32(packed.len() as u32));
            info.extend(3u16.to_be_bytes());
        }
        info.extend(be32(1));
        info.extend(0u64.to_be_bytes());
        info.extend((payload.len() as u64).to_be_bytes());
        info.extend(be32(4));
        info.extend(b"CAB-test\0");
        let packed_info = lz4_flex::block::compress(&info);

        let mut out = b"UnityFS\0".to_vec();
        out.extend(be32(8));
        out.extend(b"5.x.x\x002020.3.49f1\x00");
        out.extend(0u64.to_be_bytes());
        out.extend(be32(packed_info.len() as u32));
        out.extend(be32(info.len() as u32));
        out.extend(be32(0x200 | 0x40 | 2));
        out.resize(out.len().div_ceil(16) * 16, 0);
        out.extend(&packed_info);
        out.resize(out.len().div_ceil(16) * 16, 0);
        for b in &blocks {
            out.extend(b);
        }
        out
    }

    #[test]
    fn reads_nodes_across_blocks() {
        let payload: Vec<u8> = (0..50_000u32).map(|i| (i % 251) as u8).collect();
        let path = std::env::temp_dir().join(format!("ti-unity-test-{}.bundle", std::process::id()));
        std::fs::write(&path, synthetic_bundle(&payload)).unwrap();
        let bundle = Bundle::open(&path);
        std::fs::remove_file(&path).unwrap();
        let bundle = bundle.unwrap();
        assert_eq!(bundle.unity_version, "2020.3.49f1");
        assert_eq!(bundle.node_index("CAB-test"), Some(0));
        assert_eq!(bundle.read_node(0).unwrap(), payload);
        assert_eq!(bundle.read(0, 24_990, 20).unwrap(), payload[24_990..25_010]);
        assert!(bundle.read(0, 49_990, 20).is_err());
    }

    #[test]
    fn lzma_blocks_without_size_header() {
        let raw: Vec<u8> = b"terra invicta ".iter().copied().cycle().take(10_000).collect();
        let mut packed = Vec::new();
        lzma_rs::lzma_compress(&mut &raw[..], &mut packed).unwrap();
        // Unity drops the 8-byte unpacked size after the 5 property bytes.
        let unity: Vec<u8> = packed[..5].iter().chain(&packed[13..]).copied().collect();
        assert_eq!(decompress(&unity, raw.len(), 1).unwrap(), raw);
    }
}
