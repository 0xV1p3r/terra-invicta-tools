//! `ScopeArgs` (frontend) → `ti_data::Scope`.

use ti_data::{Dataset, Scope, model};

use crate::dto::ScopeArgs;
use crate::error::{CmdError, CmdResult};

pub fn resolve(ds: &Dataset, args: &ScopeArgs) -> CmdResult<Scope> {
    let mut scope = Scope::OVERVIEW;
    if let Some(name) = args.scenario.as_deref() {
        let s = ds.scenario_by_name(name).ok_or_else(|| CmdError::msg(format!("Unknown scenario {name:?}")))?;
        scope.scenario = Some(s.idx);
    }
    if let Some(name) = args.faction.as_deref() {
        let f = ds
            .find(model::FACTION, name)
            .ok_or_else(|| CmdError::msg(format!("Unknown faction {name:?}")))?;
        scope.faction = Some(f);
    }
    Ok(scope)
}
