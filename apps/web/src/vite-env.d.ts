declare module "virtual:nuvora-tokens.css";

interface ImportMetaEnv {
  /** `"1"`: answer the API from `src/dev/fixtures.ts` instead of the Rust server. */
  readonly VITE_FIXTURES?: string;
}
