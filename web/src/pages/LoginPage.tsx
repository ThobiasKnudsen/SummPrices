import { useState } from 'react';
import type { FormEvent } from 'react';
import { Link, Navigate, useNavigate } from 'react-router-dom';
import { useAuth } from '../auth/AuthContext';
import { ApiError } from '../lib/apiClient';

export function LoginPage() {
  const { user, login } = useAuth();
  const navigate = useNavigate();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  if (user) {
    return <Navigate to="/" replace />;
  }

  async function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await login(email, password);
      navigate('/', { replace: true });
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Unable to sign in. Please try again.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-slate-50 px-4">
      <div className="w-full max-w-sm">
        <div className="mb-6 text-center">
          <h1 className="text-2xl font-bold tracking-tight text-slate-900">SummPrices</h1>
          <p className="mt-1 text-sm text-slate-500">Sign in to your account</p>
        </div>
        <form
          onSubmit={onSubmit}
          className="space-y-4 rounded-xl border border-slate-200 bg-white p-6 shadow-sm"
        >
          <div>
            <label htmlFor="email" className="mb-1 block text-sm font-medium text-slate-700">
              Email
            </label>
            <input
              id="email"
              type="email"
              autoComplete="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full rounded-md border border-slate-300 px-3 py-2 text-sm outline-none focus:border-slate-500 focus:ring-2 focus:ring-slate-200"
            />
          </div>
          <div>
            <label htmlFor="password" className="mb-1 block text-sm font-medium text-slate-700">
              Password
            </label>
            <input
              id="password"
              type="password"
              autoComplete="current-password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full rounded-md border border-slate-300 px-3 py-2 text-sm outline-none focus:border-slate-500 focus:ring-2 focus:ring-slate-200"
            />
          </div>
          {error ? (
            <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>
          ) : null}
          <button
            type="submit"
            disabled={submitting}
            className="w-full rounded-md bg-slate-900 px-3 py-2 text-sm font-semibold text-white transition hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {submitting ? 'Signing in…' : 'Sign in'}
          </button>
        </form>
        <p className="mt-4 text-center text-sm text-slate-500">
          No account?{' '}
          <Link to="/register" className="font-medium text-slate-900 hover:underline">
            Create one
          </Link>
        </p>
      </div>
    </div>
  );
}
