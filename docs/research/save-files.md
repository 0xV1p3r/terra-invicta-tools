# Terra Invicta save structure and parser design

Investigated on 2026-11-02. The source directory was:

```text
/home/user/.local/share/Steam/steamapps/compatdata/1176470/pfx/drive_c/users/steamuser/Documents/My Games/TerraInvicta/Saves
```

## Recommendation

Build a read-only save importer that produces a small, versioned campaign snapshot. Join that snapshot to the existing static game dataset through template `dataName` strings. This is enough to mark completed research, selected research, paused projects, and projects actually unlocked for the player's faction. The saves already contain those facts; reconstructing the whole simulation is unnecessary.

For this repository, a separate Rust `ti-save` crate is the best fit. Keep save decoding independent of `ti-data`, and add an adapter that combines a snapshot with a scenario/faction-scoped `ti_data::View`. Keep campaign state separate from the reusable static dataset. Start with research tracking and explicit save selection, then add a local directory watcher.

The difficult parts are serializer compatibility, selecting the correct campaign and faction, and interpreting progress against matching template definitions. Reading gzip itself is straightforward.

## Evidence and limits

All 11 `.gz` files were decompressed and inspected. A separate validation agent repeated the container, ID, reference, version and research checks. A review agent reviewed this report and its proposed approach. The reproducible audit is [inspect_saves.py](inspect_saves.py), with results in [save-file-inventory.json](save-file-inventory.json). Run it with Python 3:

```sh
python3 docs/research/inspect_saves.py \
  '/home/user/.local/share/Steam/steamapps/compatdata/1176470/pfx/drive_c/users/steamuser/Documents/My Games/TerraInvicta/Saves' \
  > /tmp/save-file-inventory.json
```

The audit reads saves without modifying them. Its full-document Python decoder is an investigation tool, not the proposed production parser. It checks entity ID uniqueness, outer-key/inner-ID equality, FullSerializer reference resolution, and the single-human-player assumption in this corpus. It records raw progress and counts without copying full saves into the repository.

Observed file families:

| Files | Campaign start version | Latest save version | Scenario |
|---|---|---|---|
| `HF_CHINA_1.gz` | `0.4.90` | `0.4.90` | Field absent; time template `ModernDayStart` |
| `RE_US_1-1.gz` | `1.0.51` | `1.0.53a` | `2026Scenario` |
| Four `RE_ASIA_EU_*` manual saves and five automatic saves | `1.0.53b` | `1.0.53b` | `2026Scenario` |

The last row means the four Asia manual files, `RE_ASIA_EU_1.gz`, `RE_ASIA_EU_1-1.gz`, `RE_ASIA_EU_1-2.gz`, `RE_ASIA_EU_1-3.gz`, and the five automatic files, `Autosave.gz`, `Autosave2.gz`, `Autosave3.gz`, `CombatAutosave.gz`, `ExitSave.gz`.

Compressed sizes range from 1,033,716 to 2,607,187 bytes. Decompressed sizes range from 24,250,585 to 78,020,738 bytes, approximately 23.1 to 74.4 MiB. `ExitSave.gz` has 69 type buckets and 11,975 entity records. `ExitSave.gz` and `RE_ASIA_EU_1-3.gz` have identical decompressed SHA-256 hashes. Filename differences do not necessarily identify different snapshots.

Interpretations were also checked against the existing local decompilation in `/tmp/ti-decompile`, documented as game version `1.0.53b`, Steam build `25216249`, in [game-semantics.md](game-semantics.md). Those files are temporary local evidence and are not repository dependencies. Source-derived behavior below refers to that version. Parsing common fields in `0.4.90` worked; this does not establish that all old-version research rules match the current code.

No DLC campaign, modded campaign, corrupt/truncated save, or uncompressed campaign save was present. These require additional fixtures. None of the inspected project progress records had `completed: true`, and no end-game category completion counter was positive. Their behavior below comes from source inspection.

## Container and document shape

Each campaign file is an ordinary gzip stream containing pretty-printed UTF-8 text with CRLF line endings. Ten have a UTF-8 BOM; the `0.4.90` file does not. Gzip decompression validates the stream checksum. No encryption, archive member selection, or binary object decoding was needed.

The root has two fields:

```json
{
  "currentID": { "value": 41170 },
  "gamestates": {
    "PavonisInteractive.TerraInvicta.TIPlayerState": [
      {
        "Key": { "value": 4970 },
        "Value": {
          "isAI": false,
          "faction": { "value": 4962 },
          "ID": { "value": 4970 },
          "templateName": "ResistPlayer",
          "$type": "PavonisInteractive.TerraInvicta.TIPlayerState"
        }
      }
    ]
  }
}
```

This is an excerpt of `ExitSave.gz`, with unrelated fields and records omitted. `currentID` is the game's ID allocation state, not a campaign identifier. `gamestates` is a dictionary from C# type names to dictionaries of game-state IDs and objects. Nonempty state dictionaries serialize as arrays of `Key`/`Value` pairs. Empty ones can be `{}`. Some type names are unqualified, such as `TIHistoricalData`.

Entity objects commonly include `ID`, `templateName`, `displayName`, `archived`, `exists`, and `$type`, plus type-specific fields. Examples include nations, regions, councilors, factions, habs, ships, research, events and AI goals. The type bucket set grows with campaign activity; it is not a fixed list shared by every save.

`PlayerOptions.TIProfile` is a line-oriented `key:value` configuration file. `steam_autocloud.vdf` is Steam metadata. Neither is a campaign snapshot or a source of research progress.

The current game also supports uncompressed text saves when compression is disabled, according to `GameStateManager.SaveAllGameStates` and `SaveStructure.Load`. Support these by detecting gzip magic bytes `1f 8b`, otherwise decoding text, rather than deciding solely from the extension. Plain-text support is source-confirmed but not exercised by this corpus.

### FullSerializer details that affect parsing

The game uses FullSerializer through `StringSerializationAPI`, plus `TIGameStateConverter`. These are not JSON.NET object graphs.

| Detail | Observed behavior | Parser requirement |
|---|---|---|
| Non-finite numbers | Nine saves contain bare `Infinity` or `-Infinity`, 50 occurrences in total. No observed `NaN`; the serializer supports it. | A strict whole-document JSON decoder needs a compatibility pass, even if these fields will be discarded. |
| Dictionary forms | String-keyed dictionaries are objects; complex-keyed dictionaries are `Key`/`Value` arrays; empty dictionaries can be `{}`. | Decode according to the expected field type. Do not globally convert every `{}` into `[]`. |
| Game-state links | Usually `{ "value": 4962 }`, sometimes with `$type`; can be `null`. ID zero is a sentinel in game code. | Keep a dedicated game-state reference type and resolve only required links. |
| Serializer object links | Definitions have string `$id`; references have string `$ref`. | This is a separate namespace from numeric game-state IDs. |
| Polymorphism | `$type` contains a C# type name. | Treat it as data; dispatch through an allowlist of supported records. |
| Missing and extra fields | Fields and buckets differ across versions. | Ignore unknown fields, validate required fields, and use explicit compatibility defaults for known omissions. |

In `ExitSave.gz`, there are 1,078 `$id` definitions and 1,508 `$ref` occurrences. Every referenced ID has a definition. These include shared nested objects, not just game entities. The validation agent found no `$id` or `$ref` within the player, faction, global research, time or metadata buckets in either `ExitSave.gz` or `HF_CHINA_1.gz`. An initial research importer can extract these records without rebuilding the general serializer graph. If a required value in a future save uses `$ref`, resolve it with an object-reference index or return an explicit unsupported-shape diagnostic. Never silently read it as an empty record.

Game-state IDs were globally unique within each inspected save, and every outer `Key.value` matched `Value.ID.value`. IDs are campaign-local and must not become permanent identifiers in the planning tool. FullSerializer `$id` values are serialization-local as well.

## Finding the player's campaign state

Use exact supported bucket names and validate their shapes. In the paths below, `S` means `root.gamestates`, and bucket names omit the common prefix `PavonisInteractive.TerraInvicta.` for readability. `[singleton]` means validate that there is one applicable record, rather than assuming that the first array element is always correct.

1. Read `S[TIPlayerState][*].Value` and select the record with `isAI == false`. Every inspected save has exactly one. If none or several exist, require a player/faction selection or report ambiguity.
2. Follow its `faction.value` to `S[TIFactionState][*].Key.value`.
3. Use that faction's `Value.templateName`, such as `ResistCouncil` or `DestroyCouncil`, as the static faction template key. Display names can be localized or customized.
4. Read `S[TITimeState][singleton].Value.currentDateTime` as structured game time. Avoid parsing `TIMetadataState.gameTimeString`, which is presentation text.
5. Read `scenarioMetaTemplateName` from the time record. It is `2026Scenario` in recent samples. The `0.4.90` sample omits it; the current game's bootstrap code falls back to `ModernScenario` for this omission. Record that the scenario was inferred.
6. Read versions and campaign provenance from `S[TIGlobalValuesState][singleton].Value`.

For `ExitSave.gz`, player 4970 points to faction 4962, `ResistCouncil`. In the HF save, player 4098 points to faction 4090, `DestroyCouncil`. Hardcoding either IDs or the first faction would select the wrong player in another campaign.

Useful `TIGlobalValuesState` fields include `campaignStartVersion`, `latestSaveVersion`, `realWorldCampaignStart`, `scenarioCustomizations`, `moddingActive`, and `moddingUsedAnytime`. Metadata adds `requiredDLC` and `playedWithMods`. Preserve these flags as provenance; they do not identify the exact contents of installed mods.

`realWorldCampaignStart` separates the three observed campaigns. Combine it with scenario and initial version as a grouping heuristic, and assign an application-owned campaign ID. There is no confirmed campaign UUID in the inspected envelope. Allow the user to correct grouping after copying or branching campaigns. Use a decompressed content hash for exact snapshot deduplication, not the filename or `currentID`.

## Research fields to extract

Let `G` be the singleton `TIGlobalResearchState.Value`, `F` the selected `TIFactionState.Value`, and `V` the singleton `TIGlobalValuesState.Value`.

| Path | What the importer can report |
|---|---|
| `G.finishedTechsNames[]` | Completed public technologies, shared across factions. |
| `G.finishedOneTimeOnlyProjectNames[]` | Projects completed globally under the one-time rule; useful both for uniqueness and prerequisite evaluation. This does not mean the player completed them. |
| `G.techProgress[i].techTemplateName` | Selected public technology in slot `i`, observed indices 0, 1, 2. |
| `G.techProgress[i].accumulatedResearch` | Total research points accumulated in that public slot. |
| `G.techProgress[i].factionContributions` | Game-state faction reference to contributed points, encoded as `Key`/`Value` pairs. |
| `G.techProgress[i].selector` | Faction associated with selecting the slot's current technology. Preserve separately from the current contribution leader. |
| `G.endGameTechsCompletedByCategory` | Repeat counts for end-game technology cost scaling. Absent in the old sample. |
| `G.useHarshTree` | Saved tree mode flag. Preserve for compatibility; all samples are false. |
| `F.finishedProjectNames[]` | Completed faction projects. Preserve duplicate entries as completion counts for repeatable projects. |
| `F.availableProjectNames[]` | Projects that have actually unlocked for this faction. Includes selected projects and can overlap completed repeatable projects. |
| `F.activeProjectTriggers[]` | Projects waiting on discovery/unlock rolls, with `projectTemplateName` and `monthlyTriggerValue`. These are not yet available. |
| `F.missedProjects[]` | Projects whose availability roll failed. The game can permit later retries. |
| `F.currentProjectProgress[]` | Selected or paused project progress, including `projectTemplateName`, `accumulatedResearch`, `slot`, `completed`. |
| `F.researchWeights[0..5]` | Allocation weights, not research points or completion percentages. |
| `F.orgProjectSlotUnlocked`, `F.habProjectSlotUnlocked` | Whether project slots 4 and 5 are enabled. |
| `F.techNameContributionHistory` | Historical faction contribution fractions by tech name, useful for unlock explanations; not the global completion list. |
| `F.objectiveNames`, `F.milestones` | Objective status map and reached milestones for explaining project requirements. |
| `F.hiddenProjects`, `F.favoredProjects` | Player/UI preferences. Hidden does not mean unavailable. |
| `V.scenarioCustomizations` | Numeric `researchSpeedMultiplier`, `variableProjectUnlocks`, `showTriggeredProjects`, and other campaign options. |

### Active and paused research

Public technology slots are indices 0 to 2 in `techProgress`. Project records carry explicit `slot` values and need not be stored in slot order:

- Slot 3 is the base faction project slot.
- Slot 4 is enabled by `orgProjectSlotUnlocked`.
- Slot 5 is enabled by `habProjectSlotUnlocked`.
- Slots 6 and above hold paused projects with retained research. They are not extra active slots.
- Slots 4 or 5 are also paused while the corresponding unlock flag is false.

Keep `selected`, `paused`, `completed`, and `weight` as separate facts. A selected project with weight zero is not receiving allocated research. A `completed: true` progress entry must not be presented as an unfinished selected project. Completion history remains a separate list, particularly for repeatable research.

In `ExitSave.gz`, the Resistance has 58 completed global tech entries, 74 completed project entries and 113 available projects. Its current public slots are `NuclearFusioninSpace` at 28349.14 points, `IndustrializationofSpace` at 10384.84, and `QuantumEncryption` at 279.4355. The five project progress records consist of three enabled selections in slots 3, 4 and 5, plus paused `Project_ManagementResearch` in slot 6 and `Project_XenoscienceResearchCenter` in slot 7. These are useful initial acceptance cases.

### Research cost and completion percentages

Saves store accumulated points. Costs come from the matching scenario-resolved templates and campaign settings. In the inspected current code:

```text
speed = V.scenarioCustomizations.researchSpeedMultiplier
ordinary public tech cost = template.researchCost / speed
end-game tech cost = template.researchCost * (1 + categoryCompletionCount) / speed
ordinary project cost = template.researchCost / speed
repeatable project cost = template.researchCost * (1 + factionCompletionCount) / speed
completion fraction = accumulatedResearch / effectiveCost
```

The numeric campaign multiplier is preferable to metadata text such as `"200%"`. All Asia saves use 1.0; the HF and US saves use 2.0. This setting changes the cost denominator in current code. Do not apply it a second time to accumulated points.

For example, current base `NuclearFusioninSpace` costs 50000 points at speed 1.0. `ExitSave.gz` therefore records about 56.70% completion. `Project_NationalLabs` costs 10000 and has 9573.105 points, about 95.73%. These calculations use the current matching scenario definitions. Keep raw values and mark the effective cost as unavailable when definitions or required settings are unresolved. Handle zero-cost templates without division by zero, and preserve raw fractions if a UI clamps a displayed bar.

Do not deduplicate `finishedProjectNames` before counting repeats. `TIFactionState.AddCompletedProject` appends each completion, and `TIProjectTemplate.GetResearchCost` counts those completions. End-game public research instead uses the category counters. The old save's absent counters should be represented as unavailable in raw state; any zero default belongs in a documented version adapter.

Exact completion dates require the game's research-income calculations and future allocations. A first importer should report saved points and percentages. A later estimator can use changes across compatible snapshots, clearly labeled as an estimate, or port the relevant income/effect rules.

### Availability is more than prerequisite completion

For planning, retain independent facts rather than forcing every project into one mutually exclusive enum:

```text
completion_count
available_in_save
selected_slot
paused
allocated_weight
trigger_pending + monthly_trigger_value
missed_availability_roll
hidden_by_player
prerequisites_satisfied + unresolved_requirements
blocked_by_global_uniqueness
```

Then derive presentation states. A completed repeatable project can also be available and selected again. Missing a project from `availableProjectNames` does not establish that its prerequisites are unsatisfied: it might be awaiting a roll, have failed a roll, require a milestone, or be excluded for another reason.

Use saved availability as the authority for what has actually unlocked. Use `ResearchGraph::requirements`, completed techs, completed player projects, and applicable globally completed one-time projects to explain the prerequisite chain. The graph describes static dependencies; it does not evaluate all runtime availability conditions. Respect faction restrictions, objective/milestone requirements, nation requirements and global uniqueness. The detailed current rules are in [game-semantics.md, section 3](game-semantics.md#3-tech-and-project-prerequisites-and-availability).

The initial snapshot below omits nation state. Nation-existence requirements therefore remain unknown in derived explanations until a nation adapter supplies the state needed to evaluate them. Preserve that uncertainty instead of claiming that every runtime requirement passed. Saved `availableProjectNames` still provides direct evidence of actual unlocks.

`monthlyTriggerValue` is a rolling unlock parameter, not progress toward researching the project. `showTriggeredProjects` also matters when deciding whether to display pending projects. Keep other factions' project details out of the default player view, and apply the game's visibility rules before revealing undiscovered project names. Full saves contain information beyond what the player can currently see.

## Joining the existing static data

This repository already extracts templates into `data/<SteamBuildId>/` and exposes them through `crates/ti-data`. Reuse that work:

1. Select a compatible extracted dataset. Saves contain game-version strings; the inspected envelope does not supply a confirmed Steam build ID. Maintain an explicit version-to-dataset mapping and record the evidence used. The current manifest has build ID `25216249` but does not itself include a game version, so this association currently comes from the local code investigation.
2. Resolve the save's scenario with `Dataset::scenarios`, then construct `Scope::scenario(...).with_faction(...)`. Do not use `overview()` for live campaign rules; it can select a different scenario's record.
3. Resolve public names through `View::find(model::TECH, name)` and project names through `View::find(model::PROJECT, name)`. Resolve the faction through `model::FACTION`. Use `(template type, dataName)` as the stable external key.
4. Preserve names that do not resolve, alongside diagnostics. `EntityId` is an index local to one loaded dataset, not a stable save or storage key.
5. Decorate the static graph with snapshot facts, and calculate derived requirements/costs in an adapter. Do not overwrite templates or bake one campaign's completion state into `Dataset`.

The examined research names in HF, US and ExitSave all resolve somewhere in the current templates when checking the union of layers. That proves name continuity for those checks, not scenario correctness or unchanged costs and prerequisites. An older save should still import raw progress when its exact template dataset is unavailable, while derived planning results carry a compatibility warning. Never silently substitute the latest definitions as an exact match.

The save includes some nested template objects, for example inside scenario customization data. Their presence does not make the save a complete replacement for the extracted static dataset. Resolving exact modded rules will need matching mod-aware extraction or a separately validated source of effective definitions.

## Proposed importer boundaries

### 1. Decode a stable file snapshot

Accept a path or byte stream and detect compression. For automatic tracking, debounce directory notifications and check file identity, size and modification time before and after reading. Retry on change or decode failure after a short bounded delay. The game's compressed save writer uses `File.Create` and writes directly, so a watcher can observe a partially written file. Publish a new snapshot only after decompression, parsing and required-field validation succeed. Keep the previous valid snapshot and report a failed refresh.

Enforce configurable compressed/decompressed size limits with enough headroom above the observed 78 MB, and return a specific size-limit error. Hash the decoded bytes for exact deduplication. For filenames containing equivalent logical data but different BOM/formatting, a normalized snapshot hash can provide a second level of deduplication.

### 2. Handle FullSerializer syntax before strict JSON decoding

Strip an optional BOM, then use a string-aware lexical pass that recognizes the bare number tokens `Infinity`, `-Infinity`, and `NaN` only outside quoted strings and at valid token boundaries. Preserve their location and token in diagnostics. A practical Rust approach is to replace these tokens with `null`, then run `serde_json`. Maintain a source-offset map if diagnostics need original byte positions. Whitespace padding can preserve the length of the Infinity forms, but `NaN` is shorter than `null`. Never use a global text replacement: descriptions and other strings can contain the same words.

Discarded simulation fields may contain these normalized nulls. A non-finite value in required research progress must produce an explicit field error or unknown value, never zero. Normalize only this known serializer extension, not arbitrary malformed JSON. Test escaped quotes, escaped backslashes, EOF boundaries, and tokens crossing input chunks if the pass becomes streaming.

### 3. Deserialize selected state records

Use typed envelopes and small structs for metadata, time, global values, player, faction and global research records. Implement a reusable dictionary decoder that accepts `Key`/`Value` arrays and empty `{}` for the relevant dictionary types. Reject duplicate keys rather than silently keeping the last. Validate bucket and record types, integer IDs, singleton cardinality, key/ID agreement and required faction links. Use signed integer handling consistent with the game's `int` IDs.

Start with bounded decompressed bytes and selective Serde deserialization, skipping unknown fields and unrelated buckets with `IgnoredAny`. This avoids constructing a whole 78 MB document as `serde_json::Value`, with a much larger heap of strings and objects. The compatibility pass is still necessary because skipped JSON values must be syntactically valid. Measure peak memory and latency on the corpus before choosing full streaming decompression/tokenization. Memory and timing were not benchmarked in this investigation.

Unknown optional fields should not break import. Missing completion lists, unresolved player links, or malformed required progress should not silently turn into an empty campaign. Distinguish absent fields, explicit nulls, empty collections and known version defaults. Add support for general `$id`/`$ref` resolution only when needed by extracted fields.

### 4. Normalize independently of template resolution

Suggested public types, not implemented by this report:

```text
SaveSnapshot {
  snapshot_schema_version,
  source { path, content_hash, observed_at },
  game { campaign_start_version, latest_save_version, required_dlc, mod_flags },
  campaign { app_campaign_id, real_world_start, scenario, scenario_inferred, game_time },
  player { faction_template_name },
  options { research_speed_multiplier, variable_unlocks, show_triggered_projects },
  global_research { completed_names, unique_project_names, slots, endgame_counts },
  faction_research { completion_counts, available_names, project_progress,
                     unlock_triggers, missed_names, weights, slot_unlocks,
                     objectives, milestones, hidden_names, favored_names },
  diagnostics
}
```

Retain game-state IDs internally for links and contribution maps, but translate faction references to faction template names in the portable snapshot where possible. Keep unresolved original references in diagnostics. Persist template strings and raw points even if enrichment fails. Give the normalized snapshot its own schema version, independent of game version and static dataset format.

### 5. Integrate with planning and refresh

An adapter combines `SaveSnapshot` with a compatible `ti-data` view and produces node status, completion percentage, remaining points and unmet requirements. Keep selected-but-zero-weight projects distinguishable from funded research. Emit changes such as newly completed research, availability changes, selection changes and progress updates relative to the previous accepted snapshot.

For a desktop/local tool, read the save directory directly through a local process. The nested Svelte application could consume normalized snapshots through a local helper or desktop bridge. A hosted browser application needs explicit file/directory access or a local companion; it cannot silently read the Steam directory. Initial manual import can use file selection, with automatic refresh added where the deployment permits it.

Pin the watched campaign or selected file. Do not pick the newest file in the whole folder on every refresh: this folder contains multiple campaigns, and Steam synchronization or a copied file can change filesystem times. Detect game-time rollback within the same campaign and offer to follow a new branch instead of assuming progress can only increase. Multiple saves can also share the same game time while containing different orders and research points.

## Implementation order and acceptance checks

1. Implement the read-only Rust decoder and raw research snapshot. Add gzip support, BOM handling, the non-finite-token compatibility pass, dictionary decoding, player selection and version/provenance extraction.
2. Validate against every local save and small synthetic fixtures. The existing audit provides reference values, but keep a separate expected-results fixture so tests do not merely compare two paths through the same implementation. Do not require personal full saves in CI.
3. Add the `ti-data` adapter, compatible-dataset selection, scenario/faction resolution, repeat costs and graph decoration.
4. Add manual import to the planning tool, followed by campaign-pinned watching and snapshot diffs.
5. Add research-rate estimates and other domains only after current-state extraction is reliable. Nations/control points, resources, habs and fleets already have their own buckets for later adapters.

Meaningful parser tests should cover:

- The HF save's missing scenario field, no BOM, zero completed global techs, and a different human faction.
- Empty `{}` buckets and dictionaries, optional absent buckets, shuffled bucket and record ordering, and duplicate/mismatched IDs.
- Non-finite tokens in discarded and required fields, plus quoted strings containing the same text.
- Distinct serializer `$id`/`$ref` and numeric game-state IDs; references in a newly required field must resolve or produce an unsupported-shape error.
- Paused slots 6/7, disabled slots 4/5, zero allocation weights, and `completed: true` progress entries.
- Duplicate finished project names for repeat counts, repeated end-game research, zero-cost templates and speed 2.0 cost scaling.
- Pending triggers, failed rolls, hidden-but-available projects and a one-time project completed by another faction.
- Unknown template names, unavailable matching datasets, mods, and inferred scenario provenance.
- Truncated gzip, invalid text, decompression limits, write-in-progress retries, exact duplicate snapshots and game-time rollback.

The first useful milestone is small: import `ExitSave.gz`, identify the Resistance in `2026Scenario`, mark 58 global techs and 74 project completion entries, expose 113 available projects, and classify its three selected and two paused projects correctly. That supplies the tech planner with current progress without requiring a simulation engine.

## Source map

The inventory records direct save observations. The following inspected methods explain behavior; paths are relative to `/tmp/ti-decompile`:

| Source | Relevant behavior |
|---|---|
| `PavonisInteractive.TerraInvicta/SaveStructure.cs`, `Load` | Envelope, gzip/plain decoding, legacy `exists` compatibility. |
| `PavonisInteractive.TerraInvicta/GameStateManager.cs`, `SaveAllGameStates` | FullSerializer output, compression setting, direct file writes. |
| `StringSerializationAPI.cs` | Serializer entry points and converter reset. |
| `FullSerializer.Internal/fsDictionaryConverter.cs`, `TrySerialize` | Object versus `Key`/`Value` dictionary forms. |
| `FullSerializer/fsJsonPrinter.cs`, `ConvertDoubleToString`; `fsJsonParser.cs`, `TryParseNumber` | Non-finite numeric tokens. |
| `PavonisInteractive.TerraInvicta/TIGameStateConverter.cs` | Full state records versus nested game-state ID references. |
| `PavonisInteractive.TerraInvicta/GameStateID.cs` | Integer IDs and zero sentinel. |
| `PavonisInteractive.TerraInvicta.Systems.Bootstrap/SolarSystemBootstrap.cs` | Old-save fallback to `ModernScenario`. |
| `PavonisInteractive.TerraInvicta/TIGlobalResearchState.cs` | Public slots, completion names, repeat counters and selector assignment. |
| `PavonisInteractive.TerraInvicta/TechProgress.cs` | Accumulated research, contributions and progress fraction. |
| `ProjectProgress.cs`, `ProjectTrigger.cs` | Saved project progress and trigger fields. |
| `PavonisInteractive.TerraInvicta/TIFactionState.cs`, `ProjectPaused`, `SetProjectInSlot`, `AddCompletedProject`, `TotalResearchWeights` | Paused slots, repeat history and allocation weights. |
| `TITechTemplate.cs`, `TIProjectTemplate.cs`, `GetResearchCost` | Speed and repeat cost scaling. |
| `PavonisInteractive.TerraInvicta/TIGlobalValuesState.cs`, `GetResearchSpeedModifier` | Numeric customization multiplier. |
| Repository `crates/ti-data/src/{dataset,view,graph}.rs` and [game-semantics.md](game-semantics.md) | Template identities, scenario resolution and dependency semantics. |
