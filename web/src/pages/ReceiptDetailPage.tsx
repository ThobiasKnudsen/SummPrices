import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { deleteReceipt, getReceipt } from '../api/receipts';
import type { Transaction } from '../api/types';
import { formatDateTime, formatMoney, formatQuantity } from '../lib/format';
import { StatusBadge } from '../components/StatusBadge';
import { Spinner } from '../components/Spinner';
import { EmptyState } from '../components/EmptyState';
import { ApiError } from '../lib/apiClient';

function itemTypeLabel(type: Transaction['item_type']): string {
  return type.charAt(0).toUpperCase() + type.slice(1);
}

export function ReceiptDetailPage() {
  const { id = '' } = useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { data: receipt, isLoading, isError, error } = useQuery({
    queryKey: ['receipt', id],
    queryFn: () => getReceipt(id),
    enabled: id !== '',
  });

  const remove = useMutation({
    mutationFn: () => deleteReceipt(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['receipts'] });
      navigate('/', { replace: true });
    },
  });

  function onDelete() {
    if (window.confirm('Delete this receipt? This cannot be undone.')) {
      remove.mutate();
    }
  }

  if (isLoading) {
    return (
      <div className="py-16">
        <Spinner label="Loading receipt…" className="justify-center" />
      </div>
    );
  }

  if (isError || !receipt) {
    return (
      <EmptyState
        title="Couldn’t load receipt"
        description={error instanceof ApiError ? error.message : 'This receipt may have been removed.'}
        action={
          <Link
            to="/"
            className="rounded-md bg-slate-900 px-4 py-2 text-sm font-semibold text-white transition hover:bg-slate-800"
          >
            Back to receipts
          </Link>
        }
      />
    );
  }

  const currency = receipt.currency;

  return (
    <div>
      <div className="mb-6 flex items-center justify-between gap-4">
        <Link to="/" className="text-sm text-slate-500 hover:text-slate-800">
          ← Back to receipts
        </Link>
        <button
          type="button"
          onClick={onDelete}
          disabled={remove.isPending}
          className="rounded-md border border-red-200 px-3 py-1.5 text-sm font-medium text-red-600 transition hover:bg-red-50 disabled:opacity-60"
        >
          {remove.isPending ? 'Deleting…' : 'Delete'}
        </button>
      </div>

      {remove.isError ? (
        <p className="mb-4 rounded-md bg-red-50 px-3 py-2 text-sm text-red-700">
          {remove.error instanceof ApiError ? remove.error.message : 'Failed to delete receipt.'}
        </p>
      ) : null}

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[320px_1fr]">
        <div className="space-y-4">
          {receipt.image_url ? (
            <img
              src={receipt.image_url}
              alt="Receipt scan"
              className="w-full rounded-xl border border-slate-200 bg-white object-contain"
            />
          ) : (
            <div className="flex h-48 items-center justify-center rounded-xl border border-dashed border-slate-300 bg-white text-sm text-slate-400">
              No image available
            </div>
          )}
        </div>

        <div className="space-y-6">
          <div className="rounded-xl border border-slate-200 bg-white p-5">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h1 className="text-lg font-bold text-slate-900">
                  {receipt.store_name_raw ?? 'Unknown store'}
                </h1>
                <p className="text-sm text-slate-500">{formatDateTime(receipt.purchase_at)}</p>
              </div>
              <StatusBadge status={receipt.extraction_status} />
            </div>

            <dl className="mt-4 grid grid-cols-2 gap-x-4 gap-y-3 text-sm sm:grid-cols-3">
              <div>
                <dt className="text-xs uppercase tracking-wide text-slate-400">Subtotal</dt>
                <dd className="font-medium text-slate-800">{formatMoney(receipt.subtotal, currency)}</dd>
              </div>
              <div>
                <dt className="text-xs uppercase tracking-wide text-slate-400">MVA</dt>
                <dd className="font-medium text-slate-800">{formatMoney(receipt.mva_total, currency)}</dd>
              </div>
              <div>
                <dt className="text-xs uppercase tracking-wide text-slate-400">Total</dt>
                <dd className="font-semibold text-slate-900">{formatMoney(receipt.total, currency)}</dd>
              </div>
            </dl>

            {receipt.needs_review ? (
              <p className="mt-4 rounded-md bg-amber-50 px-3 py-2 text-sm text-amber-800">
                This receipt needs review — some values may be inaccurate.
              </p>
            ) : null}
          </div>

          <div className="rounded-xl border border-slate-200 bg-white">
            <div className="border-b border-slate-100 px-5 py-3">
              <h2 className="text-sm font-semibold text-slate-700">
                Line items ({receipt.transactions.length})
              </h2>
            </div>
            {receipt.transactions.length === 0 ? (
              <p className="px-5 py-8 text-center text-sm text-slate-400">No line items.</p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full min-w-[520px] text-sm">
                  <thead>
                    <tr className="border-b border-slate-100 text-left text-xs uppercase tracking-wide text-slate-400">
                      <th className="px-5 py-2 font-medium">Description</th>
                      <th className="px-3 py-2 font-medium">Qty</th>
                      <th className="px-3 py-2 text-right font-medium">Unit price</th>
                      <th className="px-3 py-2 text-right font-medium">Line total</th>
                      <th className="px-5 py-2 font-medium">Type</th>
                    </tr>
                  </thead>
                  <tbody>
                    {receipt.transactions.map((tx) => (
                      <tr key={tx.id} className="border-b border-slate-50 last:border-0">
                        <td className="px-5 py-2.5 text-slate-800">
                          {tx.description_clean ?? tx.description_raw}
                        </td>
                        <td className="px-3 py-2.5 text-slate-600">
                          {formatQuantity(tx.quantity, tx.unit)}
                        </td>
                        <td className="px-3 py-2.5 text-right text-slate-600">
                          {formatMoney(tx.unit_price, currency)}
                        </td>
                        <td className="px-3 py-2.5 text-right font-medium text-slate-800">
                          {formatMoney(tx.line_total, currency)}
                        </td>
                        <td className="px-5 py-2.5 text-slate-500">{itemTypeLabel(tx.item_type)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
