import { ApiError, describeError, type ErrorCode } from "@nuvora/core";

/**
 * Where a failed request's message belongs: under the field the server named
 * (`validation` and `username_taken` carry one), or above the submit button.
 */
export function serverError<F extends string>(
  error: unknown,
  fields: readonly F[],
  overrides?: Partial<Record<ErrorCode, string>>,
): { form: string | null; field: Partial<Record<F, string>> } {
  if (!error) return { form: null, field: {} };
  const message = describeError(error, overrides);
  const named = error instanceof ApiError ? fields.find((f) => f === error.field) : undefined;
  return named ? { form: null, field: { [named]: message } as Partial<Record<F, string>> } : { form: message, field: {} };
}
