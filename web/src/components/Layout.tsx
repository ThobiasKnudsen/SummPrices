import { NavLink, Outlet } from 'react-router-dom';
import { useAuth } from '../auth/AuthContext';

const NAV_ITEMS: Array<{ to: string; label: string; end: boolean }> = [
  { to: '/', label: 'Receipts', end: true },
  { to: '/items', label: 'Items', end: false },
  { to: '/analytics', label: 'Analytics', end: false },
];

function navClass({ isActive }: { isActive: boolean }): string {
  return `rounded-md px-3 py-1.5 text-sm font-medium transition ${
    isActive ? 'bg-slate-900 text-white' : 'text-slate-600 hover:bg-slate-100'
  }`;
}

export function Layout() {
  const { user, logout } = useAuth();

  return (
    <div className="min-h-screen bg-slate-50 text-slate-900">
      <header className="sticky top-0 z-10 border-b border-slate-200 bg-white">
        <div className="mx-auto flex max-w-5xl items-center justify-between gap-4 px-4 py-3">
          <div className="flex items-center gap-6">
            <span className="text-lg font-bold tracking-tight text-slate-900">SummPrices</span>
            <nav className="hidden items-center gap-1 sm:flex">
              {NAV_ITEMS.map((item) => (
                <NavLink key={item.to} to={item.to} end={item.end} className={navClass}>
                  {item.label}
                </NavLink>
              ))}
            </nav>
          </div>
          <div className="flex items-center gap-4">
            <div className="text-right">
              <div className="text-[11px] uppercase tracking-wide text-slate-400">Credits</div>
              <div className="text-sm font-semibold text-slate-900">
                {user?.credit_balance ?? 0}
              </div>
            </div>
            <button
              type="button"
              onClick={logout}
              className="rounded-md border border-slate-200 px-3 py-1.5 text-sm font-medium text-slate-700 transition hover:bg-slate-100"
            >
              Log out
            </button>
          </div>
        </div>
        <nav className="flex items-center gap-1 border-t border-slate-100 px-4 py-2 sm:hidden">
          {NAV_ITEMS.map((item) => (
            <NavLink key={item.to} to={item.to} end={item.end} className={navClass}>
              {item.label}
            </NavLink>
          ))}
        </nav>
      </header>
      <main className="mx-auto max-w-5xl px-4 py-6">
        <Outlet />
      </main>
    </div>
  );
}
