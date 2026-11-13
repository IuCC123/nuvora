import { queryKeys, type GenrePage, type HomeResponse, type ItemDetail, type LibraryPage, type PosterCard, type RowPage } from "@nuvora/core";
import { queryClient } from "../nuvora";

/**
 * The card that opened a detail page, from whatever already loaded it: home
 * (its hero too), a home row's later pages, a library grid page, a genre
 * page, or another title's "More Like This". An episode card names its show
 * too, so a show page opened from Continue Watching still paints its title
 * and poster in the first frame; its backdrop is the episode's, so it is left
 * out rather than swapped for the show's a moment later.
 */
export function cachedCard(id: number, kind: "movie" | "show"): Pick<PosterCard, "title" | "year" | "poster" | "backdrop"> | undefined {
  const cards: PosterCard[] = [];
  const home = queryClient.getQueryData<HomeResponse>(queryKeys.home);
  for (const f of home?.featured ?? []) cards.push(f.card);
  for (const row of home?.rows ?? []) cards.push(...row.items);
  for (const [, item] of queryClient.getQueriesData<ItemDetail>({ queryKey: ["item"] })) if (item && "similar" in item) cards.push(...item.similar);
  for (const [, data] of queryClient.getQueriesData<{ pages: GenrePage[] }>({ queryKey: ["genre"] })) for (const p of data?.pages ?? []) cards.push(...p.items);
  for (const [, data] of queryClient.getQueriesData<{ pages: RowPage[] }>({ queryKey: queryKeys.rows })) for (const p of data?.pages ?? []) cards.push(...p.items);
  for (const [, page] of queryClient.getQueriesData<LibraryPage>({ queryKey: queryKeys.library })) if (page && "items" in page) cards.push(...page.items);
  const own = cards.find((c) => c.id === id && c.kind === kind && c.backdrop) ?? cards.find((c) => c.id === id && c.kind === kind);
  if (own) return own;
  const episode = kind === "show" ? cards.find((c) => c.kind === "episode" && c.show_id === id) : undefined;
  return episode && { ...episode, backdrop: null };
}
