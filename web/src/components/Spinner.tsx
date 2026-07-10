export function Spinner({ label, className = '' }: { label?: string; className?: string }) {
  return (
    <div className={`flex items-center gap-3 text-slate-500 ${className}`}>
      <span
        className="h-5 w-5 shrink-0 animate-spin rounded-full border-2 border-slate-300 border-t-slate-600"
        aria-hidden="true"
      />
      {label ? <span className="text-sm">{label}</span> : null}
    </div>
  );
}
