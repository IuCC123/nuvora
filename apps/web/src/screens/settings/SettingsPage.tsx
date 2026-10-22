import type { ReactNode } from "react";
import { describeError } from "@nuvora/core";
import { useDocumentTitle } from "../../lib/useDocumentTitle";
import { AlertIcon, CheckIcon } from "../../components/Icon";
import "../../styles/forms.css";
import "../../styles/settings.css";

export function SettingsPage({ title, lede, action, children }: { title: string; lede: string; action?: ReactNode; children: ReactNode }) {
  useDocumentTitle(`${title} · Settings`);
  return (
    <div className="settings-page">
      <header className="settings-head">
        <div>
          <h1>{title}</h1>
          <p>{lede}</p>
        </div>
        {action}
      </header>
      {children}
    </div>
  );
}

/** A section's data couldn't load: why, and the retry. */
export function LoadError({ error, retry }: { error: unknown; retry: () => void }) {
  return (
    <div className="settings-load-error" role="alert">
      <AlertIcon />
      <p>{describeError(error)}</p>
      <button type="button" className="btn btn-secondary btn-small" onClick={retry}>
        Try again
      </button>
    </div>
  );
}

/** Rows shaped like the content they stand in for, while it loads. */
export function SkeletonRows({ count, label }: { count: number; label: string }) {
  return (
    <ul role="list" className="settings-list" aria-busy="true" aria-label={label}>
      {Array.from({ length: count }, (_, i) => (
        <li key={i} className="settings-row skeleton-row">
          <span />
          <span />
        </li>
      ))}
    </ul>
  );
}

/** "Saved", next to the button that saved, until the next edit. */
export function Saved({ show, children = "Saved" }: { show: boolean; children?: ReactNode }) {
  return (
    <span className="saved" role="status">
      {show && (
        <>
          <CheckIcon />
          {children}
        </>
      )}
    </span>
  );
}
