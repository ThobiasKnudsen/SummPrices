import { useEffect, useState } from 'react';

interface ImageLightboxProps {
  src: string;
  alt?: string;
}

const MAX_ZOOM = 6;
const MIN_ZOOM = 1;

/**
 * Receipt thumbnail that opens a full-screen, zoomable/pannable viewer so the raw
 * scan can be inspected line-by-line. Zoom with the buttons, +/- keys, or by
 * clicking the image; scroll to pan; Escape or the backdrop closes it.
 */
export function ImageLightbox({ src, alt = 'Receipt scan' }: ImageLightboxProps) {
  const [open, setOpen] = useState(false);
  const [zoom, setZoom] = useState(MIN_ZOOM);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
      else if (e.key === '+' || e.key === '=') setZoom((z) => Math.min(z + 0.5, MAX_ZOOM));
      else if (e.key === '-') setZoom((z) => Math.max(z - 0.5, MIN_ZOOM));
    };
    window.addEventListener('keydown', onKey);
    document.body.style.overflow = 'hidden';
    return () => {
      window.removeEventListener('keydown', onKey);
      document.body.style.overflow = '';
    };
  }, [open]);

  return (
    <>
      <button
        type="button"
        onClick={() => {
          setZoom(MIN_ZOOM);
          setOpen(true);
        }}
        className="group relative block w-full overflow-hidden rounded-xl border border-slate-200 bg-white"
        aria-label="Zoom receipt image"
      >
        <img src={src} alt={alt} className="w-full object-contain" />
        <span className="pointer-events-none absolute bottom-2 right-2 rounded-md bg-slate-900/70 px-2 py-1 text-xs font-medium text-white opacity-0 transition group-hover:opacity-100">
          Click to zoom
        </span>
      </button>

      {open ? (
        <div
          className="fixed inset-0 z-50 flex flex-col bg-black/90"
          role="dialog"
          aria-modal="true"
          aria-label="Receipt image viewer"
          onClick={() => setOpen(false)}
        >
          <div
            className="flex items-center justify-end gap-2 p-3"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              type="button"
              onClick={() => setZoom((z) => Math.max(z - 0.5, MIN_ZOOM))}
              className="rounded-md bg-white/10 px-3 py-1.5 text-sm text-white transition hover:bg-white/20"
              aria-label="Zoom out"
            >
              −
            </button>
            <span className="w-14 text-center text-sm tabular-nums text-white">
              {Math.round(zoom * 100)}%
            </span>
            <button
              type="button"
              onClick={() => setZoom((z) => Math.min(z + 0.5, MAX_ZOOM))}
              className="rounded-md bg-white/10 px-3 py-1.5 text-sm text-white transition hover:bg-white/20"
              aria-label="Zoom in"
            >
              +
            </button>
            <button
              type="button"
              onClick={() => setZoom(MIN_ZOOM)}
              className="rounded-md bg-white/10 px-3 py-1.5 text-sm text-white transition hover:bg-white/20"
            >
              Reset
            </button>
            <button
              type="button"
              onClick={() => setOpen(false)}
              className="ml-2 rounded-md bg-white/10 px-3 py-1.5 text-sm text-white transition hover:bg-white/20"
              aria-label="Close viewer"
            >
              Close ✕
            </button>
          </div>
          <div className="flex-1 overflow-auto p-4" onClick={(e) => e.stopPropagation()}>
            <img
              src={src}
              alt={alt}
              onClick={() => setZoom((z) => (z >= MAX_ZOOM ? MIN_ZOOM : z + 1))}
              style={{
                width: `${zoom * 100}%`,
                maxWidth: 'none',
                cursor: zoom >= MAX_ZOOM ? 'zoom-out' : 'zoom-in',
              }}
              className="mx-auto"
            />
          </div>
        </div>
      ) : null}
    </>
  );
}
