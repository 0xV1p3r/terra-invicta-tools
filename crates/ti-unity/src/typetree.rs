//! Type trees and reading objects through them (UnityPy's `TypeTreeHelper.read_value`).

use crate::read::Reader;
use crate::{Error, Result, format_err};

/// Unity's shared type tree string buffer: offsets with the high bit set index into it.
const COMMON_STRINGS: &str = "AABB\0AnimationClip\0AnimationCurve\0AnimationState\0Array\0Base\0BitField\0bitset\0bool\0char\0ColorRGBA\0\
Component\0data\0deque\0double\0dynamic_array\0FastPropertyName\0first\0float\0Font\0GameObject\0Generic Mono\0GradientNEW\0GUID\0\
GUIStyle\0int\0list\0long long\0map\0Matrix4x4f\0MdFour\0MonoBehaviour\0MonoScript\0m_ByteSize\0m_Curve\0m_EditorClassIdentifier\0\
m_EditorHideFlags\0m_Enabled\0m_ExtensionPtr\0m_GameObject\0m_Index\0m_IsArray\0m_IsStatic\0m_MetaFlag\0m_Name\0m_ObjectHideFlags\0\
m_PrefabInternal\0m_PrefabParentObject\0m_Script\0m_StaticEditorFlags\0m_Type\0m_Version\0Object\0pair\0PPtr<Component>\0\
PPtr<GameObject>\0PPtr<Material>\0PPtr<MonoBehaviour>\0PPtr<MonoScript>\0PPtr<Object>\0PPtr<Prefab>\0PPtr<Sprite>\0PPtr<TextAsset>\0\
PPtr<Texture>\0PPtr<Texture2D>\0PPtr<Transform>\0Prefab\0Quaternionf\0Rectf\0RectInt\0RectOffset\0second\0set\0short\0size\0SInt16\0\
SInt32\0SInt64\0SInt8\0staticvector\0string\0TextAsset\0TextMesh\0Texture\0Texture2D\0Transform\0TypelessData\0UInt16\0UInt32\0UInt64\0\
UInt8\0unsigned int\0unsigned long long\0unsigned short\0vector\0Vector2f\0Vector3f\0Vector4f\0m_ScriptingClassIdentifier\0Gradient\0\
Type*\0int2_storage\0int3_storage\0BoundsInt\0m_CorrespondingSourceObject\0m_PrefabInstance\0m_PrefabAsset\0FileSize\0Hash128\0\
RenderingLayerMask\0fixed_array\0EntityId\0LoadableObjectId\0LoadableSceneId\0";

const ALIGN: i32 = 0x4000;

#[derive(Debug, Clone)]
pub struct TypeNode {
    pub ty: String,
    pub name: String,
    pub level: u8,
    pub meta_flag: i32,
    pub children: Vec<usize>,
}

impl TypeNode {
    fn aligned(&self) -> bool {
        self.meta_flag & ALIGN != 0
    }
}

/// A type tree; node 0 is the root.
#[derive(Debug, Clone, Default)]
pub struct TypeTree {
    pub nodes: Vec<TypeNode>,
}

fn tree_string(buf: &[u8], offset: u32) -> String {
    let (buf, offset) = if offset & 0x8000_0000 != 0 {
        (COMMON_STRINGS.as_bytes(), offset & 0x7fff_ffff)
    } else {
        (buf, offset)
    };
    let rest = buf.get(offset as usize..).unwrap_or_default();
    let n = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..n]).into_owned()
}

impl TypeTree {
    /// The blob format (serialized file version >= 12).
    pub(crate) fn read_blob(r: &mut Reader<'_>, version: u32) -> Result<Self> {
        let count = r.len()?;
        let strings_len = r.len()?;
        let node_size = if version >= 19 { 32 } else { 24 };
        let raw = r.take(count.checked_mul(node_size).ok_or_else(|| format_err("type tree size"))?)?;
        let strings = r.take(strings_len)?;
        let mut nr = Reader::new(raw, r.big_endian);
        let mut nodes: Vec<TypeNode> = Vec::with_capacity(count);
        let mut stack: Vec<usize> = Vec::new();
        for i in 0..count {
            let _version = nr.u16()?;
            let level = nr.u8()?;
            let _type_flags = nr.u8()?;
            let ty = tree_string(strings, nr.u32()?);
            let name = tree_string(strings, nr.u32()?);
            let _byte_size = nr.i32()?;
            let _index = nr.i32()?;
            let meta_flag = nr.i32()?;
            if version >= 19 {
                nr.u64()?;
            }
            while stack.last().is_some_and(|&p| nodes[p].level >= level) {
                stack.pop();
            }
            if let Some(&p) = stack.last() {
                nodes[p].children.push(i);
            } else if i != 0 {
                return Err(format_err("type tree with several roots"));
            }
            nodes.push(TypeNode {
                ty,
                name,
                level,
                meta_flag,
                children: Vec::new(),
            });
            stack.push(i);
        }
        if nodes.is_empty() {
            return Err(format_err("empty type tree"));
        }
        Ok(Self { nodes })
    }

    /// Names of the root's fields.
    pub fn field_names(&self) -> impl Iterator<Item = &str> {
        self.nodes[0].children.iter().map(|&c| self.nodes[c].name.as_str())
    }

    /// Read one object.
    pub(crate) fn read(&self, r: &mut Reader<'_>) -> Result<Value> {
        self.read_value(0, r)
    }

    fn read_value(&self, i: usize, r: &mut Reader<'_>) -> Result<Value> {
        let node = &self.nodes[i];
        let mut align = node.aligned();
        let value = if let Some(v) = read_primitive(&node.ty, r)? {
            v
        } else if node.ty == "pair" {
            let [a, b] = node.children[..] else {
                return Err(format_err("pair without two children"));
            };
            Value::Pair(Box::new((self.read_value(a, r)?, self.read_value(b, r)?)))
        } else if node.ty == "ReferencedObject" || node.ty == "ManagedReferencesRegistry" {
            return Err(Error::Unsupported(format!("managed references ({})", node.name)));
        } else if let Some(&array) = node.children.first().filter(|&&c| self.nodes[c].ty == "Array") {
            let array = &self.nodes[array];
            align |= array.aligned();
            let &elem = array.children.get(1).ok_or_else(|| format_err("array without element type"))?;
            let size = r.len()?;
            self.read_array(elem, size, r)?
        } else {
            let mut fields = Vec::with_capacity(node.children.len());
            for &c in &node.children {
                fields.push((self.nodes[c].name.clone(), self.read_value(c, r)?));
            }
            Value::Object(fields)
        };
        if align {
            r.align(4);
        }
        Ok(value)
    }

    /// `size` elements of type `elem`; the element's own alignment applies once, after the array.
    fn read_array(&self, elem: usize, size: usize, r: &mut Reader<'_>) -> Result<Value> {
        let node = &self.nodes[elem];
        let value = match node.ty.as_str() {
            "UInt8" | "char" | "SInt8" => Value::Bytes(r.take(size)?.to_vec()),
            "string" => Value::Array((0..size).map(|_| r.aligned_string().map(Value::String)).collect::<Result<_>>()?),
            "TypelessData" => Value::Array((0..size).map(|_| read_bytes(r)).collect::<Result<_>>()?),
            ty if is_primitive(ty) => {
                let mut items = Vec::with_capacity(size.min(1 << 20));
                for _ in 0..size {
                    items.push(read_primitive(ty, r)?.unwrap());
                }
                Value::Array(items)
            }
            _ if node.children.first().is_some_and(|&c| self.nodes[c].ty == "Array") => {
                let array = &self.nodes[node.children[0]];
                let &inner = array.children.get(1).ok_or_else(|| format_err("array without element type"))?;
                let mut items = Vec::with_capacity(size.min(1 << 16));
                for _ in 0..size {
                    let n = r.len()?;
                    items.push(self.read_array(inner, n, r)?);
                }
                Value::Array(items)
            }
            "pair" => {
                let [a, b] = node.children[..] else {
                    return Err(format_err("pair without two children"));
                };
                let mut items = Vec::with_capacity(size.min(1 << 16));
                for _ in 0..size {
                    items.push(Value::Pair(Box::new((self.read_value(a, r)?, self.read_value(b, r)?))));
                }
                Value::Array(items)
            }
            "ReferencedObject" | "ManagedReferencesRegistry" => return Err(Error::Unsupported(format!("managed references ({})", node.name))),
            _ => {
                let mut items = Vec::with_capacity(size.min(1 << 16));
                for _ in 0..size {
                    let mut fields = Vec::with_capacity(node.children.len());
                    for &c in &node.children {
                        fields.push((self.nodes[c].name.clone(), self.read_value(c, r)?));
                    }
                    items.push(Value::Object(fields));
                }
                Value::Array(items)
            }
        };
        if node.aligned() {
            r.align(4);
        }
        Ok(value)
    }
}

fn is_primitive(ty: &str) -> bool {
    matches!(
        ty,
        "SInt8"
            | "UInt8"
            | "char"
            | "short"
            | "SInt16"
            | "unsigned short"
            | "UInt16"
            | "int"
            | "SInt32"
            | "unsigned int"
            | "UInt32"
            | "Type*"
            | "long long"
            | "SInt64"
            | "unsigned long long"
            | "UInt64"
            | "FileSize"
            | "float"
            | "double"
            | "bool"
            | "string"
            | "TypelessData"
    )
}

fn read_bytes(r: &mut Reader<'_>) -> Result<Value> {
    let n = r.len()?;
    Ok(Value::Bytes(r.take(n)?.to_vec()))
}

fn read_primitive(ty: &str, r: &mut Reader<'_>) -> Result<Option<Value>> {
    Ok(Some(match ty {
        "SInt8" => Value::Int(r.i8()?.into()),
        "UInt8" | "char" => Value::Int(r.u8()?.into()),
        "short" | "SInt16" => Value::Int(r.i16()?.into()),
        "unsigned short" | "UInt16" => Value::Int(r.u16()?.into()),
        "int" | "SInt32" => Value::Int(r.i32()?.into()),
        "unsigned int" | "UInt32" | "Type*" => Value::Int(r.u32()?.into()),
        "long long" | "SInt64" => Value::Int(r.i64()?),
        "unsigned long long" | "UInt64" | "FileSize" => Value::UInt(r.u64()?),
        "float" => Value::Float(r.f32()?.into()),
        "double" => Value::Float(r.f64()?),
        "bool" => Value::Bool(r.bool()?),
        "string" => Value::String(r.aligned_string()?),
        "TypelessData" => read_bytes(r)?,
        _ => return Ok(None),
    }))
}

/// An object read through its type tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
    /// `UInt8`/`SInt8` arrays and `TypelessData`.
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Pair(Box<(Value, Value)>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, name: &str) -> Option<&Value> {
        match self {
            Self::Object(fields) => fields.iter().find(|(n, _)| n == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Field `name`, or an error naming it.
    pub fn field(&self, name: &str) -> Result<&Value> {
        self.get(name).ok_or_else(|| format_err(format!("missing field {name}")))
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Self::Int(v) => Some(v),
            Self::UInt(v) => i64::try_from(v).ok(),
            Self::Bool(v) => Some(v.into()),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Self::Float(v) => Some(v),
            Self::Int(v) => Some(v as f64),
            Self::UInt(v) => Some(v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn i64_field(&self, name: &str) -> Result<i64> {
        self.field(name)?.as_i64().ok_or_else(|| format_err(format!("field {name} is not an integer")))
    }

    pub fn f64_field(&self, name: &str) -> Result<f64> {
        self.field(name)?.as_f64().ok_or_else(|| format_err(format!("field {name} is not a number")))
    }

    pub fn str_field(&self, name: &str) -> Result<&str> {
        self.field(name)?.as_str().ok_or_else(|| format_err(format!("field {name} is not a string")))
    }

    pub fn array_field(&self, name: &str) -> Result<&[Value]> {
        self.field(name)?.as_array().ok_or_else(|| format_err(format!("field {name} is not an array")))
    }

    /// `(m_FileID, m_PathID)` of a `PPtr` field.
    pub fn pptr_field(&self, name: &str) -> Result<(i64, i64)> {
        let p = self.field(name)?;
        Ok((p.i64_field("m_FileID")?, p.i64_field("m_PathID")?))
    }
}
