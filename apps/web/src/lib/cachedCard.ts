import { queryKeys, type HomeResponse, type LibraryPage, type PosterCard, type RowPage } from "@nuvora/core";
import { queryClient } from "../nuvora";

/**
 * The card that opened a detail page, from whatever already loaded it: home,
 * a home row's later pages, or a library grid page. An episode card names its
 * show too, so a show page opened from Continue Watching still paints its
 * title and poster in the first frame.
 */
export function cachedCard(id: number, kind: "movie" | "show"): Pick<PosterCard, "title" | "year" | "poster"> | undefined {
  const cards: PosterCard[] = [];
  const home = queryClient.getQueryData<HomeResponse>(queryKeys.home);
  for (const row of home?.rows ?? []) cards.push(...row.items);
  for (const [, data] of queryClient.getQueriesData<{ pages: RowPage[] }>({ queryKey: queryKeys.rows })) for (const p of data?.pages ?? []) cards.push(...p.items);
  for (const [, page] of queryClient.getQueriesData<LibraryPage>({ queryKey: queryKeys.library })) if (page && "items" in page) cards.push(...page.items);
  return cards.find((c) => c.id === id && c.kind === kind) ?? (kind === "show" ? cards.find((c) => c.kind === "episode" && c.show_id === id) : undefined);
}
