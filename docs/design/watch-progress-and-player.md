# Watch progress and the Nuvora player

Requested by the owner after accounts landed. Every user gets their own progress, Netflix style, and playback uses a custom Nuvora player instead of the browser's built-in controls.

## Watch state

Migration `0003_watch_state.sql` in `crates/library`:

- `watch_state(user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE, item_id INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE, file_id INTEGER, position_ms INTEGER, duration_ms INTEGER, play_count INTEGER NOT NULL DEFAULT 0, last_watched_at INTEGER, updated_at INTEGER NOT NULL, PRIMARY KEY (user_id, item_id)) STRICT, WITHOUT ROWID`.
- An index on `(user_id, updated_at DESC) WHERE position_ms IS NOT NULL` serves Continue Watching.
- `user.watch_rev INTEGER NOT NULL DEFAULT 0` is bumped in the same transaction as every watch-state write for that user.

The server owns every rule. The client only reports where playback is.

- **Too early.** A position under 60 seconds stores no progress and clears any earlier progress. Opening a movie by accident doesn't put it in Continue Watching.
- **In progress.** A position of 60 seconds or more and under 90% of the duration stores `position_ms`, `duration_ms` and `file_id`.
- **Finished.** A position at 90% of the duration or more marks the movie watched: `play_count + 1`, `last_watched_at = now`, and the position is cleared. A report at 90% or more doesn't count again while the position is already cleared and `last_watched_at` is within the last 3 hours, so a seek near the end doesn't count twice.
- **By hand.** Mark watched works like finishing. Mark unwatched clears the position, `play_count` and `last_watched_at`.
- **Validation.** `file_id` must belong to the item. A `position_ms` that exceeds `duration_ms` by more than 5 seconds is a validation error.

## HTTP API

Every catalog response is now per user, because cards and details carry that user's state.

| method | path | body | response |
|---|---|---|---|
| PUT | `/api/items/{id}/progress` | `{ file_id, position_ms, duration_ms }` | 204 |
| PUT | `/api/items/{id}/watched` | | 204 |
| DELETE | `/api/items/{id}/watched` | | 204 |

Wire changes, all additive:

- `PosterCard` gains `progress: number | null` (0 to 1, only while in progress) and `watched: boolean`.
- `MovieDetail` gains `watch: { position_ms: number | null, file_id: number | null, watched: boolean, play_count: number, last_watched_at: number | null }`.
- `RowKind` gains `continue_watching`. Home puts it first when it has any items, ordered by `updated_at` descending. It pages like every other row.
- Home, row and item ETags become `"<catalog_rev>.<watch_rev>"` (item detail keeps its device-profile hash suffix). One user's progress never makes another user's cache stale, and a 304 can never serve another user's state, because the ETag is checked against the signed-in user's own revision.

## The player

A full-window Nuvora player replaces the native `<video controls>`. It follows `DESIGN.md`: a black stage, amber only for played progress and focus, and Inter with tabular numerals.

- **Top bar.** A back arrow, then the title and year. Clicking back returns to the movie detail.
- **Bottom bar:**
  - A scrubber: 4px high, 6px on hover, amber for the played part, translucent white for buffered. On hover it shows a time tooltip, and dragging seeks.
  - Elapsed time on the left. Remaining time on the right, which toggles to the duration on click.
  - Play/pause, back 10 s, forward 10 s, mute plus a volume slider, picture-in-picture where the browser supports it, and fullscreen.
- **Controls fade.** They fade in on mouse move or any key, and fade out after 3 s of playback without input. The cursor hides with them. Controls stay visible while paused and while focus is inside them.
- **Center.** A brief play or pause pulse when toggling. A buffering spinner after 500 ms of `waiting`, never instantly. The error state reuses the "can't play" notice.
- **Keyboard:**
  - Space or K plays and pauses.
  - Left and Right (or J and L) skip 10 s.
  - Up and Down change the volume by 10%.
  - M mutes. F toggles fullscreen.
  - Escape leaves fullscreen first, then goes back.
  - 0 to 9 jump to that tenth of the movie.
- **Reduced motion and touch.** Reduced motion turns the fades into instant changes. On touch screens, a tap toggles the controls instead of pausing.
- **Remembered between sessions.** Volume and mute are stored in `localStorage` (web only).

Progress reporting goes through `@nuvora/core`, so the TV apps share the cadence:

- It sends every 10 s while playing, on pause, after a seek settles, and when leaving the player.
- When the page is hidden or closed, it uses `navigator.sendBeacon` with a JSON blob. This is the one web-only path, kept in `apps/web`.

## Resume

- **Movie detail.** With progress, the primary button reads "Resume" with "from 42:10" beside it, and a secondary "Start over" button sits next to it. Without progress it is "Play". A secondary "Mark as watched" or "Mark as unwatched" toggle sits in the same row.
- **Player URL.** `/movie/$id/play?file=<id>&t=<seconds>`. The player seeks to `t` before the first frame is shown, so playback never flashes the beginning.
- **Posters.** An in-progress poster shows a 3px amber progress bar along its bottom edge. A watched poster shows a small check badge in its top-right corner. Unwatched posters carry no marker.
