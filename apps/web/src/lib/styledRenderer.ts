import JASSUB from "jassub";

/**
 * libass in WebAssembly (JASSUB), drawing an ASS track onto `canvas` in step
 * with `video`. Only the player's styled-subtitle layer imports this, through
 * a dynamic import, so the renderer, its worker and its wasm load only when a
 * styled track is on. Vite emits the worker, both wasm builds and JASSUB's
 * fallback font (Liberation Sans, metric-compatible with the Arial most
 * scripts name) as assets from the package's own `new URL(...)` references.
 */
export interface StyledRenderer {
  /** Resolves once the track is parsed and the first frame can draw. */
  ready: Promise<void>;
  destroy: () => void;
}

export function createStyledRenderer(video: HTMLVideoElement, canvas: HTMLCanvasElement, content: string, fonts: string[]): StyledRenderer {
  const jassub = new JASSUB({ video, canvas, subContent: content, fonts, queryFonts: false });
  const ready = jassub.ready.then(() => {
    // The renderer draws on each new video frame; a paused video shows none until it plays or seeks.
    if (video.paused && video.videoWidth) {
      void jassub.manualRender({ mediaTime: video.currentTime, width: video.videoWidth, height: video.videoHeight, expectedDisplayTime: performance.now() });
    }
  });
  return { ready, destroy: () => void jassub.destroy().catch(() => {}) };
}
