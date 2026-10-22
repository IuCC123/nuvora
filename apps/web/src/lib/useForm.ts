import { useState, type ChangeEvent, type FormEvent } from "react";

type Values<K extends string> = Record<K, string>;
type Errors<K extends string> = Partial<Record<K, string | null>>;

/**
 * Field state and inline validation for a form of text inputs. A field's
 * message appears once it was left with something typed in, or after a submit
 * attempt; from then on it tracks every keystroke. Tabbing past an empty field
 * says nothing. `check` holds the rules, so they stay pure and testable.
 */
export function useForm<K extends string>(initial: Values<K>, check: (values: Values<K>) => Errors<K>) {
  const [values, setValues] = useState(initial);
  const [visited, setVisited] = useState<Partial<Record<K, boolean>>>({});
  const [submitted, setSubmitted] = useState(false);
  const errors = check(values);
  const keys = Object.keys(initial) as K[];

  const field = (name: K) => ({
    name,
    value: values[name],
    error: submitted || visited[name] ? (errors[name] ?? null) : null,
    onChange: (e: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => setValues((v) => ({ ...v, [name]: e.target.value })),
    onBlur: () => values[name] && setVisited((t) => ({ ...t, [name]: true })),
  });

  /** Runs `onValid` with the values, or shows every message and focuses the first invalid field. */
  const submit = (onValid: (values: Values<K>) => void) => (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setSubmitted(true);
    const invalid = keys.find((k) => errors[k]);
    if (invalid === undefined) return onValid(values);
    const el = e.currentTarget.elements.namedItem(invalid);
    if (el instanceof HTMLElement) el.focus();
  };

  const reset = (next: Values<K> = initial) => {
    setValues(next);
    setVisited({});
    setSubmitted(false);
  };

  return { values, field, submit, reset, set: (name: K, value: string) => setValues((v) => ({ ...v, [name]: value })) };
}
