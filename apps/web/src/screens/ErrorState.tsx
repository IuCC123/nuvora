import { Link } from "@tanstack/react-router";
import { AlertIcon } from "../components/Icon";

/** A screen that couldn't load: what happened, and the one action that helps. */
export function ErrorState({ title, detail, retry, home }: { title: string; detail: string; retry?: () => void; home?: boolean }) {
  return (
    <div className="error-state" role="alert">
      <AlertIcon size={24} />
      <h1>{title}</h1>
      <p>{detail}</p>
      {retry && (
        <button type="button" className="btn btn-secondary" onClick={retry}>
          Try again
        </button>
      )}
      {home && (
        <Link to="/" className="btn btn-secondary">
          Go to Home
        </Link>
      )}
    </div>
  );
}
