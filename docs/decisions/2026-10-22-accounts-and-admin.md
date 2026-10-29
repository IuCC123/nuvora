# Accounts, login and the admin panel

Pulled forward from Steps 2 and 4 at the owner's request, so Nuvora can be installed and tested by the household and remote friends. This builds on the Step 1 foundation and does not change its catalog design.

## Who sees what

- **Admin.** Owns the server. Created on first run. Manages libraries, users and server settings.
- **Member.** A household member or a remote friend. Browses and plays every library. Can change their own display name and password. Per-library sharing and invite links come later.

## Data

Migration `0002_accounts.sql` in `crates/library` (the library crate stays the only owner of persistent state):

- `user(id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT NOT NULL UNIQUE COLLATE NOCASE, display_name TEXT NOT NULL, password_hash TEXT NOT NULL, role TEXT NOT NULL, disabled_at INTEGER, created_at INTEGER NOT NULL) STRICT`. `password_hash` is an argon2id PHC string. `role` is `admin` or `member`, parsed by a Rust enum.
- `session(token_hash BLOB PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE, created_at INTEGER NOT NULL, last_seen_at INTEGER NOT NULL, expires_at INTEGER NOT NULL, user_agent TEXT) STRICT, WITHOUT ROWID`. The token is 32 random bytes. Only its SHA-256 is stored.
- `setting(key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT, WITHOUT ROWID`. Step 1 uses `server_name`.

Libraries move to database ownership. The existing `library` and `library_root` tables are unchanged. Config and `--movies` only add a library or root that does not exist yet. After that, the admin panel owns it. Removing a folder from the config does not delete anything.

## Sessions and security

- **Cookie.** `nuvora_session`, `HttpOnly`, `SameSite=Lax`, `Path=/`, `Max-Age` 30 days, sliding (refreshed when older than a day). `Secure` when the request arrived over HTTPS (directly or `X-Forwarded-Proto: https`).
- **Bearer.** The auth extractor also accepts `Authorization: Bearer <token>`, for the TV apps in Step 5. No endpoint hands out a bearer token yet.
- **Public routes.** `GET /api/health`, `GET /api/auth/state`, `POST /api/auth/login`, `POST /api/setup`. Everything else under `/api`, including images and streams, needs a session. `/api/admin/*` needs the admin role.
- **CSRF.** State-changing requests must be `application/json` and, when an `Origin` header is present, it must match the `Host`.
- **Passwords.** argon2id with the crate's default parameters, minimum 8 characters. Login failure always says "Wrong username or password". After 5 failures for one username or IP within 15 minutes, login answers 429 for 15 minutes (in memory).
- **Admin guards.** The last enabled admin cannot be deleted, demoted or disabled. Nobody can disable or delete themselves. Disabling a user deletes their sessions.
- **First run.** With zero users, `GET /api/auth/state` returns `setup_required: true`, and `POST /api/setup` creates the first admin and signs them in. The zero-user check and the insert happen in one write transaction, so two racing setup requests cannot both win. With any user present, `POST /api/setup` returns 409.

## HTTP API

Auth:

| method | path | body | response |
|---|---|---|---|
| GET | `/api/health` | | `{ status: "ok", version }` |
| GET | `/api/auth/state` | | `{ setup_required: bool, user: User \| null, server_name }` |
| POST | `/api/setup` | `{ username, display_name, password }` | `User`, sets cookie |
| POST | `/api/auth/login` | `{ username, password }` | `User`, sets cookie |
| POST | `/api/auth/logout` | | 204, clears cookie |
| PATCH | `/api/auth/me` | `{ display_name?, current_password?, new_password? }` | `User` |

`User = { id, username, display_name, role: "admin" | "member" }`.

Admin:

| method | path | body | response |
|---|---|---|---|
| GET | `/api/admin/libraries` | | `AdminLibrary[]` |
| POST | `/api/admin/libraries` | `{ name, kind: "movies", paths: string[] }` | `AdminLibrary`, starts a scan |
| PATCH | `/api/admin/libraries/{id}` | `{ name?, paths? }` | `AdminLibrary`, scans if paths changed |
| DELETE | `/api/admin/libraries/{id}` | | 204, removes its items |
| POST | `/api/admin/libraries/{id}/scan` | | `ScanStatus` |
| GET | `/api/admin/fs?path=` | | `{ path, parent: string \| null, dirs: { name, path }[] }` |
| GET | `/api/admin/users` | | `AdminUser[]` |
| POST | `/api/admin/users` | `{ username, display_name, password, role }` | `AdminUser` |
| PATCH | `/api/admin/users/{id}` | `{ display_name?, role?, disabled?, password? }` | `AdminUser` |
| DELETE | `/api/admin/users/{id}` | | 204 |
| GET | `/api/admin/server` | | `{ server_name, version, data_dir, bind, ffmpeg: { path, version }, ffprobe: { path, version }, uptime_s }` |
| PATCH | `/api/admin/server` | `{ server_name }` | same as GET |

- `AdminLibrary = { id, name, kind, paths: string[], item_count, missing_paths: string[] }`. `missing_paths` lists roots that are unreadable right now.
- `AdminUser = User & { created_at, disabled: bool, last_seen_at: number | null }`.
- `/api/admin/fs` lists directory names only, never files, and skips hidden ones. With no `path`, it lists the filesystem roots (`/` on Unix, drive letters on Windows). In Docker, `/media` is where libraries are mounted.
- Errors use the existing `ApiError` shape with a stable `code` (`setup_done`, `bad_credentials`, `rate_limited`, `last_admin`, `validation`).

## Server configuration

- Every flag also reads an environment variable: `NUVORA_BIND`, `NUVORA_DATA_DIR`, `NUVORA_CONFIG`, `NUVORA_MOVIES` (colon-separated).
- With auth in place the default bind becomes `0.0.0.0:8640`, so phones and TVs on the home network can reach the server. The non-loopback warning is removed.
- The server shuts down gracefully on SIGTERM as well as Ctrl-C, because `docker stop` sends SIGTERM through tini.
- The server writes only under the data directory. The container runs as an arbitrary uid with `HOME=/data`.
- The Docker healthcheck and `scripts/install.sh` switch from their TCP and `/` probes to `GET /api/health`. The README's "don't expose this outside your home network until login ships" sentence is replaced with guidance on remote access.

## Web app

- **First run.** A two-step welcome. Step 1 creates the admin account. Step 2 adds the first library with the folder picker, starts the scan, and lands on Home while the rows fill in.
- **Sign in.** A centered card on the dark canvas, with a slow drift of title cards behind it at low opacity. No sign-up link: only an admin creates accounts.
- **Top bar.** The account button (initials in a circle) opens a menu with Settings, which shows only for admins, and Sign out.
- **Settings** (`/settings/*`) uses the same shell with its own section list:
  - **General.** Server name, version, ffmpeg and ffprobe, data folder, uptime.
  - **Libraries.** A list with item counts and per-library Scan now. Add and edit use the folder picker. Remove asks for confirmation with the item count.
  - **Users.** A list with role, status and last seen. Add user. Edit covers role, disable and reset password.
  - **Account.** Display name and password change, for every user.
- A 401 from any request returns to Sign in and then back to where the user was.
- Everything follows `DESIGN.md`. Forms use `surface` fields with 6px corners, inline validation, and the amber primary button.
