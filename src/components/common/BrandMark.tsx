/** Rounded gradient mark with a small sound-wave glyph. */
export function BrandMark({ size = 28 }: { size?: number }) {
  return (
    <span className="brand-mark" style={{ width: size, height: size, borderRadius: size * 0.34 }} aria-hidden data-tauri-drag-region>
      <svg viewBox="0 0 24 24" width={size * 0.58} height={size * 0.58} data-tauri-drag-region>
        <path d="M4 12h1.5M8 8.5v7M11.5 5.5v13M15 8.5v7M18.5 11v2" fill="none" stroke="currentColor" strokeWidth={2.2} strokeLinecap="round" />
      </svg>
    </span>
  );
}
