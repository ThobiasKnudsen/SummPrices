import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from 'react-router-dom';
import { getReceiptStatus, uploadReceipt } from '../api/receipts';
import type { ExtractionStatus } from '../api/types';
import { Spinner } from '../components/Spinner';
import { ApiError } from '../lib/apiClient';

const TERMINAL_STATUSES: ExtractionStatus[] = ['done', 'failed', 'needs_review'];

function isTerminal(status: ExtractionStatus | undefined): boolean {
  return status !== undefined && TERMINAL_STATUSES.includes(status);
}

export function UploadPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [file, setFile] = useState<File | null>(null);
  const [receiptId, setReceiptId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const upload = useMutation({
    mutationFn: (f: File) => uploadReceipt(f),
    onSuccess: (receipt) => {
      setReceiptId(receipt.id);
      queryClient.invalidateQueries({ queryKey: ['receipts'] });
    },
    onError: (err) => {
      setError(err instanceof ApiError ? err.message : 'Upload failed. Please try again.');
    },
  });

  const statusQuery = useQuery({
    queryKey: ['receipt-status', receiptId],
    queryFn: () => getReceiptStatus(receiptId as string),
    enabled: receiptId !== null,
    refetchInterval: (query) => (isTerminal(query.state.data?.extraction_status) ? false : 1500),
  });

  const status = statusQuery.data?.extraction_status;

  useEffect(() => {
    if (!receiptId || !isTerminal(status)) return;
    if (status === 'failed') {
      setError('Extraction failed. Please try uploading a clearer scan.');
      return;
    }
    navigate(`/receipts/${receiptId}`, { replace: true });
  }, [receiptId, status, navigate]);

  function reset() {
    setFile(null);
    setReceiptId(null);
    setError(null);
    upload.reset();
  }

  function onSubmit() {
    if (!file) return;
    setError(null);
    upload.mutate(file);
  }

  const polling = receiptId !== null && !isTerminal(status);
  const busy = upload.isPending || polling;

  return (
    <div className="mx-auto max-w-xl">
      <div className="mb-6">
        <Link to="/" className="text-sm text-slate-500 hover:text-slate-800">
          ← Back to receipts
        </Link>
        <h1 className="mt-2 text-xl font-bold tracking-tight text-slate-900">Upload a receipt</h1>
        <p className="text-sm text-slate-500">Upload an image (JPG, PNG, WEBP) or a PDF.</p>
      </div>

      <div className="rounded-xl border border-slate-200 bg-white p-6 shadow-sm">
        {busy ? (
          <div className="flex flex-col items-center gap-4 py-8 text-center">
            <Spinner label={upload.isPending ? 'Uploading…' : 'Extracting receipt…'} />
            <p className="text-sm text-slate-500">
              This can take a few seconds. You’ll be taken to the receipt automatically.
            </p>
            {status ? (
              <p className="text-xs uppercase tracking-wide text-slate-400">Status: {status}</p>
            ) : null}
          </div>
        ) : (
          <div className="space-y-5">
            <div>
              <label
                htmlFor="receipt-file"
                className="flex cursor-pointer flex-col items-center justify-center rounded-lg border-2 border-dashed border-slate-300 px-6 py-10 text-center transition hover:border-slate-400"
              >
                <span className="text-sm font-medium text-slate-700">
                  {file ? file.name : 'Choose a file to upload'}
                </span>
                <span className="mt-1 text-xs text-slate-400">
                  {file ? 'Click to choose a different file' : 'Image or PDF'}
                </span>
                <input
                  id="receipt-file"
                  type="file"
                  accept="image/*,application/pdf"
                  className="hidden"
                  onChange={(e) => {
                    setError(null);
                    setFile(e.target.files?.[0] ?? null);
                  }}
                />
              </label>
            </div>

            {error ? (
              <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-700">{error}</p>
            ) : null}

            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={onSubmit}
                disabled={!file}
                className="rounded-md bg-slate-900 px-4 py-2 text-sm font-semibold text-white transition hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-60"
              >
                Upload
              </button>
              {file ? (
                <button
                  type="button"
                  onClick={reset}
                  className="rounded-md border border-slate-200 px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-100"
                >
                  Clear
                </button>
              ) : null}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
