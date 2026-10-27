"use client";

import { useRef, useState } from "react";
import { motion, useMotionValueEvent, useScroll, useSpring } from "motion/react";
import { EASE } from "./Reveal";

export const GITHUB = "https://github.com/IuCC123/nuvora";

export function Mark({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={(size * 18) / 16} viewBox="0 0 16 18" aria-hidden focusable={false}>
      <path d="M1.5 1.6v14.8a1 1 0 0 0 1.5.86l12-7.4a1 1 0 0 0 0-1.72L3 .74A1 1 0 0 0 1.5 1.6z" fill="currentColor" />
    </svg>
  );
}

export function Wordmark() {
  return (
    <a href="#top" className="wordmark" aria-label="Nuvora, back to top">
      <Mark />
      <span aria-hidden>nuvora</span>
    </a>
  );
}

export function GitHubIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 16 16" aria-hidden focusable={false}>
      <path
        fill="currentColor"
        d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
      />
    </svg>
  );
}

/** How far the page must move in one direction before the nav hides or comes back. */
const TRAVEL = 24;

export function Nav() {
  const { scrollY, scrollYProgress } = useScroll();
  const [hidden, setHidden] = useState(false);
  const [solid, setSolid] = useState(false);
  const progress = useSpring(scrollYProgress, { stiffness: 140, damping: 30, mass: 0.4 });
  // Where the current scroll direction began. Smooth scrolling eases out in sub-pixel
  // steps, so the nav follows distance travelled, never a single frame's delta.
  const turn = useRef({ at: 0, down: true });

  useMotionValueEvent(scrollY, "change", (y) => {
    const previous = scrollY.getPrevious() ?? y;
    setSolid(y > 24);
    if (y < 640) {
      setHidden(false);
      return;
    }
    if (y === previous) return;
    const down = y > previous;
    if (down !== turn.current.down) turn.current = { at: previous, down };
    if (Math.abs(y - turn.current.at) > TRAVEL) setHidden(down);
  });

  return (
    <motion.header
      className="nav"
      data-solid={solid}
      initial={{ y: -80, opacity: 0 }}
      animate={{ y: hidden ? -80 : 0, opacity: 1 }}
      transition={{ duration: 0.5, ease: EASE }}
    >
      <motion.div className="nav-progress" style={{ scaleX: progress }} />
      <div className="nav-inner">
        <Wordmark />
        <nav className="nav-links" aria-label="Sections">
          <a href="#features">Features</a>
          <a href="#compare">Compare</a>
          <a href="#tv">TV</a>
          <a href="#install">Install</a>
          <a href="#faq">FAQ</a>
        </nav>
        <div className="nav-actions">
          <a className="icon-link" href={GITHUB} aria-label="Nuvora on GitHub">
            <GitHubIcon />
          </a>
          <a className="btn btn-primary btn-sm" href="#install">
            Get Nuvora
          </a>
        </div>
      </div>
    </motion.header>
  );
}
