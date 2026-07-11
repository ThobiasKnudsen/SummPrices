import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getDebugModels } from '../api/receipts';

const STORAGE_KEY = 'summprices_debug_model';

function readStored(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? '';
  } catch {
    return '';
  }
}

function writeStored(model: string): void {
  try {
    localStorage.setItem(STORAGE_KEY, model);
  } catch {
    /* ignore (private mode etc.) */
  }
}

/**
 * The extraction model chosen for rescans (persisted in localStorage), plus the server's
 * available options. Until the user picks one, `model` is the recommended (first) option.
 */
export function useModelChoice() {
  const { data } = useQuery({
    queryKey: ['debug-models'],
    queryFn: getDebugModels,
    staleTime: Infinity,
  });
  const [chosen, setChosen] = useState<string>(readStored);
  const options = data?.options ?? [];
  const model = chosen || options[0] || data?.current || '';
  const setModel = (m: string) => {
    setChosen(m);
    writeStored(m);
  };
  return { model, setModel, options, current: data?.current };
}

export function ModelPicker({
  value,
  options,
  current,
  onChange,
  disabled,
  className = '',
}: {
  value: string;
  options: string[];
  current?: string;
  onChange: (model: string) => void;
  disabled?: boolean;
  className?: string;
}) {
  // Always show the current value even if it isn't in the server list.
  const list = value && !options.includes(value) ? [value, ...options] : options;
  return (
    <label className={`flex items-center gap-2 text-xs text-slate-600 ${className}`}>
      <span className="font-medium text-slate-500">Model</span>
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="max-w-[15rem] truncate rounded-md border border-slate-300 bg-white px-2 py-1 text-xs outline-none focus:border-slate-500 focus:ring-2 focus:ring-slate-200 disabled:opacity-60"
      >
        {list.map((m) => (
          <option key={m} value={m}>
            {m}
            {m === current ? '  (server default)' : ''}
          </option>
        ))}
      </select>
    </label>
  );
}
