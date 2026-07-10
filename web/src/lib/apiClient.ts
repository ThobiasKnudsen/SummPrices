const BASE_URL = (import.meta.env.VITE_API_BASE ?? 'http://localhost:3000').replace(/\/$/, '');
const TOKEN_KEY = 'summprices_token';

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string): void {
  localStorage.setItem(TOKEN_KEY, token);
}

export function clearToken(): void {
  localStorage.removeItem(TOKEN_KEY);
}

/** Error thrown for any non-2xx response (or a network failure with status 0). */
export class ApiError extends Error {
  readonly status: number;
  readonly body: unknown;

  constructor(status: number, message: string, body?: unknown) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.body = body;
  }
}

export type QueryParams = Record<string, string | number | boolean | null | undefined>;

function buildQuery(params?: QueryParams): string {
  if (!params) return '';
  const usp = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null || value === '') continue;
    usp.set(key, String(value));
  }
  const qs = usp.toString();
  return qs ? `?${qs}` : '';
}

function extractMessage(data: unknown): string | null {
  if (typeof data === 'string' && data.trim()) return data;
  if (data && typeof data === 'object') {
    const obj = data as Record<string, unknown>;
    for (const key of ['error', 'message', 'detail'] as const) {
      if (typeof obj[key] === 'string' && obj[key]) return obj[key] as string;
    }
  }
  return null;
}

interface RequestOptions {
  method?: string;
  body?: unknown;
  query?: QueryParams;
}

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const { method = 'GET', body, query } = options;
  const token = getToken();
  const headers = new Headers();
  if (token) headers.set('Authorization', `Bearer ${token}`);

  let payload: BodyInit | undefined;
  if (body instanceof FormData) {
    // Let the browser set the multipart boundary.
    payload = body;
  } else if (body !== undefined) {
    headers.set('Content-Type', 'application/json');
    payload = JSON.stringify(body);
  }

  let res: Response;
  try {
    res = await fetch(`${BASE_URL}${path}${buildQuery(query)}`, { method, headers, body: payload });
  } catch (err) {
    throw new ApiError(0, 'Network error — could not reach the server.', err);
  }

  // An authenticated request that comes back 401 means the token is dead.
  if (res.status === 401 && token) {
    clearToken();
    if (window.location.pathname !== '/login') {
      window.location.assign('/login');
    }
    throw new ApiError(401, 'Your session has expired. Please sign in again.');
  }

  if (res.status === 204) {
    return undefined as T;
  }

  const contentType = res.headers.get('content-type') ?? '';
  const data = contentType.includes('application/json')
    ? await res.json().catch(() => null)
    : await res.text().catch(() => null);

  if (!res.ok) {
    const message = extractMessage(data) ?? `Request failed (${res.status})`;
    throw new ApiError(res.status, message, data);
  }

  return data as T;
}

export const api = {
  get: <T>(path: string, query?: QueryParams) => request<T>(path, { method: 'GET', query }),
  post: <T>(path: string, body?: unknown) => request<T>(path, { method: 'POST', body }),
  put: <T>(path: string, body?: unknown) => request<T>(path, { method: 'PUT', body }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
  upload: <T>(path: string, form: FormData) => request<T>(path, { method: 'POST', body: form }),
};
