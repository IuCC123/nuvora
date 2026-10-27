"use client";

import { useEffect, useRef } from "react";
import type { MotionValue } from "motion/react";
import { cellHash, noise3, SCRAMBLE } from "@/lib/ascii";

/** Dense to light. Index 0 is the brightest cell. */
const RAMP = "@%#*+=-:.";
const GLYPHS = Array.from(new Set((RAMP + SCRAMBLE + "·").split("")));
const AMBER_LEVELS = 12;
const WHITE_LEVELS = 8;

type Geometry = {
  cols: number;
  rows: number;
  cw: number;
  ch: number;
  sdf: Float32Array;
  hash: Float32Array;
  reveal: Float32Array;
  size: number;
};

/**
 * The Nuvora play mark drawn in characters over a slow field of noise. It
 * assembles on load, flares under the pointer, and crumbles as `dissolve`
 * goes from 0 to 1.
 */
export function AsciiMark({ dissolve }: { dissolve?: MotionValue<number> }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const family = getComputedStyle(document.documentElement).getPropertyValue("--font-mono").trim() || "monospace";
    let dpr = 1;
    let width = 0;
    let height = 0;
    let geo: Geometry | null = null;
    let atlas: HTMLCanvasElement | null = null;
    let raf = 0;
    let visible = true;
    let start = performance.now();
    const pointer = { x: -9999, y: -9999, strength: 0, target: 0 };

    function buildAtlas(fontSize: number, cw: number, ch: number) {
      const a = document.createElement("canvas");
      a.width = Math.ceil(GLYPHS.length * cw * dpr);
      a.height = Math.ceil((AMBER_LEVELS + WHITE_LEVELS) * ch * dpr);
      const g = a.getContext("2d")!;
      g.scale(dpr, dpr);
      g.font = `500 ${fontSize}px ${family}`;
      g.textBaseline = "middle";
      g.textAlign = "center";
      for (let level = 0; level < AMBER_LEVELS + WHITE_LEVELS; level++) {
        if (level < AMBER_LEVELS) {
          const t = (level + 1) / AMBER_LEVELS;
          // The brightest levels warm toward a pale gold.
          const hot = Math.max(0, (t - 0.8) / 0.2);
          const r = Math.round(229 + (255 - 229) * hot);
          const gch = Math.round(160 + (214 - 160) * hot);
          const b = Math.round(13 + (130 - 13) * hot);
          g.fillStyle = `rgba(${r},${gch},${b},${0.12 + 0.88 * t})`;
        } else {
          const t = (level - AMBER_LEVELS + 1) / WHITE_LEVELS;
          g.fillStyle = `rgba(242,242,242,${0.05 + 0.4 * t})`;
        }
        GLYPHS.forEach((glyph, i) => g.fillText(glyph, i * cw + cw / 2, level * ch + ch / 2));
      }
      return a;
    }

    function layout() {
      const rect = canvas!.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas!.width = Math.round(width * dpr);
      canvas!.height = Math.round(height * dpr);

      const fontSize = width < 700 ? 10 : 12;
      ctx!.font = `500 ${fontSize}px ${family}`;
      const cw = Math.ceil(ctx!.measureText("M").width);
      const ch = Math.round(fontSize * 1.3);
      const cols = Math.ceil(width / cw);
      const rows = Math.ceil(height / ch);

      // The logo's triangle is 14 wide by 16.5 tall, with its centroid on the anchor.
      const wide = width >= 900;
      const triH = wide ? Math.min(height * 0.54, width * 0.38) : Math.min(height * 0.3, width * 0.6);
      const triW = triH / 1.18;
      const anchorX = wide ? width * 0.76 : width * 0.52;
      const anchorY = wide ? height * 0.4 : height * 0.25;
      const x0 = anchorX - triW / 3;
      const y0 = anchorY - triH / 2;
      const radius = triW * 0.06;

      const n = cols * rows;
      const sdf = new Float32Array(n);
      const hash = new Float32Array(n);
      const reveal = new Float32Array(n);
      for (let y = 0; y < rows; y++) {
        for (let x = 0; x < cols; x++) {
          const i = y * cols + x;
          const px = x * cw + cw / 2;
          const py = y * ch + ch / 2;
          sdf[i] = sdTriangle(px, py, x0, y0, x0, y0 + triH, x0 + triW, y0 + triH / 2) - radius;
          hash[i] = cellHash(x, y);
          const dist = Math.hypot(px - anchorX, py - anchorY) / triH;
          reveal[i] = 0.2 + dist * 0.9 + hash[i]! * 0.45;
        }
      }
      geo = { cols, rows, cw, ch, sdf, hash, reveal, size: triW };
      atlas = buildAtlas(fontSize, cw, ch);
    }

    function draw(now: number) {
      if (!geo || !atlas) return;
      const t = reduced ? 10 : (now - start) / 1000;
      const { cols, rows, cw, ch, sdf, hash, reveal, size } = geo;
      const melt = Math.min(1, Math.max(0, (dissolve?.get() ?? 0) * 1.35));
      pointer.strength += (pointer.target - pointer.strength) * 0.06;
      const radius = Math.max(110, size * 0.38);
      const sweep = ((t * 0.18) % 1.6) - 0.3; // a slow diagonal glint, in units of the mark's size
      const fieldIn = Math.min(1, t / 1.6);

      ctx!.setTransform(1, 0, 0, 1, 0, 0);
      ctx!.clearRect(0, 0, canvas!.width, canvas!.height);
      const sw = cw * dpr;
      const sh = ch * dpr;

      for (let y = 0; y < rows; y++) {
        const py = y * ch + ch / 2;
        for (let x = 0; x < cols; x++) {
          const i = y * cols + x;
          const px = x * cw + cw / 2;
          const d = sdf[i]!;
          const h = hash[i]!;
          const dx = px - pointer.x;
          const dy = py - pointer.y;
          const near = pointer.strength * Math.exp(-(dx * dx + dy * dy) / (radius * radius));

          let glyph = -1;
          let level = -1;

          const crumbled = h < melt;
          if (d < size * 0.05 && !crumbled) {
            const since = t - reveal[i]!;
            if (since < 0) continue;
            if (since < 0.32) {
              glyph = Math.floor(h * 997 + t * 30) % GLYPHS.length;
              level = AMBER_LEVELS - 1 - Math.floor(since * 10);
            } else {
              const depth = Math.min(1, Math.max(0, -d / (size * 0.22)));
              const grain = noise3(x * 0.16, y * 0.26, t * 0.55);
              const along = (px + py * 0.6) / (size * 2.2) - sweep;
              const glint = Math.exp(-(along * along) / 0.004);
              let v = 0.32 + 0.38 * depth + 0.3 * grain + 0.55 * glint + 0.5 * near;
              if (d > 0) v *= 0.45; // the soft rim outside the edge
              v = Math.min(1, v);
              if (near > 0.35 && h > 0.6) glyph = Math.floor(h * 131 + t * 18) % GLYPHS.length;
              else glyph = GLYPHS.indexOf(RAMP[Math.min(RAMP.length - 1, Math.floor((1 - v) * RAMP.length))]!);
              level = Math.min(AMBER_LEVELS - 1, Math.floor(v * AMBER_LEVELS));
            }
          } else {
            const n = noise3(x * 0.055, y * 0.1, t * 0.12) * 0.75 + noise3(x * 0.21, y * 0.33, t * 0.3) * 0.25;
            const threshold = 0.6 - near * 0.35 - (crumbled && d < 0 ? 0.25 : 0);
            if (n < threshold) continue;
            const v = Math.min(1, (n - threshold) / 0.22 + near * 0.6) * fieldIn;
            if (v <= 0.02) continue;
            glyph = GLYPHS.indexOf(RAMP[Math.min(RAMP.length - 1, Math.floor((1 - v) * 4) + 5)]!);
            level = AMBER_LEVELS + Math.min(WHITE_LEVELS - 1, Math.floor(v * WHITE_LEVELS));
          }

          if (glyph < 0 || level < 0) continue;
          ctx!.drawImage(atlas, glyph * sw, level * sh, sw, sh, x * sw, y * sh, sw, sh);
        }
      }
    }

    function frame(now: number) {
      draw(now);
      if (visible && !reduced) raf = requestAnimationFrame(frame);
    }

    function restart() {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(frame);
    }

    const onMove = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      pointer.x = e.clientX - rect.left;
      pointer.y = e.clientY - rect.top;
      pointer.target = e.pointerType === "mouse" ? 1 : 0.7;
    };
    const onLeave = () => (pointer.target = 0);

    const ro = new ResizeObserver(() => {
      layout();
      if (reduced || !visible) draw(performance.now());
    });
    const io = new IntersectionObserver(([entry]) => {
      visible = !!entry?.isIntersecting;
      if (visible && !reduced) restart();
    });

    let cancelled = false;
    document.fonts.load(`500 12px ${family}`).finally(() => {
      if (cancelled) return;
      layout();
      start = performance.now();
      ro.observe(canvas);
      io.observe(canvas);
      restart();
    });
    window.addEventListener("pointermove", onMove, { passive: true });
    document.documentElement.addEventListener("pointerleave", onLeave);

    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
      ro.disconnect();
      io.disconnect();
      window.removeEventListener("pointermove", onMove);
      document.documentElement.removeEventListener("pointerleave", onLeave);
    };
  }, [dissolve]);

  return <canvas ref={canvasRef} className="ascii-mark" aria-hidden />;
}

/** Signed distance from a point to a triangle, negative inside (after Inigo Quilez). */
function sdTriangle(px: number, py: number, ax: number, ay: number, bx: number, by: number, cx: number, cy: number) {
  const e0x = bx - ax, e0y = by - ay;
  const e1x = cx - bx, e1y = cy - by;
  const e2x = ax - cx, e2y = ay - cy;
  const v0x = px - ax, v0y = py - ay;
  const v1x = px - bx, v1y = py - by;
  const v2x = px - cx, v2y = py - cy;
  const clamp = (v: number) => Math.min(1, Math.max(0, v));
  const t0 = clamp((v0x * e0x + v0y * e0y) / (e0x * e0x + e0y * e0y));
  const t1 = clamp((v1x * e1x + v1y * e1y) / (e1x * e1x + e1y * e1y));
  const t2 = clamp((v2x * e2x + v2y * e2y) / (e2x * e2x + e2y * e2y));
  const q0x = v0x - e0x * t0, q0y = v0y - e0y * t0;
  const q1x = v1x - e1x * t1, q1y = v1y - e1y * t1;
  const q2x = v2x - e2x * t2, q2y = v2y - e2y * t2;
  const s = Math.sign(e0x * e2y - e0y * e2x);
  const d0 = q0x * q0x + q0y * q0y, s0 = s * (v0x * e0y - v0y * e0x);
  const d1 = q1x * q1x + q1y * q1y, s1 = s * (v1x * e1y - v1y * e1x);
  const d2 = q2x * q2x + q2y * q2y, s2 = s * (v2x * e2y - v2y * e2x);
  const d = Math.min(d0, d1, d2);
  const sgn = Math.min(s0, s1, s2);
  return -Math.sqrt(d) * Math.sign(sgn);
}
