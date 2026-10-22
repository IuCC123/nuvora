import { expect, test } from "bun:test";
import { createClient, createQueries, rowCards, type HomeRow, type RowPage } from "../src";

const card = (id: number) => ({ id, kind: "movie" as const, title: `Movie ${id}`, watched: false });
const row: HomeRow = { kind: "all_movies", title: "All Movies", total: 5, items: [card(1), card(2)], next: "c2" };

test("rowCards appends loaded pages and follows the last cursor", () => {
  expect(rowCards(row)).toEqual({ cards: [card(1), card(2)], hasMore: true });
  const pages: RowPage[] = [{ items: [card(3), card(4)], next: "c4" }, { items: [card(5)], next: null }];
  expect(rowCards(row, pages)).toEqual({ cards: [1, 2, 3, 4, 5].map(card), hasMore: false });
  expect(rowCards({ ...row, next: null })).toEqual({ cards: [card(1), card(2)], hasMore: false });
});

test("rowPages requests the row after each cursor", async () => {
  const urls: string[] = [];
  const client = createClient({
    baseUrl: "http://nas:8640",
    device: { containers: [], video: [], audio: [] },
    fetch: async (url) => {
      urls.push(url);
      return { ok: true, status: 200, json: async () => ({ items: [], next: "c9" }) };
    },
  });
  const def = createQueries(client).rowPages("all_movies", "c2");
  expect(def.queryKey).toEqual(["row", "all_movies", "c2"]);
  const page = await def.queryFn({ signal: { aborted: false }, pageParam: def.initialPageParam });
  expect(urls).toEqual(["http://nas:8640/api/rows/all_movies?after=c2"]);
  expect(def.getNextPageParam(page)).toBe("c9");
  expect(def.getNextPageParam({ items: [], next: null })).toBeUndefined();
});

test("the folder picker's query asks for the roots without a path and a folder with one", async () => {
  const urls: string[] = [];
  const client = createClient({
    baseUrl: "",
    device: { containers: [], video: [], audio: [] },
    fetch: async (url) => {
      urls.push(url);
      return { ok: true, status: 200, json: async () => ({ path: "/", parent: null, dirs: [] }) };
    },
  });
  const queries = createQueries(client);
  await queries.adminFs(null).queryFn({ signal: { aborted: false } });
  await queries.adminFs("/media/Movies & TV").queryFn({ signal: { aborted: false } });
  expect(urls).toEqual(["/api/admin/fs", "/api/admin/fs?path=%2Fmedia%2FMovies%20%26%20TV"]);
  expect(queries.adminFs(null).queryKey).toEqual(["admin", "fs", null]);
});
