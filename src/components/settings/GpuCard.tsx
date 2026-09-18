import { useEffect, useState } from "react";
import { Cpu, Download, Trash2, Zap } from "lucide-react";
import { ipc, on, toAppError } from "../../app/ipc";
import { formatBytes, t } from "../../app/strings";
import type { AppErrorPayload, GpuProgress, GpuStatus } from "../../app/types";
import { Dialog, ErrorNotice, Switch } from "../common/controls";
import { Card, Row } from "./layout";

/**
 * GPU acceleration for the local speech models. The CUDA libraries are a
 * separate download because they are far larger than the app itself, so this
 * card owns the whole lifecycle: install, switch on, restart, remove.
 */
export function GpuCard() {
  const [status, setStatus] = useState<GpuStatus | null>(null);
  const [progress, setProgress] = useState<GpuProgress | null>(null);
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const refresh = () => ipc.gpuStatus().then(setStatus, (e) => setError(toAppError(e)));
  useEffect(() => {
    void refresh();
    const subs = [on("gpu://changed", () => void refresh()), on<GpuProgress>("gpu://download", setProgress)];
    return () => void subs.forEach((s) => void s.then((u) => u()));
  }, []);

  const busy = progress?.state === "downloading" || progress?.state === "extracting";
  const percent = progress && progress.totalBytes > 0 ? Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100)) : 0;

  const install = async () => {
    setError(null);
    setProgress({ state: "downloading", downloadedBytes: 0, totalBytes: status?.downloadBytes ?? 0, error: null });
    try {
      await ipc.gpuInstall();
    } catch (e) {
      setProgress(null);
      setError(toAppError(e));
    }
  };

  const remove = async () => {
    setError(null);
    setConfirmRemove(false);
    try {
      await ipc.gpuRemove();
      setProgress(null);
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  const setEnabled = async (v: boolean) => {
    try {
      await ipc.gpuSetEnabled(v);
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  return (
    <Card
      title={t("settings.gpu.title")}
      actions={
        status?.installed ? (
          <button className="btn btn-sm" onClick={() => setConfirmRemove(true)} disabled={busy}>
            <Trash2 size={13} /> {t("settings.gpu.remove")}
          </button>
        ) : busy ? (
          <button className="btn btn-sm" onClick={() => void ipc.gpuCancel()}>
            {t("app.cancel")}
          </button>
        ) : (
          <button className="btn btn-sm btn-primary" onClick={() => void install()} disabled={!status?.supported}>
            <Download size={13} /> {t("settings.gpu.install", { size: formatBytes(status?.downloadBytes ?? 0) })}
          </button>
        )
      }
    >
      <p className="row-hint" style={{ margin: "10px 0 6px" }}>
        {t("settings.gpu.intro")}
      </p>
      <Row label={t("settings.gpu.state")}>
        <span className="badge">
          {status?.active ? (
            <>
              <Zap size={12} /> {t("settings.gpu.onGpu", { gpu: status.gpuName ?? "GPU" })}
            </>
          ) : (
            <>
              <Cpu size={12} /> {t("settings.gpu.onCpu")}
            </>
          )}
        </span>
      </Row>
      {!status?.supported && <p className="row-hint">{t("settings.gpu.noNvidia")}</p>}
      {status?.installed && (
        <Row label={t("settings.gpu.enable")} hint={t("settings.gpu.enableHint")} htmlFor="sw-gpu">
          <Switch id="sw-gpu" label={t("settings.gpu.enable")} checked={status.restartRequired || status.active} onChange={(v) => void setEnabled(v)} />
        </Row>
      )}
      {status?.installed && <p className="row-hint">{t("settings.gpu.installed", { size: formatBytes(status.sizeBytes) })}</p>}
      {status?.restartRequired && <p className="row-hint">{t("settings.gpu.restart")}</p>}
      {busy && (
        <div style={{ marginTop: 10 }}>
          <div className="progress">
            <span style={{ width: `${percent}%` }} />
          </div>
          <p className="row-hint">
            {progress?.state === "extracting"
              ? t("settings.gpu.extracting")
              : t("settings.gpu.downloading", { done: formatBytes(progress?.downloadedBytes ?? 0), total: formatBytes(progress?.totalBytes ?? 0) })}
          </p>
        </div>
      )}
      {progress?.state === "error" && progress.error && <p className="row-hint">{progress.error}</p>}
      {error && <ErrorNotice error={error} actions={<button className="btn btn-sm" onClick={() => setError(null)}>{t("app.close")}</button>} />}
      {confirmRemove && (
        <Dialog
          title={t("settings.gpu.remove")}
          onClose={() => setConfirmRemove(false)}
          actions={
            <>
              <button className="btn" onClick={() => setConfirmRemove(false)}>
                {t("app.cancel")}
              </button>
              <button className="btn btn-primary" onClick={() => void remove()}>
                {t("settings.gpu.remove")}
              </button>
            </>
          }
        >
          <p>{t("settings.gpu.removeConfirm", { size: formatBytes(status?.sizeBytes ?? 0) })}</p>
        </Dialog>
      )}
    </Card>
  );
}
