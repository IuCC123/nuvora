//! Production serving of the web app, embedded in the binary.
//!
//! Embedded (rust-embed) rather than served from a directory: one file to ship
//! for Windows, macOS, NAS and Docker (Step 5), server and UI can never be from
//! different versions, and there is no path to configure. In debug builds
//! rust-embed reads from disk, so `cargo run` picks up `bun run build` output
//! without recompiling. Day-to-day UI work uses the Vite dev server instead;
//! on a test server, `--web-dir` serves a folder `scripts/deploy-dev.sh` fills.

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use rust_embed::{EmbeddedFile, RustEmbed};

#[derive(RustEmbed)]
#[folder = "../../apps/web/dist"]
struct WebAssets;

/// The newest TV app, attached to every GitHub release under one name.
pub const TV_APP_URL: &str =
    "https://github.com/IuCC123/nuvora/releases/latest/download/nuvora-tv.apk";

/// Set by `--web-dir`: serve this folder instead of the embedded app.
static WEB_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn serve_from(dir: PathBuf) {
    let _ = WEB_DIR.set(dir);
}

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const NO_CACHE: &str = "no-cache";

/// Router fallback for every non-`/api` path:
/// - an existing asset => the file; `/assets/*` (Vite's hashed names) get
///   `Cache-Control: public, max-age=31536000, immutable`, everything else
///   `no-cache`;
/// - any other path => `index.html` (client-side routing), `no-cache`;
/// - `/api/*` that reached the fallback => 404 `ErrorBody`, never index.html;
/// - `/tv` => a redirect to the TV app's download ([`TV_APP_URL`]): short
///   enough to type with a remote, in Downloader on a Fire TV;
/// - no `index.html` in the embedded (or, in debug, on-disk) dist => [`not_built`].
pub async fn serve_app(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return crate::api::ApiError::not_found().into_response();
    }
    if matches!(path, "tv" | "tv/" | "tv.apk") {
        return Redirect::temporary(TV_APP_URL).into_response();
    }
    if let Some(dir) = WEB_DIR.get() {
        return from_dir(dir, path).await;
    }
    let Some(index) = WebAssets::get("index.html") else {
        return not_built();
    };
    match WebAssets::get(path) {
        Some(file) if !path.is_empty() => embedded(path, file),
        _ => embedded("index.html", index),
    }
}

fn embedded(path: &str, file: EmbeddedFile) -> Response {
    let mime = file.metadata.mimetype().to_owned();
    asset(path, mime, file.data.into_owned())
}

/// Like the embedded app, read from `dir` on every request.
async fn from_dir(dir: &Path, path: &str) -> Response {
    let inside = Path::new(path)
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    if inside
        && !path.is_empty()
        && let Ok(data) = tokio::fs::read(dir.join(path)).await
    {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return asset(path, mime.to_string(), data);
    }
    match tokio::fs::read(dir.join("index.html")).await {
        Ok(data) => asset("index.html", "text/html".to_owned(), data),
        Err(_) => not_built(),
    }
}

fn asset(path: &str, mime: String, data: Vec<u8>) -> Response {
    let cache = if path.starts_with("assets/") {
        IMMUTABLE
    } else {
        NO_CACHE
    };
    (
        [(CONTENT_TYPE, mime), (CACHE_CONTROL, cache.to_owned())],
        data,
    )
        .into_response()
}

/// `build.rs` creates an empty dist so a fresh clone compiles; say how to fill
/// it instead of serving a blank page.
fn not_built() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "The Nuvora web app is not built. Run `bun install && bun run build`, then restart nuvora.\n",
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;

    use super::*;

    async fn get(dir: &Path, path: &str) -> (String, String, String) {
        let res = from_dir(dir, path).await;
        let header = |h| res.headers()[h].to_str().unwrap().to_owned();
        let (mime, cache) = (header(CONTENT_TYPE), header(CACHE_CONTROL));
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (mime, cache, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn a_web_dir_serves_its_files_and_index_for_the_rest() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("web");
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), "index").unwrap();
        std::fs::write(dir.join("assets/app-1.js"), "js").unwrap();
        std::fs::write(root.path().join("secret"), "secret").unwrap();

        let js = get(&dir, "assets/app-1.js").await;
        assert_eq!(
            js,
            ("text/javascript".into(), IMMUTABLE.into(), "js".into())
        );
        let index = ("text/html".into(), NO_CACHE.into(), "index".into());
        assert_eq!(get(&dir, "").await, index);
        assert_eq!(get(&dir, "shows/283").await, index);
        assert_eq!(get(&dir, "assets").await, index);
        assert_eq!(get(&dir, "../secret").await, index);
    }
}
