//! Sessions, passwords, sign-in rate limiting and CSRF. The HTTP-facing half of
//! accounts; the rows live in the catalog (`nuvora_library::accounts`).
//!
//! A session token is 32 random bytes, sent as 64 hex chars in the
//! `nuvora_session` cookie or an `Authorization: Bearer` header. The database
//! keeps only its SHA-256.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, HOST, ORIGIN, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, request::Parts};
use axum::middleware::Next;
use axum::response::Response;
use nuvora_library::{Role, TokenHash, User, WatchRev};
use password_hash::rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::api::{ApiError, AppState, SESSION_COOKIE};

pub const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
/// A session's expiry (and cookie) moves forward once it is this old.
const REFRESH_AFTER: Duration = Duration::from_secs(24 * 3600);
/// `last_seen_at` is written at most this often per session, not per request.
const LAST_SEEN_EVERY: Duration = Duration::from_secs(5 * 60);
const MAX_FAILURES: usize = 5;
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
const LOCKOUT: Duration = Duration::from_secs(15 * 60);

fn ms(d: Duration) -> i64 {
    d.as_millis() as i64
}

/// Shared auth state: the clock and the sign-in failure counts (in memory, so
/// a restart forgets them).
#[derive(Default)]
pub struct Auth {
    /// Added to the system time; tests move it forward to expire sessions.
    offset_ms: AtomicI64,
    limiter: Mutex<HashMap<String, Failures>>,
}

#[derive(Default)]
struct Failures {
    at: Vec<i64>,
    blocked_until: i64,
}

impl Auth {
    pub fn now_ms(&self) -> i64 {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, ms);
        now + self.offset_ms.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub fn advance(&self, by: Duration) {
        self.offset_ms.fetch_add(ms(by), Ordering::Relaxed);
    }

    /// Seconds until sign in is allowed again for any of `keys`, if blocked.
    pub fn blocked_for(&self, keys: &[String]) -> Option<u64> {
        let now = self.now_ms();
        let limiter = self.limiter.lock().unwrap_or_else(|p| p.into_inner());
        keys.iter()
            .filter_map(|k| limiter.get(k))
            .map(|f| f.blocked_until - now)
            .filter(|left| *left > 0)
            .max()
            .map(|left| (left as u64).div_ceil(1000))
    }

    /// Counts a failed sign in against each key; the fifth within the window
    /// blocks that key for [`LOCKOUT`].
    pub fn record_failure(&self, keys: &[String]) {
        let now = self.now_ms();
        let mut limiter = self.limiter.lock().unwrap_or_else(|p| p.into_inner());
        limiter.retain(|_, f| {
            f.blocked_until > now || f.at.last().is_some_and(|t| now - t < ms(FAILURE_WINDOW))
        });
        for key in keys {
            let failures = limiter.entry(key.clone()).or_default();
            failures.at.retain(|t| now - t < ms(FAILURE_WINDOW));
            failures.at.push(now);
            if failures.at.len() >= MAX_FAILURES {
                failures.at.clear();
                failures.blocked_until = now + ms(LOCKOUT);
            }
        }
    }

    pub fn record_success(&self, keys: &[String]) {
        let mut limiter = self.limiter.lock().unwrap_or_else(|p| p.into_inner());
        for key in keys {
            limiter.remove(key);
        }
    }
}

/// Rate-limit keys for a sign in: the username (ignoring case) and, when known,
/// the peer address. Behind a reverse proxy the address is the proxy's.
pub fn limiter_keys(username: &str, parts: &Parts) -> Vec<String> {
    let mut keys = vec![format!("user:{}", username.to_lowercase())];
    if let Some(ConnectInfo(addr)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
        keys.push(format!("ip:{}", addr.ip()));
    }
    keys
}

/// argon2id with the crate's default parameters. Runs on the blocking pool.
pub async fn hash_password(password: String) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::internal)
}

/// Verifies against `hash`, or against a fixed dummy hash when there is no
/// user, so an unknown username takes as long as a wrong password.
pub async fn verify_password(password: String, hash: Option<String>) -> bool {
    static DUMMY: LazyLock<String> = LazyLock::new(|| {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(b"not a password", &salt)
            .expect("hashing a constant")
            .to_string()
    });
    tokio::task::spawn_blocking(move || {
        let known = hash.is_some();
        let hash = hash.unwrap_or_else(|| DUMMY.clone());
        let ok = PasswordHash::new(&hash).is_ok_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        });
        known && ok
    })
    .await
    .unwrap_or(false)
}

/// A fresh token (64 hex chars) and the hash to store.
pub fn new_token() -> (String, TokenHash) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let text: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    (text, digest(&bytes))
}

fn digest(bytes: &[u8; 32]) -> TokenHash {
    TokenHash(Sha256::digest(bytes).into())
}

/// The hash of a presented token; `None` unless it is 64 hex chars.
fn parse_token(text: &str) -> Option<TokenHash> {
    if text.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(digest(&bytes))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Carrier {
    Cookie,
    Bearer,
}

/// The token a request carries (raw text, as the client holds it), from the
/// `Authorization: Bearer` header or else the session cookie.
fn presented_token(headers: &HeaderMap) -> Option<(String, Carrier)> {
    if let Some(auth) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        let token = auth.strip_prefix("Bearer ")?;
        return Some((token.trim().to_owned(), Carrier::Bearer));
    }
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| (value.to_owned(), Carrier::Cookie))
}

/// True when the request reached us over HTTPS, directly or via a proxy that
/// sets `X-Forwarded-Proto`.
fn is_https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("https"))
}

/// `Set-Cookie` for a session token, or an expired one that clears it.
pub fn session_cookie(token: Option<&str>, headers: &HeaderMap) -> HeaderValue {
    let secure = if is_https(headers) { "; Secure" } else { "" };
    let (value, max_age) = match token {
        Some(token) => (token, SESSION_TTL.as_secs()),
        None => ("", 0),
    };
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={value}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}{secure}"
    ))
    .expect("cookie is ascii")
}

/// The signed-in user of a request that passed [`require_member`] or
/// [`require_admin`].
#[derive(Clone)]
pub struct SignedIn {
    pub user: User,
    pub token: TokenHash,
    /// Read with the session, so never newer than anything read after it.
    pub watch_rev: WatchRev,
}

impl<S: Send + Sync> FromRequestParts<S> for SignedIn {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        parts
            .extensions
            .get::<SignedIn>()
            .cloned()
            .ok_or_else(ApiError::unauthorized)
    }
}

/// The session of a request, if valid. Does not record activity.
pub async fn current_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<SignedIn>, ApiError> {
    let Some(token) = presented_token(headers).and_then(|(text, _)| parse_token(&text)) else {
        return Ok(None);
    };
    let session = state
        .library
        .session(token, state.auth.now_ms())
        .await
        .map_err(ApiError::internal)?;
    Ok(session.map(|s| SignedIn {
        user: s.user,
        token,
        watch_rev: s.watch_rev,
    }))
}

pub async fn require_member(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    authenticate(state, req, next, Role::Member).await
}

pub async fn require_admin(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    authenticate(state, req, next, Role::Admin).await
}

/// Resolves the session, checks the role, slides the expiry, and hands the
/// handler a [`SignedIn`].
async fn authenticate(
    state: AppState,
    mut req: Request,
    next: Next,
    need: Role,
) -> Result<Response, ApiError> {
    let (text, carrier) = presented_token(req.headers()).ok_or_else(ApiError::unauthorized)?;
    let token = parse_token(&text).ok_or_else(ApiError::unauthorized)?;
    let now = state.auth.now_ms();
    let session = state
        .library
        .session(token, now)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    if need == Role::Admin && session.user.role != Role::Admin {
        return Err(ApiError::forbidden("Only an admin can do this"));
    }
    let refresh = session.expires_at - now < ms(SESSION_TTL) - ms(REFRESH_AFTER);
    if refresh || now - session.last_seen_at >= ms(LAST_SEEN_EVERY) {
        state
            .library
            .touch_session(token, now, refresh.then(|| now + ms(SESSION_TTL)))
            .await
            .map_err(ApiError::internal)?;
    }
    let resend_cookie = refresh && carrier == Carrier::Cookie;
    let headers = req.headers().clone();
    req.extensions_mut().insert(SignedIn {
        user: session.user,
        token,
        watch_rev: session.watch_rev,
    });
    let mut response = next.run(req).await;
    if resend_cookie && !response.headers().contains_key(SET_COOKIE) {
        response
            .headers_mut()
            .insert(SET_COOKIE, session_cookie(Some(&text), &headers));
    }
    Ok(response)
}

/// State-changing requests must be JSON, and when the browser says where it
/// came from (`Origin`), it must be this server. JSON forces a CORS preflight
/// on cross-site requests, which this server never approves.
pub async fn csrf(req: Request, next: Next) -> Result<Response, ApiError> {
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return Ok(next.run(req).await);
    }
    let json = req
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json {
        return Err(ApiError::forbidden(
            "Requests that change something must send Content-Type: application/json",
        ));
    }
    if let Some(origin) = req.headers().get(ORIGIN) {
        let origin_host = origin
            .to_str()
            .ok()
            .and_then(|o| o.split_once("://"))
            .map(|(_, host)| host.trim_end_matches('/'));
        let host = req
            .headers()
            .get(HOST)
            .and_then(|h| h.to_str().ok())
            .or_else(|| req.uri().authority().map(|a| a.as_str()));
        let same = matches!((origin_host, host), (Some(o), Some(h)) if o.eq_ignore_ascii_case(h));
        if !same {
            return Err(ApiError::forbidden("Cross-origin request refused"));
        }
    }
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_round_trip_and_reject_garbage() {
        let (text, hash) = new_token();
        assert_eq!(text.len(), 64);
        assert_eq!(parse_token(&text), Some(hash));
        assert_eq!(parse_token(&text[..63]), None);
        assert_eq!(parse_token(&"zz".repeat(32)), None);
        assert_ne!(new_token().1, hash);
    }

    #[test]
    fn five_failures_block_for_fifteen_minutes() {
        let auth = Auth::default();
        let keys = vec!["user:a".to_owned()];
        for _ in 0..4 {
            auth.record_failure(&keys);
        }
        assert_eq!(auth.blocked_for(&keys), None);
        auth.record_failure(&keys);
        assert_eq!(auth.blocked_for(&keys), Some(LOCKOUT.as_secs()));
        assert_eq!(auth.blocked_for(&["user:b".to_owned()]), None);
        auth.advance(LOCKOUT);
        assert_eq!(auth.blocked_for(&keys), None);
    }
}
