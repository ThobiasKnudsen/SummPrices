export type ExtractionStatus =
  | 'pending'
  | 'queued'
  | 'processing'
  | 'done'
  | 'failed'
  | 'needs_review';

export type ItemType =
  | 'product'
  | 'deposit'
  | 'discount'
  | 'fee'
  | 'rounding'
  | 'unknown';

export interface User {
  id: string;
  email: string;
  display_name: string | null;
  credit_balance: number;
}

export interface AuthResponse {
  token: string;
  user: User;
}

/**
 * Money fields (`subtotal`, `mva_total`, `total`) arrive as JSON strings.
 * Parse with `money()` before display/math.
 */
export interface ReceiptHeader {
  id: string;
  user_id: string;
  store_name_raw: string | null;
  purchase_at: string | null;
  subtotal: string | null;
  mva_total: string | null;
  total: string | null;
  currency: string;
  extraction_status: ExtractionStatus;
  extraction_conf: number | null;
  needs_review: boolean;
  created_at: string;
  updated_at: string;
}

export interface Transaction {
  id: number;
  receipt_id: string;
  description_raw: string;
  description_clean: string | null;
  item_type: ItemType;
  quantity: string | null;
  unit: string | null;
  unit_price: string | null;
  line_total: string | null;
  mva_rate: string | null;
}

export interface TxWithContext extends Transaction {
  store_name_raw: string | null;
  purchase_at: string | null;
}

export interface ReceiptDetail extends ReceiptHeader {
  transactions: Transaction[];
  image_url: string | null;
}

export interface ReceiptSummary {
  id: string;
  store_name_raw: string | null;
  purchase_at: string | null;
  total: string | null;
  currency: string;
  extraction_status: ExtractionStatus;
  created_at: string;
}

export interface ReceiptListResponse {
  receipts: ReceiptSummary[];
  total_count: number;
}

export interface ReceiptStatus {
  extraction_status: ExtractionStatus;
  extraction_conf: number | null;
}

export interface TransactionListResponse {
  transactions: TxWithContext[];
  total_count: number;
}

export interface SpendingPeriod {
  label: string;
  total: string | null;
}

export interface SpendingResponse {
  periods: SpendingPeriod[];
}

export interface StoreSpending {
  name: string;
  total: string | null;
  count: number;
}

export interface ByStoreResponse {
  stores: StoreSpending[];
}
