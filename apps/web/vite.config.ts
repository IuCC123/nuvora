import { createReadStream, existsSync, statSync } from "node:fs";
import { join } from "node:path";
import { defineConfig, loadEnv, type Plugin, type ViteDevServer } from "vite";
import react from "@vitejs/plugin-react";
import { cssVariables } from "@nuvora/tokens";

/** `import "virtual:nuvora-tokens.css"` = the `:root` variables from @nuvora/tokens. No generated file to forget. */
function tokensCss(): Plugin {
  const id = "virtual:nuvora-tokens.css";
  const entry = join(import.meta.dirname, "../../packages/tokens/src/index.ts");
  let server: ViteDevServer | undefined;
  return {
    name: "nuvora-tokens",
    configureServer(s) {
      server = s;
    },
    resolveId: (source) => (source === id ? "\0" + id : undefined),
    async load(resolved) {
      if (resolved !== "\0" + id) return undefined;
      this.addWatchFile(entry);
      // In dev, load tokens through Vite rather than this config's own import, so a token edit shows without a restart.
      const mod = server ? ((await server.ssrLoadModule(entry)) as { cssVariables: typeof cssVariables }) : { cssVariables };
      return mod.cssVariables();
    },
  };
}

/**
 * Dev server only, with `VITE_FIXTURES=1`: serves fixture artwork from
 * `fixtures/images/<id>/<size>.webp` and one sample video for every stream URL
 * (`NUVORA_FIXTURE_VIDEO`, default: the synthetic library's Tears of Steel),
 * with Range support so the player can seek.
 */
function fixtureAssets(video: string): Plugin {
  return {
    name: "nuvora-fixture-assets",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const image = /^\/api\/images\/([0-9a-f]{32})\/(small|large)$/.exec(req.url ?? "");
        if (image) {
          const file = join(import.meta.dirname, "fixtures/images", image[1]!, `${image[2]}.webp`);
          if (!existsSync(file)) return void res.writeHead(404).end();
          res.writeHead(200, { "content-type": "image/webp", "cache-control": "public, max-age=31536000, immutable" });
          return void createReadStream(file).pipe(res);
        }
        if (!/^\/api\/files\/\d+\/stream$/.test(req.url ?? "")) return next();
        if (!existsSync(video)) return void res.writeHead(404).end();
        const size = statSync(video).size;
        const type = video.endsWith(".webm") ? "video/webm" : "video/mp4";
        const range = /bytes=(\d*)-(\d*)/.exec(req.headers.range ?? "");
        if (!range) {
          res.writeHead(200, { "content-type": type, "content-length": size, "accept-ranges": "bytes" });
          return void createReadStream(video).pipe(res);
        }
        const start = range[1] ? Number(range[1]) : 0;
        const end = range[2] ? Math.min(Number(range[2]), size - 1) : size - 1;
        res.writeHead(206, {
          "content-type": type,
          "content-length": end - start + 1,
          "content-range": `bytes ${start}-${end}/${size}`,
          "accept-ranges": "bytes",
        });
        createReadStream(video, { start, end }).pipe(res);
      });
    },
  };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const fixtures = env.VITE_FIXTURES === "1";
  const video = env.NUVORA_FIXTURE_VIDEO ?? "/tmp/nuvora-media/Movies/Tears of Steel (2012)/Tears of Steel (2012).webm";
  return {
    plugins: [react(), tokensCss(), ...(fixtures ? [fixtureAssets(video)] : [])],
    // Dev: Vite serves the app with HMR; the Rust server answers /api (default bind, or NUVORA_SERVER).
    // The Host header stays Vite's, so it matches the browser's Origin and passes the server's CSRF check.
    server: fixtures ? {} : { proxy: { "/api": { target: env.NUVORA_SERVER ?? "http://127.0.0.1:8640", changeOrigin: false } } },
    build: {
      // Read by the Rust binary (rust-embed) for production.
      outDir: "dist",
      assetsDir: "assets",
      target: "es2022",
    },
  };
});
