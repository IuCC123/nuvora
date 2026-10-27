import { useId, type CSSProperties, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes } from "react";
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

/** An on/off setting: the label and hint on the left, the switch on the right. The whole row toggles it. */
export function SwitchField({
  label,
  hint,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  hint?: ReactNode;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
}) {
  const { fieldId, describedBy, messages } = useDescription(undefined, null, hint);
  return (
    <div className="field switch-field">
      <label htmlFor={fieldId} className="switch-row">
        <span className="switch-label">{label}</span>
        <input
          id={fieldId}
          type="checkbox"
          role="switch"
          className="switch"
          checked={checked}
          disabled={disabled}
          aria-describedby={describedBy}
          onChange={(e) => onChange(e.currentTarget.checked)}
        />
      </label>
      {messages}
    </div>
  );
}

/** A number picked on a slider, with its value (`format`ted) beside the label. */
export function RangeField({
  label,
  hint,
  value,
  min,
  max,
  step = 1,
  format = String,
  error,
  onChange,
}: Described & { value: number; min: number; max: number; step?: number; format?: (value: number) => string; onChange: (value: number) => void }) {
  const { fieldId, describedBy, messages } = useDescription(undefined, error, hint);
  return (
    <div className="field range-field">
      <div className="range-head">
        <label htmlFor={fieldId}>{label}</label>
        <output htmlFor={fieldId}>{format(value)}</output>
      </div>
      <input
        id={fieldId}
        type="range"
        className="range"
        min={min}
        max={max}
        step={step}
        value={value}
        aria-valuetext={format(value)}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        style={{ "--fill": `${((value - min) / (max - min)) * 100}%` } as CSSProperties}
        onChange={(e) => onChange(Number(e.currentTarget.value))}
      />
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
