import { api } from '../lib/apiClient';
import type {
  DebugModels,
  ReceiptDetail,
  ReceiptHeader,
  ReceiptListResponse,
  ReceiptStatus,
  ReprocessAllResponse,
} from './types';

export interface ReceiptFilters {
  page?: number;
  per_page?: number;
  store?: string;
  from?: string;
  to?: string;
  status?: string;
}

export function listReceipts(filters: ReceiptFilters = {}): Promise<ReceiptListResponse> {
  return api.get<ReceiptListResponse>('/api/receipts', { ...filters });
}

export function getReceipt(id: string): Promise<ReceiptDetail> {
  return api.get<ReceiptDetail>(`/api/receipts/${id}`);
}

export function getReceiptStatus(id: string): Promise<ReceiptStatus> {
  return api.get<ReceiptStatus>(`/api/receipts/${id}/status`);
}

/** Uploads jpg/png/webp under field `image`, PDFs under field `pdf`. */
export function uploadReceipt(file: File): Promise<ReceiptHeader> {
  const isPdf = file.type === 'application/pdf' || file.name.toLowerCase().endsWith('.pdf');
  const form = new FormData();
  form.append(isPdf ? 'pdf' : 'image', file);
  return api.upload<ReceiptHeader>('/api/receipts', form);
}

export interface UpdateReceiptBody {
  store_name?: string;
  purchase_at?: string;
  total?: number;
}

export function updateReceipt(id: string, body: UpdateReceiptBody): Promise<ReceiptHeader> {
  return api.put<ReceiptHeader>(`/api/receipts/${id}`, body);
}

export function deleteReceipt(id: string): Promise<void> {
  return api.delete<void>(`/api/receipts/${id}`);
}

/** Re-run extraction on an already-uploaded receipt, optionally with a specific model. */
export function reprocessReceipt(id: string, model?: string): Promise<void> {
  return api.post<void>(`/api/receipts/${id}/reprocess`, { model: model || undefined });
}

/** Debug: re-extract all of the current user's receipts, optionally with a specific model. */
export function reprocessAllReceipts(model?: string): Promise<ReprocessAllResponse> {
  return api.post<ReprocessAllResponse>('/api/debug/reprocess-all', { model: model || undefined });
}

/** Debug: the selectable extraction models for the in-app model picker. */
export function getDebugModels(): Promise<DebugModels> {
  return api.get<DebugModels>('/api/debug/models');
}
