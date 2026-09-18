import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { SpokenSentence } from "../../app/voiceStore";
import { textDir } from "../common/controls";

/** A word of the spoken sentence and the slice of the sentence it occupies. */
interface Word {
  text: string;
  /** Fraction of the sentence, 0..1, at which this word starts. */
  from: number;
  /** Fraction of the sentence, 0..1, at which this word ends. */
  until: number;
}

/**
 * Splits a sentence into words weighted by length: a long word is spoken for
 * longer than a short one, which is close enough to follow the voice without
 * per-word timings from the speech engine.
 */
function split(text: string): Word[] {
  const parts = text.split(/\s+/).filter(Boolean);
  const weight = (w: string) => w.length + 2; // +2 so short words still take time
  const total = parts.reduce((sum, w) => sum + weight(w), 0) || 1;
  let acc = 0;
  return parts.map((w) => {
    const from = acc / total;
    acc += weight(w);
    return { text: w, from, until: acc / total };
  });
}

/** Rests on a word, then glides to the next one instead of snapping. */
function ease(t: number): number {
  const x = Math.min(1, Math.max(0, (t - 0.55) / 0.45));
  return x * x * (3 - 2 * x);
}

const reducedMotion = () => typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Shows what the assistant is saying as a single line that slides along with
 * the voice, highlighting the word being spoken.
 *
 * Everything here is built to keep the animation cheap: the line is moved every
 * frame with a composited transform written straight to the node (never through
 * React, never through scrollLeft), React re-renders only when the spoken word
 * actually changes, and word positions are measured once per sentence rather
 * than on every step.
 */
export function SpeechTicker({ sentence, paused }: { sentence: SpokenSentence; paused: boolean }) {
  const words = useMemo(() => split(sentence.text), [sentence.text]);
  const [index, setIndex] = useState(0);
  const frameRef = useRef(0);
  const shownRef = useRef(0);
  const viewRef = useRef<HTMLDivElement>(null);
  const lineRef = useRef<HTMLParagraphElement>(null);
  /** Distance the line has to move to centre each word, measured once. */
  const shiftsRef = useRef<number[]>([]);

  useEffect(() => {
    shownRef.current = 0;
    setIndex(0);
  }, [sentence.text, sentence.startedAt]);

  // Measure once per sentence: reading layout while the line moves would force
  // the browser to recompute it on every frame.
  useLayoutEffect(() => {
    const view = viewRef.current;
    const line = lineRef.current;
    if (!view || !line) return;
    const spans = Array.from(line.children) as HTMLElement[];
    const middle = view.clientWidth / 2;
    shiftsRef.current = spans.map((s) => middle - (s.offsetLeft + s.offsetWidth / 2));
    line.style.transform = `translate3d(${shiftsRef.current[0] ?? 0}px, 0, 0)`;
  }, [words]);

  useEffect(() => {
    if (paused || words.length === 0) return;
    const smooth = !reducedMotion();
    const step = () => {
      const progress = Math.min(1, Math.max(0, (performance.now() - sentence.startedAt) / Math.max(1, sentence.durationMs)));
      const at = words.findIndex((w) => w.until > progress);
      const next = at === -1 ? words.length - 1 : at;
      const line = lineRef.current;
      const shifts = shiftsRef.current;
      if (line && shifts.length > 0) {
        const here = shifts[next] ?? 0;
        const after = shifts[next + 1] ?? here;
        const word = words[next];
        const within = smooth ? (progress - word.from) / Math.max(1e-6, word.until - word.from) : 0;
        // Sub-word interpolation: the line creeps toward the next word instead
        // of jumping when the highlight moves, which is what makes it readable.
        line.style.transform = `translate3d(${here + (after - here) * ease(within)}px, 0, 0)`;
      }
      // Re-render only when the spoken word changes.
      if (next !== shownRef.current) {
        shownRef.current = next;
        setIndex(next);
      }
      frameRef.current = requestAnimationFrame(step);
    };
    frameRef.current = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frameRef.current);
  }, [paused, words, sentence.startedAt, sentence.durationMs]);

  return (
    <div className="ticker" ref={viewRef} dir={textDir(sentence.text)} aria-live="off">
      <p className="ticker-line" ref={lineRef}>
        {words.map((w, i) => (
          <span key={`${i}-${w.text}`} className={`ticker-word${i === index ? " on" : i < index ? " done" : ""}`}>
            {w.text}
          </span>
        ))}
      </p>
    </div>
  );
}
