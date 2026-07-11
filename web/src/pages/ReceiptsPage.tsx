import { useState } from 'react';
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { listReceipts, reprocessAllReceipts } from '../api/receipts';
import type { ReceiptFilters } from '../api/receipts';
import type { ExtractionStatus } from '../api/types';
import { ReceiptCard } from '../components/ReceiptCard';
import { ModelPicker, useModelChoice } from '../components/ModelPicker';
import { Spinner } from '../components/Spinner';
import { EmptyState } from '../components/EmptyState';
import { ApiError } from '../lib/apiClient';

const STATUS_OPTIONS: Array<{ value: '' | ExtractionStatus; label: string }> = [
  { value: '', label: 'All statuses' },
  { value: 'done', label: 'Done' },
  { value: 'needs_review', label: 'Needs review' },
  { value: 'processing', label: 'Processing' },
  { value: 'queued', label: 'Queued' },
  { value: 'pending', label: 'Pending' },
  { value: 'failed', label: 'Failed' },
];

const inputClass =
  'w-full rounded-md border border-slate-300 px-3 py-2 text-sm outline-none focus:border-slate-500 focus:ring-2 focus:ring-slate-200';

export function ReceiptsPage() {
  const [store, setStore] = useState('');
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [status, setStatus] = useState('');

  const filters: ReceiptFilters = {
    store: store.trim() || undefined,
    from: from || undefined,
    to: to || undefined,
    status: status || undefined,
    per_page: 60,
  };

  const queryClient = useQueryClient();
  const { model, setModel, options, current } = useModelChoice();

  const { data, isLoading, isError, error, isFetching } = useQuery({
    queryKey: ['receipts', filters],
    queryFn: () => listReceipts(filters),
    placeholderData: keepPreviousData,
    // While any receipt is (re)scanning, poll so the list updates as each finishes.
    refetchInterval: (query) => {
      const list = query.state.data?.receipts ?? [];
      return list.some((r) => ['pending', 'queued', 'processing'].includes(r.extraction_status))
        ? 3000
        : false;
    },
  });

  const rescanAll = useMutation({
    mutationFn: () => reprocessAllReceipts(model),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['receipts'] });
      queryClient.invalidateQueries({ queryKey: ['transactions'] });
      queryClient.invalidateQueries({ queryKey: ['analytics'] });
    },
  });

  const receipts = data?.receipts ?? [];
  const hasFilters = Boolean(store || from || to || status);

  return (
    <div>
      <div className="mb-6 flex items-center justify-between gap-4">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-slate-900">Receipts</h1>
          <p className="text-sm text-slate-500">
            {data ? `${data.total_count} total` : 'Your scanned receipts'}
          </p>
        </div>
        <Link
          to="/upload"
          className="rounded-md bg-slate-900 px-4 py-2 text-sm font-semibold text-white transition hover:bg-slate-800"
        >
          Upload receipt
        </Link>
      </div>

      <div className="mb-4 flex flex-wrap items-center gap-3 rounded-xl border border-dashed border-slate-300 bg-slate-50 px-4 py-3">
        <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">Debug</span>
        <ModelPicker
          value={model}
          options={options}
          current={current}
          onChange={setModel}
          disabled={rescanAll.isPending}
        />
        <button
          type="button"
          onClick={() => rescanAll.mutate()}
          disabled={rescanAll.isPending}
          className="rounded-md border border-slate-300 bg-white px-3 py-1.5 text-sm font-medium text-slate-700 transition hover:bg-slate-100 disabled:opacity-60"
          title="Re-run extraction on every receipt with the selected model"
        >
          {rescanAll.isPending ? 'Queuing…' : 'Rescan all'}
        </button>
        {rescanAll.isSuccess ? (
          <span className="text-xs text-emerald-600">
            Queued {rescanAll.data.queued} receipt(s) — they’ll refresh as each finishes.
          </span>
        ) : null}
        {rescanAll.isError ? (
          <span className="text-xs text-red-600">
            {rescanAll.error instanceof ApiError ? rescanAll.error.message : 'Failed to queue rescan.'}
          </span>
        ) : null}
      </div>

      <div className="mb-6 grid grid-cols-1 gap-3 rounded-xl border border-slate-200 bg-white p-4 sm:grid-cols-2 lg:grid-cols-4">
        <div>
          <label className="mb-1 block text-xs font-medium text-slate-500">Store</label>
          <input
            type="text"
            placeholder="Search store…"
            value={store}
            onChange={(e) => setStore(e.target.value)}
            className={inputClass}
          />
        </div>
        <div>
          <label className="mb-1 block text-xs font-medium text-slate-500">From</label>
          <input type="date" value={from} onChange={(e) => setFrom(e.target.value)} className={inputClass} />
        </div>
        <div>
          <label className="mb-1 block text-xs font-medium text-slate-500">To</label>
          <input type="date" value={to} onChange={(e) => setTo(e.target.value)} className={inputClass} />
        </div>
        <div>
          <label className="mb-1 block text-xs font-medium text-slate-500">Status</label>
          <select value={status} onChange={(e) => setStatus(e.target.value)} className={inputClass}>
            {STATUS_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      {isLoading ? (
        <div className="py-16">
          <Spinner label="Loading receipts…" className="justify-center" />
        </div>
      ) : isError ? (
        <EmptyState
          title="Couldn’t load receipts"
          description={error instanceof ApiError ? error.message : 'Something went wrong.'}
        />
      ) : receipts.length === 0 ? (
        <EmptyState
          title={hasFilters ? 'No receipts match your filters' : 'No receipts yet'}
          description={
            hasFilters
              ? 'Try clearing or adjusting the filters above.'
              : 'Upload your first receipt to get started.'
          }
          action={
            hasFilters ? undefined : (
              <Link
                to="/upload"
                className="rounded-md bg-slate-900 px-4 py-2 text-sm font-semibold text-white transition hover:bg-slate-800"
              >
                Upload receipt
              </Link>
            )
          }
        />
      ) : (
        <div
          className={`grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 ${
            isFetching ? 'opacity-70' : ''
          }`}
        >
          {receipts.map((receipt) => (
            <ReceiptCard key={receipt.id} receipt={receipt} />
          ))}
        </div>
      )}
    </div>
  );
}
