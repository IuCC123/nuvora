import { useState, type ChangeEvent, type FormEvent } from "react";

type FormValues = Record<string, string | readonly string[]>;
type TextKeys<V> = { [K in keyof V]: V[K] extends string ? K : never }[keyof V] & string;

/**
 * Field state and inline validation. A field's message appears once it was
 * left with something typed in, or changed by `set`, or after a submit
 * attempt; from then on it follows every edit. Tabbing past an empty field
 * says nothing. `check` holds the rules, so they stay pure.
 */
export function useForm<V extends FormValues>(
  initial: V,
  check: (values: V) => Partial<Record<keyof V, string | null>>,
  /** Called on every edit; pass the mutation's `reset` so a server error clears once the person changes something. */
  onEdit?: () => void,
) {
  const [values, setValues] = useState(initial);
  const [visited, setVisited] = useState<Partial<Record<keyof V, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const errors = check(values);

  const error = (name: keyof V) => (submitted || visited[name] ? (errors[name] ?? null) : null);

  const set = <K extends keyof V>(name: K, value: V[K]) => {
    onEdit?.();
    setValues((v) => ({ ...v, [name]: value }));
    setVisited((t) => ({ ...t, [name]: true }));
  };

  /** Props for a `TextField` or `SelectField`. */
  const field = (name: TextKeys<V>) => ({
    name,
    value: values[name] as string,
    error: error(name),
    onChange: (e: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
      onEdit?.();
      setValues((v) => ({ ...v, [name]: e.target.value }));
    },
    onBlur: () => values[name] && setVisited((t) => ({ ...t, [name]: true })),
  });

  /**
   * Runs `onValid`, or shows every message and focuses the first invalid
   * control (the element whose `name` is the field's key).
   */
  const submit = (onValid: (values: V) => void) => (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setSubmitted(true);
    const invalid = (Object.keys(initial) as (keyof V & string)[]).find((k) => errors[k]);
    if (invalid === undefined) return onValid(values);
    const el = e.currentTarget.elements.namedItem(invalid);
    if (el instanceof HTMLElement) el.focus();
  };

  const reset = (next: V = initial) => {
    setValues(next);
    setVisited({});
    setSubmitted(false);
  };

  const dirty = (Object.keys(initial) as (keyof V)[]).some((k) => String(values[k]) !== String(initial[k]));

  return { values, field, error, set, submit, reset, dirty };
}
