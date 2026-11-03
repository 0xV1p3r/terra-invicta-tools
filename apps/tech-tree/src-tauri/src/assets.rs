//! `tiasset` URI scheme: serves image files of the build's `assets/` directory.
//!
//! The frontend builds URLs with `convertFileSrc(path, 'tiasset')`, i.e.
//! `tiasset://localhost/<encodeURIComponent(path)>` (Linux/macOS) or
//! `http://tiasset.localhost/<…>` (Windows). Both carry the asset path as the
//! (percent-encoded) URI path, so only the path is looked at.

use std::path::{Component, Path, PathBuf};

use tauri::http::{Request, Response, StatusCode, header};

pub const SCHEME: &str = "tiasset";

/// Asset path (relative to the assets dir) of a request URI path, or `None` if it is unsafe.
pub fn relative_path(uri_path: &str) -> Option<PathBuf> {
    let decoded = percent_encoding::percent_decode_str(uri_path.strip_prefix('/').unwrap_or(uri_path)).decode_utf8().ok()?;
    let rel = Path::new(&*decoded);
    if decoded.is_empty() || decoded.contains('\\') {
        return None;
    }
    rel.components().all(|c| matches!(c, Component::Normal(_))).then(|| rel.to_path_buf())
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn status(code: StatusCode, msg: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .header(header::CONTENT_TYPE, "text/plain")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(msg.as_bytes().to_vec())
        .expect("valid response")
}

/// Answer an asset request from canonical `assets_dir` (`None`: unavailable → 404).
/// Symlinks within the root are allowed; targets outside it return 403.
pub fn respond(assets_dir: Option<&Path>, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(dir) = assets_dir else { return status(StatusCode::NOT_FOUND, "no data loaded") };
    let Some(rel) = relative_path(request.uri().path()) else {
        return status(StatusCode::BAD_REQUEST, "invalid asset path");
    };
    let path = dir.join(&rel);
    let Ok(target) = path.canonicalize() else {
        return status(StatusCode::NOT_FOUND, "asset not found");
    };
    if !target.starts_with(dir) {
        return status(StatusCode::FORBIDDEN, "asset outside root");
    }
    if !target.is_file() {
        return status(StatusCode::NOT_FOUND, "asset not found");
    }
    // The remaining TOCTOU window between checking and opening is acceptable for
    // this local, read-only data directory. Read only after validating the target.
    match std::fs::read(&target) {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, content_type(&path))
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
            .body(bytes)
            .expect("valid response"),
        Err(_) => status(StatusCode::NOT_FOUND, "asset not found"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_rejects() {
        assert_eq!(relative_path("/icons_2d%2Ftech_energy_icon.png"), Some(PathBuf::from("icons_2d/tech_energy_icon.png")));
        assert_eq!(relative_path("/icons_2d/a%20b.png"), Some(PathBuf::from("icons_2d/a b.png")));
        assert_eq!(relative_path("/..%2Fmanifest.json"), None);
        assert_eq!(relative_path("/a/../../x"), None);
        assert_eq!(relative_path("/%2Fetc%2Fpasswd"), None);
        assert_eq!(relative_path("//etc/passwd"), None);
        assert_eq!(relative_path("/a%5C..%5Cb"), None);
        assert_eq!(relative_path("/"), None);
        assert_eq!(relative_path("/./x.png"), None);
    }

    struct TempAssets(PathBuf);

    impl TempAssets {
        fn new() -> Self {
            let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("tiasset-{}-{unique}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }

    impl Drop for TempAssets {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn request_asset(root: &Path, asset: &str, expected: StatusCode) {
        for origin in ["tiasset://localhost", "http://tiasset.localhost"] {
            let request = Request::builder().uri(format!("{origin}/{asset}")).body(Vec::new()).unwrap();
            let response = respond(Some(root), &request);
            assert_eq!(response.status(), expected, "{origin}/{asset}");
            assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
            if expected == StatusCode::OK {
                assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
                assert_eq!(response.body(), b"image contents");
            }
        }
    }

    #[test]
    fn serves_temp_assets_and_rejects_unsafe_paths() {
        let temp = TempAssets::new();
        std::fs::write(temp.0.join("normal.png"), b"image contents").unwrap();
        request_asset(&temp.0, "normal.png", StatusCode::OK);
        request_asset(&temp.0, "%2e%2e%2Foutside.png", StatusCode::BAD_REQUEST);
        request_asset(&temp.0, "missing.png", StatusCode::NOT_FOUND);
        std::fs::create_dir(temp.0.join("directory.png")).unwrap();
        request_asset(&temp.0, "directory.png", StatusCode::NOT_FOUND);
    }

    #[cfg(unix)]
    #[test]
    fn contains_symlink_targets() {
        use std::os::unix::fs::symlink;

        let temp = TempAssets::new();
        let root = temp.0.join("assets");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(temp.0.join("outside.png"), b"private contents").unwrap();
        std::fs::write(root.join("normal.png"), b"image contents").unwrap();
        symlink(temp.0.join("outside.png"), root.join("leak.png")).unwrap();
        symlink(root.join("normal.png"), root.join("inside.png")).unwrap();
        symlink(&temp.0, root.join("outside-dir")).unwrap();
        request_asset(&root, "leak.png", StatusCode::FORBIDDEN);
        request_asset(&root, "outside-dir/outside.png", StatusCode::FORBIDDEN);
        // In-root symlinks preserve normal asset behavior.
        request_asset(&root, "inside.png", StatusCode::OK);
    }

    #[test]
    fn serves_both_url_forms() {
        let Some(b) = crate::commands::tests::backend() else { return };
        let dir = b.assets_dir.as_deref();
        for uri in [
            "tiasset://localhost/icons_2d%2Ftech_energy_icon.png",
            "http://tiasset.localhost/icons_2d%2Ftech_energy_icon.png",
        ] {
            let res = respond(dir, &Request::builder().uri(uri).body(Vec::new()).unwrap());
            assert_eq!(res.status(), StatusCode::OK, "{uri}");
            assert_eq!(res.headers()[header::CONTENT_TYPE], "image/png");
            assert!(res.body().starts_with(b"\x89PNG"));
        }
        let missing = Request::builder().uri("tiasset://localhost/nope.png").body(Vec::new()).unwrap();
        assert_eq!(respond(dir, &missing).status(), StatusCode::NOT_FOUND);
        let evil = Request::builder().uri("tiasset://localhost/..%2Fmanifest.json").body(Vec::new()).unwrap();
        assert_eq!(respond(dir, &evil).status(), StatusCode::BAD_REQUEST);
    }
}
