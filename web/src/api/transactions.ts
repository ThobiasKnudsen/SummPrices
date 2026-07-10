import { api } from '../lib/apiClient';
import type { ItemType, TransactionListResponse } from './types';

export interface TransactionFilters {
  page?: number;
  per_page?: number;
  q?: string;
  store?: string;
  from?: string;
  to?: string;
}

export function listTransactions(
  filters: TransactionFilters = {},
): Promise<TransactionListResponse> {
  return api.get<TransactionListResponse>('/api/transactions', { ...filters });
}

export interface UpdateTransactionBody {
  description_clean?: string;
  item_type?: ItemType;
  quantity?: number;
  unit_price?: number;
  line_total?: number;
}

export function updateTransaction(id: number, body: UpdateTransactionBody): Promise<void> {
  return api.put<void>(`/api/transactions/${id}`, body);
}

export function deleteTransaction(id: number): Promise<void> {
  return api.delete<void>(`/api/transactions/${id}`);
}
