import { expect, test } from "bun:test";
import {
  ApiError,
  checkPassword,
  checkPasswordsMatch,
  checkUsername,
  createClient,
  describeError,
  folderCrumbs,
  formatLastSeen,
  formatUptime,
  initials,
  safeRedirect,
  type ResponseLike,
} from "../src";

const reply = (status: number, body: unknown): ResponseLike => ({
  ok: status >= 200 && status < 300,
  status,
  json: async () => {
    if (body === undefined) throw new SyntaxError("Unexpected end of JSON input");
    return body;
  },
});

function clientAnswering(res: ResponseLike, onUnauthorized?: () => void) {
  const sent: { url: string; method: string; headers: Record<string, string>; body?: string }[] = [];
  const client = createClient({
    baseUrl: "",
    device: { containers: [], video: [], audio: [] },
    fetch: async (url, init) => {
      sent.push({ url, ...init });
      return res;
    },
    ...(onUnauthorized && { onUnauthorized }),
  });
  return { client, sent };
}

test("an error body's code and message reach the ApiError", async () => {
  const { client } = clientAnswering(reply(409, { code: "setup_done", message: "already set up" }));
  const error = await client.startScan().catch((e: unknown) => e);
  expect(error).toBeInstanceOf(ApiError);
  expect(error).toMatchObject({ status: 409, code: "setup_done", detail: "already set up" });
});

test("a plain-text error body leaves the code empty", async () => {
  const { client } = clientAnswering(reply(500, undefined));
  expect(await client.startScan().catch((e: unknown) => e)).toMatchObject({ status: 500, code: null, detail: null });
});

test("state-changing requests declare JSON, so they pass the server's CSRF check", async () => {
  const { client, sent } = clientAnswering(reply(200, { state: "idle", last: null }));
  await client.startScan();
  await client.scanStatus();
  expect(sent.map((r) => [r.method, r.headers["content-type"]])).toEqual([
    ["POST", "application/json"],
    ["GET", undefined],
  ]);
});

test("a 401 calls onUnauthorized, except a wrong password", async () => {
  let calls = 0;
  const expired = clientAnswering(reply(401, null), () => calls++);
  await expired.client.scanStatus().catch(() => {});
  expect(calls).toBe(1);
  const wrong = clientAnswering(reply(401, { code: "bad_credentials" }), () => calls++);
  await wrong.client.scanStatus().catch(() => {});
  expect(calls).toBe(1);
});

test("describeError gives one plain sentence per server code", () => {
  expect(describeError(new ApiError(401, "", "bad_credentials"))).toBe("Wrong username or password.");
  expect(describeError(new ApiError(429, "", "rate_limited"))).toBe("Too many sign-in attempts. Try again in 15 minutes.");
  expect(describeError(new ApiError(409, "", "last_admin"))).toBe(
    "Nuvora needs at least one admin who can sign in. Make someone else an admin first.",
  );
  expect(describeError(new ApiError(409, "", "setup_done"))).toBe("This server is already set up. Sign in instead.");
  expect(describeError(new ApiError(400, "", "validation", "username is already taken"))).toBe("Username is already taken.");
  expect(describeError(new ApiError(400, "", "validation"))).toBe("Check the highlighted fields and try again.");
  expect(describeError(new ApiError(401, "", "bad_credentials"), { bad_credentials: "Your current password is wrong." })).toBe(
    "Your current password is wrong.",
  );
  expect(describeError(new TypeError("Failed to fetch"))).toBe(
    "Can't reach your Nuvora server. Check that it's running, then try again.",
  );
  expect(describeError(new ApiError(500, ""))).toBe("Something went wrong on the server. Try again.");
});

test("form checks", () => {
  expect(checkUsername("")).toBe("Enter a username.");
  expect(checkUsername("jamie r")).toBe("Usernames can't contain spaces.");
  expect(checkUsername("jamie")).toBeNull();
  expect(checkPassword("short")).toBe("Use at least 8 characters.");
  expect(checkPassword("long enough")).toBeNull();
  expect(checkPasswordsMatch("long enough", "")).toBe("Enter the password again.");
  expect(checkPasswordsMatch("long enough", "long enougj")).toBe("The passwords don't match.");
  expect(checkPasswordsMatch("long enough", "long enough")).toBeNull();
});

test("safeRedirect keeps in-app paths only", () => {
  expect(safeRedirect("/movie/4?x=1")).toBe("/movie/4?x=1");
  expect(safeRedirect("https://evil.example")).toBe("/");
  expect(safeRedirect("//evil.example")).toBe("/");
  expect(safeRedirect("/\\evil.example")).toBe("/");
  expect(safeRedirect("/signin?redirect=/")).toBe("/");
  expect(safeRedirect("/welcome/library")).toBe("/");
  expect(safeRedirect("/signing-bonus")).toBe("/signing-bonus");
  expect(safeRedirect(undefined)).toBe("/");
});

test("initials, uptime and last seen", () => {
  expect(initials("Jamie Rivera")).toBe("JR");
  expect(initials("  ana maría de la cruz ")).toBe("AC");
  expect(initials("jamie")).toBe("J");
  expect(initials("")).toBe("?");
  expect(formatUptime(3 * 86_400 + 4 * 3_600 + 59)).toBe("3 days, 4 hours");
  expect(formatUptime(2 * 3_600 + 5 * 60)).toBe("2 hours, 5 minutes");
  expect(formatUptime(86_400 + 30)).toBe("1 day");
  expect(formatUptime(40)).toBe("40 seconds");
  expect(formatUptime(0)).toBe("0 seconds");
  const now = Date.UTC(2026, 9, 22, 12);
  expect(formatLastSeen(null, now)).toBe("Never");
  expect(formatLastSeen(now - 20_000, now)).toBe("Just now");
  expect(formatLastSeen(now - 60_000, now)).toBe("1 minute ago");
  expect(formatLastSeen(now - 3 * 3_600_000, now)).toBe("3 hours ago");
  expect(formatLastSeen(now - 30 * 3_600_000, now)).toBe("Yesterday");
  expect(formatLastSeen(now - 4 * 86_400_000, now)).toBe("4 days ago");
  expect(formatLastSeen(Date.UTC(2026, 2, 3, 12), now)).toBe("3 Mar 2026");
});

test("folderCrumbs walks from the root to the folder", () => {
  expect(folderCrumbs("/media/Movies")).toEqual([
    { name: "/", path: "/" },
    { name: "media", path: "/media" },
    { name: "Movies", path: "/media/Movies" },
  ]);
  expect(folderCrumbs("/")).toEqual([{ name: "/", path: "/" }]);
  expect(folderCrumbs("/media/Movies/")).toHaveLength(3);
  expect(folderCrumbs("D:\\Films\\4K")).toEqual([
    { name: "D:", path: "D:\\" },
    { name: "Films", path: "D:\\Films" },
    { name: "4K", path: "D:\\Films\\4K" },
  ]);
  expect(folderCrumbs("C:\\")).toEqual([{ name: "C:", path: "C:\\" }]);
  expect(folderCrumbs("")).toEqual([]);
});
