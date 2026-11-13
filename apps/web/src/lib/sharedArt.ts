/**
 * The card-to-detail transition: the clicked card's art takes the shared
 * view-transition name, and the detail page's hero art carries the same name
 * (`SHARED_ART`), so the browser morphs one into the other. Only one element
 * may hold a name at a time, so taking it clears the last holder.
 */
export const SHARED_ART = "title-art";

let holder: HTMLElement | null = null;

export function claimSharedArt(el: HTMLElement | null) {
  if (holder && holder !== el) holder.style.viewTransitionName = "";
  holder = el;
  if (el) el.style.viewTransitionName = SHARED_ART;
}
