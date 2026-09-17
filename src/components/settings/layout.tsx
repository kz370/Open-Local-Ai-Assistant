import type { ReactNode } from "react";

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
  return (
    <div className={`row${props.stack ? " stack" : ""}`}>
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
