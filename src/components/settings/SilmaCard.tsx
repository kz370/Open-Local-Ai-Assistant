import { useEffect, useState } from "react";
import { Download, Trash2, Volume2 } from "lucide-react";
import { ipc, on, toAppError } from "../../app/ipc";
import { formatBytes, t } from "../../app/strings";
import type { AppErrorPayload, SilmaProgress, SilmaStatus } from "../../app/types";
import { Dialog, ErrorNotice } from "../common/controls";
import { Card, Row } from "./layout";

const BUSY_STAGES = ["runtime", "packages", "weights", "prepare"];

/**
 * SILMA, the natural Arabic voice. It runs on PyTorch in a private Python
 * environment, so it is a separate one-time setup; after that the app starts
 * and stops it on its own.
 */
export function SilmaCard() {
  const [status, setStatus] = useState<SilmaStatus | null>(null);
  const [progress, setProgress] = useState<SilmaProgress | null>(null);
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const [confirm, setConfirm] = useState<"install" | "remove" | null>(null);

  const refresh = () => ipc.silmaStatus().then(setStatus, (e) => setError(toAppError(e)));
  useEffect(() => {
    void refresh();
    const subs = [on<SilmaStatus>("silma://status", setStatus), on<SilmaProgress>("silma://progress", setProgress)];
    return () => void subs.forEach((s) => void s.then((u) => u()));
  }, []);

  const busy = !!progress && BUSY_STAGES.includes(progress.stage);
  const percent = progress && progress.totalBytes > 0 ? Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100)) : 0;

  const install = async () => {
    setConfirm(null);
    setError(null);
    setProgress({ stage: "runtime", downloadedBytes: 0, totalBytes: status?.downloadBytes ?? 0, detail: null, error: null });
    try {
      await ipc.silmaInstall();
    } catch (e) {
      setProgress(null);
      setError(toAppError(e));
    }
  };

  const remove = async () => {
    setConfirm(null);
    setError(null);
    try {
      await ipc.silmaRemove();
      setProgress(null);
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  const test = async () => {
    setError(null);
    try {
      await ipc.silmaTest();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  const stateLabel = !status?.installed
    ? t("settings.silma.notInstalled")
    : status.state === "ready"
      ? t("settings.silma.ready", { device: status.device === "cuda" ? "GPU" : "CPU" })
      : status.state === "starting"
        ? t("settings.silma.starting")
        : status.state === "failed"
          ? t("settings.silma.failed")
          : t("settings.silma.idle");

  return (
    <Card
      title={t("settings.silma.title")}
      actions={
        status?.installed && !busy ? (
          <>
            <button className="btn btn-sm" onClick={() => void test()}>
              <Volume2 size={13} /> {t("settings.silma.test")}
            </button>
            <button className="btn btn-sm" onClick={() => setConfirm("remove")}>
              <Trash2 size={13} /> {t("settings.silma.remove")}
            </button>
          </>
        ) : busy ? (
          <button className="btn btn-sm" onClick={() => void ipc.silmaCancel()}>
            {t("app.cancel")}
          </button>
        ) : (
          <button className="btn btn-sm btn-primary" onClick={() => setConfirm("install")}>
            <Download size={13} /> {t("settings.silma.install", { size: formatBytes(status?.downloadBytes ?? 0) })}
          </button>
        )
      }
    >
      <p className="row-hint" style={{ margin: "10px 0 6px" }}>
        {t("settings.silma.intro")}
      </p>
      <Row label={t("settings.silma.state")}>
        <span className={`badge${status?.state === "ready" ? " ok" : status?.state === "failed" ? " err" : ""}`}>{stateLabel}</span>
      </Row>
      {status?.installed && <p className="row-hint">{t("settings.silma.installed", { size: formatBytes(status.sizeBytes) })}</p>}
      {status?.state === "failed" && status.error && <p className="row-hint">{status.error}</p>}
      {busy && (
        <div style={{ marginTop: 10 }}>
          <div className="progress">
            <span style={{ width: `${percent}%` }} />
          </div>
          <p className="row-hint">
            {t(`settings.silma.stages.${progress.stage}`)} {progress.stage === "weights" && t("settings.silma.of", { done: formatBytes(progress.downloadedBytes), total: formatBytes(progress.totalBytes) })}
          </p>
          {progress.detail && (
            <p className="row-hint" style={{ fontFamily: "var(--font-mono, monospace)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
              {progress.detail}
            </p>
          )}
        </div>
      )}
      {progress?.stage === "error" && progress.error && (
        <details className="tech">
          <summary>{t("settings.silma.setupFailed")}</summary>
          <pre>{progress.error}</pre>
        </details>
      )}
      {error && <ErrorNotice error={error} actions={<button className="btn btn-sm" onClick={() => setError(null)}>{t("app.close")}</button>} />}
      {confirm && (
        <Dialog
          title={confirm === "install" ? t("settings.silma.title") : t("settings.silma.remove")}
          onClose={() => setConfirm(null)}
          actions={
            <>
              <button className="btn" onClick={() => setConfirm(null)}>
                {t("app.cancel")}
              </button>
              <button className="btn btn-primary" onClick={() => void (confirm === "install" ? install() : remove())}>
                {confirm === "install" ? t("settings.silma.installShort") : t("settings.silma.remove")}
              </button>
            </>
          }
        >
          <p>
            {confirm === "install"
              ? t(status?.gpu ? "settings.silma.installConfirmGpu" : "settings.silma.installConfirmCpu", { size: formatBytes(status?.downloadBytes ?? 0) })
              : t("settings.silma.removeConfirm", { size: formatBytes(status?.sizeBytes ?? 0) })}
          </p>
        </Dialog>
      )}
    </Card>
  );
}
