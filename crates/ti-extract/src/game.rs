//! Locating the game install and reading its data layers.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::output::io_error;
use crate::{Error, GAME_DIR_ENV, Result};

/// Steam app id of Terra Invicta.
pub const APP_ID: &str = "1176470";
pub const INSTALL_DIR_NAME: &str = "Terra Invicta";
pub const BASE_LAYER: &str = "base";

/// A folder of template JSON files: the base game or one DLC folder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Layer {
    /// `base`, or derived from the DLC folder (see [`layer_id`]).
    pub id: String,
    /// `base` or `dlc`.
    pub kind: String,
    pub templates_dir: PathBuf,
    pub dlc: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstall {
    pub root: PathBuf,
    /// Game version from the game files, e.g. `1.0.53b`.
    pub version: Option<String>,
    /// Steam build id from `appmanifest_<appid>.acf`; `None` outside Steam.
    pub build_id: Option<String>,
    /// Unix time of the last Steam update.
    pub last_updated: Option<i64>,
    pub layers: Vec<Layer>,
    pub bundle_dirs: Vec<PathBuf>,
}

impl GameInstall {
    pub fn streaming_assets(&self) -> PathBuf {
        self.root.join("TerraInvicta_Data/StreamingAssets")
    }

    /// Localization folder names, sorted (`en`, `deu`, ...).
    pub fn languages(&self) -> Vec<String> {
        let mut langs: Vec<String> = subdirs(&self.streaming_assets().join("Localization"))
            .iter()
            .filter_map(|p| file_name(p))
            .collect();
        langs.sort();
        langs
    }
}

pub(crate) fn file_name(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().into_owned())
}

/// Sub-directories of `dir`, sorted; empty if `dir` is unreadable.
pub(crate) fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// `path` relative to `base` with `/` separators (Python's `relative_to(base).as_posix()`).
pub(crate) fn rel_posix(path: &Path, base: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// All files under `dir` (recursively) whose name ends in `suffix`.
pub(crate) fn files_recursive(dir: &Path, suffix: &str, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_recursive(&path, suffix, out);
        } else if path.is_file() && has_suffix(&path, suffix) {
            out.push(path);
        }
    }
}

/// Files directly in `dir` whose name ends in `suffix` (a glob `*<suffix>`).
pub(crate) fn files_in(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && has_suffix(p, suffix))
        .collect()
}

fn has_suffix(path: &Path, suffix: &str) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    // Globs are case-insensitive on Windows, as in Python.
    if cfg!(windows) {
        name.to_lowercase().ends_with(&suffix.to_lowercase())
    } else {
        name.ends_with(suffix)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

#[cfg(windows)]
fn registry_steam_roots() -> Vec<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let read = |hive, key: &str, value: &str| RegKey::predef(hive).open_subkey(key).and_then(|k| k.get_value::<String, _>(value)).ok();
    [
        read(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath"),
        read(HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect()
}

#[cfg(not(windows))]
fn registry_steam_roots() -> Vec<PathBuf> {
    Vec::new()
}

fn steam_roots() -> Vec<PathBuf> {
    let mut candidates = registry_steam_roots();
    if let Some(home) = home_dir() {
        candidates.extend([
            home.join(".local/share/Steam"),
            home.join(".steam/steam"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ]);
    }
    candidates.push(PathBuf::from("C:/Program Files (x86)/Steam"));
    if let Some(home) = home_dir() {
        candidates.push(home.join("Library/Application Support/Steam"));
    }
    candidates.into_iter().filter(|p| p.join("steamapps").is_dir()).collect()
}

/// Python's `re.findall(r'"([^"]+)"\s+"([^"]*)"', text)`.
pub fn vdf_pairs(text: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut pairs = Vec::new();
    let mut i = 0;
    'scan: while i < chars.len() {
        let start = i;
        i += 1;
        if chars[start] != '"' {
            continue;
        }
        let Some(key_end) = (start + 1..chars.len()).find(|&j| chars[j] == '"') else {
            continue;
        };
        if key_end == start + 1 {
            continue;
        }
        let mut j = key_end + 1;
        let ws = j;
        while j < chars.len() && crate::jsonc::py_isspace(chars[j]) {
            j += 1;
        }
        if j == ws || chars.get(j) != Some(&'"') {
            continue;
        }
        let Some(value_end) = (j + 1..chars.len()).find(|&k| chars[k] == '"') else {
            continue 'scan;
        };
        pairs.push((chars[start + 1..key_end].iter().collect(), chars[j + 1..value_end].iter().collect()));
        i = value_end + 1;
    }
    pairs
}

fn read_lossy(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn library_folders(steam_root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam_root.to_path_buf()];
    if let Some(text) = read_lossy(&steam_root.join("steamapps/libraryfolders.vdf")) {
        libs.extend(
            vdf_pairs(&text)
                .into_iter()
                .filter(|(k, _)| k == "path")
                .map(|(_, v)| PathBuf::from(v.replace("\\\\", "\\"))),
        );
    }
    libs
}

/// The first Steam library containing Terra Invicta.
pub fn find_game_dir() -> Option<PathBuf> {
    steam_roots()
        .iter()
        .flat_map(|root| library_folders(root))
        .map(|lib| lib.join("steamapps/common").join(INSTALL_DIR_NAME))
        .find(|candidate| candidate.join("TerraInvicta_Data").is_dir())
}

/// Top-level key/values of `appmanifest_<appid>.acf` next to the install.
pub fn read_app_manifest(game_dir: &Path) -> Vec<(String, String)> {
    let Some(steamapps) = game_dir.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    let Some(text) = read_lossy(&steamapps.join(format!("appmanifest_{APP_ID}.acf"))) else {
        return Vec::new();
    };
    let mut values: Vec<(String, String)> = Vec::new();
    for (k, v) in vdf_pairs(&text) {
        // first occurrence = top level
        if !values.iter().any(|(key, _)| *key == k) {
            values.push((k, v));
        }
    }
    values
}

/// Prefix the complete DLC-relative path, retaining a trailing Templates.
///
/// `DarkSkies/2003_Scenario/Templates` -> `dlc.darkskies.2003_scenario.templates`.
/// Keeping every directory component avoids conflating different source folders.
pub fn layer_id(relative_folder: &str) -> String {
    format!("dlc.{}", relative_folder.to_lowercase().replace('/', ".").replace(' ', "_"))
}

/// End of `\d+\.\d+\.\d+[a-z]?\d*` matched at `i`, if any.
fn match_version(data: &[u8], i: usize) -> Option<usize> {
    let digits = |mut j: usize| {
        while j < data.len() && data[j].is_ascii_digit() {
            j += 1;
        }
        j
    };
    let mut j = i;
    for part in 0..3 {
        let end = digits(j);
        if end == j {
            return None;
        }
        j = end;
        if part < 2 {
            if data.get(j) != Some(&b'.') {
                return None;
            }
            j += 1;
        }
    }
    if data.get(j).is_some_and(u8::is_ascii_lowercase) {
        j += 1;
    }
    Some(digits(j))
}

/// Full match of `\d{4}\.\d+\.\d+[abfp]\d+`.
fn is_unity_version(v: &[u8]) -> bool {
    let s = std::str::from_utf8(v).unwrap_or("");
    let mut parts = s.splitn(3, '.');
    let (Some(year), Some(minor), Some(rest)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let Some(letter) = rest.find(|c: char| !c.is_ascii_digit()) else {
        return false;
    };
    year.len() == 4 && all_digits(year) && all_digits(minor) && letter > 0 && "abfp".contains(&rest[letter..letter + 1]) && all_digits(&rest[letter + 1..])
}

/// Find a length-prefixed bundleVersion, excluding the Unity engine version.
pub fn extract_version(data: &[u8]) -> Option<String> {
    let mut i = 0;
    while i < data.len() {
        if !data[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let Some(end) = match_version(data, i) else {
            // Every later start in this digit run fails the same way.
            while i < data.len() && data[i].is_ascii_digit() {
                i += 1;
            }
            continue;
        };
        let value = &data[i..end];
        if i >= 4 && !is_unity_version(value) && i32::from_le_bytes(data[i - 4..i].try_into().unwrap()) as i64 == value.len() as i64 {
            return Some(String::from_utf8_lossy(value).into_owned());
        }
        i = end;
    }
    None
}

pub fn read_game_version(root: &Path) -> Option<String> {
    std::fs::read(root.join("TerraInvicta_Data/globalgamemanagers"))
        .ok()
        .and_then(|data| extract_version(&data))
}

/// Read an install. Returns `Error::NotAnInstall` if `root` is missing or has no
/// `TerraInvicta_Data/StreamingAssets/Templates` directory.
pub fn discover(root: &Path) -> Result<GameInstall> {
    let sa = root.join("TerraInvicta_Data/StreamingAssets");
    if !sa.join("Templates").is_dir() {
        return Err(Error::NotAnInstall(format!("no Templates directory under {}", sa.display())));
    }
    let mut layers = vec![Layer {
        id: BASE_LAYER.into(),
        kind: "base".into(),
        templates_dir: sa.join("Templates"),
        dlc: None,
        folder: None,
    }];
    let mut bundle_dirs = vec![sa.join("AssetBundles")];

    let dlc_root = root.join("DLC_Content");
    if dlc_root.is_dir() {
        bundle_dirs.extend(subdirs(&dlc_root).into_iter().map(|d| d.join("AssetBundles")).filter(|d| d.is_dir()));
        let mut files = Vec::new();
        files_recursive(&dlc_root, ".json", &mut files);
        let directories: BTreeSet<String> = files.iter().filter_map(|p| p.parent()).map(|d| rel_posix(d, &dlc_root)).collect();
        for relative in directories {
            let dlc = relative.split('/').next().unwrap_or_default().to_owned();
            layers.push(Layer {
                id: layer_id(&relative),
                kind: "dlc".into(),
                templates_dir: dlc_root.join(&relative),
                dlc: Some(dlc),
                folder: Some(relative),
            });
        }
    }

    let ids: BTreeSet<&str> = layers.iter().map(|l| l.id.as_str()).collect();
    if ids.len() != layers.len() {
        return Err(Error::Other(format!(
            "layer id collision: {:?}",
            layers.iter().map(|l| &l.id).collect::<Vec<_>>()
        )));
    }

    let manifest = read_app_manifest(root);
    let get = |key: &str| manifest.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let last_updated = get("LastUpdated")
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|v| v.parse().ok());
    Ok(GameInstall {
        root: root.to_path_buf(),
        version: read_game_version(root),
        build_id: get("buildid"),
        last_updated,
        layers,
        bundle_dirs: bundle_dirs.into_iter().filter(|d| d.is_dir()).collect(),
    })
}

fn expand_user(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    match (s.strip_prefix('~'), home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(['/', '\\']) => home.join(rest.trim_start_matches(['/', '\\'])),
        _ => path.to_path_buf(),
    }
}

/// `explicit`, else `TI_GAME_DIR`, else [`find_game_dir`].
pub fn resolve_game_dir(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit.filter(|p| !p.as_os_str().is_empty()) {
        return Ok(expand_user(path));
    }
    if let Some(env) = std::env::var_os(GAME_DIR_ENV).filter(|v| !v.is_empty()) {
        return Ok(expand_user(Path::new(&env)));
    }
    find_game_dir().ok_or(Error::GameNotFound)
}

/// Read a file, naming it in errors.
pub(crate) fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| io_error(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefixed(value: &[u8]) -> Vec<u8> {
        let mut out = (value.len() as i32).to_le_bytes().to_vec();
        out.extend_from_slice(value);
        out
    }

    #[test]
    fn layer_ids() {
        for (folder, expected) in [
            ("DarkSkies/2003_Scenario/Templates", "dlc.darkskies.2003_scenario.templates"),
            ("DarkSkies/Broken Earth Scenario/Templates", "dlc.darkskies.broken_earth_scenario.templates"),
            ("Other DLC/Nested/Extras", "dlc.other_dlc.nested.extras"),
        ] {
            assert_eq!(layer_id(folder), expected);
        }
    }

    #[test]
    fn versions() {
        for version in [&b"1.0.53b"[..], b"1.2.3", b"1.0.53b12"] {
            let mut blob = b"noise1.2.3\x00".to_vec();
            blob.extend(prefixed(b"2020.3.49f1"));
            blob.push(0);
            blob.extend(prefixed(version));
            blob.push(0);
            assert_eq!(extract_version(&blob).as_deref(), Some(std::str::from_utf8(version).unwrap()));
        }
        for blob in [b"".to_vec(), b"1.0.53b".to_vec(), b"\x06\x00\x00\x001.0.53b".to_vec(), prefixed(b"2020.3.49f1")] {
            assert_eq!(extract_version(&blob), None);
        }
    }

    #[test]
    fn vdf() {
        let text = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\Games\\\\Steam\"\n\t\t\"label\"\t\t\"\"\n\t}\n}";
        assert_eq!(
            vdf_pairs(text),
            [("path".into(), "D:\\\\Games\\\\Steam".into()), ("label".into(), String::new())]
        );
    }
}
