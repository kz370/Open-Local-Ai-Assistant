import { useMemo, useState } from "react";
import type { KnownLanguage } from "../../app/knownLanguages";

const label = (k: KnownLanguage) => (k.native !== k.name ? `${k.name} — ${k.native}` : k.name);

/** Search box with a list of languages under it; typing filters by English name, native name or code. */
export function LanguagePicker(props: {
  options: KnownLanguage[];
  value: string;
  onChange: (code: string) => void;
  placeholder: string;
  label: string;
  emptyText: string;
}) {
  const selected = props.options.find((k) => k.code === props.value);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return props.options;
    const hit = (k: KnownLanguage) => k.name.toLowerCase().includes(q) || k.native.toLowerCase().includes(q) || k.code === q;
    // Names that start with the query first, then the rest.
    return props.options.filter(hit).sort((a, b) => Number(b.name.toLowerCase().startsWith(q)) - Number(a.name.toLowerCase().startsWith(q)));
  }, [props.options, query]);

  const pick = (k: KnownLanguage) => {
    props.onChange(k.code);
    setQuery("");
    setOpen(false);
  };

  return (
    <div className="lang-picker">
      <input
        className="input"
        role="combobox"
        aria-expanded={open}
        aria-label={props.label}
        placeholder={props.placeholder}
        value={open ? query : selected ? label(selected) : query}
        onFocus={() => {
          setOpen(true);
          setQuery("");
        }}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
          setOpen(true);
          if (props.value) props.onChange("");
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setOpen(true);
            setActive((a) => Math.min(a + 1, matches.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive((a) => Math.max(a - 1, 0));
          } else if (e.key === "Enter" && open && matches[active]) {
            e.preventDefault();
            pick(matches[active]);
          } else if (e.key === "Escape") {
            setOpen(false);
          }
        }}
        onBlur={() => setTimeout(() => setOpen(false), 120)}
      />
      {open && (
        <ul className="lang-list" role="listbox" aria-label={props.label}>
          {matches.length === 0 && <li className="lang-empty">{props.emptyText}</li>}
          {matches.map((k, i) => (
            <li
              key={k.code}
              role="option"
              aria-selected={k.code === props.value}
              className={`lang-option${i === active ? " active" : ""}`}
              onMouseEnter={() => setActive(i)}
              onMouseDown={(e) => {
                e.preventDefault();
                pick(k);
              }}
            >
              <span>{k.name}</span>
              <span className="lang-native" dir={k.direction}>
                {k.native !== k.name ? k.native : ""}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
