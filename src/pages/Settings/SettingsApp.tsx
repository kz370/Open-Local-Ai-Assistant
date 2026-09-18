import { useEffect, useState, type ComponentType } from "react";
import { Activity, AudioLines, Bot, Keyboard, Languages, Mic, Palette, PenLine, Plug, Settings2, Shield, Volume2 } from "lucide-react";
import { on } from "../../app/ipc";
import { t } from "../../app/strings";
import { AiSection } from "./sections/Ai";
import { AppearanceSection, GeneralSection, LanguageSection, ShortcutsSection } from "./sections/Basic";
import { DictationSection } from "./sections/Dictation";
import { McpSection } from "./sections/Mcp";
import { DiagnosticsSection, PrivacySection } from "./sections/System";
import { SpeechSection, VoiceSection } from "./sections/Voice";

const SECTIONS: { id: string; icon: ComponentType<{ size?: number }>; view: ComponentType }[] = [
  { id: "general", icon: Settings2, view: GeneralSection },
  { id: "ai", icon: Bot, view: AiSection },
  { id: "speech", icon: Mic, view: SpeechSection },
  { id: "voice", icon: Volume2, view: VoiceSection },
  { id: "language", icon: Languages, view: LanguageSection },
  { id: "dictation", icon: PenLine, view: DictationSection },
  { id: "mcp", icon: Plug, view: McpSection },
  { id: "privacy", icon: Shield, view: PrivacySection },
  { id: "appearance", icon: Palette, view: AppearanceSection },
  { id: "shortcuts", icon: Keyboard, view: ShortcutsSection },
  { id: "diagnostics", icon: Activity, view: DiagnosticsSection },
];

function sectionFromHash(): string {
  const id = location.hash.replace(/^#\/settings\/?/, "").split("/")[0];
  return SECTIONS.some((s) => s.id === id) ? id : "general";
}

export function SettingsApp() {
  const [section, setSection] = useState(sectionFromHash);

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
