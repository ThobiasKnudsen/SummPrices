import type { ExtractionStatus } from '../api/types';

const STYLES: Record<ExtractionStatus, string> = {
  pending: 'bg-slate-100 text-slate-600',
  queued: 'bg-slate-100 text-slate-600',
  processing: 'bg-blue-100 text-blue-700',
  done: 'bg-emerald-100 text-emerald-700',
  failed: 'bg-red-100 text-red-700',
  needs_review: 'bg-amber-100 text-amber-800',
};

const LABELS: Record<ExtractionStatus, string> = {
  pending: 'Pending',
  queued: 'Queued',
  processing: 'Processing',
  done: 'Done',
  failed: 'Failed',
  needs_review: 'Needs review',
};

export function StatusBadge({ status }: { status: ExtractionStatus }) {
  const style = STYLES[status] ?? 'bg-slate-100 text-slate-600';
  const label = LABELS[status] ?? status;
  return (
    <span
      className={`inline-flex items-center whitespace-nowrap rounded-full px-2.5 py-0.5 text-xs font-medium ${style}`}
    >
      {label}
    </span>
  );
}
