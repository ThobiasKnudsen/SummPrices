import { money } from './money';

const DASH = '—';

/** Format a money value (string or number) as a currency amount. */
export function formatMoney(
  value: string | number | null | undefined,
  currency = 'NOK',
): string {
  const amount = money(value);
  if (amount === null) return DASH;
  try {
    return new Intl.NumberFormat('nb-NO', {
      style: 'currency',
      currency: currency || 'NOK',
    }).format(amount);
  } catch {
    // Unknown currency code — fall back to a plain number + code.
    return `${new Intl.NumberFormat('nb-NO', { minimumFractionDigits: 2 }).format(amount)} ${currency}`;
  }
}

/** Format an ISO timestamp as a short date (e.g. "10 Jul 2026"). */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return DASH;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return DASH;
  return new Intl.DateTimeFormat('nb-NO', {
    day: '2-digit',
    month: 'short',
    year: 'numeric',
  }).format(date);
}

/** Format an ISO timestamp as date + time. */
export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return DASH;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return DASH;
  return new Intl.DateTimeFormat('nb-NO', {
    day: '2-digit',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

/** Format a quantity (string) optionally suffixed with a unit. */
export function formatQuantity(
  quantity: string | number | null | undefined,
  unit: string | null | undefined,
): string {
  const value = money(quantity);
  if (value === null) return unit ?? DASH;
  const num = new Intl.NumberFormat('nb-NO', { maximumFractionDigits: 3 }).format(value);
  return unit ? `${num} ${unit}` : num;
}
