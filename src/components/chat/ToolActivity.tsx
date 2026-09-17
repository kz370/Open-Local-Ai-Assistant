import { CheckCircle2, CircleSlash, Globe, Search, ShieldAlert, Wrench, XCircle } from "lucide-react";
import type { UiToolActivity } from "../../app/chatStore";
import { t } from "../../app/strings";

function label(a: UiToolActivity): string {
  const tool = a.toolName;
  if (a.status === "failed") return t("tools.failed", { tool });
  if (a.status === "denied") return t("tools.denied", { tool });
  const done = a.status === "done";
  switch (a.category) {
    case "search":
      return done ? t("tools.searchDone") : t("tools.searching");
    case "fetch":
      return done ? t("tools.readDone") : t("tools.reading");
    case "read":
      return done ? t("tools.readFilesDone") : t("tools.readingFiles");
    default:
      return done ? t("tools.workingDone", { tool }) : t("tools.working", { tool });
  }
}

function Icon({ a }: { a: UiToolActivity }) {
  if (a.status === "running" || a.status === "awaiting") {
    if (a.category === "search") return <Search size={13} aria-hidden />;
    if (a.category === "fetch") return <Globe size={13} aria-hidden />;
    if (a.status === "awaiting") return <ShieldAlert size={13} aria-hidden />;
    return <Wrench size={13} aria-hidden />;
  }
  if (a.status === "done") return <CheckCircle2 size={13} className="ok" aria-hidden />;
  if (a.status === "denied") return <CircleSlash size={13} aria-hidden />;
  return <XCircle size={13} className="fail" aria-hidden />;
}

export function ToolActivity({ tools, developer }: { tools: UiToolActivity[]; developer: boolean }) {
  if (!tools.length) return null;
  return (
    <div className="tools" aria-live="polite">
      {tools.map((a) => (
        <div key={a.callId}>
          <div className="tool-line">
            <Icon a={a} />
            <span>{label(a)}</span>
            {a.status === "running" && <span className="spinner" style={{ width: 10, height: 10 }} aria-hidden />}
          </div>
          {developer && (
            <details className="tool-dev">
              <summary>
                {a.serverName} · {a.toolName}
                {a.durationMs !== undefined ? ` · ${a.durationMs} ms` : ""}
              </summary>
              <dl>
                <dt>{t("tools.dev.server")}</dt>
                <dd>{a.serverName}</dd>
                <dt>{t("tools.dev.tool")}</dt>
                <dd>{a.toolName}</dd>
                <dt>{t("tools.dev.args")}</dt>
                <dd>{JSON.stringify(a.args, null, 2)}</dd>
                {a.resultPreview !== undefined && (
                  <>
                    <dt>{t("tools.dev.result")}</dt>
                    <dd>{a.resultPreview}</dd>
                  </>
                )}
                {a.durationMs !== undefined && (
                  <>
                    <dt>{t("tools.dev.time")}</dt>
                    <dd>{a.durationMs} ms</dd>
                  </>
                )}
              </dl>
            </details>
          )}
        </div>
      ))}
    </div>
  );
}
