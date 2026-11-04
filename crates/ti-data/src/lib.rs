//! Query library for Terra Invicta game data extracted by `ti-extract`.
//!
//! ```no_run
//! use ti_data::{DataRoot, Scope, Texts, model};
//! let ds = DataRoot::open("data")?.latest()?;
//! let texts = Texts::load(&ds, "en")?;
//! let view = ds.overview(); // or ds.view(Scope::scenario(idx).with_faction(f))
//! let tech = view.find(model::TECH, "AdAstra").unwrap();
//! println!("{} unlocks {} things", view.name(&texts, tech), view.unlocks(tech).len());
//! # Ok::<(), ti_data::Error>(())
//! ```

pub mod dataset;
pub mod effect_text;
pub mod effects;
pub mod error;
pub mod graph;
pub mod manifest;
pub mod model;
pub mod refs;
pub mod scenario;
pub mod search;
pub mod source;
pub mod space;
pub mod text;
pub mod view;

#[cfg(test)]
mod test_support;

pub use dataset::{DataRoot, Dataset, Entity, EntityId, Layer, LayerIdx, Reference, Variant};
pub use effect_text::{EffectContext, EffectSegment};
pub use error::{Error, Result};
pub use graph::{PrereqGroup, ResearchGraph, Subgraph, Traversal};
pub use scenario::{Scenario, ScenarioIdx, ScenarioSet};
pub use search::{Hit, MatchField, SearchIndex};
pub use text::{Locale, Texts};
pub use view::{Annotations, Scope, Unlock, UnlockKind, View};
