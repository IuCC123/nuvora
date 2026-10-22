import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { cssVariables } from "@nuvora/tokens";

/** `import "virtual:nuvora-tokens.css"` = the `:root` variables from @nuvora/tokens. No generated file to forget. */
function tokensCss(): Plugin {
  const id = "virtual:nuvora-tokens.css";
  return {
    name: "nuvora-tokens",
    resolveId: (source) => (source === id ? "\0" + id : undefined),
    load: (resolved) => (resolved === "\0" + id ? cssVariables() : undefined),
  };
}

export default defineConfig({
  plugins: [react(), tokensCss()],
  server: {
    // Dev: Vite serves the app with HMR; the Rust server answers /api (default bind).
    proxy: { "/api": "http://127.0.0.1:8640" },
  },
  build: {
    // Read by the Rust binary (rust-embed) for production.
    outDir: "dist",
    assetsDir: "assets",
  },
});
