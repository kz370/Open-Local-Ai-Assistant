import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { SpokenSentence } from "../../app/voiceStore";
import { textDir } from "../common/controls";

/** A word of the spoken sentence and the share of the sentence it takes. */
interface Word {
  text: string;
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
    acc += weight(w);
    return { text: w, until: acc / total };
  });
}

/**
 * Shows what the assistant is saying as a single line that slides along with
 * the voice, highlighting the word being spoken.
 *
 * Everything here is built to keep the animation cheap: the frame loop only
 * touches React when the spoken word actually changes (a few times a second,
 * not sixty), the line is moved with a composited transform instead of
 * scrolling, and word positions are measured once per sentence rather than on
 * every step.
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

  useLayoutEffect(() => {
    const line = lineRef.current;
    const shift = shiftsRef.current[index];
    if (!line || shift === undefined) return;
    line.style.transform = `translate3d(${shift}px, 0, 0)`;
  }, [index]);

  useEffect(() => {
    if (paused || words.length === 0) return;
    const step = () => {
      const progress = (performance.now() - sentence.startedAt) / Math.max(1, sentence.durationMs);
      const at = words.findIndex((w) => w.until > progress);
      const next = at === -1 ? words.length - 1 : at;
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
