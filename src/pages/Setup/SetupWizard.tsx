import { useEffect, useMemo, useState } from "react";
import { AudioLines } from "lucide-react";
import { ipc, on } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { formatBytes, t } from "../../app/strings";
import type { CapabilityReport, CatalogEntry, DownloadProgress, LangSetting } from "../../app/types";
import { prettyAccelerator } from "../../components/settings/ShortcutInput";
import { CheckItem } from "../Settings/sections/System";

type Step = "scan" | "language" | "voice" | "ready";

export function SetupWizard({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<Step>("scan");
  const [report, setReport] = useState<CapabilityReport | null>(null);
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [progress, setProgress] = useState<Record<string, DownloadProgress>>({});
  const [downloading, setDownloading] = useState(false);
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);

  const scan = async () => {
    setReport(null);
    const [r, c] = await Promise.all([ipc.scanCapabilities(), ipc.modelsCatalog()]);
    setReport(r);
    setCatalog(c);
  };

  useEffect(() => {
    void scan();
    const sub = on<DownloadProgress>("models://download", (p) => setProgress((prev) => ({ ...prev, [p.modelId]: p })));
    return () => void sub.then((u) => u());
  }, []);

  const missing = useMemo(() => catalog.filter((c) => c.recommended && !c.installed), [catalog]);
  const missingSize = missing.reduce((a, c) => a + c.downloadBytes, 0);
  const allDone = downloading && missing.every((m) => ["done", "error", "cancelled"].includes(progress[m.id]?.state ?? ""));
  const totalProgress = missing.length ? missing.reduce((a, m) => a + (progress[m.id]?.state === "done" ? m.downloadBytes : progress[m.id]?.downloadedBytes ?? 0), 0) / Math.max(1, missingSize) : 0;

  useEffect(() => {
    if (allDone) setStep("ready");
  }, [allDone]);

  const finish = async () => {
    await ipc.completeFirstRun();
    onDone();
  };

  const lm = report?.lmStudio;
  const v = report?.voice;

  return (
    <div className="setup">
      <div data-tauri-drag-region style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <span className="brand-mark" style={{ width: 28, height: 28, borderRadius: 8 }} aria-hidden>
          <AudioLines size={16} />
        </span>
        <div data-tauri-drag-region>
          <h1 data-tauri-drag-region>{t("setup.welcome")}</h1>
          <p>{t("setup.intro")}</p>
        </div>
      </div>

      {step === "scan" && (
        <>
          <div className="setup-card" aria-live="polite">
            {!report ? (
              <p>
                <span className="spinner" style={{ display: "inline-block", verticalAlign: "middle", marginInlineEnd: 8 }} />
                {t("setup.scanning")}
              </p>
            ) : (
              <ul className="check-list">
                <CheckItem level={lm?.connected ? "ok" : "err"} label={lm?.connected ? t("setup.lmStudio") : t("setup.lmStudioMissing")} detail={lm?.connected ? null : t("setup.lmStudioHint")} />
                <CheckItem level={lm?.selection ? "ok" : "warn"} label={lm?.selection ? `${t("setup.model")}: ${lm.selection.modelId}` : t("setup.modelMissing")} />
                <CheckItem level={v?.sttModel ? "ok" : "warn"} label={v?.sttModel ? t("setup.stt") : t("setup.sttMissing")} />
                <CheckItem level={v?.ttsEn || v?.ttsAr || v?.ttsDe ? "ok" : "warn"} label={v?.ttsEn || v?.ttsAr || v?.ttsDe ? t("setup.tts") : t("setup.ttsMissing")} />
                <CheckItem level={report.microphones.length ? "ok" : "warn"} label={report.microphones.length ? t("setup.microphone") : t("setup.microphoneMissing")} />
                <CheckItem level={report.audioOutputs.length ? "ok" : "warn"} label={report.audioOutputs.length ? t("setup.speakers") : t("setup.speakersMissing")} />
              </ul>
            )}
          </div>
          <div className="setup-actions">
            <button className="btn" onClick={() => void scan()} disabled={!report}>
              {t("app.retry")}
            </button>
            <button className="btn btn-primary" onClick={() => setStep("language")} disabled={!report}>
              {t("app.continue")}
            </button>
          </div>
        </>
      )}

      {step === "language" && settings && (
        <>
          <div className="setup-card">
            <h2 style={{ margin: "0 0 4px", fontSize: "var(--text-md)" }}>{t("setup.languageTitle")}</h2>
            <p style={{ marginBottom: 10 }}>{t("settings.language.detectHint")}</p>
            <div className="radio-list" role="radiogroup" aria-label={t("setup.languageTitle")}>
              {(["auto", "en", "ar", "de"] as LangSetting[]).map((l) => (
                <label key={l}>
                  <input type="radio" name="setup-lang" checked={settings.language.responseLanguage === l} onChange={() => void update((d) => void (d.language.responseLanguage = l))} />
                  {l === "auto" ? t("app.automatic") : t(`languages.${l}`)}
                </label>
              ))}
            </div>
          </div>
          <div className="setup-actions">
            <button className="btn" onClick={() => setStep("scan")}>
              {t("app.back")}
            </button>
            <button className="btn btn-primary" onClick={() => setStep(missing.length ? "voice" : "ready")}>
              {t("app.continue")}
            </button>
          </div>
        </>
      )}

      {step === "voice" && (
        <>
          <div className="setup-card">
            <h2 style={{ margin: "0 0 4px", fontSize: "var(--text-md)" }}>{t("setup.voiceTitle")}</h2>
            <p style={{ marginBottom: 10 }}>{t("setup.voiceIntro")}</p>
            <ul className="check-list">
              {missing.map((m) => {
                const p = progress[m.id];
                const pct = p?.totalBytes ? Math.round((p.downloadedBytes / p.totalBytes) * 100) : 0;
                return (
                  <li key={m.id}>
                    <span style={{ flex: 1 }}>
                      {m.name}
                      <small>
                        {formatBytes(m.downloadBytes)}
                        {p && ` · ${p.state === "downloading" ? `${pct}%` : p.state === "done" ? t("settings.models.installed") : p.state === "error" ? t("errors.download") : t(`settings.models.${p.state === "extracting" ? "extracting" : "verifying"}`)}`}
                      </small>
                    </span>
                  </li>
                );
              })}
            </ul>
            {downloading && (
              <div className="progress" style={{ width: "100%", marginTop: 12 }} role="progressbar" aria-valuenow={Math.round(totalProgress * 100)} aria-valuemin={0} aria-valuemax={100}>
                <span style={{ width: `${Math.round(totalProgress * 100)}%` }} />
              </div>
            )}
            <p className="row-hint" style={{ marginTop: 10 }}>
              {t("settings.models.consent")}
            </p>
          </div>
          <div className="setup-actions">
            <button className="btn" onClick={() => setStep("ready")} disabled={downloading && !allDone}>
              {t("setup.voiceSkip")}
            </button>
            <button
              className="btn btn-primary"
              disabled={downloading}
              onClick={() => {
                setDownloading(true);
                void ipc.modelsDownload(missing.map((m) => m.id));
              }}
            >
              {t("setup.voiceDownload", { size: formatBytes(missingSize) })}
            </button>
          </div>
        </>
      )}

      {step === "ready" && (
        <>
          <div className="setup-card" style={{ textAlign: "center", padding: 24 }}>
            <h2 style={{ margin: "0 0 6px", fontSize: "var(--text-lg)" }}>{t("setup.ready")}</h2>
            <p>{t("setup.shortcutHint", { shortcut: prettyAccelerator(settings?.general.globalShortcut ?? "CommandOrControl+Space") })}</p>
          </div>
          <div className="setup-actions">
            <button className="btn btn-primary" onClick={() => void finish()} autoFocus>
              {t("setup.start")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
