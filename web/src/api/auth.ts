import { api } from '../lib/apiClient';
import type { AuthResponse, User } from './types';

export function login(email: string, password: string): Promise<AuthResponse> {
  return api.post<AuthResponse>('/api/auth/login', { email, password });
}

export function register(
  email: string,
  password: string,
  display_name?: string,
): Promise<AuthResponse> {
  return api.post<AuthResponse>('/api/auth/register', { email, password, display_name });
}

export function fetchMe(): Promise<User> {
  return api.get<User>('/api/auth/me');
}
