//! The `extract` pipeline (port of `cmd_extract` in `cli.py`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::assets::{self, AssetOptions, Previous};
use crate::game::{APP_ID, file_name, rel_posix, subdirs};
use crate::jsonc::{self, Value, object};
use crate::output::{RecordContext, canonical_record, io_error, sha256_bytes, write_json, write_str};
use crate::{Error, ExtractOptions, ExtractReport, FORMAT_VERSION, GameInstall, Images, Progress, Result, Stage, locale, schema, templates};

/// Builds with a `manifest.json`, sorted by directory name.
fn complete_builds(data_dir: &Path) -> Vec<PathBuf> {
    subdirs(data_dir).into_iter().filter(|b| b.join("manifest.json").is_file()).collect()
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = crate::game::read(path)?;
    jsonc::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| Error::Other(format!("{}: {e}", path.display())))
}

pub fn write_index(data_dir: &Path) -> Result<()> {
    let mut builds = Vec::new();
    for b in complete_builds(data_dir) {
        let m = read_json(&b.join("manifest.json"))?;
        let game = m.get("game").cloned().unwrap_or(Value::Null);
        let missing = |key: &str| Error::Other(format!("{}: manifest has no {key}", b.display()));
        let last_updated = game.get("lastUpdated").cloned().unwrap_or(Value::Null);
        let sort_key = match last_updated {
            Value::Int(i) => i,
            Value::Bool(b) => b as i128,
            _ => 0,
        };
        let dir = file_name(&b).unwrap_or_default();
        let entry = object([
            ("dir", Value::from(&dir)),
            ("buildId", game.get("buildId").cloned().ok_or_else(|| missing("game.buildId"))?),
            ("lastUpdated", last_updated),
            ("formatVersion", m.get("formatVersion").cloned().ok_or_else(|| missing("formatVersion"))?),
        ]);
        builds.push((sort_key, dir, entry));
    }
    builds.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let latest = builds.last().map(|(_, dir, _)| dir.clone());
    write_json(
        &data_dir.join("index.json"),
        &object([
            ("latest", latest.into()),
            ("builds", Value::Array(builds.into_iter().map(|(_, _, e)| e).collect())),
        ]),
        false,
    )
}

/// `json.dumps(map, sort_keys=True)` with Python's default separators and `ensure_ascii`.
fn python_dumps_sorted(map: &BTreeMap<String, String>) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in map.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_str(&mut out, k, true);
        out.push_str(": ");
        write_str(&mut out, v, true);
    }
    out.push('}');
    out
}

/// Current UTC time as Python's `datetime.now(timezone.utc).replace(microsecond=0).isoformat()`.
fn utc_now_iso() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil_from_days
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}+00:00", rem / 3600, rem % 3600 / 60, rem % 60)
}

struct Run<'a> {
    progress: &'a mut dyn FnMut(Progress),
    cancel: &'a AtomicBool,
}

impl Run<'_> {
    fn stage(&mut self, stage: Stage) {
        (self.progress)(Progress::Stage { stage });
    }

    fn log(&mut self, message: impl Into<String>) {
        (self.progress)(Progress::Log { message: message.into() });
    }

    fn step(&mut self, done: u64, total: u64) -> Result<()> {
        (self.progress)(Progress::Step { done, total, label: None });
        self.check()
    }

    fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) { Err(Error::Cancelled) } else { Ok(()) }
    }
}

pub fn run(install: &GameInstall, opts: &ExtractOptions, progress: &mut dyn FnMut(Progress), cancel: &AtomicBool) -> Result<ExtractReport> {
    let started = Instant::now();
    let mut run = Run { progress, cancel };
    let data_dir = &opts.data_dir;

    run.stage(Stage::Templates);
    run.log("reading templates …");
    let mut ts = templates::load_all_with(&install.layers, &mut |done, total| run.step(done, total))?;
    let mut source_hashes: BTreeMap<String, String> = ts
        .sources
        .iter()
        .map(|(path, digest)| (rel_posix(path, &install.root), digest.clone()))
        .collect();
    let parse_errors = ts
        .issues
        .iter()
        .filter(|i| i.get("kind").and_then(Value::as_str) == Some("parse_error"))
        .count();
    let build_id = match opts.build_id.clone().or_else(|| install.build_id.clone()).filter(|id| !id.is_empty()) {
        Some(id) => id,
        None => {
            let digest = sha256_bytes(python_dumps_sorted(&source_hashes).as_bytes());
            let id = format!("local-{}", &digest[..12]);
            run.log(format!("no Steam build id found, using {id}"));
            id
        }
    };

    let final_dir = data_dir.join(&build_id);
    if final_dir.join("manifest.json").is_file() && !opts.force && parse_errors == 0 {
        run.log(format!("build {build_id} already extracted at {} (use --force to redo)", final_dir.display()));
        write_index(data_dir)?;
        return Ok(ExtractReport {
            build_id,
            build_dir: final_dir,
            extracted: false,
            issues: ts.issues.len(),
            parse_errors,
            seconds: started.elapsed().as_secs_f64(),
        });
    }

    let work = data_dir.join(format!(".{build_id}.partial"));
    if work.exists() {
        std::fs::remove_dir_all(&work).map_err(|e| io_error(&work, e))?;
    }
    std::fs::create_dir_all(&work).map_err(|e| io_error(&work, e))?;
    let result = write_build(&mut run, install, opts, &mut ts, &mut source_hashes, &build_id, &work);
    let (issues, dangling) = match result {
        Ok(counts) => counts,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&work);
            return Err(e);
        }
    };

    if final_dir.exists() {
        std::fs::remove_dir_all(&final_dir).map_err(|e| io_error(&final_dir, e))?;
    }
    std::fs::rename(&work, &final_dir).map_err(|e| io_error(&final_dir, e))?;
    write_index(data_dir)?;

    let seconds = started.elapsed().as_secs_f64();
    run.log(format!(
        "done: {}  ({seconds:.0}s, {issues} issues, {dangling} fields with dangling refs)",
        final_dir.display()
    ));
    Ok(ExtractReport {
        build_id,
        build_dir: final_dir,
        extracted: true,
        issues,
        parse_errors,
        seconds,
    })
}

/// Write everything into `work`; returns the issue count and the number of fields with dangling refs.
fn write_build(
    run: &mut Run<'_>,
    install: &GameInstall,
    opts: &ExtractOptions,
    ts: &mut templates::TemplateSet,
    source_hashes: &mut BTreeMap<String, String>,
    build_id: &str,
    work: &Path,
) -> Result<(usize, usize)> {
    run.log("writing templates …");
    let total = ts.files.iter().map(|(_, files)| files.len()).sum::<usize>() as u64;
    let mut done = 0;
    for (layer_id, files) in &mut ts.files {
        for tf in files {
            let ctx = RecordContext {
                template: Some(&tf.name),
                layer: Some(layer_id),
            };
            tf.records = std::mem::take(&mut tf.records)
                .into_iter()
                .map(|r| canonical_record(r, &mut ts.issues, ctx))
                .collect();
            let records = Value::Array(std::mem::take(&mut tf.records));
            write_json(&work.join("templates").join(&*layer_id).join(format!("{}.json", tf.name)), &records, false)?;
            // Keep the canonical records for the schema and image steps.
            if let Value::Array(records) = records {
                tf.records = records;
            }
            done += 1;
            run.step(done, total)?;
        }
    }

    run.stage(Stage::Locale);
    run.log("reading localization …");
    let ls = locale::load_all_with(&install.root, &mut |done, total| run.step(done, total))?;
    for (lang, entries) in &ls.entries {
        let value = object(entries.iter().map(|(k, v)| (k.clone(), Value::from(v))));
        write_json(&work.join("locale").join(lang).join("strings.json"), &value, false)?;
    }
    source_hashes.extend(ls.sources.iter().map(|(k, v)| (k.clone(), v.clone())));

    run.stage(Stage::Schema);
    run.log("discovering schema …");
    let sch = schema::discover(ts);
    write_json(&work.join("schema.json"), &sch, false)?;
    run.check()?;

    let skip: Option<Vec<String>> = match &opts.images {
        Images::None => None,
        Images::All => Some(Vec::new()),
        Images::Skip(names) => Some(names.iter().map(|s| s.to_lowercase()).collect::<BTreeSet<_>>().into_iter().collect()),
    };
    let mut asset_index = None;
    if let Some(skip) = skip {
        run.stage(Stage::Images);
        run.log("extracting images …");
        asset_index = Some(extract_images(run, install, opts, ts, &skip, work)?);
    }

    run.stage(Stage::Finish);
    let issues: Vec<Value> = ts.issues.iter().chain(&ls.issues).cloned().collect();
    let issue_count = issues.len();
    let dangling = sch.get("dangling").cloned().unwrap_or(Value::Array(Vec::new()));
    let dangling_count = dangling.as_array().map_or(0, <[Value]>::len);
    write_json(
        &work.join("reports/issues.json"),
        &object([
            ("issues", Value::Array(issues)),
            ("danglingRefs", dangling),
            (
                "missingImages",
                asset_index.as_ref().and_then(|a| a.get("missing")).cloned().unwrap_or(Value::Null),
            ),
        ]),
        false,
    )?;

    let templates_of = |id: &str| {
        let mut names: Vec<Value> = ts.layer(id).unwrap_or_default().iter().map(|f| Value::from(&f.name)).collect();
        names.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        Value::Array(names)
    };
    let sch_templates = sch.get("templates").and_then(Value::as_object).unwrap_or_default();
    let manifest = object([
        ("formatVersion", Value::from(FORMAT_VERSION)),
        (
            "extractor",
            object([("name", Value::from("ti-extract")), ("version", env!("CARGO_PKG_VERSION").into())]),
        ),
        ("extractedAt", utc_now_iso().into()),
        (
            "game",
            object([
                ("appId", Value::from(APP_ID)),
                ("buildId", build_id.into()),
                ("version", install.version.clone().into()),
                ("lastUpdated", install.last_updated.into()),
            ]),
        ),
        (
            "layers",
            Value::Array(
                install
                    .layers
                    .iter()
                    .map(|l| {
                        object([
                            ("id", Value::from(&l.id)),
                            ("kind", (&l.kind).into()),
                            ("dlc", l.dlc.clone().into()),
                            ("folder", l.folder.clone().into()),
                            ("templates", templates_of(&l.id)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("languages", Value::Array(ls.entries.keys().map(Into::into).collect())),
        ("hasAssets", asset_index.is_some().into()),
        (
            "counts",
            object(
                sch_templates
                    .iter()
                    .map(|(name, t)| (name.clone(), t.get("records").cloned().unwrap_or(Value::Null))),
            ),
        ),
        ("sources", object(source_hashes.iter().map(|(k, v)| (k.clone(), Value::from(v))))),
    ]);
    // written last: marks the build complete
    write_json(&work.join("manifest.json"), &manifest, false)?;
    Ok((issue_count, dangling_count))
}

fn to_serde(v: &Value) -> serde_json::Value {
    use serde_json::Value as J;
    match v {
        Value::Null => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Int(i) => i64::try_from(*i)
            .map(J::from)
            .or_else(|_| u64::try_from(*i).map(J::from))
            .unwrap_or_else(|_| J::from(*i as f64)),
        Value::BigInt(s) => J::String(s.clone()),
        Value::Float(f) => serde_json::Number::from_f64(*f).map_or(J::Null, J::Number),
        Value::Str(s) => J::String(s.clone()),
        Value::Array(items) => J::Array(items.iter().map(to_serde).collect()),
        Value::Object(pairs) => J::Object(pairs.iter().map(|(k, v)| (k.clone(), to_serde(v))).collect()),
    }
}

fn from_serde(v: &serde_json::Value) -> Value {
    use serde_json::Value as J;
    match v {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(*b),
        J::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => Value::Int(i.into()),
            (_, Some(u)) => Value::Int(u.into()),
            _ => Value::Float(n.as_f64().unwrap_or(f64::NAN)),
        },
        J::String(s) => Value::Str(s.clone()),
        J::Array(items) => Value::Array(items.iter().map(from_serde).collect()),
        J::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), from_serde(v))).collect()),
    }
}

/// The `assets/index.json` value, after exporting the images into `work/assets`.
fn extract_images(run: &mut Run<'_>, install: &GameInstall, opts: &ExtractOptions, ts: &templates::TemplateSet, skip: &[String], work: &Path) -> Result<Value> {
    let bundles = assets::read_bundle_manifests(&install.bundle_dirs)?;
    let mut strings = Vec::new();
    for (_, files) in &ts.files {
        for rec in files.iter().flat_map(|f| &f.records) {
            schema::strings(rec, &mut strings);
        }
    }
    let resources = assets::collect_resources(strings, &bundles);
    let mut previous = Vec::new();
    for b in complete_builds(&opts.data_dir).into_iter().rev() {
        let idx = b.join("assets/index.json");
        if idx.is_file() {
            previous.push(Previous {
                dir: b.join("assets"),
                index: to_serde(&read_json(&idx)?),
            });
        }
    }
    let out_dir = work.join("assets");
    let mut asset_opts = AssetOptions {
        skip_bundles: skip,
        previous: &previous,
        image_format: opts.image_format,
        progress: &mut *run.progress,
        cancel: run.cancel,
    };
    let result = assets::extract(&resources, &bundles, &out_dir, &mut asset_opts)?;
    let mut index = vec![
        ("resources", object(result.resources.iter().map(|(k, v)| (k.clone(), from_serde(v))))),
        ("bundles", object(result.bundles.iter().map(|(k, v)| (k.clone(), from_serde(v))))),
        ("missing", Value::Array(result.missing.iter().map(Into::into).collect())),
    ];
    if !skip.iter().any(|s| s == assets::INLINE_SPRITE_BUNDLE)
        && let Some(sprites) = assets::extract_inline_sprites(&bundles, &out_dir, &mut asset_opts)?
    {
        index.push(("sprites", from_serde(&sprites)));
    }
    run.check()?;
    let index = object(index);
    write_json(&out_dir.join("index.json"), &index, false)?;
    Ok(index)
}
