import { expect, test } from "bun:test";
import { createClient, createProgressReporter, formatClock, type ProgressRequest, type Timers } from "../src";

/** A clock the test advances by hand: timers fire in due order as `advance` passes them. */
function fakeClock() {
  let now = 0;
  let nextId = 1;
  const pending = new Map<number, { at: number; fn: () => void }>();
  const timers: Timers<number> = {
    setTimeout(fn, ms) {
      pending.set(nextId, { at: now + ms, fn });
      return nextId++;
    },
    clearTimeout(id) {
      pending.delete(id);
    },
  };
  function advance(ms: number) {
    const end = now + ms;
    for (;;) {
      const due = [...pending].filter(([, t]) => t.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      pending.delete(due[0]);
      now = due[1].at;
      due[1].fn();
    }
    now = end;
  }
  return { timers, advance, now: () => now };
}

/** A player whose position moves with the fake clock while playing. */
function setup(durationMs = 6_000_000) {
  const clock = fakeClock();
  const sent: ProgressRequest[] = [];
  let base = 0;
  let since: number | null = null;
  const position = () => base + (since === null ? 0 : clock.now() - since);
  const reporter = createProgressReporter({
    fileId: 7,
    read: () => ({ positionMs: position(), durationMs }),
    send: (r) => void sent.push(r),
    timers: clock.timers,
  });
  return {
    clock,
    reporter,
    sent: () => sent.map((r) => r.position_ms),
    reports: sent,
    play() {
      since = clock.now();
      reporter.playing();
    },
    pause() {
      base = position();
      since = null;
      reporter.paused();
    },
    seek(to: number) {
      base = to;
      if (since !== null) since = clock.now();
      reporter.seeked();
    },
  };
}

test("reports every 10 s while playing and stops on pause", () => {
  const p = setup();
  p.play();
  p.clock.advance(9_999);
  expect(p.sent()).toEqual([]);
  p.clock.advance(1);
  expect(p.sent()).toEqual([10_000]);
  p.clock.advance(25_000);
  expect(p.sent()).toEqual([10_000, 20_000, 30_000]);
  p.pause();
  expect(p.sent()).toEqual([10_000, 20_000, 30_000, 35_000]);
  p.clock.advance(60_000);
  expect(p.sent()).toEqual([10_000, 20_000, 30_000, 35_000]);
  expect(p.reports[0]).toEqual({ file_id: 7, position_ms: 10_000, duration_ms: 6_000_000 });
});

test("a run of seeks reports once, after the last one settles, and restarts the cadence", () => {
  const p = setup();
  p.play();
  p.clock.advance(4_000);
  p.seek(100_000);
  p.clock.advance(300);
  p.seek(200_000);
  p.clock.advance(300);
  p.seek(300_000);
  p.clock.advance(999);
  expect(p.sent()).toEqual([]);
  p.clock.advance(1);
  expect(p.sent()).toEqual([301_000]);
  p.clock.advance(10_000);
  expect(p.sent()).toEqual([301_000, 311_000]);
});

test("a seek while paused reports where it landed", () => {
  const p = setup();
  p.seek(1_200_000);
  p.clock.advance(1_000);
  expect(p.sent()).toEqual([1_200_000]);
});

test("leaving sends the final position once and silences everything after", async () => {
  const p = setup();
  p.play();
  p.clock.advance(12_500);
  await p.reporter.leave();
  await p.reporter.leave();
  p.reporter.playing();
  p.reporter.seeked();
  p.clock.advance(60_000);
  expect(p.sent()).toEqual([10_000, 12_500]);
});

test("a pause right after a report, or a flush, sends nothing new", () => {
  const p = setup();
  p.play();
  p.clock.advance(10_000);
  p.pause();
  const beacon: number[] = [];
  p.reporter.flush((r) => beacon.push(r.position_ms));
  expect(p.sent()).toEqual([10_000]);
  expect(beacon).toEqual([]);
});

test("flush sends the current position through the given transport and playback carries on", () => {
  const p = setup();
  p.play();
  p.clock.advance(4_000);
  const beacon: number[] = [];
  p.reporter.flush((r) => beacon.push(r.position_ms));
  expect(beacon).toEqual([4_000]);
  p.clock.advance(6_000);
  expect(p.sent()).toEqual([10_000]);
});

test("nothing is reported before the duration is known", async () => {
  const sent: ProgressRequest[] = [];
  const clock = fakeClock();
  const reporter = createProgressReporter({ fileId: 1, read: () => ({ positionMs: 0, durationMs: Number.NaN }), send: (r) => void sent.push(r), timers: clock.timers });
  reporter.playing();
  clock.advance(30_000);
  await reporter.leave();
  expect(sent).toEqual([]);
});

test("nothing is reported before playback has played or seeked, so a blocked autoplay keeps the resume point", async () => {
  const p = setup();
  p.clock.advance(30_000);
  p.pause();
  const beacon: number[] = [];
  p.reporter.flush((r) => beacon.push(r.position_ms));
  await p.reporter.leave();
  expect(p.sent()).toEqual([]);
  expect(beacon).toEqual([]);
});

test("a seek alone is enough to start reporting", async () => {
  const p = setup();
  p.seek(90_000);
  await p.reporter.leave();
  expect(p.sent()).toEqual([90_000]);
});

test("a failed send does not reject leave", async () => {
  const clock = fakeClock();
  const reporter = createProgressReporter({
    fileId: 1,
    read: () => ({ positionMs: 90_000, durationMs: 600_000 }),
    send: () => Promise.reject(new Error("offline")),
    timers: clock.timers,
  });
  reporter.playing();
  await expect(reporter.leave()).resolves.toBeUndefined();
});

test("formats the player clock", () => {
  expect(formatClock(7_900)).toBe("0:07");
  expect(formatClock(2_530_000)).toBe("42:10");
  expect(formatClock(3_723_000)).toBe("1:02:03");
  expect(formatClock(-5)).toBe("0:00");
});

test("the client reports progress as a JSON PUT that can outlive the page, and marks watched both ways", async () => {
  const calls: { url: string; method: string; body?: string; keepalive?: boolean }[] = [];
  const client = createClient({
    baseUrl: "",
    device: { containers: [], video: [], audio: [] },
    fetch: async (url, init) => {
      calls.push({ url, method: init.method, ...(init.body && { body: init.body }), ...(init.keepalive && { keepalive: true }) });
      return { ok: true, status: 204, json: async () => null };
    },
  });
  await client.reportProgress(13, { file_id: 13, position_ms: 2_530_000, duration_ms: 6_000_000 }, { keepalive: true });
  await client.markWatched(13);
  await client.markUnwatched(13);
  expect(calls).toEqual([
    { url: "/api/items/13/progress", method: "PUT", body: '{"file_id":13,"position_ms":2530000,"duration_ms":6000000}', keepalive: true },
    { url: "/api/items/13/watched", method: "PUT" },
    { url: "/api/items/13/watched", method: "DELETE" },
  ]);
});
