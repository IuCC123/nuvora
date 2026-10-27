"use client";

import { Fragment, useEffect, useId, useRef, useState } from "react";
import { useInView } from "motion/react";
import { GITHUB } from "./Nav";
import { Reveal, SplitWords } from "./Reveal";

type Line =
  | { kind: "cmd"; ps: string; text: string }
  | { kind: "prompt"; text: string; answer?: string }
  | { kind: "out" | "muted" | "ok"; text: string }
  | { kind: "bar" };

type Platform = {
  id: "unix" | "windows";
  tab: string;
  shell: string;
  copy: string;
  lines: Line[];
  steps: { title: string; body: React.ReactNode }[];
  note?: React.ReactNode;
};

const SITE = "https://getnuvora.online";

const PLATFORMS: Platform[] = [
  {
    id: "unix",
    tab: "macOS & Linux",
    shell: "~ — bash",
    copy: `curl -fsSL ${SITE}/install.sh | bash`,
    lines: [
      { kind: "cmd", ps: "$ ", text: `curl -fsSL ${SITE}/install.sh | bash` },
      { kind: "ok", text: "Docker is running" },
      { kind: "out", text: "Where are your movies and shows?" },
      { kind: "muted", text: "  1  ~/Media    312 videos" },
      { kind: "prompt", text: "? Folder (1, or a path): ", answer: "1" },
      { kind: "ok", text: "Media folder: ~/Media (312 videos)" },
      { kind: "bar" },
      { kind: "ok", text: "Nuvora is running" },
      { kind: "out", text: "Open http://192.168.1.20:8640 to create your account." },
    ],
    steps: [
      {
        title: "Run the command",
        body: "In a terminal. No Docker yet? On Linux it installs it; on a Mac it opens Docker Desktop's download and waits.",
      },
      { title: "Pick your media folder", body: "It suggests folders that hold videos. Or type a path, or drag the folder in." },
      { title: "Create your account", body: "Open the address it prints, then add everyone else in Settings › Users." },
    ],
    note: "Run it again to update. Your answers, accounts and progress stay.",
  },
  {
    id: "windows",
    tab: "Windows",
    shell: "Windows PowerShell",
    copy: `irm ${SITE}/install.ps1 | iex`,
    lines: [
      { kind: "cmd", ps: "PS C:\\Users\\you> ", text: `irm ${SITE}/install.ps1 | iex` },
      { kind: "ok", text: "Docker is running" },
      { kind: "out", text: "Where are your movies and shows?" },
      { kind: "muted", text: "  1  D:\\Media    312 videos" },
      { kind: "prompt", text: "? Folder: ", answer: "1" },
      { kind: "ok", text: "Media folder: D:\\Media (312 videos)" },
      { kind: "bar" },
      { kind: "ok", text: "Nuvora is running" },
      { kind: "out", text: "Open http://192.168.1.20:8640 to create your account." },
    ],
    steps: [
      { title: "Run the command", body: "In PowerShell. No Docker Desktop yet? It installs it for you with winget." },
      { title: "Pick your media folder", body: "Choose a suggested folder, or press Enter to pick one in a window." },
      { title: "Create your account", body: "Your browser opens Nuvora. Add everyone else in Settings › Users." },
    ],
    note: (
      <>
        On a NAS (Synology, Unraid, TrueNAS), use its container app with <a href={`${GITHUB}/blob/main/compose.yaml`}>compose.yaml</a>{" "}
        and a <code>NUVORA_MEDIA</code> setting.
      </>
    ),
  },
];

/** How far through the transcript the animation is: lines before `line` are whole. */
type Cursor = { line: number; chars: number; pct: number };

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function Transcript({ lines, at }: { lines: Line[]; at: Cursor }) {
  return (
    <>
      {lines.slice(0, at.line + 1).map((line, i) => {
        const whole = i < at.line;
        if (line.kind === "cmd") {
          return (
            <div key={i} className="term-line">
              <span className="term-ps">{line.ps}</span>
              {whole ? line.text : line.text.slice(0, at.chars)}
            </div>
          );
        }
        if (line.kind === "prompt") {
          return (
            <div key={i} className="term-line prompt">
              {line.text}
              <span className="term-answer">{whole ? line.answer : line.answer?.slice(0, at.chars)}</span>
            </div>
          );
        }
        if (line.kind === "bar") {
          const pct = whole ? 100 : at.pct;
          const filled = Math.round((pct / 100) * 30);
          return (
            <div key={i} className="term-line bar">
              [{"#".repeat(filled)}
              {".".repeat(30 - filled)}] {pct}%
            </div>
          );
        }
        if (whole || line.kind === "out" || line.kind === "muted" || line.kind === "ok") {
          return (
            <div key={i} className={`term-line ${line.kind}`}>
              {line.kind === "ok" && <span className="ok-mark">✓ </span>}
              <Linked text={line.text} />
            </div>
          );
        }
        return null;
      })}
    </>
  );
}

/** Highlights the address in a line. */
function Linked({ text }: { text: string }) {
  const parts = text.split(/(http:\/\/[^\s]+?)(?=\s|$)/);
  return (
    <>
      {parts.map((part, i) =>
        part.startsWith("http://") ? (
          <span key={i} className="term-url">
            {part}
          </span>
        ) : (
          <Fragment key={i}>{part}</Fragment>
        ),
      )}
    </>
  );
}

function Terminal({ platform }: { platform: Platform }) {
  const ref = useRef<HTMLDivElement>(null);
  // A hidden tab's terminal never intersects, so it plays when its tab is first shown.
  const inView = useInView(ref, { once: true, margin: "-20% 0px" });
  const { lines } = platform;
  // The server renders the finished transcript, so the commands are readable without JavaScript.
  const [at, setAt] = useState<Cursor>({ line: lines.length, chars: 0, pct: 100 });
  const [run, setRun] = useState(0);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduced) return setAt({ line: lines.length, chars: 0, pct: 100 });
    setAt({ line: 0, chars: 0, pct: 0 });
    if (!inView) return;
    let alive = true;

    async function play() {
      await sleep(350);
      for (let i = 0; i < lines.length && alive; i++) {
        const line = lines[i]!;
        if (line.kind === "cmd" || (line.kind === "prompt" && line.answer)) {
          const text = line.kind === "cmd" ? line.text : line.answer!;
          if (line.kind === "prompt") {
            setAt({ line: i, chars: 0, pct: 0 });
            await sleep(450);
          }
          const speed = line.kind === "cmd" ? Math.max(8, 900 / text.length) : 70;
          for (let n = 1; n <= text.length && alive; n++) {
            setAt({ line: i, chars: n, pct: 0 });
            await sleep(speed);
          }
          await sleep(line.kind === "cmd" ? 380 : 250);
        } else if (line.kind === "bar") {
          for (let p = 0; p <= 100 && alive; p += 4) {
            setAt({ line: i, chars: 0, pct: p });
            await sleep(40 + Math.random() * 50);
          }
        } else {
          setAt({ line: i, chars: 0, pct: 0 });
          await sleep(line.kind === "prompt" ? 600 : 420);
        }
      }
      if (alive) setAt({ line: lines.length, chars: 0, pct: 100 });
    }
    play();
    return () => {
      alive = false;
    };
  }, [inView, run, lines]);

  async function copy() {
    await navigator.clipboard.writeText(platform.copy);
    setCopied(true);
    setTimeout(() => setCopied(false), 1600);
  }

  return (
    <div className="term" ref={ref}>
      <div className="term-bar">
        <i />
        <i />
        <i />
        <span className="mono">{platform.shell}</span>
        <div className="term-actions">
          <button type="button" onClick={() => setRun((r) => r + 1)} aria-label="Replay the install">
            Replay
          </button>
          <button type="button" onClick={copy}>
            {copied ? "Copied" : "Copy command"}
          </button>
        </div>
      </div>
      <pre className="term-body mono">
        <Transcript lines={lines} at={at} />
        <span className="term-caret" />
      </pre>
    </div>
  );
}

export function Install() {
  const [current, setCurrent] = useState(0);
  const id = useId();

  // Start Windows visitors on the Windows tab.
  useEffect(() => {
    if (/Windows/i.test(navigator.userAgent)) setCurrent(1);
  }, []);

  return (
    <section className="section container" id="install" aria-labelledby="install-title">
      <h2 className="section-title" id="install-title">
        <SplitWords text="Running in" />
        <SplitWords text="five minutes." className="dim" delay={0.12} />
      </h2>
      <Reveal as="p" className="section-lede" delay={0.15}>
        Nuvora runs in Docker on a PC, Mac, NAS or Raspberry Pi. One command sets it up, Docker included, and asks you one
        question: where your movies and shows are.
      </Reveal>

      <Reveal className="install-box" delay={0.1}>
        <div className="tabs" role="tablist" aria-label="Your system">
          {PLATFORMS.map((p, i) => (
            <button
              key={p.id}
              type="button"
              role="tab"
              id={`${id}-tab-${p.id}`}
              aria-selected={i === current}
              aria-controls={`${id}-panel-${p.id}`}
              tabIndex={i === current ? 0 : -1}
              onClick={() => setCurrent(i)}
              onKeyDown={(e) => {
                if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
                const next = (i + (e.key === "ArrowRight" ? 1 : PLATFORMS.length - 1)) % PLATFORMS.length;
                setCurrent(next);
                document.getElementById(`${id}-tab-${PLATFORMS[next]!.id}`)?.focus();
              }}
            >
              {p.tab}
            </button>
          ))}
        </div>
        {/* Both panels are in the page; CSS shows the chosen one, or both under headings without scripts. */}
        {PLATFORMS.map((p, i) => (
          <div
            key={p.id}
            id={`${id}-panel-${p.id}`}
            className="tab-panel"
            role="tabpanel"
            aria-labelledby={`${id}-tab-${p.id}`}
            data-active={i === current}
          >
            <h3 className="tab-panel-title">{p.tab}</h3>
            <Terminal platform={p} />
            <ol className="steps">
              {p.steps.map((s, n) => (
                <li key={s.title}>
                  <span className="step-n mono">{n + 1}</span>
                  <h3>{s.title}</h3>
                  <p>{s.body}</p>
                </li>
              ))}
            </ol>
            {p.note && <p className="install-note">{p.note}</p>}
          </div>
        ))}
      </Reveal>

      <Reveal as="p" className="install-more" delay={0.2}>
        <a href={`${GITHUB}#set-up-your-server`}>
          Every setting, updating and remote access in the full guide <span aria-hidden>→</span>
        </a>
      </Reveal>
    </section>
  );
}
