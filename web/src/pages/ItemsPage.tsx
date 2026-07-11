import { useEffect, useState } from 'react';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { listTransactions } from '../api/transactions';
import { formatDate, formatMoney } from '../lib/format';
import { Spinner } from '../components/Spinner';
import { EmptyState } from '../components/EmptyState';
import { ApiError } from '../lib/apiClient';

export function ItemsPage() {
  const [query, setQuery] = useState('');
  const [debounced, setDebounced] = useState('');

  useEffect(() => {
    const handle = window.setTimeout(() => setDebounced(query.trim()), 350);
    return () => window.clearTimeout(handle);
  }, [query]);

  const { data, isLoading, isError, error, isFetching } = useQuery({
    queryKey: ['transactions', debounced],
    queryFn: () => listTransactions({ q: debounced || undefined, per_page: 100 }),
    placeholderData: keepPreviousData,
  });

  const transactions = data?.transactions ?? [];

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-xl font-bold tracking-tight text-slate-900">Items</h1>
        <p className="text-sm text-slate-500">Search across all your receipt line items.</p>
      </div>

      <div className="mb-6">
        <input
          type="search"
          placeholder="Search items…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="w-full max-w-md rounded-md border border-slate-300 px-3 py-2 text-sm outline-none focus:border-slate-500 focus:ring-2 focus:ring-slate-200"
        />
      </div>

      {isLoading ? (
        <div className="py-16">
          <Spinner label="Loading items…" className="justify-center" />
        </div>
      ) : isError ? (
        <EmptyState
          title="Couldn’t load items"
          description={error instanceof ApiError ? error.message : 'Something went wrong.'}
        />
      ) : transactions.length === 0 ? (
        <EmptyState
          title={debounced ? 'No items match your search' : 'No items yet'}
          description={
            debounced ? 'Try a different search term.' : 'Upload a receipt to see its items here.'
          }
        />
      ) : (
        <div className={`overflow-x-auto rounded-xl border border-slate-200 bg-white ${isFetching ? 'opacity-70' : ''}`}>
          <table className="w-full min-w-[560px] text-sm">
            <thead>
              <tr className="border-b border-slate-100 text-left text-xs uppercase tracking-wide text-slate-400">
                <th className="px-5 py-3 font-medium">Description</th>
                <th className="px-3 py-3 font-medium">Store</th>
                <th className="px-3 py-3 font-medium">Date</th>
                <th className="px-5 py-3 text-right font-medium">Line total</th>
              </tr>
            </thead>
            <tbody>
              {transactions.map((tx) => (
                <tr key={tx.id} className="border-b border-slate-50 last:border-0">
                  <td className="px-5 py-2.5 text-slate-800">
                    {tx.description_clean ?? tx.description_raw}
                  </td>
                  <td className="px-3 py-2.5 text-slate-600">{tx.store_name_raw ?? '—'}</td>
                  <td className="px-3 py-2.5 text-slate-600">{formatDate(tx.purchase_at)}</td>
                  <td className="px-5 py-2.5 text-right font-medium text-slate-800">
                    {formatMoney(tx.line_total, tx.currency)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
