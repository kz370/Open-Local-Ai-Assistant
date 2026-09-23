import { Fragment, useEffect, useMemo, useState, type ComponentType } from "react";
import { Activity, Info, Bot, Globe, Keyboard, Languages, MemoryStick, Mic, Palette, PenLine, Plug, Search, Settings, Settings2, Shield, Volume2 } from "lucide-react";
import { useSettingsHighlight } from "../../app/settingsHighlight";
import { on } from "../../app/ipc";
import { t } from "../../app/strings";
import { AboutSection } from "./sections/About";
import { AiSection } from "./sections/Ai";
import { AppearanceSection, GeneralSection, ShortcutsSection } from "./sections/Basic";
import { DictationSection } from "./sections/Dictation";
import { LanguageSection } from "./sections/Languages";
import { McpSection } from "./sections/Mcp";
import { MemorySection } from "./sections/Memory";
import { buildSettingsIndex } from "./searchIndex";
import { WebSearchSection } from "./sections/WebSearch";
import { DiagnosticsSection, PrivacySection } from "./sections/System";
import { SpeechSection, VoiceSection } from "./sections/Voice";

type Section = { id: string; group: string; icon: ComponentType<{ size?: number }>; view: ComponentType };

// Ordered by group, so the menu reads as four short lists instead of one long one.
const SECTIONS: Section[] = [
  { id: "ai", group: "assistant", icon: Bot, view: AiSection },
  { id: "memory", group: "assistant", icon: MemoryStick, view: MemorySection },
  { id: "language", group: "assistant", icon: Languages, view: LanguageSection },
  { id: "search", group: "assistant", icon: Globe, view: WebSearchSection },
  { id: "mcp", group: "assistant", icon: Plug, view: McpSection },
  { id: "speech", group: "voice", icon: Mic, view: SpeechSection },
  { id: "voice", group: "voice", icon: Volume2, view: VoiceSection },
  { id: "dictation", group: "voice", icon: PenLine, view: DictationSection },
  { id: "general", group: "app", icon: Settings2, view: GeneralSection },
  { id: "appearance", group: "app", icon: Palette, view: AppearanceSection },
  { id: "shortcuts", group: "app", icon: Keyboard, view: ShortcutsSection },
  { id: "privacy", group: "app", icon: Shield, view: PrivacySection },
  { id: "about", group: "app", icon: Info, view: AboutSection },
  { id: "diagnostics", group: "advanced", icon: Activity, view: DiagnosticsSection },
];

/** Index of the match, preferring one at the start of a word over a mid-word hit. */
function matchIndex(text: string, q: string): number {
  const lower = text.toLowerCase();
  const needle = q.toLowerCase();
  let at = lower.indexOf(needle);
  while (at > 0 && /[\p{L}\p{N}]/u.test(lower[at - 1])) {
    const next = lower.indexOf(needle, at + 1);
    if (next < 0) return lower.indexOf(needle);
    at = next;
  }
  return at;
}

function isWordStart(text: string, at: number): boolean {
  return at === 0 || !/[\p{L}\p{N}]/u.test(text[at - 1]);
}

function highlightMatch(text: string, q: string) {
  const at = matchIndex(text, q);
  if (at < 0 || q.length < 2 || !isWordStart(text, at)) return text;
  return (
    <>
      {text.slice(0, at)}
      <mark className="settings-search-mark">{text.slice(at, at + q.length)}</mark>
      {text.slice(at + q.length)}
    </>
  );
}

function sectionFromHash(): string {
  const id = location.hash.replace(/^#\/settings\/?/, "").split("/")[0];
  return SECTIONS.some((s) => s.id === id) ? id : "general";
}

export function SettingsApp() {
  const [section, setSection] = useState(sectionFromHash);
  const [query, setQuery] = useState("");
  const setHighlight = useSettingsHighlight((s) => s.set);
  const index = useMemo(() => buildSettingsIndex(SECTIONS.map((s) => s.id)), []);
  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    const rank = (text: string) => {
      const at = matchIndex(text, q);
      return at === 0 ? 0 : isWordStart(text, at) ? 1 : 2;
    };
    return index
      .filter((e) => e.text.toLowerCase().includes(q))
      .map((e) => ({ e, r: rank(e.text) }))
      .sort((a, b) => a.r - b.r || a.e.text.length - b.e.text.length)
      .slice(0, 8)
      .map((x) => x.e);
  }, [query, index]);

  const [activeIdx, setActiveIdx] = useState(0);
  useEffect(() => setActiveIdx(0), [query]);

  const goToResult = (r: { text: string; sectionId: string }) => {
    setHighlight(r.text);
    setQuery("");
    if (r.sectionId === section) return;
    location.hash = `#/settings/${r.sectionId}`;
  };

  const onSearchKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") return setQuery("");
    if (!results.length) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActiveIdx((i) => (i + step + results.length) % results.length);
    } else if (e.key === "Enter") {
      goToResult(results[activeIdx] ?? results[0]);
    }
  };

  useEffect(() => {
    const onHash = () => setSection(sectionFromHash());
    window.addEventListener("hashchange", onHash);
    const sub = on<string>("app://navigate", (route) => {
      location.hash = `#${route}`;
    });
    document.title = `${t("app.name")} — ${t("settings.title")}`;
    return () => {
      window.removeEventListener("hashchange", onHash);
      void sub.then((u) => u());
    };
  }, []);

  const View = SECTIONS.find((s) => s.id === section)!.view;
  return (
    <div className="settings-shell">
      <nav className="settings-nav" aria-label={t("settings.title")}>
        <div className="settings-nav-title">
          <span className="brand-mark settings-mark" aria-hidden>
            <Settings size={14} />
          </span>
          {t("settings.title")}
        </div>
        <div className="settings-search">
          <Search size={14} className="settings-search-icon" aria-hidden />
          <input
            type="search"
            className="input settings-search-input"
            placeholder={t("settings.quickSearch.placeholder")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onSearchKeyDown}
          />
          {query.trim() && (
            <div className="settings-search-results" role="listbox">
              {results.length === 0 && <div className="settings-search-empty">{t("settings.quickSearch.noResults")}</div>}
              {results.map((r, i) => {
                const Icon = SECTIONS.find((s) => s.id === r.sectionId)!.icon;
                return (
                  <button
                    key={`${r.sectionId}-${i}`}
                    type="button"
                    role="option"
                    aria-selected={i === activeIdx}
                    className="settings-search-result"
                    onMouseMove={() => setActiveIdx(i)}
                    onClick={() => goToResult(r)}
                  >
                    <span className="settings-search-result-text">{highlightMatch(r.text, query.trim())}</span>
                    <span className="settings-search-result-section">
                      <Icon size={11} />
                      {t(`settings.sections.${r.sectionId}`)}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
        {SECTIONS.map(({ id, group, icon: Icon }, i) => (
          <Fragment key={id}>
            {(i === 0 || SECTIONS[i - 1].group !== group) && <div className="settings-nav-group">{t(`settings.groups.${group}`)}</div>}
            <button aria-current={section === id ? "page" : undefined} onClick={() => (location.hash = `#/settings/${id}`)}>
              <Icon size={16} />
              {t(`settings.sections.${id}`)}
            </button>
          </Fragment>
        ))}
      </nav>
      <main className="settings-main" key={section}>
        <View />
      </main>
    </div>
  );
}
