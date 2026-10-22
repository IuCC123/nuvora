//! Production serving of the web app, embedded in the binary.
//!
//! Embedded (rust-embed) rather than served from a directory: one file to ship
//! for Windows, macOS, NAS and Docker (Step 5), server and UI can never be from
//! different versions, and there is no path to configure. In debug builds
//! rust-embed reads from disk, so `cargo run` picks up `bun run build` output
//! without recompiling. Day-to-day UI work uses the Vite dev server instead.

use axum::http::Uri;
use axum::response::Response;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../apps/web/dist"]
struct WebAssets;

/// Router fallback for every non-`/api` path:
/// - an existing asset => the file; `/assets/*` (Vite's hashed names) get
///   `Cache-Control: public, max-age=31536000, immutable`, everything else
///   `no-cache`;
/// - any other path => `index.html` (client-side routing), `no-cache`;
/// - `/api/*` that reached the fallback => 404 JSON, never index.html.
pub async fn serve_app(uri: Uri) -> Response {
    let _ = (uri, WebAssets::get("index.html"));
    todo!()
}
