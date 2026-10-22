import { describe, expect, test } from "bun:test";
import { partyCorrection, partyPosition, partyStartsIn, ServerClock, type PartyPlayback } from "../src";

function playback(over: Partial<PartyPlayback> = {}): PartyPlayback {
  return {
    item_id: 1,
    file_id: 2,
    kind: "movie",
    title: "Sintel",
    subtitle: null,
    playing: true,
    position_ms: 60_000,
    at_ms: 10_000,
    seq: 1,
    by: null,
    action: "play",
    ...over,
  };
}

describe("the party clock", () => {
  test("advances from the moment it describes while playing", () => {
    expect(partyPosition(playback(), 12_500)).toBe(62.5);
    expect(partyPosition(playback({ playing: false }), 12_500)).toBe(60);
  });

  test("holds during a countdown", () => {
    expect(partyPosition(playback(), 9_000)).toBe(60);
    expect(partyStartsIn(playback(), 9_000)).toBe(1_000);
    expect(partyStartsIn(playback(), 11_000)).toBe(0);
    expect(partyStartsIn(playback({ playing: false }), 9_000)).toBe(0);
  });

  test("small drift is nudged, large drift jumps", () => {
    expect(partyCorrection(10.1, 10)).toEqual({ kind: "in_sync" });
    expect(partyCorrection(11, 10)).toEqual({ kind: "nudge", rate: 0.95 });
    expect(partyCorrection(9, 10)).toEqual({ kind: "nudge", rate: 1.05 });
    expect(partyCorrection(30, 10)).toEqual({ kind: "seek", to: 10 });
  });
});

describe("ServerClock", () => {
  test("keeps the offset from the fastest round trip", () => {
    const clock = new ServerClock();
    clock.seed(5_000, 1_000);
    expect(clock.now(1_000)).toBe(5_000);
    clock.sample(1_000, 5_100, 1_200); // rtt 200: server = local + 4000
    expect(clock.now(2_000)).toBe(6_000);
    clock.sample(3_000, 7_020, 3_040); // rtt 40, more precise: server = local + 4000
    expect(clock.now(2_000)).toBe(6_000);
    clock.sample(4_000, 9_999, 4_500); // slower, ignored
    expect(clock.now(2_000)).toBe(6_000);
  });
});
