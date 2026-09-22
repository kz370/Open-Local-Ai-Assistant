import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { SpokenSentence } from "../../app/voiceStore";
import { textDir } from "../common/controls";

/** Markdown the reply may contain, which is not spoken and should not show. */
function plain(md: string): string {
  return md
    .replace(/```[^\n]*\n?/g, "")
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^\s{0,3}(#{1,6}|>|[-*+]|\d+[.)])\s+/gm, "")
    .replace(/[*_`~]+/g, "");
}

/**
 * Comparable form of a word: lower case, without Arabic diacritics (spoken
 * Arabic gets tashkeel added) or Latin accents, without punctuation.
 */
function norm(w: string): string {
  return w
    .normalize("NFD")
    .replace(/[̀-ًͯ-ٰٟۖ-ۭ]/g, "")
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]/gu, "");
}

/** Weight of each word in a sentence: a long word is spoken for longer. */
function weights(words: string[]): number[] {
  const w = words.map((x) => x.length + 2); // +2 so short words still take time
  const total = w.reduce((a, b) => a + b, 0) || 1;
  let acc = 0;
  return w.map((x) => (acc += x) / total); // end of each word, 0..1
}

/** How far ahead of the last spoken word a sentence's first word is looked for. */
const SEARCH_AHEAD = 60;

/**
 * Shows the whole reply and highlights the word being spoken, keeping it in
 * view. Pauses in playback (the next sentence not synthesized yet) simply hold
 * the highlight where it is.
 *
 * The spoken text differs from the reply (markdown removed, numbers read out,
 * diacritics added), so each spoken sentence is located in the reply by its
 * first words, searching forward from where the previous sentence ended.
 */
export function SpokenText({ text, sentence, paused }: { text: string; sentence: SpokenSentence | null; paused: boolean }) {
  // Words alternate with the whitespace between them, so the layout (line
  // breaks included) stays as written.
  const tokens = useMemo(() => plain(text).split(/(\s+)/), [text]);
  const words = useMemo(() => tokens.map((t, i) => ({ i, key: norm(t) })).filter((w) => w.key && !/^\s+$/.test(tokens[w.i])), [tokens]);

  // Where the latest sentence sits in `words`. Resuming after a pause shifts
  // its start time, so it is recognized by its text, not its timing.
  const placed = useRef<{ tag: string; text: string; start: number; ends: number[] } | null>(null);
  const cursor = useRef(0);
  const [current, setCurrent] = useState(-1); // index into `words`
  const shown = useRef(-1);
  const viewRef = useRef<HTMLDivElement>(null);

  const place = (s: SpokenSentence) => {
    const known = placed.current;
    if (known && known.tag === s.tag && known.text === s.text) return known;
    const spoken = s.text.split(/\s+/).map(norm).filter(Boolean);
    let start = Math.min(cursor.current, Math.max(0, words.length - 1));
    const limit = Math.min(words.length, cursor.current + SEARCH_AHEAD);
    for (let i = cursor.current; i < limit; i++) {
      if (words[i].key === spoken[0] && (spoken.length < 2 || words[i + 1]?.key === spoken[1] || words[i + 2]?.key === spoken[1])) {
        start = i;
        break;
      }
    }
    const p = { tag: s.tag, text: s.text, start, ends: weights(spoken) };
    placed.current = p;
    cursor.current = Math.min(words.length, start + spoken.length);
    return p;
  };

  // A different reply starts over; a gap with no sentence keeps the place.
  const tag = useRef<string | null>(null);
  const nextTag = sentence?.tag ?? tag.current;
  useEffect(() => {
    if (nextTag === tag.current) return;
    tag.current = nextTag;
    placed.current = null;
    cursor.current = 0;
    shown.current = -1;
    setCurrent(-1);
  }, [nextTag]);

  useEffect(() => {
    if (!sentence || words.length === 0) return;
    const p = place(sentence);
    const step = () => {
      const progress = Math.min(1, Math.max(0, (performance.now() - sentence.startedAt) / Math.max(1, sentence.durationMs)));
      const k = p.ends.findIndex((end) => end > progress);
      const next = Math.min(words.length - 1, p.start + (k === -1 ? p.ends.length - 1 : k));
      // Re-render only when the spoken word changes.
      if (next !== shown.current) {
        shown.current = next;
        setCurrent(next);
      }
    };
    step();
    if (paused) return;
    let frame = 0;
    const loop = () => {
      step();
      frame = requestAnimationFrame(loop);
    };
    frame = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(frame);
    // `place` reads refs only; `words` changes as the reply streams in.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sentence, paused, words]);

  // Keep the spoken word near the middle of the box.
  useLayoutEffect(() => {
    const view = viewRef.current;
    const el = view?.querySelector<HTMLElement>(".spoken-word.on");
    if (!view || !el) return;
    const top = el.offsetTop - view.clientHeight / 2 + el.offsetHeight / 2;
    view.scrollTo?.({ top: Math.max(0, top), behavior: "smooth" });
  }, [current]);

  const tokenState = new Map<number, string>();
  words.forEach((w, n) => tokenState.set(w.i, n < current ? "done" : n === current ? "on" : ""));
  const dir = textDir(text);
  return (
    <div className="spoken" ref={viewRef} dir={dir} lang={dir === "rtl" ? "ar" : undefined} aria-live="off">
      <p className="spoken-text">
        {tokens.map((t, i) => {
          const state = tokenState.get(i);
          return state === undefined ? (
            t
          ) : (
            <span key={i} className={`spoken-word${state ? ` ${state}` : ""}`}>
              {t}
            </span>
          );
        })}
      </p>
    </div>
  );
}
