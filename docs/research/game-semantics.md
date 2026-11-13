# Terra Invicta: data-loading semantics (from decompiled code)

Source: `Assembly-CSharp.dll` decompiled with ilspycmd 8.2 into `/tmp/ti-decompile` (not committed).
Game build: `1.0.53b` (Steam buildid 25216249), Unity 2020.3.49f1. Data counts are from this build.
Class names are the decompiled names. Files without a namespace live at the decompile root; the others are under `PavonisInteractive.TerraInvicta/`.

---

## 1. How DLC scenario templates are applied

**Mechanism: whole-record selection by `scenarioTags`. Fields are never merged.**

Loading happens in `TemplateManager.Initialize` → `RegisterFileBasedTemplates(templatePath)`:

```csharp
string[] files = Directory.GetFiles(templatePath, "*.json", SearchOption.AllDirectories);
if (Directory.Exists("DLC_Content"))
    second = Directory.GetFiles("DLC_Content", "*.json", SearchOption.AllDirectories);
array = files.Concat(second).OrderByDescending(x => x.Contains("TIMetaTemplate.json"));  // stable sort
```

- All DLC JSON files load at startup, **whether or not the DLC is owned or the scenario is chosen**. Base files load before DLC files. All `TIMetaTemplate.json` files load first.
- `ModTemplateManager.LoadJsonFromDLC`/`GetDLCForTemplate` exist but are never called. The JSON-patch merge (`JsonController.CombineJson`) applies only to user mods in `Mods/Enabled`, not to DLC.
- Each record is deserialized on its own into a fresh C# object, so **missing fields get class defaults** (field initializers such as `initialUnlockChance = 100f`, or 0/null/false). They never fall back to the base record's values.

`TemplateManager.Add(newTemplate, type)`, called for each record:

| Situation | Result |
|---|---|
| `disable: true` | The record is **not registered at all**. Exception: if its dataName is in some meta template's `templateNames`, it logs an error and is added anyway. |
| New `(type, dataName)` | Becomes the primary record. |
| Same `(type, dataName)`, **same tag set** as an existing record | The later record is **ignored** (first wins). Log line: `Found template with the same dataName AND same scenarioTags`. Seen in this build for `Ship35`, `dummy`, and `Sol [Millennium]`. |
| Same `(type, dataName)`, **different tag set** | Stored in `duplicateTemplatesByType[type][dataName]`, a list that starts with the primary record. |

Identity is `(C# type, dataName)`. The same dataName in two different template types is unrelated.

**Resolution at game start.** `TemplateManager.ResolveScenarioTemplates(scenarioMeta)` is called from `TIMetaTemplate.LoadMetaTemplates` for a new game and from `SolarSystemBootstrap` when loading a save. It runs in two steps, with `S` = the scenario meta template's `scenarioTags`:

1. `ResolveTaggedTemplates`: a record is *eligible* iff `record.scenarioTags ⊆ S`. Untagged records are always eligible. If a primary record is ineligible, it is replaced by the first eligible duplicate. If there is none, the dataName is **removed** for that type.
2. `ResolveDuplicateTemplates`: for every remaining duplicate list, it picks `OrderByDescending(|tags ∩ S|).ThenBy(min index in S of a matching tag)`, then load order. Edge case: this step does not re-check eligibility, so a record that only partially matches could win. No such record exists in the current data.

Answers to the specific questions:

| Question | Answer |
|---|---|
| Merge by dataName? | Per `(type, dataName)`, the **whole record is replaced**. No field merge. |
| Missing fields | Take C# class defaults, not base values. |
| New records | Added as normal (for example `BS_*` techs). Untagged DLC records such as `2003_*` nations, regions, and bilaterals exist in every scenario but are only instantiated through that scenario's meta `templateNames`. |
| Can a scenario remove a base record? | Not with `disable` (a disabled record is simply dropped). Removal happens through tags: base records tagged `NotPostApoc` (20 projects, 2 techs) are removed in Broken Earth, whose tags are `[PostApoc]`. |
| Which types? | All of them. The logic is generic in `TemplateManager`. Types that use it in the data: Tech, Project, Effect, Faction, GlobalConfig (BE replaces `globalConfig`), HabModule, Hab, Mission, NarrativeEvent, Objective, Org, PriorityPreset, SpaceBody, TimeEvent, Trait, Meta. |

Data overlap (DLC records whose dataName also exists in base):

| Type | 2003 records (overlapping base) | BE records (overlapping base) |
|---|---|---|
| TITechTemplate | 44 (41) | 9 (8) |
| TIProjectTemplate | 168 (138) | 51 (35) |
| TIEffectTemplate | 101 (0) | 70 (0) |
| TIFactionTemplate | 8 (8) | 8 (8) |
| TISpaceBodyTemplate | 17 (17) | 74 (74) |

All tagged 2003 records use `[Millennium]` and all tagged BE records use `[PostApoc]`. One exception: a BE SpaceBody `Sol` tagged `[Millennium]`, which is ignored as a duplicate of the 2003 one.

**Localization folders** are a separate mechanism (see §5). Every `*.<lang>` file anywhere under `DLC_Content` is loaded into the same per-language dictionary for every scenario. A DLC key overrides a base key with the same name. Scenario-specific text uses key postfixes instead:
- `TIMetaTemplate.scenarioLocalizationPostfix` is `.2003` or `.BrokenEarth`.
- `Loc.T_Scenario(key)` tries `key + postfix` first, then falls back to `key`.

`T_Scenario` is used only for nation, region, and org names, faction leader descriptions, the faction intro notification, and a few `UI.Notifications.*` strings. Tech, project, and effect text uses plain `Loc.T`, so the DLC adds new keys (for example `TITechTemplate.displayName.BS_*`). It also reuses keys through `localizationAlias`: 48 effects and 7 projects in 2003, and 39 effects and 7 projects in BE. The only DLC loc key that overrides a base key is `UI.Science.Panel.Effects` (2003). The 2003 and BE key sets do not overlap.

*Confidence: high (code read in full; behavior matches the log output in `Logs/TerraInvicta.log`).*

---

## 2. What a "scenario" is

A scenario is a `TIMetaTemplate` with `newCampaignOptionCategory == "Scenario"`. The menu (`NewGameOptionController.InitWithMetaTemplateCategory`) lists meta templates where `isNewCampaignOption` is set, `listPriority != 999` (999 is editor-only), and the dataName is `2003Scenario`/`BrokenEarthScenario` only if `GameControl.DLCValidated`. The selection becomes `GameControl.scenarioTemplate`. Saves store `TITimeState.scenarioMetaTemplateName`; older saves default to `ModernScenario`.

A scenario meta lists other metas by name, one per template type: `*_StartTime` (TIStartTimeTemplate), Regions, Nations, Armies, Councilors, StartFleets, Orgs, Habs, and RepeatingEvents. The full game is `FullScenario` = solar-system meta + scenario meta + faction-count meta.

| Scenario meta | Menu | scenarioTags | StartTime meta → TIStartTimeTemplate | Start date |
|---|---|---|---|---|
| `ModernScenario` ("2022") | yes | NotPostApoc | ModernStartTime → `ModernDayStart` | 2022-09-30 |
| `2026Scenario` | yes | NotPostApoc | 2026_StartTime → `2026Start` | 2026-01-31 |
| `2070Scenario` | yes | 2070Scenario, NotPostApoc | 2070_StartTime → `2070Start` | 2070-05-31 |
| `2030Scenario` | editor only (listPriority 999) | NotPostApoc | 2030StartTime → `2030AugustStart` | 2030-07-31 |
| `SkirmishScenario` | no (skirmish mode) | NotPostApoc | SkirmishStartTime → `SkirmishModeStart` | 2020-02-16 |
| `2003Scenario` (DLC) | if DLC validated | Millennium, NotPostApoc | 2003_StartTime → `2003Start` | 2003-03-31 |
| `BrokenEarthScenario` (DLC) | if DLC validated | PostApoc | BrokenEarthStartTime → `1962_Start` | year field = **2112**-10-31 |

How the TIStartTimeTemplate is used:
- `TIGlobalResearchState`: `startingTechs[0..2]` are the three initial research slots. `globalTechsCompleted` are granted at start. Any tech with `year > 0 && year < startYear` is also granted (no tech in the current data sets `year`).
- `TIFactionState`: `projectsCompleted` are completed for each faction if `FactionPrereqsSatisfied`. Projects with `researchCost <= 0` are also completed at start.
- `techTreeUIStarters` only affects the tech-tree UI.

**`scenarioTags`** is a field on `TIDataTemplate` (`string[]`), so every template type has it, including Tech and Project. It is evaluated only by the resolution in §1. Inside a scenario, "is this tech/project present?" means "did its `(type, dataName)` survive resolution against the scenario meta's tags?". Nothing else reads `scenarioTags` at runtime. The `scenarioPrefix` key in the meta JSON has no matching C# field and is ignored.

*Confidence: high.*

---

## 3. Tech and project prerequisites and availability

`prereqs` (and both alternatives) resolve with `TemplateManager.Find<TIGenericTechTemplate>(name, allowChild:true)`, so a name can be a tech or a project. Empty strings and `"PLACEHOLDER"` are skipped. Unresolved names are logged and dropped.

**Prereq rule** (`TITechTemplate.TechPrereqsSatisfied`, `TIProjectTemplate.TechPrereqsSatisfied`): all prereqs are required (AND). Only two positions have alternatives:
- `prereqs[0]` is satisfied by itself **or** by `altPrereq0`.
- `prereqs[1]` is satisfied by itself **or** by `altPrereq1`.
- There is no `altPrereq2` or higher.

| | Techs | Projects |
|---|---|---|
| Counts as satisfied | the prereq is in global `finishedTechs` | the prereq is in global finishedTechs, **or** the faction's `completedProjects`, **or** `finishedOneTimeOnlyProjects` (a oneTimeGlobally project finished by anyone) |
| Project as prereq | not possible: `ref_tech` of a project is null, so it is never satisfied. The data has 0 such cases. | common: 785 of 1,693 prereq entries are projects |
| Data | max 3 prereqs, `altPrereq0`/`altPrereq1` unused | max 5 prereqs; `altPrereq0` ×167, `altPrereq1` ×19 |

**Project availability** (`TIProjectTemplate.PrereqsSatisfied` ∧ `TIFactionState.RollToAddProjectTrigger`):

| Field | Semantics |
|---|---|
| `factionPrereq` (list of TIFactionTemplate names) | Empty means everyone. Otherwise the faction must be in the list (OR). Faction-locked projects are hidden from other factions (`ShouldHide`). |
| `requiresNation` (nation **referenceName**: `referenceAlias` or dataName, e.g. `RUS` matches `2003_RUS`) | Fails only if the nation state exists and is **not extant**. If no such nation exists in the scenario, the check passes. One dangling value in the data: `ZAB`. |
| `requiredObjectiveName` / `altRequiredObjectiveName` | If set and the name resolves, the faction must have completed one of the two objectives (OR). An unresolvable name means no requirement. |
| `requiredMilestone` (enum `CampaignMilestone`) | `None` means none required. Otherwise `faction.MilestoneCompleted(m)`. Completing a milestone triggers a roll for every project that requires it. The project's `requiredMilestone` hides the base-class field of the same name; both receive the JSON value. |
| `oneTimeGlobally` | Unavailable once any human faction has completed it (`UniquenessReqsSatisfied`). |
| `repeatable` | Stays available after completion. Cost = `researchCost × (1 + times completed)`. |
| `disable` | The record is never registered (§1). |
| `factionAvailableChance` (default 100) | Roll to enter the trigger pool: `chance = 100` if `factionAlways == faction`, if the campaign option `variableProjectUnlocks` is off, or if `factionAvailableChance ≥ 100`. Otherwise `max(c, c·7/#humanFactions) + techContributionBonus + effect mods + councilor bonuses + Science/5`, clamped to 0..100. A failed roll adds the project to `missedProjects`, which `Project_ReviewFailedProjects` lets you re-roll. |
| `factionAlways` (faction name or `"Random"`) | A faction name gives that faction 100%. `"Random"` gives 100% to the first faction that rolls, if no faction has triggered the project yet. |
| `initialUnlockChance` (default 100), `deltaUnlockChance`, `maxUnlockChance` (default 100) | After a successful roll, a `ProjectTrigger` starts at `monthlyTriggerValue = initial + contributionBonus + Science/5`. Each month: `+= delta + delta·researchSpeedMod + effect mods`, clamped to `[initial, max]`. Each day the project becomes available with p = `1 − (1 − v/100)^(1/30.44)`. |
| `AI_techRole`, `AI_criticalTech`, `AI_projectRole` | Used only by AI planning and evaluation (`AIEvaluators`, `AIDailyFactionPlanner`). No effect on availability. `ProjectRole.Objective` also affects AI slot handling. |

When rolls happen:
- On public tech completion: for every project that has that tech as a prereq or alt prereq.
- On project completion: for that project's dependents.
- On milestone completion.
- At start: for projects with an empty `prereqs` list.

Xenology projects, and projects whose chain contains uncompleted xenology or milestone prereqs, are hidden until available.

**`endGameTech`** (7 techs):
- `TIGlobalResearchState.AvailableTechs` offers end-game techs only when no ordinary tech is available.
- They can be researched repeatedly. Cost = `researchCost × (1 + endGameTechsCompletedByCategory[cat])`.
- Each completion re-rolls every project in the same category, plus all Xenology projects, for all factions (`OnPublicTechCompleted`).
- They are excluded from "all techs finished".

Enums: `TechCategory` {Materials, SpaceScience, Energy, LifeScience, MilitaryScience, InformationScience, SocialScience, Xenology}. Techs never use Xenology; 95 projects do. `TechRole` {None, SpaceDevelopment, SpaceExpansion, SpaceWar, EarthPolitics, Efficiency, Income, FactionObjective, Competition, Blocker}. `ProjectRole` has 35 values (None, Core, Objective, … AlienMissionDefense). `CampaignMilestone` has explicit ints (None=0 … 36; Tutorial*; UITutorial_* = 200+).

*Confidence: high for prereq and availability logic. Medium for the exact chance arithmetic (read once; minor terms such as council bonuses were not traced).*

---

## 4. Effect description placeholders

`TIEffectTemplate.description(TIGameState state1, TIGameState state2)` calls `Loc.T("TIEffectTemplate.description." + localizationName, args…)` and then `string.Format`. Techs and projects call it as `description(faction, null)`, so for tech and project tooltips `state1` is the researching faction and `state2` is empty.

| idx | value | idx | value |
|---|---|---|---|
| 0 | `Num(value)` | 15 | state2 faction name (colored) |
| 1 | state1.displayName | 16 | comma list of trait names in trait grouping `(int)value` |
| 2 | state2.displayName | 17 | state1 councilor's trait in grouping `(int)value` |
| 3 | `P0(value)` | 18 | `P0(1/value − 1)` |
| 4 | `P0(1 − value)` | 19 | `Num(−value)` |
| 5 | state1 nation, with article and place prep | 20 | `UI` string for `CouncilorAttribute` named by `strValue` |
| 6 | state2 nation, with article and place prep | 21 | state1 faction name, capitalized, colored |
| 7 | `Num(duration_months)` | 22 | state2 faction name, capitalized, colored |
| 8 | `P0(value − 1)` | 23 | `Small(clamp(value × state1.nation.priorityEffectPopScaling ?? 1, −2·abs(value), 2·abs(value)))` |
| 9 | state1 nation name ?? state1.displayName | 24 | same as 23 for state2 |
| 10 | state1 region name ?? state1.displayName | 25 | `P0(abs(value))` |
| 11 | state2 nation name ?? state2.displayName | 26 | comma list of state1 faction's executive nations |
| 12 | state2 region name ?? state2.displayName | 27 | `P0(1 − 1/value)` |
| 13 | `Find<TIDataTemplate>(strValue, allowChild).displayName` ?? raw `strValue` | 28 | `(value − 1).ToString("+0%;-0%;0%")` |
| 14 | state1 faction name (colored) | 29 | `value.ToString("+0%;-0%;0%")` |

Formatters:
- `Num` = `TIUtilities.FormatBigOrSmallNumber`. If `abs ≥ 1000`, it uses `UI.Global.Thousands/Millions/Billions/Trillions` with `N1`. Otherwise it uses `FormatSmallNumber`.
- `Small` = `FormatSmallNumber`: truncate to 7 decimals, then format as `N{d}`. `d` is 0 for integers; for other values it is 1 if abs ≥ 1, 2 if > 0.1, 3 if > 0.01, 4 if > 0.001, then 5–7. Trailing zeros and the trailing decimal point are trimmed.
- `P0` = `float.ToString("P0")`, then `HandleLocExceptionsPercent`, which strips the space on English systems. Output looks like `25%` or `-25%`.

Index usage in en `TIEffectTemplate.en` (most frequent first): {0} 242, {3} 128, {1} 93, {5}/{13} 55, {9} 46, {8} 38, {21} 32, {4} 25, … {26}–{29} ≤ 1 each.

A second formatter, `allDescription(code)`, passes 26 args (0–25). It puts "all human nations / all nations / all regions" text in every state slot, and makes 23 and 24 plain `Small(value)`. In `LocalizationManager.Find(key, args)`, a `FormatException` (for example a stray `{word}`) returns the **key itself**.

*Confidence: high (the argument list is copied from code). Offline rendering of state-dependent slots (1, 2, 5, 6, 9–12, 14–17, 21–24, 26) requires choosing a context, such as a placeholder faction name.*

---

## 5. Localization format (`LocalizationManager`)

**Load order** for each language folder in `StreamingAssets/Localization/<lang>/`:
1. Every file in the folder not ending in `meta` (any file name).
2. Every `DLC_Content/**/*.<lang>` file.
3. Every `Mods/Enabled/**/*.<lang>` file.

**Line parsing** (`ProcessLocEntry`, `File.ReadLines`, BOM-aware; base files are ASCII/UTF-8 with CRLF line endings). Steps, in order:
1. Skip blank lines.
2. `key` = text before the **first** `=`. The key is not trimmed (no keys in the data have spaces). Lines without `=` at position ≥ 1 are skipped, which covers whole-line `//` comments.
3. `value`: remove all tab characters. Then replace `<br/>` → newline, `<h>` → `<color=#EC9933>`, `</h>` → `</color>`, `<rcol>` → `<line-height=0.01%>\n<align="right">`, `</rcol>` → `</align></line-height>`.
4. Truncate at the **first `//` anywhere in the value**. This is the inline comment rule; a URL in a value would be cut too.
5. `TrimEnd(' ')`, then `<sp>` → space.
6. Skip the entry if the value is empty.
7. Remove `<skip/>`, which can leave an empty value. Some UI code also compares the value to `"<skip/>"` before removal, but after this step that comparison is effectively dead.

**Other rules:**
- Duplicate keys: within base files the first one wins (an error is logged). DLC and mod lines replace earlier values.
- No backslash escapes: the data contains no `\n` or `\"`, and `{{`/`}}` only matter to `string.Format`. Everything else is passed through to TextMeshPro rich text: `<color=…>`, `<align=…>`, `<sprite name=…>`, `<i>`, `<size>`, `<sub>`, `<b>`.
- Special values handled by code: `<habmodule>` / `<shipmodule>` as a project summary are replaced with the unlocked module's description (`TIProjectTemplate.summary`).
- Fallback: `GetKeyValue` looks in the current language. On a miss, and if the current language template is not `core`, it looks in `en`. On a second miss it returns the key itself. Only `en` has `core: true`. Example gap: `deu` lacks 1,340 en keys.
- Positional args `{0}`–`{29}` are filled with `string.Format`. When args are passed, Find checks that the highest index is less than `args.Length` (it only reads the first digit after `{`, so `{12}` counts as 1); otherwise it returns the key.

**Named tokens** are replaced with `StringBuilder.Replace` before `Format`:
- `TITechTemplate.quote`: `{appeaseLeader}` `{destroyLeader}` `{exploitLeader}` `{submitLeader}` `{resistLeader}` `{cooperateLeader}` `{escapeLeader}` → `Loc.T("TIFactionTemplate.fullLeader.<X>Council")`.
- Narrative events (`TINarrativeEventTemplate.NarrativeEventStringReplacement`): about 70 tokens such as `{actorName…}`, `{target…}`, `{secondary…}`, and `{summary}`, all resolved from runtime state.

**Display text for a tech** is the quote (if any) + blank line + description. Key families:
- `TITechTemplate.{displayName,summary,description,quote}.<localizationName>`
- `TIProjectTemplate.{displayName,summary,description}.<localizationName>`
- `TIEffectTemplate.description.<localizationName>`
- Hab modules and ship parts use `dataName`, not `localizationName`. Drives use `TIDriveTemplate.description.<dataName with last char → '1'>`.

*Confidence: high.*

---

## 6. Template JSON parsing and cross-references

**Parser:** FullSerializer (`StringSerializationAPI.Deserialize` → `fsJsonParser.Parse`), not Newtonsoft (Newtonsoft is only used for mod merging). It is lenient:
- `//` line comments and `/* */` block comments are allowed. A parser bug skips one extra character after `*/`.
- **Trailing commas and missing commas between elements are both accepted.**
- `NaN` and `Infinity` are accepted. Keys are case-sensitive.
- A duplicate key in one object throws (`Dictionary.Add`).
- Unknown keys are ignored. Examples in the data: `scenarioPrefix`, drive `thrustRating_GW` and `"req power"`, and a `displayName` key on some ship parts (the field is `_displayName`).
- Enums are deserialized from their name string, or a `"A,B"` flags list with no spaces. An unknown enum name fails that field.
- `TIDataTemplate` properties with protected setters (`dataName`, `friendlyName`, `disable`, `scenarioTags`, `referenceAlias`, `localizationAlias`) are deserialized.

**File → class:** `FindDataTemplateType(Path.GetFileNameWithoutExtension(file))` does `Assembly.GetType(name)`, so the file name must equal the global-namespace class name exactly. Each file is a JSON **array** of that class. Records with a null dataName are skipped. Subdirectories are scanned (`AllDirectories`).

**String fields resolved to other templates.** "Code" means the resolution was confirmed in the decompiled code. The other rows were checked against the data, and every value resolved to the listed type unless noted.

| Template | Field | Target |
|---|---|---|
| Tech, Project | `prereqs[]`, `altPrereq0`, `altPrereq1` | TITechTemplate or TIProjectTemplate (`Find<TIGenericTechTemplate>`, allowChild). Code. |
| Tech, Project | `effects[]` | TIEffectTemplate. Code. |
| Tech | (reverse lookups) | TIOrgTemplate.`requiredTechName`, TICodexEntryTemplate.`unlockTech`, TIMissionTemplate.`knowledgeProject` point at a tech or project. |
| Project | `factionPrereq[]`, `factionAlways` | TIFactionTemplate dataName (`factionAlways` may also be `"Random"`). |
| Project | `requiresNation` | TINationTemplate **referenceName** (`referenceAlias` ?? dataName). Code. |
| Project | `requiredObjectiveName`, `altRequiredObjectiveName` | TIObjectiveTemplate. Code. |
| Project | `orgGranted` | TIOrgTemplate. Code. |
| Project | (reverse lookups) | TIBilateralTemplate.`projectUnlockName`, TITraitTemplate (cybernetics), ship parts and hab modules `requiredProjectName`, TIObjectiveTemplate target project. |
| Effect | `initialFactionsStr[]` | TIFactionTemplate. |
| Effect | `strValue` | Polymorphic. {13} uses `Find<TIDataTemplate>(allowChild)`; {20} parses it as `CouncilorAttribute`. The data contains map regions, traits, narrative events, milestones, and attributes. |
| HabModule | `requiredProjectName`, `unlocksProjectName` | TIProjectTemplate. Code. |
| HabModule | `upgradesFromName` | TIHabModuleTemplate. Code. |
| All ship parts (`TIShipPartTemplate` base) | `requiredProjectName` | TIProjectTemplate. Code. This is the only template reference on Drive, PowerPlant, Radiator, HeatSink, Battery, ShipArmor, ShipHull, UtilityModule, Laser, MagneticGun, Gun, Missile, Particle, and Plasma. |
| Drive | (implicit) | `dataName` ends with `x1`…`x6` (thruster count); the `x1` sibling is the "single thruster" template. |

All other string fields on these types are asset or sound paths (`iconResource`, `modelResource`, `*FXResource`, `combatUIpath`, `path1/2`, `notes`).

Ship part class hierarchy:
- `TIShipPartTemplate` ← TIShipModuleTemplate ← {HeatSink, UtilityModule ← Battery, ShipHull}
- `TIShipPartTemplate` ← {Drive, PowerPlant, Radiator, ShipArmor, TIShipWeaponTemplate}
- `TIShipWeaponTemplate` ← {TIBeamWeaponTemplate ← Laser, Particle; TIGunTypeWeaponTemplate ← Gun, MagneticGun, Plasma; TIProjectileWeaponTemplate ← Missile}

The weapon class comes from the C# type (`WeaponClass` Laser/Particle/NavalGun/Magnetic/Plasma/Missile), not from JSON.

**Enum-typed fields:**

| Field | Enum |
|---|---|
| `techCategory` | TechCategory |
| `AI_techRole` | TechRole |
| `AI_projectRole` | ProjectRole |
| `requiredMilestone` | CampaignMilestone |
| `resourcesGranted[].resource` | FactionResource {None, Money, Influence, Operations, Research, Projects, Boost, MissionControl, Water, Volatiles, Metals, NobleMetals, Fissiles, Antimatter, Exotics} |
| Effect `contexts[]` | Context (213 values) |
| Effect `instantEffect` | InstantEffect (190 values) |
| Effect `operation` | StatModSetOperation {SetToFixedValue, IncreaseToValue, DecreaseToValue, Additive, AdditivePer, SubtractivePer, Multiplicative, SetToAnotherAttribute} |
| Effect `effectTarget` | EffectTargetType (14 values) |
| Effect `effectSecondaryTarget` | EffectSecondaryStateType (70 values) |
| Effect `effectDuration` | EffectDuration {permanent, temporary, instant} |
| Effect `showTotal` | TotalEffectDisplayBehavior |
| HabModule `habType` | HabType {Station, Base, Any} |
| HabModule `specialRules[]` | HabModuleSpecialRule (59 values) |
| HabModule `techBonuses[]` | TechBonus {category: TechCategory, bonus} |
| Drive `driveClassification` | DriveClassification (10 values) |
| Drive `requiredPowerPlant`, PowerPlant `powerPlantClass` | PowerPlantRequirement (17 values) |
| Drive `propellant` | Propellant |
| Drive `cooling` | CoolingCycle |
| Drive `powerGen` | PowerGenerationType |
| Radiator `radiatorType` | RadiatorType {Fin, Droplet, Spike, AlienSpike} |
| Armor `specialties[]` | {armorSpecialty: ArmorSpecialty, value} |
| Utility `specialModuleRules[]` | SpecialModuleRule (50 values) |
| Weapon `mount` | Mount (17 values) |
| Missile `warheadClass` | WarheadClass |
| Particle `dispersionModel` | ParticleBeamDispersionModel |
| Hull `shipModuleSlots[].moduleSlotType` | ShipModuleSlotType |

Costs use the `ResourceCostBuilder` struct. Some enum names appear to match template dataNames by coincidence (instantEffect ↔ mission names, specialRules ↔ hab module names); they are not references.

*Confidence: high for the parser and file mapping. High for the reference fields listed; struct-nested fields were not traced exhaustively.*

---

## 7. Game version

- `Application.version` (shown in the main menu and options, and logged as `TerraInvicta v…`) is Unity's PlayerSettings `bundleVersion`. It is stored in **`TerraInvicta_Data/globalgamemanagers`** as a length-prefixed string: int32 LE length `07 00 00 00`, then `1.0.53b`. In this build it is at offset 0x1250. The Unity version `2020.3.49f1` also appears in that file.
- To extract it, match `\d+\.\d+\.\d+[a-z]?` preceded by a matching LE length, and take the match that is not the Unity version. Alternatively, parse the Unity serialized file properly.
- Secondary sources: `Logs/TerraInvicta.log` line 2 (`TerraInvicta v1.0.53b`, written at runtime) and `AIDump.txt` (`Version: 1.0.53b`). Both can be stale.
- Not usable: `Assembly-CSharp.dll` `AssemblyVersion` is `0.0.1.0`, and the DLL contains no version string.

*Confidence: high for location and value; medium for the robustness of the byte-pattern approach across builds.*

---

## Implications for an extractor / query library

1. **Model records as `(type, dataName, scenarioTags)`, not `(type, dataName)`.** Load base and all DLC files. Drop `disable: true` records unless a meta template lists them. Keep the first record per identical tag set.
2. **Build a per-scenario view by running the game's resolution:** keep a record iff its tags ⊆ the scenario tags. Then pick among duplicates by maximum tag overlap, then tag order, then load order (base first). Never merge fields. Use C# defaults for missing fields; the important non-zero ones are `factionAvailableChance`, `initialUnlockChance`, and `maxUnlockChance`, all 100.
3. **Scenarios come from the data:** metas with `newCampaignOptionCategory == "Scenario"`, giving `{ModernScenario, 2026Scenario, 2070Scenario, 2030Scenario (hidden), SkirmishScenario (hidden), 2003Scenario, BrokenEarthScenario}`. Their tags, start-time template, starting techs, and `projectsCompleted` are all in JSON. Default to `ModernScenario`.
4. **The JSON parser must accept comments, trailing commas, missing commas, and NaN/Infinity, and must reject duplicate keys.** Strict JSON fails on base `TIMetaTemplate.json` and others. Ignore unknown keys.
5. **Localization** is a single dictionary per language: base, then all DLC `*.<lang>` files, with later entries overriding. Apply the line rules in §5 exactly, including the first-`//` truncation and `<skip/>` → empty. When querying a scenario, look up `key + postfix` first for names (nations, regions, orgs, leaders), and fall back to `en` for missing keys in other languages. Look up `localizationAlias ?? dataName` for tech, project, and effect keys, and `dataName` for hab and ship-part keys.
6. **Prereq graph:** edges go to techs or projects. Represent `prereqs[0]|altPrereq0` and `prereqs[1]|altPrereq1` as OR-groups and the rest as AND. Techs never depend on projects. Satisfying a project prereq is per-faction, except for oneTimeGlobally projects, which count globally once done.
7. **Project availability is probabilistic and per-faction.** Expose the static gates (factionPrereq, requiresNation via referenceName, objectives, milestone, oneTimeGlobally) separately from the chance parameters. Treat `factionAlways` and `"Random"` as overrides to 100%.
8. **Effect text:** implement the 30-slot argument vector in §4 with .NET-compatible `P0`, `N{d}`, and custom `+0%` formatting. Render state-dependent slots with a configurable context, such as a placeholder faction name. If formatting fails, fall back to the raw key the way the game does.
9. Record the game version from `globalgamemanagers` alongside the Steam buildid. Re-verify these semantics when the version changes.
