import { useEffect, useRef, useState } from "react";
import { useNavigate, useRouterState } from "@tanstack/react-router";
import { CloseIcon, SearchIcon } from "./Icon";

/** Typing waits this long before the results page follows. */
const SETTLE_MS = 180;

/**
 * The top bar's search field. Typing opens the results page and keeps its
 * `?q=` in step (replacing history, so Back leaves search in one step).
 * `/` or Ctrl/⌘+K focuses it from anywhere; Escape clears it, then leaves it.
 */
export function SearchBox() {
  const navigate = useNavigate();
  const location = useRouterState({ select: (s) => s.location });
  const onSearch = location.pathname === "/search";
  const urlQuery = onSearch ? (new URLSearchParams(location.searchStr).get("q") ?? "") : "";
  const [value, setValue] = useState(urlQuery);
  const input = useRef<HTMLInputElement>(null);
  const timer = useRef<number>(undefined);

  // Back, Forward or a link changed the query: show it.
  useEffect(() => {
    if (document.activeElement !== input.current) setValue(urlQuery);
  }, [urlQuery]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(e.target.tagName));
      if ((e.key === "/" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) || (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey))) {
        e.preventDefault();
        input.current?.focus();
        input.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const follow = (q: string) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void navigate({ to: "/search", search: q.trim() ? { q } : {}, replace: onSearch });
    }, SETTLE_MS);
  };

  return (
    <form
      className="search-box"
      role="search"
      onSubmit={(e) => {
        e.preventDefault();
        window.clearTimeout(timer.current);
        void navigate({ to: "/search", search: value.trim() ? { q: value } : {}, replace: onSearch });
      }}
    >
      <SearchIcon size={18} className="search-box-icon" />
      <input
        ref={input}
        type="search"
        name="q"
        value={value}
        placeholder="Search"
        aria-label="Search movies, shows and people"
        autoComplete="off"
        spellCheck={false}
        onChange={(e) => {
          setValue(e.target.value);
          follow(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key !== "Escape") return;
          e.preventDefault();
          if (value) {
            setValue("");
            follow("");
          } else {
            e.currentTarget.blur();
          }
        }}
      />
      {value ? (
        <button
          type="button"
          className="search-box-clear"
          aria-label="Clear search"
          onClick={() => {
            setValue("");
            follow("");
            input.current?.focus();
          }}
        >
          <CloseIcon size={16} />
        </button>
      ) : (
        <kbd className="search-box-key" aria-hidden>
          /
        </kbd>
      )}
    </form>
  );
}
