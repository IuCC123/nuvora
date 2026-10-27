import { useId, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes } from "react";
import { AlertIcon } from "./Icon";
import "../styles/forms.css";

interface Described {
  label: string;
  /** Shown under the field in `danger` once the form decides to show it; replaces the hint. */
  error?: string | null | undefined;
  hint?: ReactNode;
}

/**
 * A labelled `surface` input. The error and hint are wired to `aria-describedby`, so screen readers read them with the field.
 * `floating` puts the label inside the input, where it rises once there is text; the label stands in for a placeholder.
 */
export function TextField({ label, error, hint, id, floating = false, ...input }: Described & { floating?: boolean } & InputHTMLAttributes<HTMLInputElement>) {
  const { fieldId, describedBy, messages } = useDescription(id, error, hint);
  return (
    <div className={floating ? "field field-floating" : "field"}>
      <label htmlFor={fieldId}>{label}</label>
      <input
        id={fieldId}
        className="input"
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        {...input}
        // `:placeholder-shown` is how CSS knows the field is empty.
        placeholder={floating ? " " : input.placeholder}
      />
      {messages}
    </div>
  );
}

export function SelectField({ label, error, hint, id, children, ...select }: Described & SelectHTMLAttributes<HTMLSelectElement>) {
  const { fieldId, describedBy, messages } = useDescription(id, error, hint);
  return (
    <div className="field">
      <label htmlFor={fieldId}>{label}</label>
      <select id={fieldId} className="input select" aria-invalid={error ? true : undefined} aria-describedby={describedBy} {...select}>
        {children}
      </select>
      {messages}
    </div>
  );
}

function useDescription(id: string | undefined, error: string | null | undefined, hint: ReactNode) {
  const generated = useId();
  const fieldId = id ?? generated;
  const messageId = `${fieldId}-message`;
  const messages = error ? (
    <p className="field-error" id={messageId}>
      <AlertIcon size={14} />
      {error}
    </p>
  ) : hint ? (
    <p className="field-hint" id={messageId}>
      {hint}
    </p>
  ) : null;
  return { fieldId, describedBy: messages ? messageId : undefined, messages };
}

/** Why the server said no, above the submit button. Announced when it appears. */
export function FormError({ message }: { message: string | null }) {
  if (!message) return null;
  return (
    <div className="form-error" role="alert">
      <AlertIcon size={16} />
      <p>{message}</p>
    </div>
  );
}
