import { useEffect, useRef } from "react";

/** Layered sine lines, tapered at both ends, whose height follows the mic level. */
const LINES = [
  { freq: 1.6, speed: 0.9, amp: 1, alpha: 1, width: 2 },
  { freq: 2.3, speed: -1.3, amp: 0.7, alpha: 0.45, width: 1.5 },
  { freq: 3.1, speed: 1.7, amp: 0.45, alpha: 0.25, width: 1.2 },
];

export function VoiceWave({ level, label }: { level: number; label: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const levelRef = useRef(level);
  levelRef.current = level;

  useEffect(() => {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    let frame = 0;
    let smooth = 0;
    const start = performance.now();

    const draw = (now: number) => {
      const dpr = window.devicePixelRatio || 1;
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
        canvas.width = Math.round(w * dpr);
        canvas.height = Math.round(h * dpr);
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);

      // Rise fast, fall slowly; keep a faint idle ripple so it never looks frozen.
      const target = Math.min(1, levelRef.current * 1.6);
      smooth += (target - smooth) * (target > smooth ? 0.35 : 0.08);
      const amp = (h / 2 - 2) * Math.max(0.08, smooth);
      const t = (now - start) / 1000;
      const color = getComputedStyle(canvas).getPropertyValue("--accent").trim() || "#2dd4bf";

      for (const line of LINES) {
        ctx.beginPath();
        for (let x = 0; x <= w; x += 2) {
          const p = x / w;
          const taper = Math.sin(Math.PI * p) ** 2;
          const y = h / 2 + Math.sin(p * Math.PI * 2 * line.freq + t * line.speed * 4) * amp * line.amp * taper;
          if (x === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.strokeStyle = color;
        ctx.globalAlpha = line.alpha;
        ctx.lineWidth = line.width;
        ctx.lineCap = "round";
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
      frame = requestAnimationFrame(draw);
    };
    frame = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frame);
  }, []);

  return <canvas ref={canvasRef} className="voice-wave" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(level * 100)} />;
}
