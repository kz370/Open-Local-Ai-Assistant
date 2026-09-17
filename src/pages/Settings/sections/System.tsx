import { useEffect, useState } from "react";
import { AlertTriangle, CheckCircle2, Copy, FolderOpen, Info, RefreshCw, XCircle } from "lucide-react";
import { ipc } from "../../../app/ipc";
import { errorMessage, formatBytes, t } from "../../../app/strings";
import type { AppInfo, CapabilityReport, PrivacyStatus } from "../../../app/types";
import { Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { useS } from "./Basic";

type Level = "ok" | "warn" | "err" | "info";

export function CheckItem({ level, label, detail }: { level: Level; label: string; detail?: string | null }) {
  const Icon = level === "ok" ? CheckCircle2 : level === "warn" ? AlertTriangle : level === "err" ? XCircle : Info;
  return (
    <li>
      <Icon size={16} className={`ic ${level}`} aria-hidden />
      <span>
        <span className="sr-only">{level === "ok" ? "OK" : level}: </span>
        {label}
        {detail && <small>{detail}</small>}
      </span>
    </li>
  );
}

export function PrivacySection() {
  const [s, set] = useS();
  const [p, setP] = useState<PrivacyStatus | null>(null);
  useEffect(() => void ipc.privacyStatus().then(setP), []);
  return (
    <>
      <SectionHeader title={t("settings.sections.privacy")} />
      <Card title={t("settings.privacy.title")}>
        <ul className="check-list" style={{ padding: "12px 0" }}>
          <CheckItem level="ok" label={t("settings.privacy.llm")} detail={p?.llmServer} />
          {p && !p.llmIsLocalAddress && <CheckItem level="warn" label={t("settings.privacy.remoteServer")} />}
          <CheckItem level="ok" label={t("settings.privacy.stt")} />
          <CheckItem level="ok" label={t("settings.privacy.tts")} />
          <CheckItem level="ok" label={t("settings.privacy.conversations")} />
          <CheckItem level="ok" label={t("settings.privacy.settings")} />
          <CheckItem level="ok" label={t("settings.privacy.noCloudLlm")} />
          <CheckItem level="ok" label={t("settings.privacy.noCloudSpeech")} />
          <CheckItem level="ok" label={t("settings.privacy.noTelemetry")} />
        </ul>
      </Card>
      <Card title={t("settings.privacy.internet")}>
        <ul className="check-list" style={{ padding: "12px 0" }}>
          {p?.internetViaMcp ? (
            <CheckItem level="info" label={t("settings.privacy.internetVia", { servers: p.internetServers.join(", ") })} detail={t("settings.privacy.llmStaysLocal")} />
          ) : (
            <CheckItem level="ok" label={t("settings.privacy.internetNone")} />
          )}
        </ul>
      </Card>
      <Card>
        <Row label={t("settings.privacy.logContent")} hint={t("settings.privacy.logContentHint")} htmlFor="sw-logc">
          <Switch id="sw-logc" label={t("settings.privacy.logContent")} checked={s.general.logConversationContent} onChange={(v) => set((d) => void (d.general.logConversationContent = v))} />
        </Row>
      </Card>
    </>
  );
}

export function DiagnosticsSection() {
  const [report, setReport] = useState<CapabilityReport | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [log, setLog] = useState("");
  const [scanning, setScanning] = useState(false);
  const [copied, setCopied] = useState(false);

  const scan = async () => {
    setScanning(true);
    try {
      const [r, i, l] = await Promise.all([ipc.scanCapabilities(), ipc.appInfo(), ipc.readLogTail(150).catch(() => "")]);
      setReport(r);
      setInfo(i);
      setLog(l);
    } finally {
      setScanning(false);
    }
  };
  useEffect(() => void scan(), []);

  const hw = report?.hardware;
  const lm = report?.lmStudio;
  const v = report?.voice;

  return (
    <>
      <SectionHeader title={t("settings.sections.diagnostics")} />
      <Card
        title={t("settings.diagnostics.scan")}
        actions={
          <>
            <button className="btn btn-sm" onClick={() => void scan()} disabled={scanning}>
              <RefreshCw size={12} /> {t("settings.diagnostics.scan")}
            </button>
            <button
              className="btn btn-sm"
              disabled={!report}
              onClick={() => {
                void navigator.clipboard.writeText(JSON.stringify({ version: info?.version, os: info?.os, report }, null, 2));
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              }}
            >
              <Copy size={12} /> {copied ? t("app.copied") : t("settings.diagnostics.copyReport")}
            </button>
          </>
        }
      >
        {!report ? (
          <p className="row-hint" style={{ padding: "12px 0" }}>
            <span className="spinner" style={{ display: "inline-block", verticalAlign: "middle" }} /> {t("setup.scanning")}
          </p>
        ) : (
          <ul className="check-list" style={{ padding: "12px 0" }}>
            <CheckItem level={lm?.connected ? "ok" : "err"} label={`${t("settings.diagnostics.lmStudio")}: ${lm?.connected ? t("status.connected") : t("status.disconnected")}`} detail={lm?.connected ? lm.serverUrl : lm?.errorCode ? errorMessage(lm.errorCode) : null} />
            <CheckItem level={lm?.selection ? "ok" : lm?.connected ? "warn" : "err"} label={`${t("settings.diagnostics.localLlm")}: ${lm?.selection?.modelId ?? t("status.notInstalled")}`} detail={lm?.loadedModels.length ? `${t("settings.ai.loaded")}: ${lm.loadedModels.join(", ")}` : null} />
            <CheckItem level={v?.sttModel ? "ok" : "warn"} label={`${t("settings.diagnostics.stt")}: ${v?.sttModel ?? t("status.notInstalled")}`} />
            <CheckItem level={v?.vadReady ? "ok" : "warn"} label={`${t("settings.diagnostics.vad")}: ${v?.vadReady ? t("status.ready") : t("status.notInstalled")}`} />
            <CheckItem
              level={v?.ttsEn && v?.ttsAr && v?.ttsDe ? "ok" : v?.ttsEn || v?.ttsAr || v?.ttsDe ? "warn" : "warn"}
              label={`${t("settings.diagnostics.tts")}: EN ${v?.ttsEn ?? "—"} · AR ${v?.ttsAr ?? "—"} · DE ${v?.ttsDe ?? "—"}`}
              detail={t("settings.diagnostics.acceleration")}
            />
            <CheckItem level={report.microphones.length ? "ok" : "err"} label={`${t("settings.diagnostics.microphone")}: ${report.microphones.find((m) => m.isDefault)?.name ?? report.microphones[0]?.name ?? t("setup.microphoneMissing")}`} />
            <CheckItem level={report.audioOutputs.length ? "ok" : "err"} label={`${t("settings.diagnostics.speakers")}: ${report.audioOutputs.find((m) => m.isDefault)?.name ?? report.audioOutputs[0]?.name ?? t("setup.speakersMissing")}`} />
            <CheckItem level={report.mcp.enabled === 0 ? "info" : report.mcp.connected === report.mcp.enabled ? "ok" : "warn"} label={`${t("settings.diagnostics.mcp")}: ${t("settings.diagnostics.mcpSummary", { connected: report.mcp.connected, enabled: report.mcp.enabled })}`} />
          </ul>
        )}
      </Card>
      {hw && (
        <Card title={t("settings.diagnostics.hardware")}>
          <Row label={t("settings.diagnostics.os")}>{hw.os}</Row>
          <Row label={t("settings.diagnostics.cpu")}>
            {hw.cpuName} · {hw.physicalCores}C/{hw.logicalCores}T
          </Row>
          <Row label={t("settings.diagnostics.ram")}>{formatBytes(hw.totalRamBytes)}</Row>
          <Row label={t("settings.diagnostics.gpu")}>{hw.gpus.length ? hw.gpus.map((g) => `${g.name} (${formatBytes(g.vramBytes)})`).join(", ") : t("settings.diagnostics.noGpu")}</Row>
          {info && <Row label={t("settings.diagnostics.version")}>{info.version}</Row>}
        </Card>
      )}
      <Card
        title={t("settings.diagnostics.logTail")}
        actions={
          <>
            <button className="btn btn-sm" onClick={() => void ipc.openFolder("logs")}>
              <FolderOpen size={12} /> {t("settings.diagnostics.openLogs")}
            </button>
            <button className="btn btn-sm" onClick={() => void ipc.openFolder("data")}>
              <FolderOpen size={12} /> {t("settings.diagnostics.openData")}
            </button>
          </>
        }
      >
        <div style={{ padding: "12px 0" }}>
          <div className="log-view" tabIndex={0} aria-label={t("settings.diagnostics.logTail")}>
            {log || "—"}
          </div>
        </div>
      </Card>
    </>
  );
}
