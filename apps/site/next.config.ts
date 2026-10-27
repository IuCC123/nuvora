import type { NextConfig } from "next";

const config: NextConfig = {
  // Plain static files, served by Cloudflare (see wrangler.jsonc).
  output: "export",
  reactStrictMode: true,
  agentRules: false,
  devIndicators: false,
};

export default config;
