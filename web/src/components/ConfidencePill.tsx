/**
 * Shows the extractor's self-reported confidence (0..1) as a colored percentage pill.
 * Renders nothing when confidence is unavailable (older receipts / mock extractor).
 */
export function ConfidencePill({
  value,
  className = '',
}: {
  value: number | null | undefined;
  className?: string;
}) {
  if (value == null) return null;
  const pct = Math.round(value * 100);
  const tone =
    pct >= 80
      ? 'bg-emerald-100 text-emerald-700'
      : pct >= 50
        ? 'bg-amber-100 text-amber-800'
        : 'bg-red-100 text-red-700';
  return (
    <span
      title="Extractor's self-reported confidence"
      className={`inline-flex items-center whitespace-nowrap rounded-full px-2.5 py-0.5 text-xs font-medium ${tone} ${className}`}
    >
      {pct}% confidence
    </span>
  );
}
