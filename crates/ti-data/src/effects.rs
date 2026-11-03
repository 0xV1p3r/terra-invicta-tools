//! Rendering effect descriptions from a [`View`] (wires [`crate::effect_text`] to the dataset).

use serde_json::Value;

use crate::dataset::EntityId;
use crate::effect_text::{EffectContext, EffectEnv, EffectSegment, describe_effect, describe_effect_segments};
use crate::model;
use crate::text::{Texts, field, to_plain};
use crate::view::View;

struct ViewEnv<'a, 'd> {
    view: &'a View<'d>,
    texts: &'a Texts,
}

impl EffectEnv for ViewEnv<'_, '_> {
    fn loc(&self, key: &str) -> Option<String> {
        self.texts.get(key).map(str::to_string)
    }

    fn template_display_name(&self, data_name: &str) -> Option<String> {
        // `Find<TIDataTemplate>(strValue, allowChild)`: any template; prefer records in scope.
        let ds = self.view.dataset();
        let candidates = ds.find_any(data_name);
        let id = candidates.iter().copied().find(|id| self.view.contains(*id)).or(candidates.first().copied())?;
        Some(self.view.name(self.texts, id))
    }

    fn trait_names_in_grouping(&self, grouping: i64) -> Vec<String> {
        self.view
            .entities(model::TRAIT)
            .filter(|id| self.view.record(*id).and_then(|r| r.get("grouping")).and_then(Value::as_i64) == Some(grouping))
            .map(|id| self.view.name(self.texts, id))
            .collect()
    }

    fn councilor_attribute_label(&self, attribute: &str) -> Option<String> {
        self.texts.get(&format!("UI.Global.{attribute}")).map(to_plain)
    }
}

impl View<'_> {
    /// Player-facing description of an effect, rendered like the game's tech/project
    /// tooltips (state-dependent slots use `ctx` or neutral placeholders). `None` if the
    /// game hides the effect; the raw localized text if formatting fails.
    pub fn effect_description(&self, texts: &Texts, id: EntityId, ctx: &EffectContext) -> Option<String> {
        if &*self.dataset().entity(id).template != model::EFFECT {
            return None;
        }
        let template = self.text_raw(texts, id, field::DESCRIPTION)?;
        let record = self.record(id)?;
        let env = ViewEnv { view: self, texts };
        let rendered = describe_effect(template, record, ctx, &env).unwrap_or_else(|_| template.to_string());
        let plain = to_plain(&rendered);
        let plain = plain.trim();
        (!plain.is_empty()).then(|| plain.to_string())
    }

    /// [`Self::effect_description`] split into text, substituted values and inline sprites
    /// (see [`describe_effect_segments`]). `None` if the game hides the effect.
    pub fn effect_segments(&self, texts: &Texts, id: EntityId, ctx: &EffectContext) -> Option<Vec<EffectSegment>> {
        if &*self.dataset().entity(id).template != model::EFFECT {
            return None;
        }
        let template = self.text_raw(texts, id, field::DESCRIPTION)?;
        let record = self.record(id)?;
        let env = ViewEnv { view: self, texts };
        let segments = describe_effect_segments(template, record, ctx, &env)
            .unwrap_or_else(|_| vec![EffectSegment::Text(to_plain(template).trim().to_string())]);
        let visible = segments.iter().any(|s| !matches!(s, EffectSegment::Text(t) if t.trim().is_empty()));
        visible.then_some(segments)
    }
}
