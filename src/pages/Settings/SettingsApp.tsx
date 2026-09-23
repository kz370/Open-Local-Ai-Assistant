import { useEffect, useMemo, useState, type ComponentType } from "react";
import { Activity, AudioLines, Bot, Globe, Keyboard, Languages, MemoryStick, Mic, Palette, PenLine, Plug, Search, Settings2, Shield, Volume2 } from "lucide-react";
import { useSettingsHighlight } from "../../app/settingsHighlight";
import { on } from "../../app/ipc";
import { t } from "../../app/strings";
import { AiSection } from "./sections/Ai";
import { AppearanceSection, GeneralSection, LanguageSection, ShortcutsSection } from "./sections/Basic";
import { DictationSection } from "./sections/Dictation";
import { McpSection } from "./sections/Mcp";
import { MemorySection } from "./sections/Memory";
import { buildSettingsIndex } from "./searchIndex";
import { WebSearchSection } from "./sections/WebSearch";
import { DiagnosticsSection, PrivacySection } from "./sections/System";
import { SpeechSection, VoiceSection } from "./sections/Voice";

const SECTIONS: { id: string; icon: ComponentType<{ size?: number }>; view: ComponentType }[] = [
  { id: "general", icon: Settings2, view: GeneralSection },
  { id: "ai", icon: Bot, view: AiSection },
  { id: "speech", icon: Mic, view: SpeechSection },
  { id: "voice", icon: Volume2, view: VoiceSection },
  { id: "memory", icon: MemoryStick, view: MemorySection },
  { id: "language", icon: Languages, view: LanguageSection },
  { id: "dictation", icon: PenLine, view: DictationSection },
  { id: "search", icon: Globe, view: WebSearchSection },
  { id: "mcp", icon: Plug, view: McpSection },
  { id: "privacy", icon: Shield, view: PrivacySection },
  { id: "appearance", icon: Palette, view: AppearanceSection },
  { id: "shortcuts", icon: Keyboard, view: ShortcutsSection },
  { id: "diagnostics", icon: Activity, view: DiagnosticsSection },
];

function highlightMatch(text: string, q: string) {
  const at = text.toLowerCase().indexOf(q.toLowerCase());
  if (at < 0) return text;
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
    return index.filter((e) => e.text.toLowerCase().includes(q)).slice(0, 8);
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
          <span className="brand-mark" aria-hidden>
            <AudioLines size={11} />
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
        {SECTIONS.map(({ id, icon: Icon }) => (
          <button key={id} aria-current={section === id ? "page" : undefined} onClick={() => (location.hash = `#/settings/${id}`)}>
            <Icon size={16} />
            {t(`settings.sections.${id}`)}
          </button>
        ))}
      </nav>
      <main className="settings-main" key={section}>
        <View />
      </main>
    </div>
  );
}
