import { useState } from 'react';
import type { ReactNode } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { getByStore, getSpending } from '../api/analytics';
import type { SpendingPeriodKind } from '../api/analytics';
import { money } from '../lib/money';
import { formatMoney } from '../lib/format';
import { Spinner } from '../components/Spinner';
import { EmptyState } from '../components/EmptyState';

const BAR_COLOR = '#0f172a';

function tooltipMoney(value: number | string): string {
  return formatMoney(value);
}

function ChartCard({
  title,
  loading,
  empty,
  children,
}: {
  title: string;
  loading: boolean;
  empty: boolean;
  children: ReactNode;
}) {
  return (
    <div className="rounded-xl border border-slate-200 bg-white p-5">
      <h2 className="mb-4 text-sm font-semibold text-slate-700">{title}</h2>
      {loading ? (
        <div className="py-16">
          <Spinner label="Loading…" className="justify-center" />
        </div>
      ) : empty ? (
        <p className="py-16 text-center text-sm text-slate-400">No data yet.</p>
      ) : (
        children
      )}
    </div>
  );
}

export function AnalyticsPage() {
  const [period, setPeriod] = useState<SpendingPeriodKind>('month');

  const spendingQuery = useQuery({
    queryKey: ['analytics', 'spending', period],
    queryFn: () => getSpending(period),
  });

  const byStoreQuery = useQuery({
    queryKey: ['analytics', 'by-store'],
    queryFn: () => getByStore(),
  });

  const spendingData = (spendingQuery.data?.periods ?? []).map((p) => ({
    label: p.label,
    total: money(p.total) ?? 0,
  }));

  const storeData = (byStoreQuery.data?.stores ?? []).map((s) => ({
    name: s.name,
    total: money(s.total) ?? 0,
    count: s.count,
  }));

  if (spendingQuery.isError && byStoreQuery.isError) {
    return <EmptyState title="Couldn’t load analytics" description="Please try again shortly." />;
  }

  return (
    <div>
      <div className="mb-6">
        <h1 className="text-xl font-bold tracking-tight text-slate-900">Analytics</h1>
        <p className="text-sm text-slate-500">Your spending trends and top stores.</p>
      </div>

      <div className="space-y-6">
        <div className="rounded-xl border border-slate-200 bg-white p-5">
          <div className="mb-4 flex items-center justify-between gap-3">
            <h2 className="text-sm font-semibold text-slate-700">Spending</h2>
            <div className="inline-flex overflow-hidden rounded-md border border-slate-200">
              {(['week', 'month'] as const).map((option) => (
                <button
                  key={option}
                  type="button"
                  onClick={() => setPeriod(option)}
                  className={`px-3 py-1.5 text-sm font-medium capitalize transition ${
                    period === option
                      ? 'bg-slate-900 text-white'
                      : 'bg-white text-slate-600 hover:bg-slate-100'
                  }`}
                >
                  {option}
                </button>
              ))}
            </div>
          </div>
          {spendingQuery.isLoading ? (
            <div className="py-16">
              <Spinner label="Loading…" className="justify-center" />
            </div>
          ) : spendingData.length === 0 ? (
            <p className="py-16 text-center text-sm text-slate-400">No spending data yet.</p>
          ) : (
            <ResponsiveContainer width="100%" height={300}>
              <BarChart data={spendingData} margin={{ top: 8, right: 8, bottom: 8, left: 8 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" vertical={false} />
                <XAxis dataKey="label" tick={{ fontSize: 12, fill: '#64748b' }} tickLine={false} />
                <YAxis tick={{ fontSize: 12, fill: '#64748b' }} tickLine={false} axisLine={false} width={72} />
                <Tooltip formatter={tooltipMoney} cursor={{ fill: '#f1f5f9' }} />
                <Bar dataKey="total" fill={BAR_COLOR} radius={[4, 4, 0, 0]} maxBarSize={48} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </div>

        <ChartCard
          title="Spend by store"
          loading={byStoreQuery.isLoading}
          empty={storeData.length === 0}
        >
          <ResponsiveContainer width="100%" height={Math.max(200, storeData.length * 44)}>
            <BarChart
              data={storeData}
              layout="vertical"
              margin={{ top: 8, right: 16, bottom: 8, left: 8 }}
            >
              <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" horizontal={false} />
              <XAxis type="number" tick={{ fontSize: 12, fill: '#64748b' }} tickLine={false} axisLine={false} />
              <YAxis
                type="category"
                dataKey="name"
                tick={{ fontSize: 12, fill: '#64748b' }}
                tickLine={false}
                width={120}
              />
              <Tooltip formatter={tooltipMoney} cursor={{ fill: '#f1f5f9' }} />
              <Bar dataKey="total" fill={BAR_COLOR} radius={[0, 4, 4, 0]} maxBarSize={28} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>
      </div>
    </div>
  );
}
