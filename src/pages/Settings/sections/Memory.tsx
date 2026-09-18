import { useEffect, useState } from "react";
import { Download, RefreshCw, Upload } from "lucide-react";
import { ipc, on, toAppError } from "../../../app/ipc";
import { languageName, t } from "../../../app/strings";
import type { AppErrorPayload, MemoryItem } from "../../../app/types";
import { ErrorNotice, Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { useS } from "./Basic";

function rowLabel(item: MemoryItem): string {
  switch (item.kind) {
    case "stt":
      return t("settings.memory.stt");
    case "voice":
      return t("settings.memory.voice", { language: languageName(item.role) });
    case "silma":
      return t("settings.memory.silma");
    default:
      return item.role === "chat" ? t("settings.memory.chat") : t("settings.memory.otherLlm");
  }
}

const BADGE: Record<MemoryItem["state"], string> = { loaded: " ok", loading: "", idle: "", missing: "", failed: " err" };

/** What is in memory right now, with manual load/unload. */
export function MemorySection() {
  const [s, set] = useS();
  const [items, setItems] = useState<MemoryItem[] | null>(null);
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [error, setError] = useState<AppErrorPayload | null>(null);

  const refresh = () => ipc.memoryStatus().then(setItems, (e) => setError(toAppError(e)));
  useEffect(() => {
    void refresh();
    // Loads started elsewhere (startup, another window) show up here too.
    const id = window.setInterval(() => void refresh(), 3000);
    const subs = [on("memory://changed", () => void refresh()), on("silma://status", () => void refresh())];
    return () => {
      window.clearInterval(id);
      subs.forEach((u) => void u.then((f) => f()));
    };
  }, []);

  const run = async (key: string, action: "load" | "unload") => {
    setError(null);
    setBusy((b) => new Set(b).add(key));
    try {
      await (action === "load" ? ipc.memoryLoad(key) : ipc.memoryUnload(key));
    } catch (e) {
      setError(toAppError(e));
    } finally {
      setBusy((b) => {
        const n = new Set(b);
        n.delete(key);
        return n;
      });
      void refresh();
    }
  };

  return (
    <>
      <SectionHeader title={t("settings.sections.memory")} intro={t("settings.memory.intro")} />
      {error && (
        <div style={{ maxWidth: 820, marginBottom: 12 }}>
          <ErrorNotice error={error} actions={<button className="btn btn-sm" onClick={() => setError(null)}>{t("app.close")}</button>} />
        </div>
      )}
      <Card>
        <Row label={t("settings.general.preloadModels")} hint={t("settings.general.preloadModelsHint")} htmlFor="sw-preload">
          <Switch id="sw-preload" label={t("settings.general.preloadModels")} checked={s.general.preloadModels} onChange={(v) => set((d) => void (d.general.preloadModels = v))} />
        </Row>
      </Card>
      <Card
        title={t("settings.memory.models")}
        actions={
          <>
            <button className="btn btn-sm" onClick={() => void refresh()} aria-label={t("settings.memory.refresh")} title={t("settings.memory.refresh")}>
              <RefreshCw size={13} />
            </button>
            <button className="btn btn-sm btn-primary" onClick={() => void run("all", "load")}>
              <Download size={13} /> {t("settings.memory.loadAll")}
            </button>
          </>
        }
      >
        {items === null && <p className="row-hint">{t("settings.memory.checking")}</p>}
        {items?.map((item) => {
          const working = busy.has(item.key) || item.state === "loading";
          const canLoad = item.state === "idle" || item.state === "failed";
          const canUnload = item.state === "loaded" && item.key !== "llm";
          return (
            <Row key={item.key} label={rowLabel(item)} hint={item.model || t("settings.memory.none")}>
              <span className={`badge${BADGE[item.state]}`}>
                {working && <span className="spinner" style={{ width: 10, height: 10 }} />} {t(`settings.memory.states.${working ? "loading" : item.state}`)}
                {item.detail && item.state !== "failed" && item.detail !== "silma" ? ` · ${item.detail}` : ""}
              </span>
              {canLoad && item.key !== "llm" && (
                <button className="btn btn-sm" disabled={working} onClick={() => void run(item.key, "load")}>
                  <Download size={13} /> {t("settings.memory.load")}
                </button>
              )}
              {canUnload && (
                <button className="btn btn-sm" disabled={working} onClick={() => void run(item.key, "unload")}>
                  <Upload size={13} /> {t("settings.memory.unload")}
                </button>
              )}
              {item.state === "failed" && item.detail && <span className="row-hint">{item.detail}</span>}
            </Row>
          );
        })}
      </Card>
    </>
  );
}
