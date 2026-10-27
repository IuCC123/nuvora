// Renders public/og.jpg, the 1200×630 link preview, from the Home screen on the running site.
//
//   bun run dev            # in another terminal
//   bun run og [url]       # default http://localhost:3100/
//
// Needs Chrome or Chromium; set CHROME_PATH if it isn't in a usual place.
import { existsSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const url = process.argv[2] ?? "http://localhost:3100/";
const out = fileURLToPath(new URL("../public/og.jpg", import.meta.url));
const chrome = [
  process.env.CHROME_PATH,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Chromium.app/Contents/MacOS/Chromium",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].find((p) => p && existsSync(p));
if (!chrome) throw new Error("No Chrome found; set CHROME_PATH");

const browser = await chromium.launch({ executablePath: chrome, headless: true });

// 1. The Home shot, flat and fully loaded, at twice the size it will be shown.
const site = await browser.newPage({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 2 });
await site.goto(url, { waitUntil: "networkidle" });
await site.evaluate(() => {
  const shot = document.querySelector(".shot-section");
  window.scrollTo({ top: shot.getBoundingClientRect().top + window.scrollY - 140, behavior: "instant" });
});
await site.waitForTimeout(2500);
const shot = await site.locator(".shot-window").screenshot({ type: "png" });

// 2. The card: the line on the left, the app on the right running off the edge.
const card = await browser.newPage({ viewport: { width: 1200, height: 630 }, deviceScaleFactor: 1 });
await card.setContent(`<!doctype html>
<html><head>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Archivo:wdth,wght@62..125,100..900&family=Inter:wght@400..700&display=block">
<style>
  * { margin: 0; box-sizing: border-box; }
  body {
    width: 1200px; height: 630px; overflow: hidden; position: relative;
    background: radial-gradient(60% 70% at 85% 95%, rgba(229,160,13,.16), transparent 70%), #000;
    font-family: Inter, sans-serif; color: #f2f2f2; -webkit-font-smoothing: antialiased;
  }
  .copy { position: absolute; left: 64px; top: 0; bottom: 0; width: 520px; display: flex; flex-direction: column; justify-content: center; }
  .mark { display: flex; align-items: center; gap: 12px; font-family: Archivo; font-stretch: 112%; font-weight: 700; font-size: 30px; }
  .mark svg { width: 21px; color: #e5a00d; }
  h1 { margin-top: 44px; font-family: Archivo; font-weight: 600; font-size: 66px; line-height: .98; letter-spacing: -0.04em; }
  h1 span { display: block; color: #66696e; }
  p { margin-top: 26px; font-size: 23px; line-height: 1.45; color: #a6a8ab; }
  .url { margin-top: 34px; font-size: 18px; font-weight: 600; color: #e5a00d; }
  .shot { position: absolute; left: 610px; top: 92px; width: 920px; border-radius: 14px; border: 1px solid rgba(255,255,255,.14);
    box-shadow: 0 40px 120px rgba(0,0,0,.9); transform: perspective(1800px) rotateY(-12deg); transform-origin: 0 50%; }
</style></head>
<body>
  <div class="copy">
    <div class="mark"><svg viewBox="0 0 16 18"><path fill="currentColor" d="M1.5 1.6v14.8a1 1 0 0 0 1.5.86l12-7.4a1 1 0 0 0 0-1.72L3 .74A1 1 0 0 0 1.5 1.6z"/></svg>nuvora</div>
    <h1>Plex's polish.<span>Jellyfin's freedom.</span></h1>
    <p>A free, open-source media server for your movies, shows and anime.</p>
    <div class="url">getnuvora.online</div>
  </div>
  <img class="shot" src="data:image/png;base64,${shot.toString("base64")}">
</body></html>`);
await card.evaluate(async () => {
  await document.fonts.ready;
  await document.querySelector(".shot").decode();
});
writeFileSync(out, await card.screenshot({ type: "jpeg", quality: 88 }));
await browser.close();
console.log(`Wrote ${out}`);
