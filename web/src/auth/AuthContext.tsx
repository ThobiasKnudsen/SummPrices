import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { clearToken, getToken, setToken } from '../lib/apiClient';
import { fetchMe, login as loginRequest, register as registerRequest } from '../api/auth';
import type { User } from '../api/types';

interface AuthContextValue {
  user: User | null;
  /** True while the initial `/me` lookup is in flight. */
  loading: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string, displayName?: string) => Promise<void>;
  logout: () => void;
  /** Re-fetch the current user (e.g. to refresh the credit balance). */
  refresh: () => Promise<void>;
}

const AuthContext = createContext<AuthContextValue | undefined>(undefined);

export function AuthProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState<boolean>(() => getToken() !== null);

  useEffect(() => {
    let active = true;
    if (!getToken()) {
      setLoading(false);
      return;
    }
    fetchMe()
      .then((me) => {
        if (active) setUser(me);
      })
      .catch(() => {
        if (active) setUser(null);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    if (!getToken()) {
      setUser(null);
      return;
    }
    const me = await fetchMe();
    setUser(me);
  }, []);

  const login = useCallback(async (email: string, password: string) => {
    const res = await loginRequest(email, password);
    setToken(res.token);
    setUser(res.user);
  }, []);

  const register = useCallback(
    async (email: string, password: string, displayName?: string) => {
      const res = await registerRequest(email, password, displayName);
      setToken(res.token);
      setUser(res.user);
    },
    [],
  );

  const logout = useCallback(() => {
    clearToken();
    setUser(null);
    navigate('/login', { replace: true });
  }, [navigate]);

  const value = useMemo<AuthContextValue>(
    () => ({ user, loading, login, register, logout, refresh }),
    [user, loading, login, register, logout, refresh],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

// eslint-disable-next-line react-refresh/only-export-components
export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return ctx;
}
