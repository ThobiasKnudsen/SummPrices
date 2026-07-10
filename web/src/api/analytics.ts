import { api } from '../lib/apiClient';
import type { ByStoreResponse, SpendingResponse } from './types';

export type SpendingPeriodKind = 'week' | 'month';

export function getSpending(period: SpendingPeriodKind): Promise<SpendingResponse> {
  return api.get<SpendingResponse>('/api/analytics/spending', { period });
}

export function getByStore(): Promise<ByStoreResponse> {
  return api.get<ByStoreResponse>('/api/analytics/by-store');
}
