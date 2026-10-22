import type { CSSProperties, ReactNode } from "react";
import { titleCardVars } from "./TitleCard";
import { MarkIcon } from "./Icon";
import "../styles/auth.css";

/**
 * Sign in and the welcome steps: a centered card on the canvas over a slow
 * drift of title cards. The cards carry no titles, because nothing of the
 * library may show before someone signs in.
 */
export function AuthScreen({ children }: { children: ReactNode }) {
  return (
    <div className="auth">
      <PosterDrift />
      <main className="auth-column">
        <p className="auth-brand" aria-label="Nuvora">
          <MarkIcon />
          <span aria-hidden>nuvora</span>
        </p>
        {children}
      </main>
    </div>
  );
}

const COLUMNS = 10;
const CARDS_PER_COLUMN = 6;

/** Each column holds its cards twice and moves by half its height, so the loop has no seam. */
function PosterDrift() {
  return (
    <div className="drift" aria-hidden>
      <div className="drift-wall">
        {Array.from({ length: COLUMNS }, (_, c) => {
          const cards = Array.from({ length: CARDS_PER_COLUMN }, (_, i) => `drift ${c} ${i}`);
          const style = { "--drift-duration": `${150 + ((c * 37) % 70)}s`, "--drift-offset": `${-((c * 53) % 90)}s` } as CSSProperties;
          return (
            <div key={c} className={c % 2 ? "drift-column drift-down" : "drift-column"} style={style}>
              {[...cards, ...cards].map((seed, i) => (
                <span key={i} className="title-card" style={titleCardVars(seed)} />
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
