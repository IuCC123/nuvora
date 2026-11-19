import { useSyncExternalStore } from "react";
import { desktop } from "./desktop";

/**
 * The player's full screen. In a browser it is the page's. In the Mac and
 * Windows apps it is the window's own: the web view's would move the page into
 * a second window and, on a Mac, leave "Click to Exit Full Screen" in this one,
 * stuck there whenever that second window gets lost. Apps older than 0.8.1
 * can't do the window's and keep the page's.
 */

/** What the app sends whenever its window enters or leaves full screen, however that happened. */
const WINDOW_CHANGED = "nuvora-fullscreen";

/** The window's full screen, once the app has said it has one; `null` means the page's. */
let windowOn: boolean | null = null;
/** Whether the player put the window in full screen: leaving the player takes it out only then. */
let playerEntered = false;
const subscribers = new Set<() => void>();
const changed = () => subscribers.forEach((notify) => notify());

if (desktop) {
  void desktop.isFullscreen().then((on) => {
    windowOn = on;
    changed();
  }, () => {});
  window.addEventListener(WINDOW_CHANGED, (e) => {
    windowOn = (e as CustomEvent<boolean>).detail;
    if (!windowOn) playerEntered = false;
    changed();
  });
}

function subscribe(notify: () => void) {
  subscribers.add(notify);
  document.addEventListener("fullscreenchange", notify);
  return () => {
    subscribers.delete(notify);
    document.removeEventListener("fullscreenchange", notify);
  };
}

const isOn = () => windowOn ?? !!document.fullscreenElement;

/** Whether the window (in the apps) or the page (in a browser) is full screen. */
export function useFullscreen(): boolean {
  return useSyncExternalStore(subscribe, isOn);
}

/** Whether there is a full screen to toggle: not on iPhone, which only has the video's own player. */
export const canFullscreen = () => windowOn !== null || document.fullscreenEnabled;

/** Enters or leaves full screen; false when there is none (see `canFullscreen`). */
export function toggleFullscreen(): boolean {
  if (windowOn !== null && desktop) {
    playerEntered = !windowOn;
    void desktop.setFullscreen(!windowOn).catch(() => {});
    return true;
  }
  if (document.fullscreenElement) void document.exitFullscreen();
  // The whole page, not the player: the next episode mounts a new player and full screen carries over.
  else if (document.fullscreenEnabled) void document.documentElement.requestFullscreen().catch(() => {});
  else return false;
  return true;
}

/** Leaves full screen; with `onlyIfPlayerEntered`, not a window someone made full screen before playing. */
export function exitFullscreen({ onlyIfPlayerEntered = false } = {}) {
  if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
  if (windowOn && desktop && (playerEntered || !onlyIfPlayerEntered)) void desktop.setFullscreen(false).catch(() => {});
}
