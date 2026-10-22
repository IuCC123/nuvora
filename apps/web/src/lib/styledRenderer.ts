import JASSUB from "jassub";
import fallbackFont from "jassub/dist/default.woff2?url";

/**
 * libass in WebAssembly (JASSUB), drawing an ASS track onto `canvas` in step
 * with `video`. Only the player's styled-subtitle layer imports this, through
 * a dynamic import, so the renderer, its worker and its wasm load only when a
 * styled track is on. Vite emits the worker and both wasm builds as assets
 * from the package's own `new URL(...)` references. Fonts come from the file,
 * then JASSUB's bundled Liberation Sans, metric-compatible with the Arial
 * most scripts name.
 */
export interface StyledRenderer {
  /** Resolves once the track is parsed and the first frame can draw. */
  ready: Promise<void>;
  destroy: () => void;
}

/** The family name inside JASSUB's bundled `default.woff2`. */
const FALLBACK_FAMILY = "liberation sans";

export function createStyledRenderer(video: HTMLVideoElement, canvas: HTMLCanvasElement, content: string, fonts: string[]): StyledRenderer {
  // The fallback loads with the file's fonts, before the first frame. Left to load on demand, it arrives
  // after that frame is drawn, and a paused video shows a frame without its dialogue until it plays.
  const jassub = new JASSUB({
    video,
    canvas,
    subContent: content,
    fonts: [...fonts, new URL(fallbackFont, location.href).href],
    defaultFont: FALLBACK_FAMILY,
    queryFonts: false,
  });
  const ready = jassub.ready.then(() => {
    // The renderer draws on each new video frame; a paused video shows none until it plays or seeks.
    if (video.paused && video.videoWidth) {
      void jassub.manualRender({ mediaTime: video.currentTime, width: video.videoWidth, height: video.videoHeight, expectedDisplayTime: performance.now() });
    }
  });
  return { ready, destroy: () => void jassub.destroy().catch(() => {}) };
}
