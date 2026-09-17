import { ShieldAlert } from "lucide-react";
import { useChat } from "../../app/chatStore";
import { t } from "../../app/strings";
import { Dialog } from "../common/controls";

export function ToolConfirmDialog() {
  const c = useChat((s) => s.confirmation);
  const confirm = useChat((s) => s.confirmTool);
  if (!c) return null;
  const warning = c.category === "execute" ? t("tools.confirmExecute") : t("tools.confirmWrite");
  return (
    <Dialog
      title={t("tools.confirmTitle")}
      onClose={() => confirm(false)}
      actions={
        <>
          <button className="btn" onClick={() => confirm(false)} autoFocus>
            {t("tools.deny")}
          </button>
          <button className="btn btn-primary" onClick={() => confirm(true)}>
            {t("tools.allowOnce")}
          </button>
        </>
      }
    >
      <p style={{ margin: "4px 0 10px" }}>{t("tools.confirmBody", { tool: c.toolName, server: c.serverName })}</p>
      <div className="notice warn">
        <ShieldAlert size={16} aria-hidden style={{ flex: "none", color: "var(--warning)" }} />
        <span>{warning}</span>
      </div>
      <details className="tech" style={{ marginTop: 10 }} open>
        <summary>{t("tools.confirmArgs")}</summary>
        <pre>{JSON.stringify(c.args, null, 2)}</pre>
      </details>
    </Dialog>
  );
}
