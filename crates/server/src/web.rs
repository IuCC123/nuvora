//! Production serving of the web app, embedded in the binary.
//!
//! Embedded (rust-embed) rather than served from a directory: one file to ship
//! for Windows, macOS, NAS and Docker (Step 5), server and UI can never be from
//! different versions, and there is no path to configure. In debug builds
//! rust-embed reads from disk, so `cargo run` picks up `bun run build` output
//! without recompiling. Day-to-day UI work uses the Vite dev server instead.

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::{EmbeddedFile, RustEmbed};

#[derive(RustEmbed)]
#[folder = "../../apps/web/dist"]
struct WebAssets;

const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const NO_CACHE: &str = "no-cache";

/// Router fallback for every non-`/api` path:
/// - an existing asset => the file; `/assets/*` (Vite's hashed names) get
///   `Cache-Control: public, max-age=31536000, immutable`, everything else
///   `no-cache`;
/// - any other path => `index.html` (client-side routing), `no-cache`;
/// - `/api/*` that reached the fallback => 404 `ErrorBody`, never index.html;
/// - no `index.html` in the embedded (or, in debug, on-disk) dist => [`not_built`].
pub async fn serve_app(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return crate::api::ApiError::not_found().into_response();
    }
    let Some(index) = WebAssets::get("index.html") else {
        return not_built();
    };
    match WebAssets::get(path) {
        Some(file) if !path.is_empty() => asset(path, file),
        _ => asset("index.html", index),
    }
}

fn asset(path: &str, file: EmbeddedFile) -> Response {
    let cache = if path.starts_with("assets/") {
        IMMUTABLE
    } else {
        NO_CACHE
    };
    let mime = file.metadata.mimetype().to_owned();
    (
        [(CONTENT_TYPE, mime), (CACHE_CONTROL, cache.to_owned())],
        file.data.into_owned(),
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
