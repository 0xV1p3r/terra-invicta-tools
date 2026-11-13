//! Image export from the game's asset bundles (port of `assets.py`).
//!
//! Templates reference resources as `<bundle>/<assetName>` (e.g. `shipbuildericons/ICO_SteelArmor`).
//! Each bundle ships a plain-text `<bundle>.manifest` listing its asset paths, which decides which
//! bundles hold matching images without unpacking anything. Bundles are read with `ti-unity`, which
//! reproduces UnityPy + Pillow pixel for pixel; images are decoded and encoded on all cores.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufWriter, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use ti_unity::{Bundle as UnityBundle, Image, Mode, SerializedFile, Sprite, Texture2D, class_id};

use crate::{Error, ImageFormat, Progress, Result};

/// TextMeshPro sprite asset behind `<sprite name="…">` tags in localized text.
pub const INLINE_SPRITE_BUNDLE: &str = "ui";
pub const INLINE_SPRITE_DIR: &str = "sprites";

const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".tga", ".psd", ".tif", ".tiff", ".bmp", ".exr"];

/// Images the game uses that are hard-coded rather than referenced by templates.
const EXTRA_RESOURCE_CATEGORIES: &[&str] = &["energy", "info", "life", "material", "military", "social", "space", "xeno"];

#[derive(Debug, Clone)]
pub struct Bundle {
    /// Lower-case bundle name.
    pub name: String,
    pub path: PathBuf,
    /// Lower-case image stem -> original stem.
    pub images: BTreeMap<String, String>,
}

/// An earlier build's `assets/` dir and its parsed `index.json`, for reusing unchanged bundles.
#[derive(Debug, Clone)]
pub struct Previous {
    pub dir: PathBuf,
    pub index: Value,
}

/// `resources`, `bundles` and `missing` of `assets/index.json`.
#[derive(Debug, Clone, Default)]
pub struct AssetResult {
    pub resources: BTreeMap<String, Value>,
    pub bundles: BTreeMap<String, Value>,
    pub missing: Vec<String>,
}

pub struct AssetOptions<'a> {
    /// Lower-case bundle names not to export.
    pub skip_bundles: &'a [String],
    /// Newest first.
    pub previous: &'a [Previous],
    pub image_format: ImageFormat,
    pub progress: &'a mut dyn FnMut(Progress),
    pub cancel: &'a AtomicBool,
}

fn io_err(context: impl std::fmt::Display, source: std::io::Error) -> Error {
    Error::Io {
        context: context.to_string(),
        source,
    }
}

fn unity_err(e: ti_unity::Error) -> Error {
    match e {
        ti_unity::Error::Io { context, source } => Error::Io { context, source },
        e => Error::Other(e.to_string()),
    }
}

/// Python's `ast.literal_eval` of a double-quoted string: Unity escapes non-ASCII paths this way.
fn unescape_python(s: &str) -> String {
    let inner = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let hex = |chars: &mut std::str::Chars<'_>, n: usize| -> Option<char> {
            let digits: String = chars.by_ref().take(n).collect();
            u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32)
        };
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('x') => out.extend(hex(&mut chars, 2)),
            Some('u') => out.extend(hex(&mut chars, 4)),
            Some('U') => out.extend(hex(&mut chars, 8)),
            Some(c @ ('\\' | '"' | '\'')) => out.push(c),
            Some(c) => {
                out.push('\\');
                out.push(c);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn decode_manifest_path(value: &str) -> String {
    if value.starts_with('"') {
        unescape_python(value)
    } else if let Some(rest) = value.strip_prefix('\'') {
        rest.strip_suffix('\'').unwrap_or(rest).replace("''", "'")
    } else {
        value.to_owned()
    }
}

/// Unity's `Assets:` list, folding indented plain-scalar continuations.
fn manifest_assets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_assets = false;
    let mut asset: Option<String> = None;
    for line in text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) {
        if !in_assets {
            in_assets = line.starts_with("Assets:");
            continue;
        }
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("- ") {
            out.extend(asset.take().map(|a| decode_manifest_path(&a)));
            asset = Some(rest.trim().to_owned());
        } else if line.starts_with(char::is_whitespace) {
            if let Some(a) = &mut asset {
                a.push(' ');
                a.push_str(line.trim());
            }
        } else if line.contains(':') {
            break; // the next top-level key ends the list
        }
    }
    out.extend(asset.map(|a| decode_manifest_path(&a)));
    out
}

/// `(stem, suffix)` of the last path component, as `pathlib` splits them.
fn stem_suffix(path: &str) -> (&str, &str) {
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// Bundles listed in the `*.manifest` files of `bundle_dirs`, by lower-case name.
pub fn read_bundle_manifests(bundle_dirs: &[PathBuf]) -> Result<BTreeMap<String, Bundle>> {
    let mut bundles = BTreeMap::new();
    for dir in bundle_dirs {
        let entries = fs::read_dir(dir).map_err(|e| io_err(dir.display(), e))?;
        let mut manifests: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "manifest") && p.file_stem().is_some_and(|s| !s.is_empty()))
            .collect();
        manifests.sort();
        for manifest in manifests {
            let path = manifest.with_extension("");
            if !path.is_file() || path.file_name() == dir.file_name() {
                continue; // the index bundle
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            let bytes = fs::read(&manifest).map_err(|e| io_err(manifest.display(), e))?;
            let mut images = BTreeMap::new();
            for asset in manifest_assets(&String::from_utf8_lossy(&bytes)) {
                let (stem, suffix) = stem_suffix(&asset);
                if IMAGE_EXTENSIONS.contains(&suffix.to_lowercase().as_str()) {
                    images.entry(stem.to_lowercase()).or_insert_with(|| stem.to_owned());
                }
            }
            bundles.entry(name.clone()).or_insert(Bundle { name, path, images });
        }
    }
    Ok(bundles)
}

/// Image resources referenced by template strings, plus the hard-coded extras.
pub fn collect_resources<'s>(strings: impl IntoIterator<Item = &'s str>, bundles: &BTreeMap<String, Bundle>) -> BTreeSet<String> {
    let mut found: BTreeSet<String> = EXTRA_RESOURCE_CATEGORIES.iter().map(|c| format!("icons_2d/tech_{c}_icon")).collect();
    for s in strings {
        if let Some((prefix, _)) = s.split_once('/')
            && !s.starts_with("event:")
            && !s.starts_with("http")
            && bundles.contains_key(&prefix.to_lowercase())
        {
            found.insert(s.to_owned());
        }
    }
    found
}

fn file_name(stem: &str, format: ImageFormat) -> String {
    let mut out = String::with_capacity(stem.len() + 5);
    let mut in_run = false;
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ' ' | '-') {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('_');
            in_run = true;
        }
    }
    out.push('.');
    out.push_str(format.extension());
    out
}

/// bundle -> {original image stem -> [resource strings]}; plus unresolvable resources.
type Plan = BTreeMap<String, BTreeMap<String, Vec<String>>>;

fn plan(resources: &BTreeSet<String>, bundles: &BTreeMap<String, Bundle>) -> (Plan, Vec<String>) {
    let mut plan = Plan::new();
    let mut missing = Vec::new();
    for res in resources {
        let (bname, asset) = res.split_once('/').unwrap_or((res, ""));
        let Some(bundle) = bundles.get(&bname.to_lowercase()) else {
            missing.push(res.clone());
            continue;
        };
        let key = asset.rsplit('/').next().unwrap_or("").to_lowercase();
        let entry = plan.entry(bundle.name.clone()).or_default();
        if let Some(stem) = bundle.images.get(&key) {
            entry.entry(stem.clone()).or_default().push(res.clone());
            continue;
        }
        // Explicit prefixes and runtime states such as ICO_abductions_on/off.
        let matches: Vec<&String> = bundle
            .images
            .iter()
            .filter(|(k, _)| {
                if key.ends_with('_') {
                    k.starts_with(&key)
                } else {
                    k.strip_prefix(&key)
                        .and_then(|r| r.strip_prefix('_'))
                        .is_some_and(|r| !r.is_empty() && r.chars().all(char::is_alphanumeric))
                }
            })
            .map(|(_, s)| s)
            .collect();
        if matches.is_empty() {
            missing.push(res.clone());
        } else {
            for stem in matches {
                entry.entry(stem.clone()).or_default().push(res.clone());
            }
        }
    }
    plan.retain(|_, stems| !stems.is_empty());
    (plan, missing)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path).map_err(|e| io_err(path.display(), e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| io_err(path.display(), e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn save(image: &Image, dest: &Path, format: ImageFormat) -> Result<()> {
    use image::ImageEncoder;
    let err = |e: image::ImageError| Error::Other(format!("{}: {e}", dest.display()));
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent.display(), e))?;
    }
    let file = fs::File::create(dest).map_err(|e| io_err(dest.display(), e))?;
    let mut w = BufWriter::new(file);
    let color = match image.mode {
        Mode::Rgb => image::ExtendedColorType::Rgb8,
        Mode::Rgba => image::ExtendedColorType::Rgba8,
    };
    match format {
        ImageFormat::Png => {
            image::codecs::png::PngEncoder::new_with_quality(&mut w, image::codecs::png::CompressionType::Default, image::codecs::png::FilterType::Adaptive)
                .write_image(&image.data, image.width, image.height, color)
                .map_err(err)?
        }
        ImageFormat::Webp => image::codecs::webp::WebPEncoder::new_lossless(&mut w)
            .write_image(&image.data, image.width, image.height, color)
            .map_err(err)?,
    }
    std::io::Write::flush(&mut w).map_err(|e| io_err(dest.display(), e))
}

fn image_meta(rel: String, image: &Image) -> Value {
    json!({"file": rel, "width": image.width, "height": image.height})
}

/// Run `work` on every item on all cores, handing each result to `done` on this thread (in
/// completion order). Stops early when `cancel` is set or `done` fails.
fn parallel<T: Send, R: Send>(items: Vec<T>, cancel: &AtomicBool, work: impl Fn(T) -> R + Sync, mut done: impl FnMut(R) -> Result<()>) -> Result<()> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(items.len());
    let queue = Mutex::new(items.into_iter());
    let stop = AtomicBool::new(false);
    let (tx, rx) = mpsc::channel();
    let mut result = Ok(());
    std::thread::scope(|s| {
        for _ in 0..threads {
            let tx = tx.clone();
            let (queue, stop, work) = (&queue, &stop, &work);
            s.spawn(move || {
                while !stop.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) {
                    let Some(item) = queue.lock().unwrap().next() else { break };
                    if tx.send(work(item)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        for r in rx {
            if result.is_ok()
                && let Err(e) = done(r)
            {
                stop.store(true, Ordering::Relaxed);
                result = Err(e);
            }
        }
    });
    result?;
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    Ok(())
}

/// Copy (hard-link) the wanted images of a previous build's bundle entry; `None` if any is unusable.
fn reuse(prev_dir: &Path, prev: &Value, out_dir: &Path, wanted: &BTreeSet<&str>, format: ImageFormat) -> Option<BTreeMap<String, Value>> {
    let exported = prev.get("exported")?.as_object()?;
    let exported: BTreeMap<String, Value> = exported
        .iter()
        .filter(|(k, _)| wanted.contains(k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let suffix = format!(".{}", format.extension());
    let files: Vec<&str> = exported.values().map(|m| m.get("file").and_then(Value::as_str)).collect::<Option<_>>()?;
    if files.iter().any(|f| !f.ends_with(&suffix)) {
        return None;
    }
    for file in files {
        let (src, dest) = (prev_dir.join(file), out_dir.join(file));
        if !src.is_file() {
            return None;
        }
        fs::create_dir_all(dest.parent()?).ok()?;
        if dest.exists() {
            fs::remove_file(&dest).ok()?;
        }
        if fs::hard_link(&src, &dest).is_err() {
            fs::copy(&src, &dest).ok()?;
        }
    }
    Some(exported)
}

/// The objects that may supply each wanted stem: sprites first (they carry the atlas cropping),
/// then raw textures, each in file order.
fn candidates(files: &[SerializedFile], want: &BTreeMap<String, String>) -> Result<BTreeMap<String, Vec<(usize, usize)>>> {
    let mut out: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
    for class in [class_id::SPRITE, class_id::TEXTURE_2D] {
        for (fi, sf) in files.iter().enumerate() {
            for (oi, obj) in sf.objects.iter().enumerate() {
                if obj.class_id != class {
                    continue;
                }
                let name = sf.peek_name(obj).map_err(unity_err)?.unwrap_or_default();
                if let Some(stem) = want.get(&name.to_lowercase()) {
                    out.entry(stem.clone()).or_default().push((fi, oi));
                }
            }
        }
    }
    Ok(out)
}

/// Bytes of decoded images in flight across the worker threads (a 4096x4096 hab module render
/// takes about 150 MB while it is decoded, cut out and encoded).
const MEMORY_BUDGET: u64 = 1 << 30;

/// A counting semaphore over bytes, keeping peak memory bounded whatever the core count.
struct Budget {
    free: Mutex<u64>,
    freed: Condvar,
}

struct Permit<'a> {
    budget: &'a Budget,
    bytes: u64,
}

impl Budget {
    fn new() -> Self {
        Self {
            free: Mutex::new(MEMORY_BUDGET),
            freed: Condvar::new(),
        }
    }

    fn acquire(&self, bytes: u64) -> Permit<'_> {
        let bytes = bytes.min(MEMORY_BUDGET);
        let mut free = self.freed.wait_while(self.free.lock().unwrap(), |free| *free < bytes).unwrap();
        *free -= bytes;
        Permit { budget: self, bytes }
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        *self.budget.free.lock().unwrap() += self.bytes;
        self.budget.freed.notify_all();
    }
}

/// Peak bytes of decoding `tex` and cutting an image out of it.
fn decode_cost(tex: &Texture2D) -> u64 {
    let pixels = u64::from(tex.width) * u64::from(tex.height);
    tex.mip0_size().unwrap_or(0) as u64 + pixels * 4 * 2
}

/// The image of a sprite or texture object, with the memory permit it holds.
fn object_image<'b>(sf: &SerializedFile, oi: usize, budget: &'b Budget) -> ti_unity::Result<(Image, Permit<'b>)> {
    let obj = &sf.objects[oi];
    if obj.class_id == class_id::SPRITE {
        let sprite = Sprite::from_value(&sf.read(obj)?, sf)?;
        let tex = sf.object(sprite.texture).map(|o| Texture2D::read(sf, o)).transpose()?;
        let permit = budget.acquire(tex.as_ref().map_or(0, decode_cost));
        Ok((sprite.image(sprite.source_image(sf)?)?, permit))
    } else {
        let tex = Texture2D::read(sf, obj)?;
        let permit = budget.acquire(decode_cost(&tex));
        Ok((tex.image(sf)?, permit))
    }
}

struct Step<'a, 'b> {
    opts: &'a mut AssetOptions<'b>,
    done: u64,
    total: u64,
}

impl Step<'_, '_> {
    fn log(&mut self, message: String) {
        (self.opts.progress)(Progress::Log { message });
    }

    fn advance(&mut self, n: u64, label: &str) {
        self.done += n;
        (self.opts.progress)(Progress::Step {
            done: self.done,
            total: self.total,
            label: Some(label.to_owned()),
        });
    }
}

fn export_bundle(bundle: &Bundle, wanted: &BTreeMap<String, Vec<String>>, out_dir: &Path, step: &mut Step<'_, '_>) -> Result<BTreeMap<String, Value>> {
    let format = step.opts.image_format;
    let cancel = step.opts.cancel;
    let unity = Arc::new(UnityBundle::open(&bundle.path).map_err(unity_err)?);
    let files = SerializedFile::all(&unity).map_err(unity_err)?;
    let want: BTreeMap<String, String> = wanted.keys().map(|s| (s.to_lowercase(), s.clone())).collect();
    let todo = candidates(&files, &want)?;
    let unfound = wanted.keys().filter(|s| !todo.contains_key(*s)).count() as u64;
    if unfound > 0 {
        step.advance(unfound, &bundle.name);
    }
    let mut exported = BTreeMap::new();
    let mut failures = Vec::new();
    let budget = Budget::new();
    let work = |(stem, cands): (String, Vec<(usize, usize)>)| -> (String, Result<Option<Value>>, Vec<String>) {
        let mut errors = Vec::new();
        for (fi, oi) in cands {
            let sf = &files[fi];
            match object_image(sf, oi, &budget) {
                Ok((image, _permit)) => {
                    let rel = format!("{}/{}", bundle.name, file_name(&stem, format));
                    let r = save(&image, &out_dir.join(&rel), format).map(|()| Some(image_meta(rel, &image)));
                    return (stem, r, errors);
                }
                // An undecodable object: log it and try the next one of that name.
                Err(e) => errors.push(format!(
                    "  ! {}/{}: {e}",
                    bundle.name,
                    sf.peek_name(&sf.objects[oi]).ok().flatten().unwrap_or_default()
                )),
            }
        }
        (stem, Ok(None), errors)
    };
    let items: Vec<_> = todo.into_iter().collect();
    parallel(items, cancel, work, |(stem, r, errors)| {
        failures.extend(errors);
        if let Some(meta) = r? {
            exported.insert(stem, meta);
        }
        step.advance(1, &bundle.name);
        Ok(())
    })?;
    failures.sort();
    for f in failures {
        step.log(f);
    }
    Ok(exported)
}

/// Export `resources` to `out_dir`.
pub fn extract(resources: &BTreeSet<String>, bundles: &BTreeMap<String, Bundle>, out_dir: &Path, opts: &mut AssetOptions<'_>) -> Result<AssetResult> {
    let (plan, missing) = plan(resources, bundles);
    let mut result = AssetResult { missing, ..Default::default() };
    let total = plan.iter().filter(|(b, _)| !opts.skip_bundles.contains(b)).map(|(_, s)| s.len() as u64).sum();
    let format = opts.image_format;
    let previous = opts.previous;
    let mut step = Step { opts, done: 0, total };
    for (bname, wanted) in &plan {
        if step.opts.cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let bundle = &bundles[bname];
        if step.opts.skip_bundles.contains(bname) {
            result.bundles.insert(bname.clone(), json!({"skipped": true}));
            continue;
        }
        let digest = sha256_file(&bundle.path)?;
        let wanted_set: BTreeSet<&str> = wanted.keys().map(String::as_str).collect();
        let mut exported = None;
        for prev in previous {
            let Some(p) = prev.index.get("bundles").and_then(|b| b.get(bname)) else {
                continue;
            };
            let have: BTreeSet<&str> = p
                .get("exported")
                .and_then(Value::as_object)
                .map(|m| m.keys().map(String::as_str).collect())
                .unwrap_or_default();
            if p.get("sha256").and_then(Value::as_str) == Some(digest.as_str()) && have.is_superset(&wanted_set) {
                exported = reuse(&prev.dir, p, out_dir, &wanted_set, format);
                if let Some(e) = &exported {
                    step.log(format!("  = {bname}: reused {} images", e.len()));
                    step.advance(wanted.len() as u64, bname);
                    break;
                }
            }
        }
        let exported = match exported {
            Some(e) => e,
            None => {
                let file = bundle.path.file_name().unwrap_or_default().to_string_lossy();
                step.log(format!("  + {bname}: exporting {} images from {file}", wanted.len()));
                export_bundle(bundle, wanted, out_dir, &mut step)?
            }
        };
        let mut resource_images: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
        for (stem, res_list) in wanted {
            for res in res_list {
                match exported.get(stem) {
                    Some(meta) => resource_images.entry(res).or_default().push(meta),
                    None => result.missing.push(res.clone()),
                }
            }
        }
        for (res, mut images) in resource_images {
            images.sort_by(|a, b| a["file"].as_str().cmp(&b["file"].as_str()));
            let mut entry = images[0].clone();
            if images.len() > 1 {
                entry["variants"] = images.iter().map(|i| i["file"].clone()).collect();
            }
            result.resources.insert(res.to_owned(), entry);
        }
        let exported: Map<String, Value> = exported.into_iter().collect();
        result.bundles.insert(bname.clone(), json!({"sha256": digest, "exported": exported}));
    }
    let missing: BTreeSet<String> = result.missing.drain(..).filter(|r| !result.resources.contains_key(r)).collect();
    result.missing = missing.into_iter().collect();
    Ok(result)
}

/// PIL crop box of a TMP glyph rect (Unity measures y from the bottom).
pub fn glyph_box(x: i64, y: i64, width: i64, height: i64, sheet_height: i64) -> (i64, i64, i64, i64) {
    let top = sheet_height - y - height;
    (x, top, x + width, top + height)
}

/// The glyphs of the TextMeshPro sprite assets in `bundle`: (name, image) in export order.
fn inline_sprite_glyphs(bundle: &Bundle) -> Result<Vec<(String, Image)>> {
    let unity = Arc::new(UnityBundle::open(&bundle.path).map_err(unity_err)?);
    let files = SerializedFile::all(&unity).map_err(unity_err)?;
    let mut glyphs: Vec<(String, Image)> = Vec::new();
    let mut seen = BTreeSet::new();
    for sf in &files {
        for obj in sf.objects.iter().filter(|o| o.class_id == class_id::MONO_BEHAVIOUR) {
            let fields: BTreeSet<&str> = sf.tree(obj).field_names().collect();
            if !fields.contains("m_SpriteCharacterTable") || !fields.contains("m_GlyphTable") {
                continue;
            }
            let Ok(tree) = sf.read(obj) else { continue };
            let Some(sheet_id) = tree.get("spriteSheet").and_then(|s| s.get("m_PathID")).and_then(ti_unity::Value::as_i64) else {
                continue;
            };
            let Some((sheet_sf, sheet_obj)) = files.iter().rev().find_map(|f| f.object(sheet_id).map(|o| (f, o))) else {
                continue;
            };
            let sheet = Texture2D::read(sheet_sf, sheet_obj).and_then(|t| t.image(sheet_sf)).map_err(unity_err)?;
            let mut rects = BTreeMap::new();
            for g in tree.get("m_GlyphTable").and_then(ti_unity::Value::as_array).unwrap_or_default() {
                let (Some(index), Some(rect)) = (g.get("m_Index").and_then(ti_unity::Value::as_i64), g.get("m_GlyphRect")) else {
                    continue;
                };
                rects.insert(index, rect);
            }
            for c in tree.get("m_SpriteCharacterTable").and_then(ti_unity::Value::as_array).unwrap_or_default() {
                let name = c.get("m_Name").and_then(ti_unity::Value::as_str).unwrap_or("");
                let rect = c.get("m_GlyphIndex").and_then(ti_unity::Value::as_i64).and_then(|i| rects.get(&i));
                let Some(rect) = rect else { continue };
                let v = |k| rect.get(k).and_then(ti_unity::Value::as_i64).unwrap_or(0);
                let (w, h) = (v("m_Width"), v("m_Height"));
                if name.is_empty() || seen.contains(name) || w <= 0 || h <= 0 {
                    continue;
                }
                let (x0, y0, x1, y1) = glyph_box(v("m_X"), v("m_Y"), w, h, i64::from(sheet.height));
                seen.insert(name.to_owned());
                glyphs.push((name.to_owned(), sheet.crop(x0, y0, x1, y1)));
            }
        }
    }
    Ok(glyphs)
}

/// Export the inline sprite glyphs to `sprites/`; the `sprites` value of `assets/index.json`,
/// or `None` without the `ui` bundle.
pub fn extract_inline_sprites(bundles: &BTreeMap<String, Bundle>, out_dir: &Path, opts: &mut AssetOptions<'_>) -> Result<Option<Value>> {
    let Some(bundle) = bundles.get(INLINE_SPRITE_BUNDLE) else { return Ok(None) };
    let digest = sha256_file(&bundle.path)?;
    let format = opts.image_format;
    for prev in opts.previous {
        let Some(p) = prev.index.get("sprites").filter(|p| p.is_object()) else {
            continue;
        };
        if p.get("sha256").and_then(Value::as_str) != Some(digest.as_str()) {
            continue;
        }
        let names: BTreeSet<&str> = p
            .get("exported")
            .and_then(Value::as_object)
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default();
        if let Some(exported) = reuse(&prev.dir, p, out_dir, &names, format) {
            (opts.progress)(Progress::Log {
                message: format!("  = inline sprites: reused {} images", exported.len()),
            });
            let exported: Map<String, Value> = exported.into_iter().collect();
            return Ok(Some(json!({"sha256": digest, "exported": exported})));
        }
    }
    let file = bundle.path.file_name().unwrap_or_default().to_string_lossy();
    (opts.progress)(Progress::Log {
        message: format!("  + inline sprites: exporting from {file}"),
    });
    let glyphs = inline_sprite_glyphs(bundle)?;
    let mut step = Step {
        done: 0,
        total: glyphs.len() as u64,
        opts,
    };
    let mut exported = Map::new();
    let work = |(name, image): (String, Image)| {
        let rel = format!("{INLINE_SPRITE_DIR}/{}", file_name(&name, format));
        let r = save(&image, &out_dir.join(&rel), format).map(|()| image_meta(rel, &image));
        (name, r)
    };
    let cancel = step.opts.cancel;
    parallel(glyphs, cancel, work, |(name, r)| {
        exported.insert(name, r?);
        step.advance(1, INLINE_SPRITE_BUNDLE);
        Ok(())
    })?;
    Ok(Some(json!({"sha256": digest, "exported": exported})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_assets_fold_continuations_and_unescape() {
        let text = "ManifestFileVersion: 0\nAssets:\n- Assets/UI/ICO_a.png\n- Assets/Long/path\n  continued.png\n- \"Assets/caf\\xE9.png\"\n- 'Assets/it''s.tga'\nDependencies: []\n- Assets/after.png\n";
        assert_eq!(
            manifest_assets(text),
            ["Assets/UI/ICO_a.png", "Assets/Long/path continued.png", "Assets/café.png", "Assets/it's.tga"]
        );
    }

    #[test]
    fn stems_and_suffixes() {
        assert_eq!(stem_suffix("Assets/a/ICO_x.PNG"), ("ICO_x", ".PNG"));
        assert_eq!(stem_suffix("Assets/.png"), (".png", ""));
        assert_eq!(stem_suffix("Assets/a.b.tga"), ("a.b", ".tga"));
    }

    #[test]
    fn file_names_are_sanitized() {
        assert_eq!(file_name("ICO_a+b&&c (1)", ImageFormat::Png), "ICO_a_b_c _1_.png");
        assert_eq!(file_name("Ünï", ImageFormat::Webp), "_n_.webp");
    }

    fn bundle(name: &str, images: &[&str]) -> (String, Bundle) {
        let images = images.iter().map(|s| (s.to_lowercase(), (*s).to_owned())).collect();
        (
            name.to_owned(),
            Bundle {
                name: name.to_owned(),
                path: PathBuf::new(),
                images,
            },
        )
    }

    #[test]
    fn plan_matches_exact_states_and_prefixes() {
        let bundles: BTreeMap<_, _> = [bundle("icons", &["ICO_a", "ICO_b_on", "ICO_b_off", "ICO_b_x_y", "PRE_1", "PRE_2"])].into();
        let resources: BTreeSet<String> = ["icons/ico_a", "Icons/sub/ICO_B", "icons/pre_", "icons/none"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let (plan, missing) = plan(&resources, &bundles);
        let icons = &plan["icons"];
        assert_eq!(icons["ICO_a"], ["icons/ico_a"]);
        assert_eq!(icons["ICO_b_on"], ["Icons/sub/ICO_B"]);
        assert_eq!(icons["ICO_b_off"], ["Icons/sub/ICO_B"]);
        assert!(!icons.contains_key("ICO_b_x_y"));
        assert_eq!(icons["PRE_1"], ["icons/pre_"]);
        assert_eq!(missing, ["icons/none"]);
    }

    #[test]
    fn collect_resources_filters_by_bundle() {
        let bundles: BTreeMap<_, _> = [bundle("icons", &[])].into();
        let found = collect_resources(["Icons/x", "event:/icons/y", "http://icons/z", "other/w", "plain"], &bundles);
        assert!(found.contains("Icons/x") && found.contains("icons_2d/tech_xeno_icon"));
        assert_eq!(found.len(), 1 + EXTRA_RESOURCE_CATEGORIES.len());
    }

    #[test]
    fn glyph_box_flips_y() {
        assert_eq!(glyph_box(10, 20, 30, 40, 256), (10, 196, 40, 236));
    }

    // End-to-end runs against the installed game, for parity checks with the Python extractor:
    //
    //   TI_ASSET_OUT=/scratch/assets cargo test --release -p ti-extract export_game_images -- --ignored --nocapture
    //   TI_ASSET_OUT=/scratch/assets TI_ASSET_REFERENCE=data/<build>/assets \
    //     cargo test --release -p ti-extract compare_with_reference -- --ignored --nocapture
    //
    // The resource list comes from the reference `index.json` (`resources` + `missing`), so templates
    // are not needed. `TI_GAME_DIR` overrides the default Steam location. Both skip without their inputs.

    fn env_path(name: &str) -> Option<PathBuf> {
        std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
    }

    fn game_bundle_dirs() -> Option<Vec<PathBuf>> {
        let game = env_path("TI_GAME_DIR").or_else(|| Some(env_path("HOME")?.join(".local/share/Steam/steamapps/common/Terra Invicta")))?;
        let mut dirs = vec![game.join("TerraInvicta_Data/StreamingAssets/AssetBundles")];
        let mut dlc: Vec<PathBuf> = fs::read_dir(game.join("DLC_Content"))
            .ok()?
            .filter_map(|e| Some(e.ok()?.path().join("AssetBundles")))
            .filter(|d| d.is_dir())
            .collect();
        dlc.sort();
        dirs.extend(dlc);
        dirs[0].is_dir().then_some(dirs)
    }

    fn reference() -> Option<(PathBuf, Value)> {
        let dir = env_path("TI_ASSET_REFERENCE")?;
        let index = serde_json::from_slice(&fs::read(dir.join("index.json")).ok()?).ok()?;
        Some((dir, index))
    }

    #[test]
    #[ignore = "needs the installed game"]
    fn export_game_images() {
        let (Some(dirs), Some(out), Some((_, reference))) = (game_bundle_dirs(), env_path("TI_ASSET_OUT"), reference()) else {
            eprintln!("set TI_ASSET_OUT and TI_ASSET_REFERENCE (and install the game); skipping");
            return;
        };
        let bundles = read_bundle_manifests(&dirs).unwrap();
        let mut resources: BTreeSet<String> = reference["resources"].as_object().unwrap().keys().cloned().collect();
        resources.extend(reference["missing"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_owned()));
        let format = if std::env::var("TI_ASSET_FORMAT").as_deref() == Ok("webp") {
            ImageFormat::Webp
        } else {
            ImageFormat::Png
        };
        let cancel = AtomicBool::new(false);
        let mut progress = |p: Progress| {
            if let Progress::Log { message } = p {
                eprintln!("{message}");
            }
        };
        let mut opts = AssetOptions {
            skip_bundles: &[],
            previous: &[],
            image_format: format,
            progress: &mut progress,
            cancel: &cancel,
        };
        let start = std::time::Instant::now();
        let result = extract(&resources, &bundles, &out, &mut opts).unwrap();
        let sprites = extract_inline_sprites(&bundles, &out, &mut opts).unwrap();
        let mut index = json!({"resources": result.resources, "bundles": result.bundles, "missing": result.missing});
        if let Some(s) = sprites {
            index["sprites"] = s;
        }
        fs::write(out.join("index.json"), serde_json::to_vec_pretty(&index).unwrap()).unwrap();
        eprintln!("exported in {:.1} s", start.elapsed().as_secs_f64());
    }

    #[test]
    #[ignore = "needs an export and a reference extraction"]
    fn compare_with_reference() {
        let (Some(out), Some((ref_dir, reference))) = (env_path("TI_ASSET_OUT"), reference()) else {
            eprintln!("set TI_ASSET_OUT and TI_ASSET_REFERENCE; skipping");
            return;
        };
        // A WebP export is compared with a PNG reference by pixels, under the PNG names.
        let ours = fs::read_to_string(out.join("index.json")).unwrap().replace(".webp\"", ".png\"");
        let ours: Value = serde_json::from_str(&ours).unwrap();
        for key in ["resources", "bundles", "missing", "sprites"] {
            let same = ours[key] == reference[key];
            eprintln!("index.json {key}: {}", if same { "identical" } else { "DIFFERENT" });
            if !same && let (Some(a), Some(b)) = (ours[key].as_object(), reference[key].as_object()) {
                let diff: Vec<&String> = a.keys().chain(b.keys()).filter(|k| a.get(*k) != b.get(*k)).take(10).collect();
                eprintln!("  first differing keys: {diff:?}");
            }
        }
        let files = |dir: &Path| -> BTreeSet<PathBuf> {
            let mut out = BTreeSet::new();
            let mut stack = vec![dir.to_path_buf()];
            while let Some(d) = stack.pop() {
                for e in fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.file_name() != Some("index.json".as_ref()) {
                        out.insert(p.strip_prefix(dir).unwrap().with_extension("png"));
                    }
                }
            }
            out
        };
        let (a, b) = (files(&out), files(&ref_dir));
        let missing: Vec<_> = b.difference(&a).collect();
        let extra: Vec<_> = a.difference(&b).collect();
        let common: Vec<PathBuf> = a.intersection(&b).cloned().collect();
        let decode = |p: &Path| image::open(p).map(|i| (i.width(), i.height(), i.into_rgba8().into_raw()));
        let (mut identical, mut different) = (0, Vec::new());
        let cancel = AtomicBool::new(false);
        parallel(
            common,
            &cancel,
            |rel| {
                let mut ours = out.join(&rel);
                if !ours.is_file() {
                    ours.set_extension("webp");
                }
                let same = match (decode(&ours), decode(&ref_dir.join(&rel))) {
                    (Ok(x), Ok(y)) => x == y,
                    _ => false,
                };
                (rel, same)
            },
            |(rel, same)| {
                if same {
                    identical += 1;
                } else {
                    different.push(rel);
                }
                Ok(())
            },
        )
        .unwrap();
        different.sort();
        eprintln!(
            "images: {identical} identical, {} different, {} missing, {} extra",
            different.len(),
            missing.len(),
            extra.len()
        );
        for (label, list) in [("different", different.iter().collect::<Vec<_>>()), ("missing", missing), ("extra", extra)] {
            for p in list.iter().take(20) {
                eprintln!("  {label}: {}", p.display());
            }
        }
    }
}
