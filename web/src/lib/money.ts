/**
 * Backend money/quantity fields serialize as JSON *strings* (rust_decimal),
 * e.g. `"67.80"`. Parse them into numbers before any display or math.
 * Returns `null` for null/undefined/empty/non-numeric input.
 */
export function money(value: string | number | null | undefined): number | null {
  if (value === null || value === undefined) return null;
  if (typeof value === 'number') return Number.isFinite(value) ? value : null;
  const trimmed = value.trim();
  if (trimmed === '') return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}
