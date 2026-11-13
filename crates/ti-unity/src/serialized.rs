//! Serialized files (the `CAB-…` nodes of a bundle): the type and object tables.

use std::sync::Arc;

use crate::bundle::Bundle;
use crate::read::Reader;
use crate::typetree::{TypeTree, Value};
use crate::{Error, Result, format_err};

#[derive(Debug, Clone)]
pub struct SerializedType {
    pub class_id: i32,
    pub tree: TypeTree,
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectInfo {
    pub path_id: i64,
    /// Offset of the object data within the serialized file.
    pub offset: u64,
    pub size: u32,
    pub type_index: usize,
    pub class_id: i32,
}

/// A serialized file inside a bundle. Object data is read from the bundle on demand.
pub struct SerializedFile {
    pub bundle: Arc<Bundle>,
    pub node: usize,
    pub version: u32,
    pub big_endian: bool,
    pub types: Vec<SerializedType>,
    /// In file order (the order UnityPy's `env.objects` yields).
    pub objects: Vec<ObjectInfo>,
}

impl SerializedFile {
    /// Every serialized file in `bundle` (the nodes that are not stream data).
    pub fn all(bundle: &Arc<Bundle>) -> Result<Vec<SerializedFile>> {
        let mut files = Vec::new();
        for (i, node) in bundle.nodes.iter().enumerate() {
            if node.path.ends_with(".resS") || node.path.ends_with(".resource") {
                continue;
            }
            files.push(Self::parse(bundle.clone(), i)?);
        }
        Ok(files)
    }

    pub fn parse(bundle: Arc<Bundle>, node: usize) -> Result<Self> {
        let n = bundle
            .nodes
            .get(node)
            .ok_or_else(|| format_err(format!("{}: no node {node}", bundle.path().display())))?;
        let name = n.path.clone();
        let err = |e: Error| format_err(format!("{}/{name}: {e}", bundle.path().display()));
        let head = bundle.read(node, 0, n.size.min(48) as usize).map_err(err)?;
        let mut r = Reader::new(&head, true);
        let mut metadata_size = u64::from(r.u32()?);
        let _file_size = r.u32()?;
        let version = r.u32()?;
        let mut data_offset = u64::from(r.u32()?);
        if version < 17 {
            return Err(Error::Unsupported(format!("{name}: serialized file version {version}")));
        }
        let big_endian = r.u8()? != 0;
        r.take(3)?;
        if version >= 22 {
            metadata_size = u64::from(r.u32()?);
            let _file_size = r.i64()?;
            data_offset = r.u64()?;
            r.i64()?;
        }
        let meta_start = r.pos;
        let meta_size = usize::try_from(metadata_size).map_err(|_| err(format_err("metadata size")))?;
        let meta = bundle.read(node, meta_start as u64, meta_size).map_err(err)?;
        let parsed = Self::parse_metadata(&meta, version, big_endian, data_offset).map_err(err)?;
        let (types, objects) = parsed;
        Ok(Self {
            bundle,
            node,
            version,
            big_endian,
            types,
            objects,
        })
    }

    fn parse_metadata(meta: &[u8], version: u32, big_endian: bool, data_offset: u64) -> Result<(Vec<SerializedType>, Vec<ObjectInfo>)> {
        let mut r = Reader::new(meta, big_endian);
        let _unity_version = r.cstring()?;
        let _platform = r.i32()?;
        let type_tree = r.bool()?;
        if !type_tree {
            return Err(Error::Unsupported("serialized file without type trees".into()));
        }
        // Class id, stripped flag, script index, hash, and an empty type tree.
        let type_count = r.count(31)?;
        let mut types = Vec::with_capacity(type_count);
        for _ in 0..type_count {
            let class_id = r.i32()?;
            r.u8()?; // stripped
            let _script_type_index = r.i16()?;
            if class_id == 114 {
                r.take(16)?; // script id
            }
            r.take(16)?; // old type hash
            let tree = TypeTree::read_blob(&mut r, version)?;
            if version >= 21 {
                let deps = r.len()?;
                r.take(deps * 4)?;
            }
            types.push(SerializedType { class_id, tree });
        }
        let object_count = r.count(if version >= 22 { 24 } else { 20 })?;
        let mut objects = Vec::with_capacity(object_count);
        for _ in 0..object_count {
            r.align(4);
            let path_id = r.i64()?;
            let start = if version >= 22 { r.u64()? } else { u64::from(r.u32()?) };
            let size = r.u32()?;
            let type_index = r.len()?;
            let class_id = types
                .get(type_index)
                .ok_or_else(|| format_err(format!("object {path_id}: bad type index {type_index}")))?
                .class_id;
            let offset = data_offset
                .checked_add(start)
                .ok_or_else(|| format_err(format!("object {path_id}: bad offset")))?;
            objects.push(ObjectInfo {
                path_id,
                offset,
                size,
                type_index,
                class_id,
            });
        }
        Ok((types, objects))
    }

    pub fn object(&self, path_id: i64) -> Option<&ObjectInfo> {
        // Objects are stored sorted by path id.
        match self.objects.binary_search_by_key(&path_id, |o| o.path_id) {
            Ok(i) => Some(&self.objects[i]),
            Err(_) => self.objects.iter().find(|o| o.path_id == path_id),
        }
    }

    pub fn tree(&self, obj: &ObjectInfo) -> &TypeTree {
        &self.types[obj.type_index].tree
    }

    pub fn raw(&self, obj: &ObjectInfo) -> Result<Vec<u8>> {
        self.bundle.read(self.node, obj.offset, obj.size as usize)
    }

    /// `m_Name`, for objects whose first field it is (textures, sprites, ...).
    pub fn peek_name(&self, obj: &ObjectInfo) -> Result<Option<String>> {
        let tree = self.tree(obj);
        if tree.nodes[0].children.first().map(|&c| tree.nodes[c].name.as_str()) != Some("m_Name") {
            return Ok(None);
        }
        let head = self.bundle.read(self.node, obj.offset, 4.min(obj.size as usize))?;
        let mut r = Reader::new(&head, self.big_endian);
        let n = r.len()?;
        if 4 + n > obj.size as usize {
            return Err(format_err(format!("object {}: bad name length", obj.path_id)));
        }
        let name = self.bundle.read(self.node, obj.offset.saturating_add(4), n)?;
        Ok(Some(String::from_utf8_lossy(&name).into_owned()))
    }

    /// The object read through its type tree.
    pub fn read(&self, obj: &ObjectInfo) -> Result<Value> {
        let data = self.raw(obj)?;
        let mut r = Reader::new(&data, self.big_endian);
        self.tree(obj).read(&mut r).map_err(|e| format_err(format!("object {}: {e}", obj.path_id)))
    }

    /// Bytes of a `StreamingInfo` (`path` is `archive:/CAB-…/CAB-….resS`).
    pub fn stream_data(&self, path: &str, offset: u64, size: usize) -> Result<Vec<u8>> {
        let name = path.rsplit('/').next().unwrap_or(path);
        let node = self
            .bundle
            .node_index(name)
            .ok_or_else(|| format_err(format!("stream file {path} not in {}", self.bundle.path().display())))?;
        self.bundle.read(node, offset, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metadata of version 22 (big endian): one type with a one-node tree, then `objects`.
    fn metadata(type_count: i32, object_count: i32, objects: &[(u64, u32, i32)]) -> Vec<u8> {
        let mut m = b"2020.3.49f1\0".to_vec();
        m.extend(19i32.to_be_bytes()); // platform
        m.push(1); // type trees
        m.extend(type_count.to_be_bytes());
        m.extend(28i32.to_be_bytes());
        m.push(0);
        m.extend((-1i16).to_be_bytes());
        m.extend([0; 16]);
        m.extend(1i32.to_be_bytes()); // nodes
        m.extend(5i32.to_be_bytes()); // string bytes
        m.extend([0; 32]);
        m.extend(b"Base\0");
        m.extend(0i32.to_be_bytes()); // dependencies
        m.extend(object_count.to_be_bytes());
        for &(start, size, ty) in objects {
            m.resize(m.len().div_ceil(4) * 4, 0);
            m.extend(1i64.to_be_bytes());
            m.extend(start.to_be_bytes());
            m.extend(size.to_be_bytes());
            m.extend(ty.to_be_bytes());
        }
        m
    }

    #[test]
    fn metadata_tables() {
        let (types, objects) = SerializedFile::parse_metadata(&metadata(1, 1, &[(16, 8, 0)]), 22, true, 4096).unwrap();
        assert_eq!((types.len(), objects[0].offset, objects[0].class_id), (1, 4112, 28));
    }

    #[test]
    fn malformed_metadata_is_an_error() {
        for (meta, data_offset) in [
            (metadata(i32::MAX, 1, &[(16, 8, 0)]), 0),
            (metadata(-1, 1, &[(16, 8, 0)]), 0),
            (metadata(1, i32::MAX, &[(16, 8, 0)]), 0),
            (metadata(1, 1, &[(16, 8, 5)]), 0),
            (metadata(1, 1, &[(u64::MAX, 8, 0)]), 16),
        ] {
            assert!(SerializedFile::parse_metadata(&meta, 22, true, data_offset).is_err());
        }
    }
}
