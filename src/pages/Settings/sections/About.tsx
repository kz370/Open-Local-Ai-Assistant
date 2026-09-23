import { useEffect, useState, type ComponentType } from "react";
import { AudioLines, FolderOpen, Globe, Languages, MessageSquare, PenLine, ShieldCheck } from "lucide-react";
import { ipc } from "../../../app/ipc";
import { t } from "../../../app/strings";
import type { AppInfo } from "../../../app/types";
import { Card, SectionHeader } from "../../../components/settings/layout";

const FEATURES: { key: string; icon: ComponentType<{ size?: number }> }[] = [
  { key: "chat", icon: MessageSquare },
  { key: "voice", icon: AudioLines },
  { key: "dictation", icon: PenLine },
  { key: "languages", icon: Languages },
  { key: "tools", icon: Globe },
];

export function AboutSection() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    void ipc.appInfo().then(setInfo).catch(() => {});
  }, []);

  return (
    <>
      <SectionHeader title={t("settings.sections.about")} />
      <Card>
        <div className="about-hero">
          <span className="brand-mark about-logo" aria-hidden>
            <AudioLines size={24} />
          </span>
          <div style={{ minWidth: 0 }}>
            <div className="about-name">
              {t("app.name")}
              {info && <span className="about-version">{t("settings.about.version", { v: info.version })}</span>}
            </div>
            <div className="row-hint">{t("settings.about.tagline")}</div>
          </div>
        </div>
        <p className="about-intro">{t("settings.about.intro")}</p>
      </Card>

      <Card title={t("settings.about.whatTitle")}>
        <ul className="about-features">
          {FEATURES.map(({ key, icon: Icon }) => (
            <li key={key}>
              <span className="about-feature-icon" aria-hidden>
                <Icon size={16} />
              </span>
              <div>
                <div className="about-feature-title">{t(`settings.about.${key}`)}</div>
                <div className="row-hint">{t(`settings.about.${key}Hint`)}</div>
              </div>
            </li>
          ))}
        </ul>
      </Card>

      <Card title={t("settings.about.privacyTitle")}>
        <div className="about-privacy">
          <ShieldCheck size={16} aria-hidden />
          <p>{t("settings.about.privacy")}</p>
        </div>
        {info && (
          <div className="about-data">
            <div className="about-feature-title">{t("settings.about.dataFolder")}</div>
            <div className="about-path-row">
              <code className="about-path" title={info.paths.dataDir}>
                {info.paths.dataDir}
              </code>
              <button className="btn btn-sm" onClick={() => void ipc.openFolder("data")}>
                <FolderOpen size={12} /> {t("settings.about.openFolder")}
              </button>
            </div>
          </div>
        )}
      </Card>
    </>
  );
}
