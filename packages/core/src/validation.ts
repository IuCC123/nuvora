/**
 * Client-side checks for the account and admin forms. They mirror the
 * server's rules so a form can say what's wrong before the round trip; the
 * server stays the authority and answers `validation` when they disagree.
 * Each returns the sentence to show under the field, or `null` when valid.
 */

export const PASSWORD_MIN_LENGTH = 8;

export function checkRequired(value: string, what: string): string | null {
  return value.trim() ? null : `Enter ${what}.`;
}

/** The server's rule: 1 to 32 letters, digits, `.`, `_` or `-`. */
export function checkUsername(value: string): string | null {
  if (!value) return "Enter a username.";
  if (/\s/u.test(value)) return "Usernames can't contain spaces.";
  if ([...value].length > 32) return "Use 32 characters or fewer.";
  if (!/^[\p{L}\p{N}._-]+$/u.test(value)) return "Use only letters, digits, dots, dashes and underscores.";
  return null;
}

export function checkPassword(value: string): string | null {
  if (!value) return "Enter a password.";
  if ([...value].length < PASSWORD_MIN_LENGTH) return `Use at least ${PASSWORD_MIN_LENGTH} characters.`;
  return null;
}

export function checkPasswordsMatch(password: string, confirm: string): string | null {
  if (!confirm) return "Enter the password again.";
  return password === confirm ? null : "The passwords don't match.";
}
