/** `hls.js/light` is the same API without subtitles, alternate audio, DRM and CMCD; the package only types the full build. */
declare module "hls.js/light" {
  export { default } from "hls.js";
  export * from "hls.js";
}
