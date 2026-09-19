import { useEffect, useState } from "react";
import { Download, Pencil, Plus, RefreshCw, Search, Trash2 } from "lucide-react";
import { ipc, on, toAppError } from "../../../app/ipc";
import { useSettings } from "../../../app/settingsStore";
import { t } from "../../../app/strings";
import type { AppErrorPayload, ImportCandidate, McpServerConfig, Permission, PublicSearxInstance, ServerStatus } from "../../../app/types";
import { Dialog, ErrorNotice, Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";

const EMPTY: McpServerConfig = {
  id: "",
  name: "",
  description: "",
  transport: "stdio",
  command: "",
  args: [],
  env: {},
  url: "",
  headers: {},
  enabled: false,
  source: "user",
  createdAt: "",
};

function stateBadge(s: ServerStatus) {
  const map: Record<ServerStatus["state"], [string, string]> = {
    connected: ["ok", t("status.connected")],
    connecting: ["", t("status.connecting")],
    disabled: ["", t("app.disabled")],
    error: ["err", t("status.error")],
    offline: ["warn", t("status.offline")],
  };
  const [cls, label] = map[s.state];
  return (
    <span className={`badge ${cls}`}>
      <span className={`dot ${cls === "ok" ? "ok" : cls === "err" ? "err" : cls === "warn" ? "warn" : s.state === "connecting" ? "busy" : ""}`} /> {label}
    </span>
  );
}

const linesToList = (v: string) => v.split("\n").map((x) => x.trim()).filter(Boolean);
const linesToMap = (v: string, sep: string) =>
  Object.fromEntries(
    linesToList(v)
      .map((l) => {
        const i = l.indexOf(sep);
        return i > 0 ? [l.slice(0, i).trim(), l.slice(i + 1).trim()] : null;
      })
      .filter((x): x is [string, string] => !!x),
  );

function ServerForm({ initial, onClose }: { initial: McpServerConfig; onClose: () => void }) {
  const [c, setC] = useState(initial);
  const [args, setArgs] = useState(initial.args.join("\n"));
  const [env, setEnv] = useState(Object.entries(initial.env).map(([k, v]) => `${k}=${v}`).join("\n"));
  const [headers, setHeaders] = useState(Object.entries(initial.headers).map(([k, v]) => `${k}: ${v}`).join("\n"));
  const [error, setError] = useState<AppErrorPayload | null>(null);

  const save = async () => {
    try {
      await ipc.mcpSave({ ...c, args: linesToList(args), env: linesToMap(env, "="), headers: linesToMap(headers, ":") });
      onClose();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  return (
    <Dialog
      title={initial.id ? t("settings.mcp.edit") : t("settings.mcp.add")}
      onClose={onClose}
      actions={
        <>
          <button className="btn" onClick={onClose}>
            {t("app.cancel")}
          </button>
          <button className="btn btn-primary" onClick={() => void save()}>
            {t("app.save")}
          </button>
        </>
      }
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 10, marginTop: 8 }}>
        <label>
          <span className="row-label">{t("settings.mcp.name")}</span>
          <input className="input" value={c.name} onChange={(e) => setC({ ...c, name: e.target.value })} />
        </label>
        <label>
          <span className="row-label">{t("settings.mcp.description")}</span>
          <input className="input" value={c.description} onChange={(e) => setC({ ...c, description: e.target.value })} />
        </label>
        <label>
          <span className="row-label">{t("settings.mcp.transport")}</span>
          <select className="select" value={c.transport} onChange={(e) => setC({ ...c, transport: e.target.value as McpServerConfig["transport"] })}>
            <option value="stdio">{t("settings.mcp.stdio")}</option>
            <option value="http">{t("settings.mcp.http")}</option>
          </select>
        </label>
        {c.transport === "stdio" ? (
          <>
            <label>
              <span className="row-label">{t("settings.mcp.command")}</span>
              <input className="input mono" value={c.command ?? ""} placeholder="npx" onChange={(e) => setC({ ...c, command: e.target.value })} spellCheck={false} />
            </label>
            <label>
              <span className="row-label">{t("settings.mcp.args")}</span>
              <textarea className="textarea mono" rows={3} value={args} onChange={(e) => setArgs(e.target.value)} spellCheck={false} />
            </label>
            <label>
              <span className="row-label">{t("settings.mcp.env")}</span>
              <textarea className="textarea mono" rows={3} value={env} onChange={(e) => setEnv(e.target.value)} spellCheck={false} />
            </label>
          </>
        ) : (
          <>
            <label>
              <span className="row-label">{t("settings.mcp.url")}</span>
              <input className="input mono" value={c.url ?? ""} placeholder="https://example.com/mcp" onChange={(e) => setC({ ...c, url: e.target.value })} spellCheck={false} />
            </label>
            <label>
              <span className="row-label">{t("settings.mcp.headers")}</span>
              <textarea className="textarea mono" rows={2} value={headers} onChange={(e) => setHeaders(e.target.value)} spellCheck={false} />
            </label>
          </>
        )}
        <div className="notice warn">{t("settings.mcp.trustWarning")}</div>
        {error && <ErrorNotice error={error} />}
      </div>
    </Dialog>
  );
}

function ImportDialog({ onClose }: { onClose: () => void }) {
  const [candidates, setCandidates] = useState<ImportCandidate[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState<AppErrorPayload | null>(null);
  useEffect(() => {
    ipc.mcpImportPreview().then(setCandidates, (e) => setError(toAppError(e)));
  }, []);
  return (
    <Dialog
      title={t("settings.mcp.importTitle")}
      onClose={onClose}
      actions={
        <>
          <button className="btn" onClick={onClose}>
            {t("app.cancel")}
          </button>
          <button
            className="btn btn-primary"
            disabled={!selected.size}
            onClick={async () => {
              try {
                await ipc.mcpImport([...selected]);
                onClose();
              } catch (e) {
                setError(toAppError(e));
              }
            }}
          >
            {t("settings.mcp.importSelected")}
          </button>
        </>
      }
    >
      <p className="row-hint">{t("settings.mcp.importHint")}</p>
      {error && <ErrorNotice error={error} />}
      {candidates && candidates.length === 0 && <p>{t("settings.mcp.importNone")}</p>}
      <div className="radio-list" style={{ marginTop: 10 }}>
        {candidates?.map((c) => (
          <label key={c.name} style={{ alignItems: "flex-start" }}>
            <input
              type="checkbox"
              disabled={c.alreadyConfigured}
              checked={selected.has(c.name)}
              onChange={(e) => {
                const n = new Set(selected);
                if (e.target.checked) n.add(c.name);
                else n.delete(c.name);
                setSelected(n);
              }}
            />
            <span>
              <strong>{c.name}</strong> {c.alreadyConfigured && <span className="badge">{t("settings.mcp.alreadyAdded")}</span>}
              <span className="server-sub" style={{ display: "block" }}>
                {c.transport === "http" ? c.url : [c.command, ...c.args].join(" ")}
                {c.envKeys.length ? ` · env: ${c.envKeys.join(", ")}` : ""}
              </span>
            </span>
          </label>
        ))}
      </div>
    </Dialog>
  );
}

export function McpSection() {
  const [servers, setServers] = useState<ServerStatus[]>([]);
  const [editing, setEditing] = useState<McpServerConfig | null>(null);
  const [deleting, setDeleting] = useState<ServerStatus | null>(null);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const developer = useSettings((s) => s.settings?.general.developerMode);
  const search = useSettings((s) => s.settings?.search);
  const setSettings = useSettings((s) => s.update);
  const searxngOn = search?.searxngEnabled ?? true;
  const source = search?.searxngSource ?? "local";
  const [instances, setInstances] = useState<PublicSearxInstance[] | null>(null);
  const [instancesError, setInstancesError] = useState<string | null>(null);
  const [loadingInstances, setLoadingInstances] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; text: string } | null>(null);

  const refresh = () => ipc.mcpList().then(setServers, (e) => setError(toAppError(e)));
  useEffect(() => {
    void refresh();
    const sub = on("mcp://changed", () => void refresh());
    return () => void sub.then((u) => u());
  }, []);

  const loadInstances = async (force: boolean) => {
    setLoadingInstances(true);
    setInstancesError(null);
    try {
      setInstances(await ipc.searchPublicInstances(force));
    } catch (e) {
      setInstancesError(toAppError(e).detail);
    } finally {
      setLoadingInstances(false);
    }
  };
  useEffect(() => {
    if (source === "public" && searxngOn && !instances) void loadInstances(false);
  }, [source, searxngOn]);

  const runTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const r = await ipc.searchTest();
      setTestResult({ ok: true, text: t("settings.mcp.searchTestOk", { count: r.count, engine: r.engine }) });
    } catch (e) {
      setTestResult({ ok: false, text: toAppError(e).detail });
    } finally {
      setTesting(false);
    }
  };

  const setPermission = async (serverId: string, tool: string, p: Permission) => {
    try {
      await ipc.mcpSetPermission(serverId, tool, p);
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  return (
    <>
      <SectionHeader title={t("settings.sections.mcp")} intro={t("settings.mcp.intro")} />
      {error && (
        <div style={{ maxWidth: 820, marginBottom: 12 }}>
          <ErrorNotice error={error} actions={<button className="btn btn-sm" onClick={() => setError(null)}>{t("app.close")}</button>} />
        </div>
      )}
      <Card title={t("settings.mcp.builtinSearch")}>
        <Row label={t("settings.mcp.builtinSearch")} hint={t("settings.mcp.builtinSearchHint")} htmlFor="sw-builtin-search">
          <Switch
            id="sw-builtin-search"
            label={t("settings.mcp.builtinSearch")}
            checked={search?.enabled ?? true}
            onChange={(v) => setSettings((x) => void (x.search.enabled = v))}
          />
        </Row>
        <Row label={t("settings.mcp.searxngEnabled")} hint={t("settings.mcp.searxngEnabledHint")} htmlFor="sw-searxng">
          <Switch
            id="sw-searxng"
            label={t("settings.mcp.searxngEnabled")}
            checked={searxngOn}
            onChange={(v) => setSettings((x) => void (x.search.searxngEnabled = v))}
          />
        </Row>
        <Row label={t("settings.mcp.searxngSource")} htmlFor="sel-searxng-source">
          <select
            id="sel-searxng-source"
            className="select"
            disabled={!searxngOn}
            value={source}
            onChange={(e) => setSettings((x) => void (x.search.searxngSource = e.target.value as "local" | "public"))}
          >
            <option value="local">{t("settings.mcp.searxngSourceLocal")}</option>
            <option value="public">{t("settings.mcp.searxngSourcePublic")}</option>
          </select>
        </Row>
        {source === "local" ? (
          <Row label={t("settings.mcp.searxng")} hint={t("settings.mcp.searxngHint")} htmlFor="in-searxng">
            <input
              id="in-searxng"
              className="input"
              disabled={!searxngOn}
              placeholder="http://localhost:8080"
              value={search?.searxngUrl ?? ""}
              onChange={(e) => setSettings((x) => void (x.search.searxngUrl = e.target.value.trim()))}
              spellCheck={false}
            />
          </Row>
        ) : (
          <Row label={t("settings.mcp.searxngPublic")} hint={t("settings.mcp.searxngPublicHint")} htmlFor="sel-searxng-public">
            <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
              <select
                id="sel-searxng-public"
                className="select"
                disabled={!searxngOn}
                value={search?.searxngPublicUrl ?? ""}
                onChange={(e) => setSettings((x) => void (x.search.searxngPublicUrl = e.target.value))}
              >
                <option value="">{t("settings.mcp.searxngAuto")}</option>
                {/* Keep a saved pick visible even when the fresh list dropped it. */}
                {search?.searxngPublicUrl && !instances?.some((i) => i.url === search.searxngPublicUrl) && (
                  <option value={search.searxngPublicUrl}>{search.searxngPublicUrl}</option>
                )}
                {instances?.map((i) => (
                  <option key={i.url} value={i.url}>
                    {i.url.replace(/^https?:\/\//, "").replace(/\/$/, "")}
                    {i.searchTime != null ? ` · ${i.searchTime.toFixed(1)}s` : ""}
                    {` · ${Math.round(i.searchSuccess)}%`}
                  </option>
                ))}
              </select>
              <button
                className="icon-btn"
                aria-label={t("settings.mcp.searxngRefresh")}
                title={t("settings.mcp.searxngRefresh")}
                disabled={!searxngOn || loadingInstances}
                onClick={() => void loadInstances(true)}
              >
                <RefreshCw size={14} className={loadingInstances ? "spin" : undefined} />
              </button>
            </div>
            {instancesError && <p className="row-hint" role="alert">{t("settings.mcp.searxngListError")} {instancesError}</p>}
          </Row>
        )}
        <Row label={t("settings.mcp.searchTest")} hint={t("settings.mcp.searchTestHint")}>
          <button className="btn btn-sm" disabled={!(search?.enabled ?? true) || testing} onClick={() => void runTest()}>
            <Search size={13} /> {testing ? t("settings.mcp.searchTesting") : t("settings.mcp.searchTest")}
          </button>
        </Row>
        {testResult && (
          <div className={`notice ${testResult.ok ? "info" : "err"}`} role="status" style={{ marginTop: 8 }}>
            {testResult.text}
          </div>
        )}
        <Row label={t("settings.mcp.builtinSearchResults")} htmlFor="sel-search-results">
          <select
            id="sel-search-results"
            className="select"
            value={String(search?.maxResults ?? 5)}
            disabled={!(search?.enabled ?? true)}
            onChange={(e) => setSettings((x) => void (x.search.maxResults = Number(e.target.value)))}
          >
            {[3, 5, 8, 10].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </Row>
      </Card>
      <Card
        title={t("settings.mcp.servers")}
        actions={
          <>
            <button className="btn btn-sm" onClick={() => setImporting(true)}>
              <Download size={13} /> {t("settings.mcp.import")}
            </button>
            <button className="btn btn-sm btn-primary" onClick={() => setEditing(EMPTY)}>
              <Plus size={13} /> {t("settings.mcp.add")}
            </button>
          </>
        }
      >
        {servers.length === 0 && <p className="row-hint" style={{ padding: "12px 0" }}>{t("settings.mcp.empty")}</p>}
        {servers.map((s) => (
          <div className="server" key={s.config.id}>
            <div className="server-head">
              <Switch label={`${s.config.name}: ${t("app.enabled")}`} checked={s.config.enabled} onChange={(v) => void ipc.mcpSetEnabled(s.config.id, v).then(refresh)} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                  <span className="server-name">{s.config.name}</span>
                  {stateBadge(s)}
                  {s.internet && s.config.enabled && <span className="badge accent">{t("settings.mcp.internet")}</span>}
                  {s.config.source === "lmstudio" && <span className="badge">LM Studio</span>}
                </div>
                {s.config.description && <div className="row-hint">{s.config.description}</div>}
                <div className="server-sub" title={s.config.transport === "http" ? s.config.url ?? "" : [s.config.command, ...s.config.args].join(" ")}>
                  {s.config.transport === "http" ? s.config.url : [s.config.command, ...s.config.args].join(" ")}
                </div>
              </div>
              {s.config.enabled && (
                <button className="icon-btn" aria-label={t("settings.mcp.reconnect")} title={t("settings.mcp.reconnect")} onClick={() => void ipc.mcpReconnect(s.config.id).then(refresh)}>
                  <RefreshCw size={14} />
                </button>
              )}
              <button className="icon-btn" aria-label={t("settings.mcp.edit")} title={t("settings.mcp.edit")} onClick={() => setEditing(s.config)}>
                <Pencil size={14} />
              </button>
              <button className="icon-btn danger" aria-label={t("app.remove")} title={t("app.remove")} onClick={() => setDeleting(s)}>
                <Trash2 size={14} />
              </button>
            </div>
            {s.error && (s.state === "error" || s.state === "offline") && (
              <details className="tech" style={{ marginTop: 6 }}>
                <summary>{t("errors.details")}</summary>
                <pre>{s.error}</pre>
              </details>
            )}
            {s.config.enabled && (
              <>
                {s.tools.length === 0 ? (
                  <p className="row-hint">{t("settings.mcp.noTools")}</p>
                ) : (
                  <table className="tool-table">
                    <caption className="sr-only">{t("settings.mcp.tools")}</caption>
                    <tbody>
                      {s.tools.map((tool) => {
                        const sensitive = tool.category === "write" || tool.category === "execute" || tool.category === "other";
                        return (
                          <tr key={tool.name}>
                            <td style={{ width: "34%" }}>
                              <div className="mono" style={{ fontSize: "var(--text-xs)" }}>
                                {tool.name}
                              </div>
                              {developer && <div className="tool-desc" title={tool.description}>{tool.description}</div>}
                            </td>
                            <td>
                              <span className={`badge ${tool.category === "execute" ? "err" : sensitive ? "warn" : ""}`}>{t(`settings.mcp.category.${tool.category}`)}</span>
                            </td>
                            <td style={{ width: 170 }}>
                              <select
                                className="select"
                                aria-label={`${tool.name} permission`}
                                value={tool.permission}
                                onChange={(e) => void setPermission(s.config.id, tool.name, e.target.value as Permission)}
                              >
                                <option value="allow" disabled={sensitive}>
                                  {t("settings.mcp.permission.allow")}
                                </option>
                                <option value="ask">{t("settings.mcp.permission.ask")}</option>
                                <option value="deny">{t("settings.mcp.permission.deny")}</option>
                              </select>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                )}
                {s.tools.some((x) => x.category === "write" || x.category === "execute") && <p className="row-hint">{t("settings.mcp.sensitiveHint")}</p>}
              </>
            )}
          </div>
        ))}
      </Card>
      {editing && (
        <ServerForm
          initial={editing}
          onClose={() => {
            setEditing(null);
            void refresh();
          }}
        />
      )}
      {importing && (
        <ImportDialog
          onClose={() => {
            setImporting(false);
            void refresh();
          }}
        />
      )}
      {deleting && (
        <Dialog
          title={t("app.remove")}
          onClose={() => setDeleting(null)}
          actions={
            <>
              <button className="btn" onClick={() => setDeleting(null)}>
                {t("app.cancel")}
              </button>
              <button
                className="btn btn-primary"
                onClick={async () => {
                  await ipc.mcpDelete(deleting.config.id);
                  setDeleting(null);
                  await refresh();
                }}
              >
                {t("app.remove")}
              </button>
            </>
          }
        >
          <p>{t("settings.mcp.deleteConfirm", { name: deleting.config.name })}</p>
        </Dialog>
      )}
    </>
  );
}
