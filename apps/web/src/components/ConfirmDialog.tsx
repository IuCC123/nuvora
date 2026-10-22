import { useEffect, useRef, type ReactNode } from "react";
import { FormError } from "./Field";
import "../styles/dialog.css";

/**
 * Asks before something that can't be undone. A modal `<dialog>`: focus moves
 * into it and stays there, Escape cancels, and focus returns to the button that
 * opened it. Cancel takes focus first, so Enter never destroys by accident.
 */
export function ConfirmDialog({
  title,
  children,
  confirmLabel,
  pending,
  error,
  onConfirm,
  onCancel,
}: {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  pending: boolean;
  error: string | null;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const el = dialog.current;
    const opener = document.activeElement as HTMLElement | null;
    el?.showModal();
    return () => {
      el?.close();
      opener?.focus();
    };
  }, []);

  return (
    <dialog
      ref={dialog}
      className="dialog"
      aria-labelledby="dialog-title"
      onCancel={(e) => {
        e.preventDefault();
        if (!pending) onCancel();
      }}
    >
      <h2 id="dialog-title">{title}</h2>
      <div className="dialog-body">{children}</div>
      <FormError message={error} />
      <div className="dialog-actions">
        <button type="button" className="btn btn-secondary" onClick={onCancel} disabled={pending} autoFocus>
          Cancel
        </button>
        <button type="button" className="btn btn-danger" onClick={onConfirm} disabled={pending} aria-busy={pending}>
          {confirmLabel}
        </button>
      </div>
    </dialog>
  );
}
