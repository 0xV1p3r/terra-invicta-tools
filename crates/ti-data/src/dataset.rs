//! Loading an extracted build into an entity table.
//!
//! The game pools the records of all template files (base game and every DLC
//! folder). Every `(template, dataName)` pair is one [`Entity`]; it keeps each
//! raw record defining it as a [`Variant`], in load order. Which variant is
//! used depends on the scenario (see [`crate::scenario`]).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::error::{Error, Result};
use crate::manifest::{AssetEntry, AssetIndex, DataIndex, FORMAT_VERSION, Manifest, Schema};
use crate::scenario::{Resolution, Scenario};
use crate::source::{DataSource, DirSource, read_json};
use crate::text::Locale;

/// Index of an entity inside a [`Dataset`]. Only valid for the dataset that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub(crate) u32);

impl EntityId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Index into [`Dataset::layers`]. Layer 0 is always the base game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerIdx(pub(crate) u8);

impl LayerIdx {
    pub const BASE: LayerIdx = LayerIdx(0);

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A directory of template files: the base game or one DLC folder.
#[derive(Debug, Clone)]
pub struct Layer {
    pub idx: LayerIdx,
    pub id: String,
    pub is_dlc: bool,
    pub dlc: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub layer: LayerIdx,
    /// Position in the game's global load order.
    pub load_order: u32,
    /// `scenarioTags` of the record.
    pub tags: Vec<Arc<str>>,
    /// `disable: true` records are not registered by the game (unless a meta template lists them).
    pub disabled: bool,
    /// Later record with the same tag set as an earlier one; the game ignores it.
    pub shadowed: bool,
    /// The record exactly as extracted (a JSON object).
    pub record: Value,
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: EntityId,
    /// Template file name, e.g. `TITechTemplate`.
    pub template: Arc<str>,
    pub data_name: Arc<str>,
    /// All records defining this entity, in load order.
    pub variants: Vec<Variant>,
}

impl Entity {
    /// True if any variant comes from a DLC folder.
    pub fn has_dlc_variant(&self) -> bool {
        self.variants.iter().any(|v| v.layer != LayerIdx::BASE)
    }
}

/// A reference from one record to another, found at a field path such as `prereqs[]`.
#[derive(Debug, Clone)]
pub struct Reference {
    pub from: EntityId,
    /// Index into `Entity::variants` of the referencing record.
    pub variant: u16,
    pub path: Arc<str>,
    pub to: EntityId,
}

/// One extracted game build. Localization is loaded lazily per language.
pub struct Dataset {
    source: Box<dyn DataSource>,
    manifest: Manifest,
    layers: Vec<Layer>,
    entities: Vec<Entity>,
    /// template -> dataName -> entity
    by_key: HashMap<Arc<str>, HashMap<Arc<str>, EntityId>>,
    by_template: BTreeMap<Arc<str>, Vec<EntityId>>,
    by_data_name: HashMap<Arc<str>, Vec<EntityId>>,
    pub(crate) references: Vec<Reference>,
    pub(crate) refs_from: HashMap<EntityId, Vec<usize>>,
    pub(crate) refs_to: HashMap<EntityId, Vec<usize>>,
    pub(crate) scenarios: Vec<Scenario>,
    pub(crate) resolution: Resolution,
    assets: AssetIndex,
    locales: Mutex<HashMap<String, Arc<Locale>>>,
}

impl std::fmt::Debug for Dataset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dataset")
            .field("build", &self.manifest.game.build_id)
            .field("version", &self.manifest.game.version)
            .field("entities", &self.entities.len())
            .field("scenarios", &self.scenarios.len())
            .finish_non_exhaustive()
    }
}

/// A directory containing one sub-directory per extracted build plus `index.json`.
#[derive(Debug, Clone)]
pub struct DataRoot {
    pub path: PathBuf,
    pub index: DataIndex,
}

impl DataRoot {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let index = read_json(&DirSource::new(&path), "index.json")?;
        Ok(Self { path, index })
    }

    /// Open the most recently updated build.
    pub fn latest(&self) -> Result<Dataset> {
        let dir = self.index.latest.as_deref().ok_or(Error::NoBuilds)?;
        Dataset::open(self.path.join(dir))
    }

    /// Open a specific Steam build id (or build directory name).
    pub fn build(&self, build_id: &str) -> Result<Dataset> {
        let entry = self
            .index
            .builds
            .iter()
            .find(|b| b.build_id == build_id || b.dir == build_id)
            .ok_or_else(|| Error::UnknownBuild(build_id.into()))?;
        Dataset::open(self.path.join(&entry.dir))
    }
}

fn tags_of(record: &Value) -> Vec<Arc<str>> {
    record
        .get("scenarioTags")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(Arc::from).collect())
        .unwrap_or_default()
}

fn same_tag_set(a: &[Arc<str>], b: &[Arc<str>]) -> bool {
    a.len() == b.len() && a.iter().all(|t| b.contains(t)) && b.iter().all(|t| a.contains(t))
}

impl Dataset {
    /// Open an extracted build directory (`data/<buildId>`).
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Self::from_source(Box::new(DirSource::new(dir.as_ref())))
    }

    pub fn from_source(source: Box<dyn DataSource>) -> Result<Self> {
        let manifest: Manifest = read_json(source.as_ref(), "manifest.json")?;
        if manifest.format_version != FORMAT_VERSION {
            return Err(Error::FormatVersion { found: manifest.format_version, supported: FORMAT_VERSION });
        }

        let layers: Vec<Layer> = manifest
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| Layer {
                idx: LayerIdx(i as u8),
                id: l.id.clone(),
                is_dlc: l.kind != "base",
                dlc: l.dlc.clone(),
                folder: l.folder.clone(),
            })
            .collect();

        let mut ds = Dataset {
            source,
            layers,
            entities: Vec::new(),
            by_key: HashMap::new(),
            by_template: BTreeMap::new(),
            by_data_name: HashMap::new(),
            references: Vec::new(),
            refs_from: HashMap::new(),
            refs_to: HashMap::new(),
            scenarios: Vec::new(),
            resolution: Resolution::default(),
            assets: AssetIndex::default(),
            locales: Mutex::new(HashMap::new()),
            manifest,
        };

        let mut load_order = 0u32;
        let layer_templates: Vec<(LayerIdx, String, Vec<String>)> = ds
            .manifest
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| (LayerIdx(i as u8), l.id.clone(), l.templates.clone()))
            .collect();
        for (layer, layer_id, templates) in layer_templates {
            for template in templates {
                let rel = format!("templates/{layer_id}/{template}.json");
                let records: Vec<Value> = read_json(ds.source.as_ref(), &rel)?;
                for record in records {
                    ds.add_record(layer, load_order, &template, record);
                    load_order += 1;
                }
            }
        }

        let schema: Schema = read_json(ds.source.as_ref(), "schema.json")?;
        crate::refs::build(&mut ds, &schema);
        crate::scenario::build(&mut ds);

        if ds.manifest.has_assets {
            ds.assets = read_json(ds.source.as_ref(), "assets/index.json")?;
        }
        Ok(ds)
    }

    fn add_record(&mut self, layer: LayerIdx, load_order: u32, template: &str, record: Value) {
        // The game skips records without a dataName.
        let Some(name) = record.get("dataName").and_then(Value::as_str) else { return };
        let template: Arc<str> = self.intern_template(template).unwrap_or_else(|| Arc::from(template));
        let data_name: Arc<str> = Arc::from(name);
        let id = match self.by_key.get(&*template).and_then(|m| m.get(&*data_name)) {
            Some(id) => *id,
            None => {
                let id = EntityId(self.entities.len() as u32);
                self.entities.push(Entity {
                    id,
                    template: template.clone(),
                    data_name: data_name.clone(),
                    variants: Vec::new(),
                });
                self.by_key.entry(template.clone()).or_default().insert(data_name.clone(), id);
                self.by_template.entry(template).or_default().push(id);
                self.by_data_name.entry(data_name).or_default().push(id);
                id
            }
        };
        let tags = tags_of(&record);
        let disabled = record.get("disable").and_then(Value::as_bool).unwrap_or(false);
        let entity = &mut self.entities[id.index()];
        // `TemplateManager.Add`: among registered (non-disabled) records, a later one with
        // the same tag set as an earlier one is ignored.
        let shadowed = !disabled && entity.variants.iter().any(|v| !v.disabled && !v.shadowed && same_tag_set(&v.tags, &tags));
        entity.variants.push(Variant { layer, load_order, tags, disabled, shadowed, record });
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn build_id(&self) -> &str {
        &self.manifest.game.build_id
    }

    /// Game version (e.g. `1.0.53b`), if the extractor could read it.
    pub fn game_version(&self) -> Option<&str> {
        self.manifest.game.version.as_deref()
    }

    pub fn languages(&self) -> &[String] {
        &self.manifest.languages
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn layer(&self, idx: LayerIdx) -> &Layer {
        &self.layers[idx.index()]
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn entity(&self, id: EntityId) -> &Entity {
        &self.entities[id.index()]
    }

    pub fn templates(&self) -> impl Iterator<Item = &str> {
        self.by_template.keys().map(|t| &**t)
    }

    /// All entities of a template, in first-load order, regardless of scenario.
    pub fn of_template(&self, template: &str) -> &[EntityId] {
        self.by_template.get(template).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn find(&self, template: &str, data_name: &str) -> Option<EntityId> {
        self.by_key.get(template)?.get(data_name).copied()
    }

    /// All entities with this dataName, across templates (a handful of names are shared).
    pub fn find_any(&self, data_name: &str) -> &[EntityId] {
        self.by_data_name.get(data_name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Outgoing references of an entity (all variants).
    pub fn references_from(&self, id: EntityId) -> impl Iterator<Item = &Reference> {
        self.refs_from.get(&id).into_iter().flatten().map(|i| &self.references[*i])
    }

    /// Incoming references to an entity (all variants).
    pub fn references_to(&self, id: EntityId) -> impl Iterator<Item = &Reference> {
        self.refs_to.get(&id).into_iter().flatten().map(|i| &self.references[*i])
    }

    /// Localization for a language, loaded on first use.
    pub fn locale(&self, lang: &str) -> Result<Arc<Locale>> {
        if let Some(l) = self.locales.lock().expect("locale cache poisoned").get(lang) {
            return Ok(l.clone());
        }
        if !self.manifest.languages.iter().any(|l| l == lang) {
            return Err(Error::UnknownLanguage(lang.into()));
        }
        let strings = read_json(self.source.as_ref(), &format!("locale/{lang}/strings.json"))?;
        let locale = Arc::new(Locale::new(lang, strings));
        self.locales.lock().expect("locale cache poisoned").insert(lang.into(), locale.clone());
        Ok(locale)
    }

    /// Image metadata for a resource string such as `shipbuildericons/ICO_SteelArmor`.
    pub fn asset(&self, resource: &str) -> Option<&AssetEntry> {
        self.assets.resources.get(resource)
    }

    /// Absolute path of an asset file, when the build is on disk.
    pub fn asset_path(&self, file: &str) -> Option<PathBuf> {
        self.source.root().map(|r| r.join("assets").join(file))
    }

    pub(crate) fn intern_template(&self, template: &str) -> Option<Arc<str>> {
        self.by_template.get_key_value(template).map(|(k, _)| k.clone())
    }
}
