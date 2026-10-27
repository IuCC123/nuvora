"use client";

import { useRef } from "react";
import { motion, useScroll, useTransform } from "motion/react";
import { EASE } from "./Reveal";
import { Mark } from "./Nav";
import { TitleCard, type PaletteName } from "./TitleCard";
import { media } from "./scenes";
import { CheckIcon, FilmIcon, HouseIcon, PlayIcon, SearchIcon, SlidersIcon, TvIcon, UsersIcon } from "./icons";

type Item = { title: string; year?: number; art?: string; palette: PaletteName; progress?: number; watched?: boolean; sub?: string };

const SINTEL: Item = { title: "Sintel", year: 2010, art: "poster-sintel", palette: "ember" };
const TOS: Item = { title: "Tears of Steel", year: 2012, art: "poster-tears-of-steel", palette: "ocean" };
const BBB: Item = { title: "Big Buck Bunny", year: 2008, art: "poster-big-buck-bunny", palette: "moss" };
const COSMOS: Item = { title: "Cosmos Laundromat", year: 2015, art: "poster-cosmos-laundromat", palette: "violet" };
const DREAM: Item = { title: "Elephants Dream", year: 2006, palette: "slate" };
const SPRING: Item = { title: "Spring", year: 2019, palette: "jade" };
const PIONEER: Item = { title: "Pioneer One", palette: "teal", sub: "Season 1" };

const ROWS: { title: string; count?: number; items: Item[] }[] = [
  {
    title: "Continue Watching",
    items: [
      { ...SINTEL, progress: 0.62, sub: "5 min left" },
      { ...PIONEER, progress: 0.35, sub: "S1 · E4" },
      { ...TOS, progress: 0.18, sub: "10 min left" },
      { ...COSMOS, progress: 0.44, sub: "6 min left" },
      { ...BBB, progress: 0.8, sub: "2 min left" },
      { ...SPRING, progress: 0.1, sub: "7 min left" },
    ],
  },
  {
    title: "Recently Added",
    count: 24,
    items: [COSMOS, SPRING, BBB, { ...DREAM, watched: true }, SINTEL, TOS, PIONEER],
  },
  { title: "Movies", count: 214, items: [BBB, COSMOS, DREAM, SINTEL, SPRING, TOS] },
];

function Poster({ item }: { item: Item }) {
  return (
    <div className="sa-card">
      <div className="sa-poster">
        <TitleCard title={item.title} year={item.year} palette={item.palette} />
        {item.art && <img src={media(item.art)} alt="" draggable={false} />}
        {item.progress !== undefined && (
          <span className="ui-progress">
            <span style={{ width: `${item.progress * 100}%` }} />
          </span>
        )}
        {item.watched && (
          <span className="ui-watched">
            <CheckIcon />
          </span>
        )}
        <span className="sa-hover" aria-hidden>
          <span>
            <PlayIcon />
          </span>
        </span>
      </div>
      <span className="sa-cap-title">{item.title}</span>
      <span className="sa-cap-sub">{item.sub ?? item.year}</span>
    </div>
  );
}

/** Nuvora's Home screen at full size, built from the app's own layout and samples from Blender's open movies. */
function HomeScreen() {
  return (
    <div className="sa">
      <div className="sa-top">
        <span className="sa-brand">
          <Mark size={14} />
          nuvora
        </span>
        <span className="sa-search">
          <SearchIcon />
          <span>Search movies and shows</span>
        </span>
        <span className="sa-btn">
          <SlidersIcon />
          <span>Customize</span>
        </span>
        <span className="sa-icon">
          <UsersIcon />
        </span>
        <span className="avatar" data-who="Ana">
          A
        </span>
      </div>
      <div className="sa-body">
        <nav className="sa-rail" aria-hidden>
          <span className="sa-rail-item" data-active="true">
            <HouseIcon />
            <span>Home</span>
          </span>
          <span className="sa-rail-group">Libraries</span>
          <span className="sa-rail-item">
            <FilmIcon />
            <span>Movies</span>
            <em>214</em>
          </span>
          <span className="sa-rail-item">
            <TvIcon />
            <span>Shows</span>
            <em>38</em>
          </span>
        </nav>
        <div className="sa-home">
          <div className="sa-wash" />
          {ROWS.map((row) => (
            <div key={row.title} className="sa-row">
              <div className="sa-row-head">
                <span className="sa-row-title">{row.title}</span>
                {row.count !== undefined && <span className="sa-row-count">{row.count}</span>}
                <span className="sa-arrows" aria-hidden>
                  <i>‹</i>
                  <i>›</i>
                </span>
              </div>
              <div className="sa-strip">
                {row.items.map((item, i) => (
                  <Poster key={item.title + i} item={item} />
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

/** Rises out of the hero and straightens as it scrolls in, so the product is on the first screen. */
export function ProductShot() {
  const ref = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start end", "start 15%"] });
  const rotateX = useTransform(scrollYProgress, [0, 1], [26, 0]);
  const scale = useTransform(scrollYProgress, [0, 1], [0.9, 1]);

  return (
    <section className="shot-section" aria-label="Nuvora's Home screen">
      <div className="container shot-stage" ref={ref}>
        <motion.div
          className="shot"
          style={{ rotateX, scale }}
          initial={{ opacity: 0, y: 80 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 1.4, delay: 1.2, ease: EASE }}
        >
          <div className="shot-window">
            <div className="browser-bar shot-bar">
              <i />
              <i />
              <i />
              <span className="browser-url">192.168.1.20:8640</span>
            </div>
            <HomeScreen />
          </div>
        </motion.div>
      </div>
    </section>
  );
}
