//! An in-process server for handler tests: a temp data dir, one empty movies
//! folder, and requests sent through the real router (guards, CSRF and all).

use std::num::NonZeroUsize;
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

pub struct TestApp {
    _dir: tempfile::TempDir,
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
        })
        .await
        .unwrap();
        let state = AppState {
            library,
            auth: Default::default(),
            server: Arc::new(ServerSetup {
                data_dir: dir.path().join("data"),
                bind: "127.0.0.1:8640".parse().unwrap(),
                tools,
                started: Instant::now(),
            }),
        };
        TestApp {
            router: super::router(state.clone()).fallback(crate::web::serve_app),
            _dir: dir,
            state,
        }
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
