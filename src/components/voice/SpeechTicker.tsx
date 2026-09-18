import { useEffect, useMemo, useRef, useState } from "react";
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
 * Shows what the assistant is saying as a single line that scrolls along with
 * the voice, highlighting the word being spoken.
 */
export function SpeechTicker({ sentence, paused }: { sentence: SpokenSentence; paused: boolean }) {
  const words = useMemo(() => split(sentence.text), [sentence.text]);
  const [index, setIndex] = useState(0);
  const trackRef = useRef<HTMLDivElement>(null);
  const activeRef = useRef<HTMLSpanElement>(null);

  useEffect(() => setIndex(0), [sentence.text, sentence.startedAt]);

  useEffect(() => {
    if (paused || words.length === 0) return;
    let frame = 0;
    const step = () => {
      const progress = (performance.now() - sentence.startedAt) / Math.max(1, sentence.durationMs);
      const at = words.findIndex((w) => w.until > progress);
      setIndex(at === -1 ? words.length - 1 : at);
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [paused, words, sentence.startedAt, sentence.durationMs]);

  // Keep the spoken word in the middle of the line, like a news ticker.
  useEffect(() => {
    const track = trackRef.current;
    const active = activeRef.current;
    if (!track || !active) return;
    const left = active.offsetLeft - track.clientWidth / 2 + active.offsetWidth / 2;
    if (typeof track.scrollTo === "function") track.scrollTo({ left, behavior: "smooth" });
    else track.scrollLeft = left;
  }, [index]);

  return (
    <div className="ticker" ref={trackRef} dir={textDir(sentence.text)} aria-live="off">
      <p className="ticker-line">
        {words.map((w, i) => (
          <span key={`${i}-${w.text}`} ref={i === index ? activeRef : undefined} className={`ticker-word${i === index ? " on" : i < index ? " done" : ""}`}>
            {w.text}
          </span>
        ))}
      </p>
    </div>
  );
}
