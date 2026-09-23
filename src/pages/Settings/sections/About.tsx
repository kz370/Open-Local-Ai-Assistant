import { useEffect, useState } from "react";
import { AudioLines } from "lucide-react";
import { ipc } from "../../../app/ipc";
import { t } from "../../../app/strings";
import type { AppInfo } from "../../../app/types";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";

export function AboutSection() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    void ipc.appInfo().then(setInfo).catch(() => {});
  }, []);

  return (
    <>
      <SectionHeader title={t("settings.sections.about")} />
      <Card>
        <div style={{ display: "flex", alignItems: "center", gap: 14, padding: "14px 0" }}>
          <span className="brand-mark" aria-hidden style={{ width: 44, height: 44, borderRadius: 12 }}>
            <AudioLines size={22} />
          </span>
          <div>
            <div style={{ fontSize: 16, fontWeight: 600 }}>{t("app.name")}</div>
            <div className="row-hint">{t("settings.about.tagline")}</div>
          </div>
        </div>
        <Row label={t("settings.about.version")}>{info?.version ?? "—"}</Row>
        {info && <Row label={t("settings.about.dataFolder")}>{info.paths.dataDir}</Row>}
      </Card>
      <p className="row-hint">{t("settings.about.privacy")}</p>
    </>
  );
}
