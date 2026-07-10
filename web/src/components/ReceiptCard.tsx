import { Link } from 'react-router-dom';
import type { ReceiptSummary } from '../api/types';
import { formatDate, formatMoney } from '../lib/format';
import { StatusBadge } from './StatusBadge';

export function ReceiptCard({ receipt }: { receipt: ReceiptSummary }) {
  return (
    <Link
      to={`/receipts/${receipt.id}`}
      className="block rounded-xl border border-slate-200 bg-white p-4 shadow-sm transition hover:border-slate-300 hover:shadow"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-sm font-semibold text-slate-800">
            {receipt.store_name_raw ?? 'Unknown store'}
          </p>
          <p className="mt-0.5 text-xs text-slate-500">
            {formatDate(receipt.purchase_at ?? receipt.created_at)}
          </p>
        </div>
        <StatusBadge status={receipt.extraction_status} />
      </div>
      <div className="mt-3 text-lg font-semibold text-slate-900">
        {formatMoney(receipt.total, receipt.currency)}
      </div>
    </Link>
  );
}
