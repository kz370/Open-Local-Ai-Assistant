export function LevelMeter({ levels, max = 36, label }: { levels: number[]; max?: number; label: string }) {
  return (
    <div className="meter" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round((levels[levels.length - 1] ?? 0) * 100)}>
      {levels.map((v, i) => (
        <span key={i} style={{ height: Math.max(3, Math.round(v * max)) }} />
      ))}
    </div>
  );
}
