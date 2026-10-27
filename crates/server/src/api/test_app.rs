//! An in-process server for handler tests: a temp data dir, one empty movies
//! folder, and requests sent through the real router (guards, CSRF and all).

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, Request, StatusCode};
use nuvora_library::{Library, LibraryConfig, LibraryKind, LibrarySpec};
use nuvora_media::MediaTools;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{AppState, ServerSetup};
use crate::config::HardwareEncoding;
use crate::downloads::Engine;

pub struct TestApp {
    dir: tempfile::TempDir,
    pub router: axum::Router,
    pub state: AppState,
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|_| panic!("not JSON: {:?}", String::from_utf8_lossy(&self.body)))
    }

    /// `nuvora_session=<token>` from `Set-Cookie`, ready for a `Cookie` header.
    pub fn cookie(&self) -> String {
        let set = self
            .headers
            .get(SET_COOKIE)
            .expect("Set-Cookie")
            .to_str()
            .unwrap();
        set.split(';').next().unwrap().to_owned()
    }
}

pub const ADMIN_PASSWORD: &str = "correct horse";

impl TestApp {
    pub async fn new() -> TestApp {
        TestApp::behind(&[]).await
    }

    /// A server started with `--trusted-proxy` for each of `proxies`.
    pub async fn behind(proxies: &[&str]) -> TestApp {
        TestApp::build(proxies, HardwareEncoding::Off, Engine::Off).await
    }

    /// A server started with `--hardware-encoding <encoder>`. Only Macs have
    /// a graphics encoder to test.
    #[cfg(target_os = "macos")]
    pub async fn with_encoder(encoder: HardwareEncoding) -> TestApp {
        TestApp::build(&[], encoder, Engine::Off).await
    }

    /// A server whose BitTorrent engine reaches only peers on this machine.
    pub async fn with_downloads() -> TestApp {
        TestApp::build(&[], HardwareEncoding::Off, Engine::Loopback).await
    }

    async fn build(proxies: &[&str], encoder: HardwareEncoding, engine: Engine) -> TestApp {
        let dir = tempfile::tempdir().unwrap();
        let movies = dir.path().join("movies");
        std::fs::create_dir_all(&movies).unwrap();
        let tools = MediaTools::locate(None, None).await.unwrap();
        let library = Library::open(LibraryConfig {
            data_dir: dir.path().join("data"),
            libraries: vec![LibrarySpec {
                key: "movies".into(),
                name: "Movies".into(),
                kind: LibraryKind::Movies,
                roots: vec![movies],
            }],
            tools: tools.clone(),
            probe_concurrency: NonZeroUsize::new(1).unwrap(),
            settle_window: Duration::ZERO,
            online_metadata: false,
        })
        .await
        .unwrap();
        let downloads =
            crate::downloads::Downloads::start(library.clone(), &dir.path().join("data"), engine)
                .await;
        let streams = crate::streams::Streams::start(
            library.clone(),
            downloads.engine(),
            tools.clone(),
            &dir.path().join("data"),
        )
        .await
        .unwrap();
        let state = AppState {
            library,
            auth: Arc::new(crate::auth::Auth::new(crate::net::TrustedProxies::new(
                proxies.iter().map(|p| p.parse().unwrap()).collect(),
            ))),
            transcoder: crate::transcode::Transcoder::start(
                tools.clone(),
                &dir.path().join("data"),
                crate::tuning::Tuning::default(),
                Some(encoder),
            )
            .await,
            parties: Default::default(),
            downloads,
            streams,
            updater: crate::update::Updater::new(
                None,
                dir.path().join("nuvora"),
                dir.path().join("data"),
            ),
            server: Arc::new(ServerSetup {
                id: "0123456789abcdef0123456789abcdef".into(),
                announcer: None,
                data_dir: dir.path().join("data"),
                bind: "127.0.0.1:8640".parse().unwrap(),
                tools,
                started: Instant::now(),
            }),
        };
        TestApp {
            router: super::router(state.clone()).fallback(crate::web::serve_app),
            dir,
            state,
        }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    pub async fn send(&self, request: Request<Body>) -> Reply {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let body = axum::body::to_bytes(body, usize::MAX)
            .await
            .unwrap()
            .to_vec();
        Reply {
            status: parts.status,
            headers: parts.headers,
            body,
        }
    }

    /// A request as the web app sends it: JSON content type on anything but
    /// GET, the session cookie when given.
    pub async fn call(
        &self,
        method: &str,
        uri: &str,
        cookie: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        let mut request = Request::builder().method(method).uri(uri);
        if method != "GET" {
            request = request.header(CONTENT_TYPE, "application/json");
        }
        if let Some(cookie) = cookie {
            request = request.header(COOKIE, cookie);
        }
        let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
        self.send(request.body(body).unwrap()).await
    }

    pub async fn get(&self, uri: &str, cookie: &str) -> Reply {
        self.call("GET", uri, Some(cookie), None).await
    }

    /// Runs first-run setup for `admin` and returns their session cookie.
    pub async fn setup_admin(&self) -> String {
        let reply = self
            .call(
                "POST",
                "/api/setup",
                None,
                Some(json!({"username": "admin", "display_name": "Admin", "password": ADMIN_PASSWORD})),
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json());
        reply.cookie()
    }

    /// One second of video per folder name (`"Title (Year)"`) in the movies
    /// folder, then a scan; returns when the scan is done.
    pub async fn add_movies(&self, folders: &[&str]) {
        let files: Vec<String> = folders
            .iter()
            .map(|f| format!("movies/{f}/{f}.mp4"))
            .collect();
        self.clips(&files.iter().map(String::as_str).collect::<Vec<_>>());
        self.scan().await;
    }

    /// One second of video at each path under the test directory, each
    /// with distinct bytes (or the scan would see one file under two names).
    pub fn clips(&self, paths: &[&str]) {
        let first = self.path(paths[0]);
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        let ffmpeg = std::process::Command::new("ffmpeg")
            .args(["-y", "-nostdin", "-v", "error", "-f", "lavfi", "-i"])
            .args([
                "testsrc2=size=320x180:rate=24:duration=1",
                "-c:v",
                "libx264",
            ])
            .args(["-preset", "ultrafast", "-pix_fmt", "yuv420p"])
            .arg(&first)
            .status()
            .unwrap();
        assert!(ffmpeg.success());
        for (i, path) in paths.iter().enumerate().skip(1) {
            let file = self.path(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            let mut bytes = std::fs::read(&first).unwrap();
            bytes.extend(i.to_le_bytes());
            std::fs::write(file, bytes).unwrap();
        }
    }

    /// Requests a scan and returns when a scan started after the request is done.
    pub async fn scan(&self) {
        let before = std::time::SystemTime::now();
        let mut status = self.state.library.watch_scan();
        self.state.library.request_scan();
        status
            .wait_for(|s| {
                matches!(s.state, nuvora_library::ScanState::Idle)
                    && s.last.as_ref().is_some_and(|l| l.started_at >= before)
            })
            .await
            .unwrap();
    }

    /// A sign in arriving over TCP from `peer`, with `X-Forwarded-For` when given.
    pub async fn login_from(
        &self,
        peer: &str,
        forwarded_for: Option<&str>,
        username: &str,
        password: &str,
    ) -> Reply {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/auth/login")
            .header(CONTENT_TYPE, "application/json");
        if let Some(hops) = forwarded_for {
            request = request.header("x-forwarded-for", hops);
        }
        let body = json!({"username": username, "password": password}).to_string();
        self.send_from(peer, request.body(Body::from(body)).unwrap())
            .await
    }

    pub async fn send_from(&self, peer: &str, mut request: Request<Body>) -> Reply {
        let addr = std::net::SocketAddr::new(peer.parse().unwrap(), 50000);
        request
            .extensions_mut()
            .insert(axum::extract::ConnectInfo(addr));
        self.send(request).await
    }

    pub async fn login(&self, username: &str, password: &str) -> Reply {
        self.call(
            "POST",
            "/api/auth/login",
            None,
            Some(json!({"username": username, "password": password})),
        )
        .await
    }
}

/// A second BitTorrent engine sharing a file or folder, behind a one-route
/// tracker. Keep it alive for as long as the test downloads.
pub struct Seeder {
    _session: Arc<librqbit::Session>,
    /// A link to the `.torrent`, as an indexer gives one.
    pub link: String,
}

impl TestApp {
    /// Seeds `rel` (under the test's folder) on this machine.
    pub async fn seed(&self, rel: &str) -> Seeder {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http = listener.local_addr().unwrap();
        let session = librqbit::Session::new_with_opts(
            self.path(&format!("seeder-{}", http.port())),
            crate::downloads::loopback_options(),
        )
        .await
        .unwrap();
        let (created, _) = session
            .create_and_serve_torrent(
                &self.path(rel),
                librqbit::CreateTorrentOptions {
                    name: None,
                    trackers: vec![format!("http://{http}/announce")],
                    piece_length: None,
                },
            )
            .await
            .unwrap();
        let torrent = created.as_bytes().unwrap();
        let std::net::SocketAddr::V4(peer) = session.listen_addr().unwrap() else {
            panic!("the seeder listens on IPv4");
        };
        let mut announce = b"d8:intervali60e5:peers6:".to_vec();
        announce.extend(peer.ip().octets());
        announce.extend(peer.port().to_be_bytes());
        announce.push(b'e');
        let routes = axum::Router::new()
            .route(
                "/release.torrent",
                axum::routing::get(move || async move { torrent }),
            )
            .route(
                "/announce",
                axum::routing::get(move || async move { announce }),
            );
        tokio::spawn(async move { axum::serve(listener, routes).await });
        Seeder {
            _session: session,
            link: format!("http://{http}/release.torrent"),
        }
    }
}
