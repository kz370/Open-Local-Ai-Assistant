import { useEffect, useMemo, useState } from "react";
import { CheckCircle2, FolderOpen, Trash2 } from "lucide-react";
import { ipc, on, toAppError } from "../../app/ipc";
import { formatBytes, languageName, t } from "../../app/strings";
import type { AppErrorPayload, CatalogEntry, DownloadProgress, InstalledModel } from "../../app/types";
import { ErrorNotice } from "../common/controls";
import { Card } from "./layout";

export function useModelCatalog() {
  const [catalog, setCatalog] = useState<CatalogEntry[]>([]);
  const [installed, setInstalled] = useState<InstalledModel[]>([]);
  const [progress, setProgress] = useState<Record<string, DownloadProgress>>({});
  const refresh = async () => {
    const [c, i] = await Promise.all([ipc.modelsCatalog(), ipc.modelsInstalled()]);
    setCatalog(c);
    setInstalled(i);
  };
  useEffect(() => {
    void refresh();
    const subs = [
      on<DownloadProgress>("models://download", (p) => {
        setProgress((prev) => ({ ...prev, [p.modelId]: p }));
        if (p.state === "done" || p.state === "error" || p.state === "cancelled") void refresh();
      }),
      on("models://changed", () => void refresh()),
    ];
    return () => subs.forEach((s) => void s.then((un) => un()));
  }, []);
  return { catalog, installed, progress, refresh };
}

function langs(l: string[]) {
  return l.includes("*") ? t("settings.models.languagesAll") : l.map(languageName).join(", ");
}

export function ModelManager({ kinds, title, languageFilter }: { kinds: ("stt" | "vad" | "tts")[]; title: string; languageFilter?: string }) {
  const { catalog, installed, progress, refresh } = useModelCatalog();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState<AppErrorPayload | null>(null);

  const matchesLanguage = (languages: string[]) => !languageFilter || languages.includes("*") || languages.includes(languageFilter);
  const entries = useMemo(() => catalog.filter((c) => kinds.includes(c.kind) && matchesLanguage(c.languages)), [catalog, kinds, languageFilter]); // eslint-disable-line react-hooks/exhaustive-deps
  const custom = installed.filter((m) => m.source === "custom" && kinds.includes(m.kind) && matchesLanguage(m.languages));

  useEffect(() => {
    // Preselect recommended models that are missing and not already queued.
    setSelected(new Set(entries.filter((e) => e.recommended && !e.installed && !e.downloading).map((e) => e.id)));
  }, [entries.map((e) => `${e.id}:${e.installed}:${e.downloading}`).join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  const selectedSize = entries.filter((e) => selected.has(e.id)).reduce((a, e) => a + e.downloadBytes, 0);

  const download = async () => {
    setError(null);
    const ids = [...selected];
    setSelected(new Set()); // the rows become "queued" and cannot be picked again
    try {
      await ipc.modelsDownload(ids);
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  return (
    <Card
      title={title}
      actions={
        <button className="btn btn-sm" onClick={() => void ipc.openFolder("models")}>
          <FolderOpen size={13} /> {t("settings.models.openFolder")}
        </button>
      }
    >
      <p className="row-hint" style={{ margin: "10px 0 4px" }}>
        {t("settings.models.consent")}
      </p>
      {error && <ErrorNotice error={error} />}
      <div className="model-list">
        {entries.map((e) => {
          const p = progress[e.id];
          const busy = e.downloading || (p && ["downloading", "verifying", "extracting"].includes(p.state));
          const active = !!busy && !e.installed;
          const pct = p && p.totalBytes ? Math.min(100, Math.round((p.downloadedBytes / p.totalBytes) * 100)) : 0;
          const stateLabel = !active
            ? null
            : p?.state === "downloading"
              ? t("settings.models.downloading", { percent: pct })
              : p?.state === "verifying"
                ? t("settings.models.verifying")
                : p?.state === "extracting"
                  ? t("settings.models.extracting")
                  : t("settings.models.queued");
          return (
            <div className={`model-item${active ? " busy" : ""}`} key={e.id}>
              {e.installed ? (
                <CheckCircle2 size={16} style={{ color: "var(--success)" }} aria-label={t("settings.models.installed")} />
              ) : (
                <input
                  type="checkbox"
                  aria-label={`${t("settings.models.download")} ${e.name}`}
                  checked={selected.has(e.id)}
                  disabled={!!active}
                  onChange={(ev) => {
                    const next = new Set(selected);
                    if (ev.target.checked) next.add(e.id);
                    else next.delete(e.id);
                    setSelected(next);
                  }}
                  style={{ accentColor: "var(--accent)" }}
                />
              )}
              <div style={{ minWidth: 0 }}>
                <div className="model-name">{e.name}</div>
                <div className="model-meta">
                  <span>{langs(e.languages)}</span>
                  <span>·</span>
                  <span>{formatBytes(e.downloadBytes)}</span>
                  {e.recommended && <span className="badge accent">{t("settings.models.recommended")}</span>}
                  {e.installed && <span className="badge ok">{t("settings.models.installed")}</span>}
                  {p?.state === "error" && <span className="badge err" title={p.error ?? ""}>{t("errors.download")}</span>}
                </div>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                {active && (
                  <>
                    <span className="row-hint" style={{ margin: 0 }}>
                      {stateLabel}
                    </span>
                    <div className="progress" aria-hidden>
                      <span style={{ width: `${pct}%` }} />
                    </div>
                    <button className="btn btn-sm" onClick={() => void ipc.modelsCancel(e.id)}>
                      {t("settings.models.cancel")}
                    </button>
                  </>
                )}
                {e.installed && e.deletable && (
                  <button className="icon-btn danger" aria-label={`${t("settings.models.delete")} ${e.name}`} title={t("settings.models.delete")} onClick={() => void ipc.modelsDelete(e.id).then(refresh)}>
                    <Trash2 size={14} />
                  </button>
                )}
              </div>
            </div>
          );
        })}
        {custom.map((m) => (
          <div className="model-item" key={m.id}>
            <CheckCircle2 size={16} style={{ color: "var(--success)" }} aria-hidden />
            <div>
              <div className="model-name">{m.name}</div>
              <div className="model-meta">
                <span className="badge">{t("settings.models.custom")}</span>
                <span>{langs(m.languages)}</span>
              </div>
            </div>
            <span />
          </div>
        ))}
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, padding: "10px 0" }}>
        <span className="row-hint">{t("settings.models.customHint")}</span>
        <button className="btn btn-primary" disabled={!selected.size} onClick={() => void download()}>
          {t("settings.models.downloadSelected", { size: formatBytes(selectedSize) })}
        </button>
      </div>
    </Card>
  );
}
