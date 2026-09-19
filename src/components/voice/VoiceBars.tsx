import { SPECTRUM_BANDS } from "../../app/voiceStore";

/**
 * Live voice spectrum: low pitch in the middle, higher pitch toward both
 * edges. Bars only move with the microphone; silence leaves them flat.
 */
export function VoiceBars({ bands, level, label }: { bands: number[]; level: number; label: string }) {
  const b = bands.length ? bands : new Array<number>(SPECTRUM_BANDS).fill(0);
  const bars = [...b].reverse().concat(b);
  return (
    <div className="voice-bars" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(level * 100)}>
      {bars.map((v, i) => (
        <span key={i} style={{ transform: `scaleY(${Math.max(0.12, v)})` }} />
      ))}
    </div>
  );
}
