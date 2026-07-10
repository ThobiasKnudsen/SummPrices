import { api } from '../lib/apiClient';
import type {
  ReceiptDetail,
  ReceiptHeader,
  ReceiptListResponse,
  ReceiptStatus,
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
