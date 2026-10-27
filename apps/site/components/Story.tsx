"use client";

import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useInView, useMotionValueEvent, useScroll, useSpring, useTransform } from "motion/react";
import { useLenis } from "lenis/react";
import { EASE } from "./Reveal";
import { EveryoneScene, LibraryScene, PlaybackScene, SCENE_MEDIA, SubtitleScene, TogetherScene } from "./scenes";

const CHAPTERS = [
  {
    short: "Library",
    title: "Point it at a folder.",
    body: "Nuvora finds the movies, shows and episodes in your folders, then fills in posters, descriptions and cast from Wikipedia, AniList and TVmaze. No API keys, no tagging by hand.",
    Scene: LibraryScene,
  },
  {
    short: "Playback",
    title: "It plays almost anything.",
    body: "When a device can't play a file as it is, Nuvora converts only the part that needs it, often just the audio, while you watch. Jump anywhere and subtitles stay in sync. A graphics chip does the work when there is one.",
    Scene: PlaybackScene,
  },
  {
    short: "Subtitles",
    title: "Subtitles done right.",
    body: "Embedded tracks, and .srt, .ass and .vtt files next to the video. Styled anime subtitles keep their fonts and placement, and your choice is remembered for each show.",
    Scene: SubtitleScene,
  },
  {
    short: "Together",
    title: "Watch together, apart.",
    body: "Invite someone and play anything at the same moment on both screens. Pausing, seeking and the next episode stay in sync.",
    Scene: TogetherScene,
  },
  {
    short: "Everyone",
    title: "Everyone gets their own.",
    body: "Each person signs in to their own progress, Continue Watching and Home screen, arranged the way they like it. No outside account, not even one with us.",
    Scene: EveryoneScene,
  },
];

export function Story() {
  const track = useRef<HTMLDivElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState(0);
  // A scene's script starts when it mounts, so mount it only once the stage is on screen.
  const showing = useInView(stage, { amount: 0.35 });
  const lenis = useLenis();
  // Fetch every still before the story arrives, so no scene paints in pieces.
  const near = useInView(track, { once: true, margin: "900px 0px" });
  useEffect(() => {
    if (!near) return;
    for (const src of SCENE_MEDIA) new Image().src = src;
  }, [near]);
  const { scrollYProgress } = useScroll({ target: track, offset: ["start start", "end end"] });
  const rail = useSpring(scrollYProgress, { stiffness: 120, damping: 28 });
  const n = CHAPTERS.length;
  const within = useTransform(scrollYProgress, (p) => {
    const scaled = Math.min(0.9999, Math.max(0, p)) * n;
    return scaled - Math.floor(scaled);
  });

  useMotionValueEvent(scrollYProgress, "change", (p) => {
    const next = Math.min(n - 1, Math.max(0, Math.floor(p * n)));
    if (next !== active) setActive(next);
  });

  function goTo(i: number) {
    const el = track.current;
    if (!el) return;
    const top = el.getBoundingClientRect().top + window.scrollY;
    const target = top + ((i + 0.08) / n) * (el.offsetHeight - window.innerHeight);
    if (lenis) lenis.scrollTo(target, { duration: 1.4 });
    else window.scrollTo({ top: target, behavior: "smooth" });
  }

  const chapter = CHAPTERS[active]!;

  return (
    <section className="story" id="features" aria-label="What Nuvora does">
      <div className="story-track" ref={track} style={{ height: `${n * 110}vh` }}>
        <div className="story-sticky">
          <div className="container story-grid">
            <div className="story-copy">
              <div className="story-rail" aria-hidden>
                <motion.span style={{ scaleY: rail }} />
              </div>
              <div className="story-count mono" aria-hidden>
                <AnimatePresence mode="popLayout" initial={false}>
                  <motion.span
                    key={active}
                    initial={{ y: "100%", opacity: 0 }}
                    animate={{ y: 0, opacity: 1 }}
                    exit={{ y: "-100%", opacity: 0 }}
                    transition={{ duration: 0.5, ease: EASE }}
                  >
                    {String(active + 1).padStart(2, "0")}
                  </motion.span>
                </AnimatePresence>
                <span className="story-total">/ {String(n).padStart(2, "0")}</span>
              </div>
              {/* Every chapter's text is on the page; CSS shows the active one, so readers and crawlers get all five. */}
              <div className="story-texts">
                {CHAPTERS.map((c, i) => (
                  <div key={c.short} className="story-text" data-active={i === active}>
                    <h3 className="story-title">{c.title}</h3>
                    <p className="story-body">{c.body}</p>
                  </div>
                ))}
              </div>
              <ol className="story-index">
                {CHAPTERS.map((c, i) => (
                  <li key={c.short}>
                    <button type="button" data-active={i === active} onClick={() => goTo(i)}>
                      <span className="mono">{String(i + 1).padStart(2, "0")}</span>
                      {c.short}
                    </button>
                  </li>
                ))}
              </ol>
            </div>

            <div className="story-stage">
              <div className="stage-frame" ref={stage}>
                <div className="stage-dots" aria-hidden>
                  <i />
                  <i />
                  <i />
                  <span className="mono">{chapter.short.toLowerCase()}</span>
                </div>
                <div className="stage-body">
                  <AnimatePresence initial={false}>
                    {showing && (
                      <motion.div
                        key={active}
                        className="stage-scene"
                        initial={{ opacity: 0, scale: 1.03, filter: "blur(10px)" }}
                        animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
                        exit={{ opacity: 0, scale: 0.97, filter: "blur(10px)" }}
                        transition={{ duration: 0.7, ease: EASE }}
                      >
                        <chapter.Scene />
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
                <motion.div className="stage-progress" style={{ scaleX: within }} />
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
