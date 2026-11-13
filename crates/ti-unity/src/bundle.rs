//! UnityFS archives. Blocks are decompressed on demand, so a 1.6 GB bundle is never held in memory.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::read::Reader;
use crate::{Error, Result, format_err, limits, with_capacity, zeroed};

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
/// Bytes of decompressed blocks kept in the cache (Unity's LZ4 blocks are 128 KiB).
const BLOCK_CACHE_BYTES: usize = 256 << 20;
/// Uncompressed bytes of the block and node table.
const MAX_INFO: usize = 64 << 20;
/// Uncompressed bytes of one block.
const MAX_BLOCK: usize = 1 << 30;

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
    if uncompressed > MAX_BLOCK {
        return Err(format_err(format!("block of {uncompressed} bytes")));
    }
    match compression {
        0 => {
            if data.len() != uncompressed {
                return Err(format_err(format!("stored block of {} bytes, expected {uncompressed}", data.len())));
            }
            let mut out = with_capacity(uncompressed)?;
            out.extend_from_slice(data);
            Ok(out)
        }
        1 => {
            // Unity writes the 5-byte LZMA properties without the size field.
            let options = lzma_rs::decompress::Options {
                unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(uncompressed as u64)),
                memlimit: Some(uncompressed),
                allow_incomplete: false,
            };
            let mut out = with_capacity(uncompressed)?;
            lzma_rs::lzma_decompress_with_options(&mut &data[..], &mut out, &options).map_err(|e| format_err(format!("LZMA: {e}")))?;
            if out.len() != uncompressed {
                return Err(format_err(format!("LZMA block: {} of {uncompressed} bytes", out.len())));
            }
            Ok(out)
        }
        2 | 3 => {
            // LZ4 expands at most about 255 times.
            if uncompressed / 255 > data.len() + 16 {
                return Err(format_err(format!("LZ4 block of {} bytes claims {uncompressed}", data.len())));
            }
            let mut out = zeroed(uncompressed)?;
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
        let compressed_info = u64::from(r.u32()?);
        let uncompressed_info = r.u32()? as usize;
        let flags = r.u32()?;
        if version >= 7 {
            r.align(16);
        }
        let bad = |what: &str| format_err(format!("{}: {what}", path.display()));
        if compressed_info > len || uncompressed_info > MAX_INFO {
            return Err(bad("bad block table size"));
        }
        let at_end = flags & 0x80 != 0;
        let info_offset = if at_end { len - compressed_info } else { r.pos as u64 };
        let info_end = info_offset.checked_add(compressed_info).filter(|&e| e <= len);
        let info_end = info_end.ok_or_else(|| bad("block table past the end of the file"))?;
        let mut info = zeroed(compressed_info as usize)?;
        read_at(&file, info_offset, &mut info).map_err(|e| io_err(path, e))?;
        let info = decompress(&info, uncompressed_info, flags & 0x3f)?;
        let mut data_start = if at_end { r.pos as u64 } else { info_end };
        if flags & 0x200 != 0 {
            data_start = data_start.div_ceil(16) * 16;
        }

        let mut r = Reader::new(&info, true);
        r.take(16)?; // hash
        let block_count = r.count(10)?;
        let mut blocks = Vec::with_capacity(block_count);
        let (mut offset, mut file_offset) = (0u64, data_start);
        for _ in 0..block_count {
            let uncompressed = r.u32()?;
            let compressed = r.u32()?;
            let flags = r.u16()?;
            let end = file_offset.checked_add(u64::from(compressed)).filter(|&e| e <= len);
            let end = end.ok_or_else(|| bad("block past the end of the file"))?;
            if uncompressed as usize > MAX_BLOCK {
                return Err(bad("oversized block"));
            }
            blocks.push(Block {
                offset,
                uncompressed,
                file_offset,
                compressed,
                flags,
            });
            offset = offset.checked_add(u64::from(uncompressed)).ok_or_else(|| bad("block data size"))?;
            file_offset = end;
        }
        let node_count = r.count(21)?;
        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let offset = r.u64()?;
            let size = r.u64()?;
            let flags = r.u32()?;
            let name = r.cstring()?;
            if offset.checked_add(size).is_none_or(|end| end > offset_end(&blocks)) {
                return Err(bad(&format!("node {name} past the block data")));
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
        let mut raw = zeroed(b.compressed as usize)?;
        read_at(&self.file, b.file_offset, &mut raw).map_err(|e| io_err(&self.path, e))?;
        let data = Arc::new(decompress(&raw, b.uncompressed as usize, u32::from(b.flags & 0x3f))?);
        let mut cache = self.cache.lock().unwrap();
        let mut bytes: usize = cache.iter().map(|(_, d)| d.len()).sum();
        while !cache.is_empty() && (cache.len() >= BLOCK_CACHE || bytes + data.len() > BLOCK_CACHE_BYTES) {
            bytes -= cache.remove(0).1.len();
        }
        cache.push((index, data.clone()));
        Ok(data)
    }

    /// `len` bytes at `offset` within node `node`.
    pub fn read(&self, node: usize, offset: u64, len: usize) -> Result<Vec<u8>> {
        let n = self
            .nodes
            .get(node)
            .ok_or_else(|| format_err(format!("{}: no node {node}", self.path.display())))?;
        if offset.checked_add(len as u64).is_none_or(|end| end > n.size) || len as u64 > limits::MAX_READ {
            return Err(format_err(format!(
                "{}: read of {len} bytes at {offset} past the end of {}",
                self.path.display(),
                n.path
            )));
        }
        // In range: `Bundle::open` checked that the node lies within the block data.
        let start = n.offset + offset;
        let end = start + len as u64;
        let mut out = with_capacity(len)?;
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
        let size = self.nodes.get(node).map_or(0, |n| n.size);
        self.read(node, 0, usize::try_from(size).map_err(|_| format_err("node too large"))?)
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

    /// A UnityFS file with an uncompressed block table `info` followed by `data`.
    fn raw_bundle(info_size: (u32, u32), info: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = b"UnityFS\0".to_vec();
        out.extend(be32(8));
        out.extend(b"5.x.x\x002020.3.49f1\x00");
        out.extend(0u64.to_be_bytes());
        out.extend(be32(info_size.0));
        out.extend(be32(info_size.1));
        out.extend(be32(0x40));
        out.resize(out.len().div_ceil(16) * 16, 0);
        out.extend(info);
        out.extend(data);
        out
    }

    /// Block table: `blocks` of (uncompressed, compressed, flags), then `nodes` of (offset, size).
    fn info(block_count: i32, blocks: &[(u32, u32, u16)], node_count: i32, nodes: &[(u64, u64)]) -> Vec<u8> {
        let mut info = vec![0; 16];
        info.extend(block_count.to_be_bytes());
        for &(u, c, f) in blocks {
            info.extend(be32(u));
            info.extend(be32(c));
            info.extend(f.to_be_bytes());
        }
        info.extend(node_count.to_be_bytes());
        for &(offset, size) in nodes {
            info.extend(offset.to_be_bytes());
            info.extend(size.to_be_bytes());
            info.extend(be32(0));
            info.extend(b"CAB-x\0");
        }
        info
    }

    fn open_bytes(name: &str, bytes: &[u8]) -> Result<Bundle> {
        let path = std::env::temp_dir().join(format!("ti-unity-test-{}-{name}.bundle", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let bundle = Bundle::open(&path);
        std::fs::remove_file(&path).unwrap();
        bundle
    }

    fn open_info(name: &str, info: &[u8], data: &[u8]) -> Result<Bundle> {
        open_bytes(name, &raw_bundle((info.len() as u32, info.len() as u32), info, data))
    }

    #[test]
    fn stored_blocks_read_back() {
        let data = b"0123456789";
        let bundle = open_info("stored", &info(1, &[(10, 10, 0)], 1, &[(2, 6)]), data).unwrap();
        assert_eq!(bundle.read_node(0).unwrap(), b"234567");
        assert!(bundle.read(0, u64::MAX, 2).is_err());
        assert!(bundle.read(0, 1, usize::MAX).is_err());
        assert!(bundle.read(7, 0, 1).is_err());
        assert!(bundle.read_node(7).is_err());
    }

    #[test]
    fn malformed_tables_are_errors() {
        let ok = info(1, &[(10, 10, 0)], 1, &[(0, 10)]);
        let data = [0u8; 10];
        // Table sizes past the file or past the limit.
        assert!(open_bytes("info-past-end", &raw_bundle((1 << 30, ok.len() as u32), &ok, &data)).is_err());
        assert!(open_bytes("info-huge", &raw_bundle((ok.len() as u32, u32::MAX), &ok, &data)).is_err());
        // Counts larger than the table.
        assert!(open_info("blocks", &info(i32::MAX, &[(10, 10, 0)], 1, &[(0, 10)]), &data).is_err());
        assert!(open_info("blocks-neg", &info(-1, &[], 0, &[]), &data).is_err());
        assert!(open_info("nodes", &info(1, &[(10, 10, 0)], i32::MAX, &[(0, 10)]), &data).is_err());
        // Ranges that overflow or leave the data.
        assert!(open_info("node-wrap", &info(1, &[(10, 10, 0)], 1, &[(u64::MAX, 2)]), &data).is_err());
        assert!(open_info("node-past", &info(1, &[(10, 10, 0)], 1, &[(5, 6)]), &data).is_err());
        assert!(open_info("block-past", &info(1, &[(10, u32::MAX, 0)], 1, &[(0, 10)]), &data).is_err());
        assert!(open_info("block-huge", &info(1, &[(u32::MAX, 10, 2)], 1, &[(0, 10)]), &data).is_err());
        let wrap = info(2, &[(u32::MAX, 5, 2), (u32::MAX, 5, 2)], 1, &[(0, 10)]);
        assert!(open_info("blocks-wrap", &wrap, &data).is_err());
        open_info("ok", &ok, &data).unwrap();
    }

    #[test]
    fn lying_blocks_are_read_errors() {
        // An LZ4 block claiming far more than it can hold, and a stored block of the wrong size.
        let data = [0u8; 10];
        let bundle = open_info("lz4-bomb", &info(1, &[(200 << 20, 10, 2)], 1, &[(0, 200 << 20)]), &data).unwrap();
        assert!(bundle.read(0, 0, 16).is_err());
        let bundle = open_info("stored-short", &info(1, &[(20, 10, 0)], 1, &[(0, 20)]), &data).unwrap();
        assert!(bundle.read(0, 0, 16).is_err());
        assert!(decompress(&[0; 4], 100, 1).is_err());
        assert!(decompress(&[0; 4], usize::MAX, 3).is_err());
    }
}
