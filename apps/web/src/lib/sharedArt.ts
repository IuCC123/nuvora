/**
 * The card-to-detail transition: the clicked card's art takes the shared
 * view-transition name, and the detail page's hero art carries the same name
 * (`[data-shared]` in CSS), so the browser morphs one into the other. Only
 * one element may hold a name at a time (a duplicate cancels the whole
 * transition), so taking it clears the last holder and any hero on screen,
 * as when "More Like This" opens one detail page from another.
 */
export const SHARED_ART = "title-art";

let holder: HTMLElement | null = null;

export function claimSharedArt(el: HTMLElement | null) {
  if (holder && holder !== el) holder.style.viewTransitionName = "";
  for (const hero of document.querySelectorAll("[data-shared]")) hero.removeAttribute("data-shared");
  holder = el;
  if (el) el.style.viewTransitionName = SHARED_ART;
}
