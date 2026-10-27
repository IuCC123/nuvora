"use client";

import { Fragment } from "react";
import { motion, type Variants } from "motion/react";

export const EASE = [0.22, 1, 0.36, 1] as const;

const fadeUp: Variants = {
  hidden: { opacity: 0, y: 28, filter: "blur(10px)" },
  shown: { opacity: 1, y: 0, filter: "blur(0px)" },
};

/** Fades and lifts its children into place the first time they scroll into view. */
export function Reveal({
  children,
  delay = 0,
  className,
  as = "div",
}: {
  children: React.ReactNode;
  delay?: number;
  className?: string;
  as?: "div" | "p" | "li" | "section";
}) {
  const Tag = motion[as];
  return (
    <Tag
      className={className}
      variants={fadeUp}
      initial="hidden"
      whileInView="shown"
      viewport={{ once: true, margin: "0px 0px -12% 0px" }}
      transition={{ duration: 1, delay, ease: EASE }}
    >
      {children}
    </Tag>
  );
}

/**
 * Each word rises out of its own mask. `play` starts it on mount for the hero;
 * otherwise it starts when the heading scrolls into view.
 */
export function SplitWords({
  text,
  delay = 0,
  stagger = 0.06,
  play,
  className,
}: {
  text: string;
  delay?: number;
  stagger?: number;
  play?: boolean;
  className?: string;
}) {
  const words = text.split(" ");
  const trigger = play
    ? { animate: "shown" as const }
    : { whileInView: "shown" as const, viewport: { once: true, margin: "0px 0px -10% 0px" } };
  return (
    <motion.span className={className} initial="hidden" {...trigger}>
      <span className="sr-only">{text}</span>
      {words.map((word, i) => (
        <Fragment key={i}>
          <span className="word-mask" aria-hidden>
            <motion.span
              className="word"
              variants={{ hidden: { y: "110%", rotate: 4 }, shown: { y: "0%", rotate: 0 } }}
              transition={{ duration: 1.1, delay: delay + i * stagger, ease: EASE }}
            >
              {word}
            </motion.span>
          </span>
          {i < words.length - 1 ? " " : null}
        </Fragment>
      ))}
    </motion.span>
  );
}
