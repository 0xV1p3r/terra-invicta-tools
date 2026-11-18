//! End-to-end extraction of small fake installs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

use serde_json::{Value, json};
use ti_extract::output::sha256_file;
use ti_extract::{Error, ExtractOptions, ExtractReport, Images, Progress};

fn write(root: &Path, relative: &str, text: &[u8]) -> PathBuf {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
    path
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn prefixed(value: &[u8]) -> Vec<u8> {
    let mut out = (value.len() as i32).to_le_bytes().to_vec();
    out.extend_from_slice(value);
    out
}

fn options(output: &Path) -> ExtractOptions {
    ExtractOptions {
        build_id: Some("test".into()),
        images: Images::None,
        ..ExtractOptions::new(output)
    }
}

fn extract(root: &Path, opts: &ExtractOptions) -> ti_extract::Result<ExtractReport> {
    let install = ti_extract::discover(root)?;
    ti_extract::extract(&install, opts, &mut |_| {}, &AtomicBool::new(false))
}

fn cli(root: &Path, output: &Path, extra: &[&str]) -> i32 {
    let status = Command::new(env!("CARGO_BIN_EXE_ti-extract"))
        .args([
            "--game-dir".as_ref(),
            root.as_os_str(),
            "extract".as_ref(),
            "--data-dir".as_ref(),
            output.as_os_str(),
        ])
        .args(["--build-id", "test", "--no-assets"])
        .args(extra)
        .output()
        .unwrap()
        .status;
    status.code().unwrap()
}

#[test]
fn manifest_and_locale_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    write(
        &root,
        "TerraInvicta_Data/StreamingAssets/Templates/TITest.json",
        br#"[{"dataName": "x", "disable": true, "scenarioTags": ["tag"]}]"#,
    );
    let dlc = write(
        &root,
        "DLC_Content/DarkSkies/Other/Templates/TITest.json",
        br#"[{"dataName": "x", "extra": 2}]"#,
    );
    let loc = write(&root, "TerraInvicta_Data/StreamingAssets/Localization/en/a.en", b"z=<skip/>\na=base\n");
    let dlc_loc = write(&root, "DLC_Content/DarkSkies/elsewhere/a.en", b"a=override\n");
    write(&root, "TerraInvicta_Data/globalgamemanagers", &prefixed(b"1.0.53b"));
    let output = tmp.path().join("output");
    assert_eq!(cli(&root, &output, &[]), 0);

    let build = output.join("test");
    let manifest = read(&build.join("manifest.json"));
    assert_eq!(manifest["formatVersion"], 1);
    assert_eq!(manifest["game"]["version"], "1.0.53b");
    assert_eq!(manifest["languages"], json!(["en"]));
    assert_eq!(
        manifest["layers"],
        json!([
            {"id": "base", "kind": "base", "dlc": null, "folder": null, "templates": ["TITest"]},
            {"id": "dlc.darkskies.other.templates", "kind": "dlc", "dlc": "DarkSkies", "folder": "DarkSkies/Other/Templates", "templates": ["TITest"]},
        ])
    );
    for (path, rel) in [
        (&loc, "TerraInvicta_Data/StreamingAssets/Localization/en/a.en"),
        (&dlc_loc, "DLC_Content/DarkSkies/elsewhere/a.en"),
        (&dlc, "DLC_Content/DarkSkies/Other/Templates/TITest.json"),
    ] {
        assert_eq!(manifest["sources"][rel], sha256_file(path).unwrap());
    }
    let names: Vec<_> = std::fs::read_dir(build.join("locale/en")).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["strings.json"]);
    let strings = std::fs::read_to_string(build.join("locale/en/strings.json")).unwrap();
    assert_eq!(strings, "{\n \"a\": \"override\",\n \"z\": \"\"\n}\n");
    assert_eq!(
        read(&build.join("templates/base/TITest.json")),
        json!([{"dataName": "x", "disable": true, "scenarioTags": ["tag"]}])
    );
    assert_eq!(
        read(&build.join("templates/dlc.darkskies.other.templates/TITest.json")),
        json!([{"dataName": "x", "extra": 2}])
    );
    assert_eq!(
        read(&output.join("index.json")),
        json!({"latest": "test", "builds": [{"dir": "test", "buildId": "test", "lastUpdated": null, "formatVersion": 1}]})
    );

    // A second run leaves the build alone.
    let report = extract(&root, &options(&output)).unwrap();
    assert!(!report.extracted);
    assert!(
        extract(
            &root,
            &ExtractOptions {
                force: true,
                ..options(&output)
            }
        )
        .unwrap()
        .extracted
    );
}

#[test]
fn parse_errors_exit_after_reports() {
    for allow in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("game");
        write(&root, "TerraInvicta_Data/StreamingAssets/Templates/Bad.json", br#"[{"dataName": ]"#);
        let output = tmp.path().join("out");
        assert_eq!(
            cli(&root, &output, if allow { &["--allow-parse-errors"] } else { &[] }),
            if allow { 0 } else { 1 }
        );
        let report = read(&output.join("test/reports/issues.json"));
        assert_eq!(
            report["issues"][0],
            json!({"kind": "parse_error", "layer": "base", "template": "Bad", "error": "Expecting value: line 1 column 15 (char 14)"})
        );

        // The library always finishes and reports the count; parse errors force a re-extract.
        let report = extract(&root, &options(&output)).unwrap();
        assert!(report.extracted);
        assert_eq!(report.parse_errors, 1);
    }
}

#[test]
fn fixed_template_replaces_build_with_parse_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    let bad = write(&root, "TerraInvicta_Data/StreamingAssets/Templates/Bad.json", br#"[{"dataName": ]"#);
    let output = tmp.path().join("out");
    assert_eq!(cli(&root, &output, &[]), 1);

    // Same build id, template repaired: the stored build is redone, not reused.
    std::fs::write(&bad, br#"[{"dataName": "x"}]"#).unwrap();
    assert_eq!(cli(&root, &output, &[]), 0);
    assert_eq!(read(&output.join("test/templates/base/Bad.json")), json!([{"dataName": "x"}]));
    assert_eq!(read(&output.join("test/reports/issues.json"))["issues"], json!([]));
    assert!(!extract(&root, &options(&output)).unwrap().extracted);
}

#[test]
fn locked_data_dir_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    write(&root, "TerraInvicta_Data/StreamingAssets/Templates/TITest.json", br#"[{"dataName": "x"}]"#);
    let output = tmp.path().join("out");
    let lock = ti_extract::DataDirLock::acquire(&output).unwrap();
    assert_ne!(cli(&root, &output, &[]), 0);
    assert!(!output.join("test").exists());
    assert!(ti_extract::write_index(&output).is_err());
    drop(lock);
    assert_eq!(cli(&root, &output, &[]), 0);
}

#[test]
fn nonfinite_output_and_report() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    write(
        &root,
        "TerraInvicta_Data/StreamingAssets/Templates/Test.json",
        br#"[{"dataName":"x", "nested":[NaN, Infinity, -Infinity, 1e999]}]"#,
    );
    let output = tmp.path().join("out");
    let report = extract(&root, &options(&output)).unwrap();
    assert_eq!(report.issues, 4);
    let build = output.join("test");
    assert_eq!(read(&build.join("templates/base/Test.json"))[0]["nested"], json!([null, null, null, null]));
    let issues = read(&build.join("reports/issues.json"))["issues"].clone();
    let expected: Vec<_> = (0..4)
        .map(|i| json!({"kind": "non_finite_number", "template": "Test", "layer": "base", "dataName": "x", "path": format!("$.nested[{i}]")}))
        .collect();
    assert_eq!(issues, Value::Array(expected));
}

#[test]
fn local_build_id_and_progress() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    write(&root, "TerraInvicta_Data/StreamingAssets/Templates/TIé.json", b"[]");
    let output = tmp.path().join("out");
    let install = ti_extract::discover(&root).unwrap();
    let mut events = Vec::new();
    let opts = ExtractOptions {
        images: Images::None,
        ..ExtractOptions::new(&output)
    };
    let report = ti_extract::extract(&install, &opts, &mut |p| events.push(p), &AtomicBool::new(false)).unwrap();
    // sha256 of Python's json.dumps(sources, sort_keys=True), ensure_ascii escaping the file name
    let digest = ti_extract::output::sha256_bytes(
        format!(
            "{{\"TerraInvicta_Data/StreamingAssets/Templates/TI\\u00e9.json\": \"{}\"}}",
            ti_extract::output::sha256_bytes(b"[]")
        )
        .as_bytes(),
    );
    assert_eq!(report.build_id, format!("local-{}", &digest[..12]));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Progress::Log { message } if message.starts_with("no Steam build id found")))
    );
    assert!(events.iter().any(|e| matches!(e, Progress::Step { done: 1, total: 1, .. })));
}

#[test]
fn cancel_removes_partial_build() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    write(&root, "TerraInvicta_Data/StreamingAssets/Templates/A.json", b"[]");
    write(&root, "TerraInvicta_Data/StreamingAssets/Localization/en/a.en", b"a=b\n");
    let output = tmp.path().join("out");
    let install = ti_extract::discover(&root).unwrap();
    let cancel = AtomicBool::new(false);
    let mut on_progress = |p: Progress| {
        if matches!(
            p,
            Progress::Stage {
                stage: ti_extract::Stage::Locale
            }
        ) {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    };
    let result = ti_extract::extract(&install, &options(&output), &mut on_progress, &cancel);
    assert!(matches!(result, Err(Error::Cancelled)));
    let names: Vec<_> = std::fs::read_dir(&output).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, [".lock"]);
}

#[test]
fn unsafe_build_ids_are_rejected_from_options_and_install() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    write(&root, "TerraInvicta_Data/StreamingAssets/Templates/A.json", b"[]");
    let output = tmp.path().join("out");
    let mut install = ti_extract::discover(&root).unwrap();
    // An empty id counts as missing (like Python): it falls back to a `local-<digest>` id.
    install.build_id = Some(String::new());
    let report = ti_extract::extract(
        &install,
        &ExtractOptions {
            build_id: Some(String::new()),
            ..options(&output)
        },
        &mut |_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(report.build_id.starts_with("local-"), "{}", report.build_id);
    std::fs::remove_dir_all(&output).unwrap();
    for id in [".", "..", ".hidden", "../x", "/absolute", "a/b", "a\\b", "C:x", "a\0b", "a\nb", "a\u{7f}b"] {
        for from_install in [false, true] {
            install.build_id = Some(id.into());
            let opts = ExtractOptions {
                build_id: if from_install { None } else { Some(id.into()) },
                force: true,
                ..options(&output)
            };
            let error = ti_extract::extract(&install, &opts, &mut |_| {}, &AtomicBool::new(false)).unwrap_err();
            assert!(
                matches!(&error, Error::Other(message) if message.contains("invalid build id")),
                "{id:?}: {error}"
            );
            assert!(!output.exists());
        }
    }
}

#[test]
fn refuses_unrecognized_build_directory_even_with_force() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    write(&root, "TerraInvicta_Data/StreamingAssets/Templates/A.json", b"[]");
    let output = tmp.path().join("out");
    let sentinel = write(&output, "test/keep.txt", b"keep me");
    for force in [false, true] {
        let error = extract(&root, &ExtractOptions { force, ..options(&output) }).unwrap_err();
        assert!(
            matches!(&error, Error::Other(message) if message.contains("no manifest.json") && message.contains(&output.join("test").display().to_string()))
        );
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep me");
        assert!(!output.join(".test.old").exists());
    }
}

#[test]
fn stale_partial_is_removed_and_deep_templates_report_parse_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("game");
    let text = format!("{}0{}", "[".repeat(2000), "]".repeat(2000));
    write(&root, "TerraInvicta_Data/StreamingAssets/Templates/Deep.json", text.as_bytes());
    let output = tmp.path().join("out");
    write(&output, ".test.partial/stale.txt", b"stale");
    let report = extract(&root, &options(&output)).unwrap();
    assert_eq!(report.parse_errors, 1);
    let issues = read(&output.join("test/reports/issues.json"));
    assert_eq!(issues["issues"][0]["kind"], "parse_error");
    assert!(issues["issues"][0]["error"].as_str().unwrap().contains("Maximum nesting depth exceeded"));
    assert!(!output.join("test/stale.txt").exists());
    assert!(!output.join(".test.partial").exists());
}
