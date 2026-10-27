/** The app's title-card palettes (`packages/tokens`), so sample posters look like Nuvora's own. */
export const PALETTES = {
  teal: { bloom: "#2f8f8a", top: "#0f4448", bottom: "#05181b" },
  red: { bloom: "#b2433b", top: "#5b1a1f", bottom: "#18070a" },
  blue: { bloom: "#4b67c9", top: "#1c2b62", bottom: "#070b1d" },
  gold: { bloom: "#d39a2e", top: "#6e4b12", bottom: "#1c1205" },
  violet: { bloom: "#9a4fb0", top: "#45204f", bottom: "#12081a" },
  moss: { bloom: "#6f9a3f", top: "#2f4a24", bottom: "#0b1408" },
  slate: { bloom: "#8a9bb3", top: "#3a4350", bottom: "#0e1116" },
  ember: { bloom: "#df6a2b", top: "#7a3418", bottom: "#200c05" },
  ocean: { bloom: "#3a8fc4", top: "#123d5a", bottom: "#051521" },
  rose: { bloom: "#c2457a", top: "#5a1838", bottom: "#17060f" },
  jade: { bloom: "#3fae7a", top: "#14503a", bottom: "#04170f" },
  indigo: { bloom: "#7a6cd6", top: "#2c2566", bottom: "#0b0920" },
} as const;

export type PaletteName = keyof typeof PALETTES;

export function paletteStyle(name: PaletteName, x = 30, y = 22): React.CSSProperties {
  const p = PALETTES[name];
  return {
    "--tc-bloom": p.bloom,
    "--tc-top": p.top,
    "--tc-bottom": p.bottom,
    "--tc-x": `${x}%`,
    "--tc-y": `${y}%`,
  } as React.CSSProperties;
}

export function TitleCard({
  title,
  year,
  palette,
  x,
  y,
  className = "",
}: {
  title: string;
  year?: string | number;
  palette: PaletteName;
  x?: number;
  y?: number;
  className?: string;
}) {
  return (
    <div className={`title-card ${className}`} style={paletteStyle(palette, x, y)}>
      <div className="title-card-text">
        <span className="title-card-title">{title}</span>
        {year !== undefined && <span className="title-card-year">{year}</span>}
      </div>
    </div>
  );
}
