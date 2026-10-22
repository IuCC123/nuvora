//! Sessions, passwords, sign-in rate limiting and CSRF. The HTTP-facing half of
//! accounts; the rows live in the catalog (`nuvora_library::accounts`).
//!
//! A session token is 32 random bytes, sent as 64 hex chars in the
//! `nuvora_session` cookie or an `Authorization: Bearer` header. The database
//! keeps only its SHA-256.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, HOST, ORIGIN, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, request::Parts};
use axum::middleware::Next;
use axum::response::Response;
use nuvora_library::{Role, TokenHash, User, WatchRev};
use password_hash::rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::api::{ApiError, AppState, SESSION_COOKIE};
use crate::net::{self, TrustedProxies};

pub const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 3600);
/// A session's expiry (and cookie) moves forward once it is this old.
const REFRESH_AFTER: Duration = Duration::from_secs(24 * 3600);
/// `last_seen_at` is written at most this often per session, not per request.
const LAST_SEEN_EVERY: Duration = Duration::from_secs(5 * 60);
/// A username's wait after its first failed sign-in; it doubles with each
/// further failure up to [`MAX_USER_DELAY`].
const FIRST_USER_DELAY: Duration = Duration::from_secs(1);
const MAX_USER_DELAY: Duration = Duration::from_secs(10);
/// Failures from one address within [`FAILURE_WINDOW`] that lock it out.
const MAX_ADDRESS_FAILURES: usize = 20;
/// Also how long a username's failures are remembered.
const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
const ADDRESS_LOCKOUT: Duration = Duration::from_secs(15 * 60);

fn ms(d: Duration) -> i64 {
    d.as_millis() as i64
}

/// Shared auth state: the clock, the trusted proxies and the sign-in failure
/// counts (in memory, so a restart forgets them).
#[derive(Default)]
pub struct Auth {
    /// Added to the system time; tests move it forward to expire sessions.
    offset_ms: AtomicI64,
    proxies: TrustedProxies,
    users: Mutex<HashMap<String, Backoff>>,
    addresses: Mutex<HashMap<IpAddr, Failures>>,
}

/// Consecutive failed sign-ins of one username. Guessing slows down without
/// locking the real user out.
struct Backoff {
    failures: u32,
    last_at: i64,
}

impl Backoff {
    fn until(&self) -> i64 {
        let delay =
            ms(FIRST_USER_DELAY).saturating_mul(1 << self.failures.saturating_sub(1).min(16));
        self.last_at + delay.min(ms(MAX_USER_DELAY))
    }
}

#[derive(Default)]
struct Failures {
    at: Vec<i64>,
    blocked_until: i64,
}

/// What one sign-in attempt is counted against.
pub struct SignInKeys {
    /// Lowercased.
    username: String,
    /// `None` when unknown or exempt (see [`Auth::sign_in_keys`]).
    address: Option<IpAddr>,
}

impl Auth {
    pub fn new(proxies: TrustedProxies) -> Auth {
        Auth {
            proxies,
            ..Auth::default()
        }
    }

    pub fn proxies(&self) -> &TrustedProxies {
        &self.proxies
    }

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

    /// The username (ignoring case) and the client address. Without a trusted
    /// proxy a local peer (loopback, RFC 1918, ULA) has no address key: it is
    /// probably an unconfigured reverse proxy that every client shares, and
    /// one guesser must not lock the household out.
    pub fn sign_in_keys(&self, username: &str, parts: &Parts) -> SignInKeys {
        let address = net::peer(&parts.extensions)
            .map(|peer| self.proxies.client(peer, &parts.headers))
            .filter(|ip| self.proxies.any() || !net::is_local(*ip));
        SignInKeys {
            username: username.to_lowercase(),
            address,
        }
    }

    /// Seconds until a sign in under `keys` is allowed again, if it is not now.
    pub fn blocked_for(&self, keys: &SignInKeys) -> Option<u64> {
        let now = self.now_ms();
        let user_until = lock(&self.users)
            .get(&keys.username)
            .map_or(0, Backoff::until);
        let address_until = keys
            .address
            .and_then(|ip| lock(&self.addresses).get(&ip).map(|f| f.blocked_until))
            .unwrap_or(0);
        let left = user_until.max(address_until) - now;
        (left > 0).then(|| (left as u64).div_ceil(1000))
    }

    /// Doubles the username's wait, and counts against the address: the
    /// [`MAX_ADDRESS_FAILURES`]th within [`FAILURE_WINDOW`] locks it for
    /// [`ADDRESS_LOCKOUT`].
    pub fn record_failure(&self, keys: &SignInKeys) {
        let now = self.now_ms();
        let mut users = lock(&self.users);
        users.retain(|_, b| now - b.last_at < ms(FAILURE_WINDOW));
        let backoff = users.entry(keys.username.clone()).or_insert(Backoff {
            failures: 0,
            last_at: now,
        });
        backoff.failures += 1;
        backoff.last_at = now;
        drop(users);

        let Some(ip) = keys.address else { return };
        let mut addresses = lock(&self.addresses);
        addresses.retain(|_, f| {
            f.blocked_until > now || f.at.last().is_some_and(|t| now - t < ms(FAILURE_WINDOW))
        });
        let failures = addresses.entry(ip).or_default();
        failures.at.retain(|t| now - t < ms(FAILURE_WINDOW));
        failures.at.push(now);
        if failures.at.len() >= MAX_ADDRESS_FAILURES {
            failures.at.clear();
            failures.blocked_until = now + ms(ADDRESS_LOCKOUT);
        }
    }

    /// Resets the username only: one account's password must not clear an
    /// address that is guessing at others.
    pub fn record_success(&self, keys: &SignInKeys) {
        lock(&self.users).remove(&keys.username);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
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
/// `Authorization: Bearer` header or else the session cookie. Another
/// `Authorization` scheme (a proxy's Basic auth) is not ours and is ignored.
fn presented_token(headers: &HeaderMap) -> Option<(String, Carrier)> {
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"));
    if let Some((_, token)) = bearer {
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
/// on cross-site requests, which this server never approves. Behind a trusted
/// proxy "this server" is `X-Forwarded-Host` when the proxy sets it.
pub async fn csrf(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // A WebSocket upgrade is a GET, but the socket it opens acts with the
    // session's cookie: hold it to the same-origin rule.
    let upgrade = req
        .headers()
        .get(axum::http::header::UPGRADE)
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket"));
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) && !upgrade {
        return Ok(next.run(req).await);
    }
    let json = upgrade
        || req
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
        let proxied = net::peer(req.extensions()).is_some_and(|p| state.auth.proxies().trusts(p));
        let forwarded_host = proxied
            .then(|| req.headers().get("x-forwarded-host"))
            .flatten()
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.split(',').next())
            .map(str::trim);
        let host = forwarded_host
            .or_else(|| req.headers().get(HOST).and_then(|h| h.to_str().ok()))
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

    fn keys(username: &str, address: Option<&str>) -> SignInKeys {
        SignInKeys {
            username: username.into(),
            address: address.map(|a| a.parse().unwrap()),
        }
    }

    #[test]
    fn a_username_waits_one_second_doubling_to_ten() {
        let auth = Auth::default();
        let ada = keys("ada", None);
        let mut waits = Vec::new();
        for _ in 0..6 {
            auth.record_failure(&ada);
            let wait = auth.blocked_for(&ada).unwrap();
            waits.push(wait);
            auth.advance(Duration::from_secs(wait));
            assert_eq!(auth.blocked_for(&ada), None, "after {wait} s");
        }
        assert_eq!(waits, [1, 2, 4, 8, 10, 10]);
        assert_eq!(auth.blocked_for(&keys("bo", None)), None);

        auth.record_failure(&ada);
        auth.record_success(&ada);
        assert_eq!(auth.blocked_for(&ada), None, "success resets");
        auth.record_failure(&ada);
        assert_eq!(auth.blocked_for(&ada), Some(1), "and starts again at 1 s");

        auth.advance(FAILURE_WINDOW);
        auth.record_failure(&keys("bo", None));
        auth.record_failure(&ada);
        assert_eq!(
            auth.blocked_for(&ada),
            Some(1),
            "forgotten after 15 quiet minutes"
        );
    }

    #[test]
    fn twenty_failures_lock_an_address_for_fifteen_minutes() {
        let auth = Auth::default();
        for i in 0..19 {
            auth.record_failure(&keys(&format!("user{i}"), Some("203.0.113.9")));
        }
        let fresh = keys("someone", Some("203.0.113.9"));
        assert_eq!(auth.blocked_for(&fresh), None);
        auth.record_failure(&keys("user19", Some("203.0.113.9")));
        assert_eq!(auth.blocked_for(&fresh), Some(ADDRESS_LOCKOUT.as_secs()));
        assert_eq!(
            auth.blocked_for(&keys("someone", Some("203.0.113.10"))),
            None
        );
        auth.record_success(&keys("user0", Some("203.0.113.9")));
        assert_eq!(
            auth.blocked_for(&fresh),
            Some(ADDRESS_LOCKOUT.as_secs()),
            "a success does not unlock the address"
        );
        auth.advance(ADDRESS_LOCKOUT);
        assert_eq!(auth.blocked_for(&fresh), None);
    }
}
