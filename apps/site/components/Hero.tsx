"use client";

import { useRef } from "react";
import { motion, useScroll, useTransform } from "motion/react";
import { AsciiMark } from "./AsciiMark";
import { GITHUB, GitHubIcon } from "./Nav";
import { EASE, SplitWords } from "./Reveal";

export function Hero() {
  const ref = useRef<HTMLElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start start", "end start"] });
  const copyY = useTransform(scrollYProgress, [0, 1], [0, -140]);
  const copyOpacity = useTransform(scrollYProgress, [0, 0.55], [1, 0]);
  const markScale = useTransform(scrollYProgress, [0, 1], [1, 1.12]);
  const markOpacity = useTransform(scrollYProgress, [0.3, 0.95], [1, 0]);

  return (
    <section className="hero" id="top" ref={ref}>
      <motion.div className="hero-mark" style={{ scale: markScale, opacity: markOpacity }}>
        <AsciiMark dissolve={scrollYProgress} />
      </motion.div>
      <div className="hero-fade" />

      <motion.div className="hero-copy container" style={{ y: copyY, opacity: copyOpacity }}>
        <h1 className="hero-title">
          <SplitWords text="Plex's polish." play delay={0.35} />
          <SplitWords text="Jellyfin's freedom." play delay={0.55} className="dim" />
        </h1>
        <motion.p
          className="hero-lede"
          initial={{ opacity: 0, y: 16, filter: "blur(8px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          transition={{ duration: 1.1, delay: 0.95, ease: EASE }}
        >
          Nuvora is a free, open-source media server for your movies, shows and anime. It runs on your own machine,
          plays almost any file, and needs no account with anyone but you.
        </motion.p>
        <motion.div
          className="hero-actions"
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 1, delay: 1.15, ease: EASE }}
        >
          <a className="btn btn-primary" href="#install">
            Install Nuvora
          </a>
          <a className="btn btn-ghost" href={GITHUB}>
            <GitHubIcon />
            View on GitHub
          </a>
        </motion.div>
        <motion.p
          className="hero-meta mono"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 1.2, delay: 1.5 }}
        >
          v0.3.0 · AGPL-3.0 · Docker · x86-64 &amp; ARM64
        </motion.p>
      </motion.div>
    </section>
  );
}

const STRIP = [
  "MKV",
  "MP4",
  "H.264",
  "HEVC",
  "4K",
  "HDR",
  "5.1 surround",
  ".srt",
  ".ass",
  ".vtt",
  "VideoToolbox",
  "NVENC",
  "VA-API",
  "x86-64",
  "ARM64",
  "Raspberry Pi",
  "Synology",
  "Unraid",
];

export function FormatStrip() {
  const items = [...STRIP, ...STRIP];
  return (
    <div className="strip" aria-label="Formats and hardware Nuvora handles">
      <div className="strip-track">
        {items.map((item, i) => (
          <span key={i} className="strip-item mono" aria-hidden={i >= STRIP.length}>
            <span className="strip-dot">+</span>
            {item}
          </span>
        ))}
      </div>
    </div>
  );
}
