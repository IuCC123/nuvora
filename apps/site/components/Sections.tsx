"use client";

import { useEffect, useRef, useState } from "react";
import { animate, motion, useInView } from "motion/react";
import { banner, randomGlyph } from "@/lib/ascii";
import { GITHUB, GitHubIcon, Mark, Wordmark } from "./Nav";
import { EASE, Reveal, SplitWords } from "./Reveal";
import { TitleCard } from "./TitleCard";
import { media } from "./scenes";

/* Speed ------------------------------------------------------------------ */

/** Counts up to a measured figure once it's on screen; the server renders the figure itself. */
function CountUp({ value, decimals = 1 }: { value: number; decimals?: number }) {
  const ref = useRef<HTMLSpanElement>(null);
  const inView = useInView(ref, { once: true, margin: "-15% 0px" });
  const [shown, setShown] = useState(value);
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (!inView) return setShown(0);
    const controls = animate(0, value, { duration: 1.4, ease: EASE, onUpdate: setShown });
    return () => controls.stop();
  }, [inView, value]);
  return <span ref={ref}>{shown.toFixed(decimals)}</span>;
}

const DECODES = ["HEVC", "HDR10", "E-AC3", "4K"];

export function Speed() {
  return (
    <section className="section container" aria-labelledby="speed-title">
      <h2 className="section-title" id="speed-title">
        <SplitWords text="Fast is a feature." />
      </h2>
      <Reveal as="p" className="section-lede" delay={0.15}>
        The server is written in Rust and built so screens arrive whole, not in pieces.
      </Reveal>

      <Reveal className="stat" delay={0.1}>
        <div className="stat-figure">
          <CountUp value={3.6} />
          <span className="stat-unit">ms</span>
        </div>
        <div className="stat-copy">
          <p className="stat-claim">Home for a 1,000-movie library, in a single 21 KB response.</p>
          <p className="stat-method">
            Median of 300 requests to <code>/api/home</code>, Nuvora 0.3.0 on an Apple M5. The test is{" "}
            <code>scripts/bench-home.sh</code> in the repo, so you can run it on your own server.
          </p>
        </div>
      </Reveal>

      <div className="speed-grid">
        <Reveal className="speed-card" delay={0.05}>
          <div className="speed-visual mono">
            <div className="req">
              <span className="req-verb">GET</span> /api/home
              <motion.span
                className="req-bar"
                initial={{ scaleX: 0 }}
                whileInView={{ scaleX: 1 }}
                viewport={{ once: true }}
                transition={{ duration: 0.8, delay: 0.5, ease: EASE }}
              />
              <span className="req-ok">200</span>
            </div>
            <div className="req-rows">
              {["Continue Watching", "Recently Added", "Movies", "Shows"].map((row, i) => (
                <motion.span
                  key={row}
                  initial={{ opacity: 0, x: -6 }}
                  whileInView={{ opacity: 1, x: 0 }}
                  viewport={{ once: true }}
                  transition={{ duration: 0.5, delay: 1.1 + i * 0.05, ease: EASE }}
                >
                  ✓ {row}
                </motion.span>
              ))}
            </div>
          </div>
          <h3>One request per screen.</h3>
          <p>Home, a movie, a show: each comes back from the server in one response, so the page draws at once.</p>
        </Reveal>

        <Reveal className="speed-card" delay={0.15}>
          <div className="speed-visual paint">
            <div className="paint-poster">
              <img src={media("poster-sintel")} alt="" loading="lazy" />
            </div>
            <div className="paint-poster">
              <img src={media("poster-tears-of-steel")} alt="" loading="lazy" />
            </div>
            <div className="paint-poster">
              <TitleCard title="Elephants Dream" year={2006} palette="slate" x={30} y={20} />
            </div>
          </div>
          <h3>Posters paint in the first frame.</h3>
          <p>A placeholder fills every poster before the artwork lands, and a film without artwork gets a designed title card. No spinners.</p>
        </Reveal>

        <Reveal className="speed-card" delay={0.25}>
          <div className="speed-visual mono hs">
            <div className="hs-row">
              <span className="hs-who">TV can play</span>
              <span className="hs-tags">
                {DECODES.map((tag, i) => (
                  <motion.i
                    key={tag}
                    initial={{ opacity: 0, y: 4 }}
                    whileInView={{ opacity: 1, y: 0 }}
                    viewport={{ once: true }}
                    transition={{ duration: 0.4, delay: 0.4 + i * 0.08 }}
                  >
                    {tag}
                  </motion.i>
                ))}
              </span>
            </div>
            <div className="hs-row">
              <span className="hs-who">File is</span>
              <span className="hs-tags">
                {DECODES.map((tag, i) => (
                  <motion.i
                    key={tag}
                    className="hs-match"
                    initial={{ opacity: 0, y: 4 }}
                    whileInView={{ opacity: 1, y: 0 }}
                    viewport={{ once: true }}
                    transition={{ duration: 0.4, delay: 0.9 + i * 0.12 }}
                  >
                    ✓ {tag}
                  </motion.i>
                ))}
              </span>
            </div>
            <motion.div
              className="hs-out"
              initial={{ opacity: 0, x: -6 }}
              whileInView={{ opacity: 1, x: 0 }}
              viewport={{ once: true }}
              transition={{ duration: 0.5, delay: 1.6, ease: EASE }}
            >
              → Sends the original file, untouched
            </motion.div>
          </div>
          <h3>The original file, when it can.</h3>
          <p>Each app tells the server what it can decode. When the file matches, it plays as it is: HEVC, HDR and surround sound included.</p>
        </Reveal>
      </div>
    </section>
  );
}

/* Compare ---------------------------------------------------------------- */

type Cell = { ok: boolean | "partial"; text?: string };

const ROWS: { label: string; nuvora: Cell; plex: Cell; jellyfin: Cell }[] = [
  {
    label: "Every feature free",
    nuvora: { ok: true },
    plex: { ok: "partial", text: "Plex Pass" },
    jellyfin: { ok: true },
  },
  {
    label: "Stream outside your home",
    nuvora: { ok: true, text: "Free" },
    plex: { ok: false, text: "Paid pass" },
    jellyfin: { ok: true, text: "Free" },
  },
  {
    label: "Convert on the graphics chip",
    nuvora: { ok: true, text: "Free" },
    plex: { ok: false, text: "Plex Pass" },
    jellyfin: { ok: true, text: "Free" },
  },
  {
    label: "No outside account to sign in",
    nuvora: { ok: true },
    plex: { ok: false, text: "plex.tv" },
    jellyfin: { ok: true },
  },
  {
    label: "Open source",
    nuvora: { ok: true, text: "AGPL-3.0" },
    plex: { ok: false },
    jellyfin: { ok: true, text: "GPL-2.0" },
  },
  {
    label: "Just your library, no ads",
    nuvora: { ok: true },
    plex: { ok: "partial", text: "Ad-supported extras" },
    jellyfin: { ok: true },
  },
  {
    label: "Plex-style interface",
    nuvora: { ok: true },
    plex: { ok: true },
    jellyfin: { ok: "partial", text: "Its own" },
  },
];

function Verdict({ cell, delay }: { cell: Cell; delay: number }) {
  return (
    <span className="cmp-cell" data-ok={String(cell.ok)}>
      <svg viewBox="0 0 20 20" width="18" height="18" aria-hidden>
        {cell.ok === true && (
          <motion.path
            d="M4.5 10.5l3.5 3.5 7.5-8"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            initial={{ pathLength: 0 }}
            whileInView={{ pathLength: 1 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5, delay, ease: EASE }}
          />
        )}
        {cell.ok === false && (
          <motion.path
            d="M6 6l8 8M14 6l-8 8"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            initial={{ pathLength: 0 }}
            whileInView={{ pathLength: 1 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5, delay, ease: EASE }}
          />
        )}
        {cell.ok === "partial" && (
          <motion.path
            d="M5.5 10h9"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            initial={{ pathLength: 0 }}
            whileInView={{ pathLength: 1 }}
            viewport={{ once: true }}
            transition={{ duration: 0.4, delay, ease: EASE }}
          />
        )}
      </svg>
      <span className="sr-only">{cell.ok === true ? "Yes" : cell.ok === false ? "No" : "Partly"}</span>
      {cell.text && <span className="cmp-note">{cell.text}</span>}
    </span>
  );
}

export function Compare() {
  return (
    <section className="section container" id="compare" aria-labelledby="compare-title">
      <h2 className="section-title" id="compare-title">
        <SplitWords text="Why not Plex" />
        <SplitWords text="or Jellyfin?" className="dim" delay={0.12} />
      </h2>
      <div className="cmp-intro">
        <Reveal delay={0.1}>
          <h3>Plex is polished, and it's a business.</h3>
          <p>
            Watching from outside your home now takes a paid pass, converting on the graphics chip takes Plex Pass,
            and everyone signs in through plex.tv.
          </p>
        </Reveal>
        <Reveal delay={0.2}>
          <h3>Jellyfin is free, and we're fans.</h3>
          <p>
            Nuvora keeps that freedom and adds Plex's look and speed: a layout you already know, screens in one
            request, and posters that never pop in.
          </p>
        </Reveal>
      </div>

      <Reveal className="cmp-table-wrap" delay={0.1}>
        <table className="cmp-table">
          <thead>
            <tr>
              <th scope="col">
                <span className="sr-only">Feature</span>
              </th>
              <th scope="col" className="cmp-us">
                <span className="wordmark">
                  <Mark />
                  nuvora
                </span>
              </th>
              <th scope="col">Plex</th>
              <th scope="col">Jellyfin</th>
            </tr>
          </thead>
          <tbody>
            {ROWS.map((row, i) => (
              <motion.tr
                key={row.label}
                initial={{ opacity: 0, y: 10 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true, margin: "0px 0px -8% 0px" }}
                transition={{ duration: 0.6, delay: i * 0.06, ease: EASE }}
              >
                <th scope="row">{row.label}</th>
                <td className="cmp-us">
                  <Verdict cell={row.nuvora} delay={0.2 + i * 0.06} />
                </td>
                <td>
                  <Verdict cell={row.plex} delay={0.3 + i * 0.06} />
                </td>
                <td>
                  <Verdict cell={row.jellyfin} delay={0.4 + i * 0.06} />
                </td>
              </motion.tr>
            ))}
          </tbody>
        </table>
      </Reveal>
      <Reveal as="p" className="footnote">
        Plex details from Plex's 2025 announcements: remote playback needs Plex Pass or a Remote Watch Pass, and
        hardware-accelerated streaming needs Plex Pass.
      </Reveal>
    </section>
  );
}

/* TV --------------------------------------------------------------------- */

const TV_W = 46;
const TV_H = 15;

type Seg = { t: string; c: string };

/** One frame of a sunset over water, as rows of colored runs. */
function sunset(time: number): Seg[][] {
  const rows: Seg[][] = [];
  const horizon = Math.round(TV_H * 0.58);
  const sunX = TV_W / 2;
  const sunY = horizon - 1;
  const radius = 6.2;
  for (let y = 0; y < TV_H; y++) {
    const row: Seg[] = [];
    const push = (ch: string, c: string) => {
      const last = row[row.length - 1];
      if (last && last.c === c) last.t += ch;
      else row.push({ t: ch, c });
    };
    for (let x = 0; x < TV_W; x++) {
      if (y < horizon) {
        const dx = (x - sunX) / 2.1;
        const dy = y - sunY;
        const d = Math.sqrt(dx * dx + dy * dy);
        // Stripes cut through the lower half of the sun and drift down.
        const stripe = dy > -2 && Math.floor(y * 1.0 + time * 1.4) % 3 === 0;
        if (d < radius && !stripe) push(d < radius - 1.6 ? "@" : "#", "a");
        else if (d < radius + 1.4 && !stripe) push(":", "a2");
        else {
          const tw = Math.sin(x * 12.9898 + y * 78.233) * 43758.5453;
          const star = tw - Math.floor(tw);
          const blink = Math.sin(time * 2 + x * 3 + y) > 0.2;
          push(star > 0.94 && y < horizon - 3 && blink ? (star > 0.985 ? "+" : ".") : " ", "s");
        }
      } else if (y === horizon) {
        push("─", "h");
      } else {
        const depth = y - horizon;
        const wave = Math.sin(x * 0.45 - time * 2.2 + depth * 1.3) + Math.sin(x * 0.17 + time * 1.1);
        const reflect = Math.abs(x - sunX) < radius * 2.1 - depth * 1.2 + Math.sin(time * 3 + depth) * 1.5;
        if (reflect && wave > -0.4) push(wave > 0.6 ? "=" : "-", "a2");
        else push(wave > 1.1 ? "~" : wave > 0.2 ? "-" : wave > -0.6 ? "." : " ", "w");
      }
    }
    rows.push(row);
  }
  return rows;
}

function AsciiTv() {
  const ref = useRef<HTMLPreElement>(null);
  const inView = useInView(ref, { margin: "100px" });
  const [frame, setFrame] = useState(() => sunset(0));

  useEffect(() => {
    if (!inView) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const start = performance.now();
    const id = setInterval(() => setFrame(sunset((performance.now() - start) / 1000)), 1000 / 14);
    return () => clearInterval(id);
  }, [inView]);

  const edge = "─".repeat(TV_W + 2);
  return (
    <pre className="tv ascii" ref={ref} aria-label="An ASCII television showing a sunset over the sea">
      <span className="f">{"            \\     /\n             \\   /\n              \\ /\n"}</span>
      <span className="f">{`  ╭─${"─".repeat(11)}┴${"─".repeat(TV_W - 8)}─╮\n`}</span>
      <span className="f">{`  │ ╭${edge}╮ │\n`}</span>
      {frame.map((row, y) => (
        <span key={y}>
          <span className="f">{"  │ │ "}</span>
          {row.map((seg, i) => (
            <span key={i} className={seg.c}>
              {seg.t}
            </span>
          ))}
          <span className="f">{" │ │\n"}</span>
        </span>
      ))}
      <span className="f">{`  │ ╰${edge}╯ │\n`}</span>
      <span className="f">{"  │  "}</span>
      <span className="a">▶</span>
      <span className="f">{` nuvora${" ".repeat(TV_W - 9)}○ ●  │\n`}</span>
      <span className="f">{`  ╰─${"─".repeat(TV_W + 4)}─╯\n`}</span>
      <span className="f">{`        ╱▔▔╲${" ".repeat(TV_W - 14)}╱▔▔╲`}</span>
    </pre>
  );
}

const DEVICES = [
  { name: "Any web browser", note: "The web app", state: "ready" },
  { name: "Fire TV", note: "TV app", state: "ready" },
  { name: "Android TV & Google TV", note: "TV app", state: "ready" },
  { name: "Fire TV with Vega OS", note: "Early release", state: "early" },
];

export function Tv() {
  return (
    <section className="section container tv-section" id="tv" aria-labelledby="tv-title">
      <div className="tv-copy">
        <h2 className="section-title" id="tv-title">
          <SplitWords text="On the big screen," />
          <SplitWords text="too." className="dim" delay={0.12} />
        </h2>
        <Reveal as="p" className="section-lede" delay={0.15}>
          The TV app has everything the web app has for watching: Home, your libraries, search, subtitles, Continue
          Watching and Watch Together. It finds your server on the network by itself.
        </Reveal>
        <ul className="devices">
          {DEVICES.map((d, i) => (
            <Reveal as="li" key={d.name} delay={0.2 + i * 0.07}>
              <span className="device-name">{d.name}</span>
              <span className="device-note" data-state={d.state}>
                {d.note}
              </span>
            </Reveal>
          ))}
        </ul>
        <Reveal as="p" className="footnote" delay={0.4}>
          On a TV, open Downloader and type your server's address followed by <code>/tv</code>.
        </Reveal>
      </div>
      <Reveal className="tv-art" delay={0.1}>
        <AsciiTv />
      </Reveal>
    </section>
  );
}

/* Footer ----------------------------------------------------------------- */

const BANNER = banner("NUVORA");

function AsciiBanner() {
  const ref = useRef<HTMLPreElement>(null);
  const inView = useInView(ref, { once: true, margin: "-10% 0px" });
  const [text, setText] = useState(() => BANNER.map((row) => row.replace(/\S/g, " ")).join("\n"));

  useEffect(() => {
    if (!inView) return;
    const final = BANNER.join("\n");
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setText(final);
      return;
    }
    const width = BANNER[0]!.length;
    const start = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      const t = (now - start) / 1400;
      let done = true;
      const rows = BANNER.map((row, y) =>
        row
          .split("")
          .map((ch, x) => {
            if (ch === " ") return ch;
            const at = x / width + y * 0.02;
            if (t > at + 0.18) return ch;
            done = false;
            return t > at ? randomGlyph() : " ";
          })
          .join(""),
      );
      setText(rows.join("\n"));
      if (!done) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [inView]);

  return (
    <div className="banner-wrap" aria-hidden>
      <pre className="banner ascii" ref={ref}>
        {text}
      </pre>
      <pre className="banner banner-glint ascii">{BANNER.join("\n")}</pre>
    </div>
  );
}

export function Footer() {
  return (
    <footer className="footer">
      <div className="container">
        <div className="cta">
          <h2 className="section-title">
            <SplitWords text="Your movies." />
            <SplitWords text="Your server." className="dim" delay={0.12} />
          </h2>
          <Reveal className="cta-actions" delay={0.2}>
            <a className="btn btn-primary" href="#install">
              Install Nuvora
            </a>
            <a className="btn btn-ghost" href={GITHUB}>
              <GitHubIcon />
              Star on GitHub
            </a>
          </Reveal>
        </div>
        <AsciiBanner />
        <div className="footer-row">
          <Wordmark />
          <nav className="footer-links" aria-label="Footer">
            <a href={GITHUB}>GitHub</a>
            <a href={`${GITHUB}/releases/latest`}>Releases</a>
            <a href={`${GITHUB}/blob/main/LICENSE`}>AGPL-3.0</a>
          </nav>
        </div>
        <p className="footer-credit">
          Film images: Big Buck Bunny, Sintel, Tears of Steel and Cosmos Laundromat © Blender Foundation; Elephants
          Dream © Orange Open Movie Project. All under{" "}
          <a href="https://creativecommons.org/licenses/by/3.0/">Creative Commons Attribution</a>.
        </p>
      </div>
    </footer>
  );
}
