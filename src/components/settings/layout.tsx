import { useEffect, useRef, type ReactNode } from "react";
import { useSettingsHighlight } from "../../app/settingsHighlight";

export function Card(props: { title?: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="card">
      {(props.title || props.actions) && (
        <div className="card-head">
          {props.title && <h2>{props.title}</h2>}
          {props.actions && <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>{props.actions}</div>}
        </div>
      )}
      <div className="card-body">{props.children}</div>
    </section>
  );
}

export function Row(props: { label: string; hint?: string; htmlFor?: string; children: ReactNode; stack?: boolean; end?: boolean }) {
  const term = useSettingsHighlight((s) => s.term);
  const clear = useSettingsHighlight((s) => s.clear);
  const hit = term !== null && term === props.label;
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!hit) return;
    ref.current?.scrollIntoView({ behavior: "smooth", block: "center" });
    const id = setTimeout(clear, 2200);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hit]);

  return (
    <div ref={ref} className={`row${props.stack ? " stack" : ""}${hit ? " row-hit" : ""}`}>
      <label className="row-label" htmlFor={props.htmlFor}>
        {props.label}
        {props.hint && <span className="row-hint">{props.hint}</span>}
      </label>
      <div className={`row-control${props.end ? " end" : ""}`}>{props.children}</div>
    </div>
  );
}

export function SectionHeader({ title, intro }: { title: string; intro?: string }) {
  return (
    <>
      <h1>{title}</h1>
      {intro ? <p className="settings-intro">{intro}</p> : <div style={{ height: 14 }} />}
    </>
  );
}
