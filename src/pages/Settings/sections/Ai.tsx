import { useCallback, useEffect, useState } from "react";
import { Eye, EyeOff, RefreshCw } from "lucide-react";
import { ipc, toAppError } from "../../../app/ipc";
import { PROVIDERS } from "../../../app/providers";
import { formatBytes, modelLabel, t } from "../../../app/strings";
import type { AppErrorPayload, ConnectionStatus, ModelInfo, ModelSelection, ProviderId } from "../../../app/types";
import { ErrorNotice, Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { useS } from "./Basic";

export function useLmModels() {
  const [s] = useS();
  // Hosted providers reject every request without a key, so don't ask until there is one.
  const skip = s.ai.provider !== "lmstudio" && !(s.ai.apiKey ?? "").trim();
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [auto, setAuto] = useState<ModelSelection | null>(null);
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const [loading, setLoading] = useState(false);
  const refresh = useCallback(async (force: boolean) => {
    if (skip) {
      setModels([]);
      setAuto(null);
      setError(null);
      return;
    }
    setLoading(true);
    try {
      const m = await ipc.lmstudioModels(force);
      setModels(m);
      setAuto(await ipc.lmstudioAutoSelection());
      setError(null);
    } catch (e) {
      setError(toAppError(e));
      setModels([]);
      setAuto(null);
    } finally {
      setLoading(false);
    }
  }, [skip]);
  useEffect(() => {
    void refresh(false);
  }, [refresh, s.ai.provider, s.ai.serverUrl, s.ai.apiKey]);
  return { models, auto, error, loading, refresh };
}

export function AiSection() {
  const [s, set] = useS();
  const [url, setUrl] = useState(s.ai.serverUrl);
  const [apiKey, setApiKey] = useState(s.ai.apiKey ?? "");
  const [showKey, setShowKey] = useState(false);
  const [status, setStatus] = useState<ConnectionStatus | null>(null);
  const [testError, setTestError] = useState<AppErrorPayload | null>(null);
  const [testing, setTesting] = useState(false);
  const [modelFilter, setModelFilter] = useState("");
  const { models, auto, error, loading, refresh } = useLmModels();
  const provider = PROVIDERS.find((p) => p.id === s.ai.provider) ?? PROVIDERS[0];
  const hosted = provider.id !== "lmstudio";

  const switchProvider = (next: ProviderId) =>
    set((d) => {
      if (next === d.ai.provider) return;
      d.ai.providerProfiles[d.ai.provider] = { serverUrl: d.ai.serverUrl, apiKey: d.ai.apiKey, model: d.ai.model, modelMode: d.ai.modelMode };
      const saved = d.ai.providerProfiles[next];
      d.ai.provider = next;
      d.ai.serverUrl = saved?.serverUrl || PROVIDERS.find((p) => p.id === next)!.url;
      d.ai.apiKey = saved?.apiKey ?? null;
      d.ai.model = saved?.model ?? null;
      d.ai.modelMode = next === "lmstudio" ? saved?.modelMode || "auto" : "manual";
    });

  useEffect(() => setUrl(s.ai.serverUrl), [s.ai.serverUrl]);
  useEffect(() => setApiKey(s.ai.apiKey ?? ""), [s.ai.apiKey]);

  const needsKey = hosted && !apiKey.trim();

  const test = async () => {
    if (needsKey) {
      setStatus(null);
      setTestError(null);
      return;
    }
    setTesting(true);
    setTestError(null);
    try {
      setStatus(await ipc.lmstudioTest(url, apiKey || undefined, s.ai.provider));
    } catch (e) {
      setStatus(null);
      setTestError(toAppError(e));
    } finally {
      setTesting(false);
    }
  };

  useEffect(() => {
    void test();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.ai.serverUrl, s.ai.apiKey, s.ai.provider]);

  useEffect(() => setModelFilter(""), [s.ai.provider]);

  const needle = modelFilter.trim().toLowerCase();
  const freeOnly = hosted && s.ai.freeModelsOnly;
  const chatModels = models.filter(
    (m) =>
      m.kind !== "embedding" &&
      // The model in use stays listed even when it is not free.
      (!freeOnly || m.free || m.id === s.ai.model) &&
      (!needle || m.id.toLowerCase().includes(needle) || (s.ai.modelAliases[m.id] ?? "").toLowerCase().includes(needle)),
  );
  const numberOrNull = (v: string) => (v.trim() === "" ? null : Math.max(1, Math.round(Number(v)) || 0) || null);

  return (
    <>
      <SectionHeader title={t("settings.sections.ai")} />
      <Card title={t("settings.ai.provider")}>
        <Row label={t("settings.ai.provider")} hint={hosted ? t("settings.ai.hostedNotice", { provider: provider.label }) : undefined} htmlFor="sel-provider">
          <select id="sel-provider" className="select" value={s.ai.provider} onChange={(e) => switchProvider(e.target.value as ProviderId)}>
            {PROVIDERS.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
        </Row>
        <Row label={hosted ? t("settings.ai.serverHosted") : t("settings.ai.server")} hint={hosted ? t("settings.ai.serverHostedHint") : t("settings.ai.serverHint")} htmlFor="lm-url">
          <input
            id="lm-url"
            className="input mono"
            value={url}
            spellCheck={false}
            onChange={(e) => setUrl(e.target.value)}
            onBlur={() => url !== s.ai.serverUrl && set((d) => void (d.ai.serverUrl = url))}
            onKeyDown={(e) => e.key === "Enter" && set((d) => void (d.ai.serverUrl = url))}
          />
        </Row>
        <Row label={t("settings.ai.apiKey")} hint={hosted ? t("settings.ai.apiKeyHostedHint", { provider: provider.label }) : t("settings.ai.apiKeyHint")} htmlFor="lm-key">
          <input
            id="lm-key"
            className="input mono"
            type={showKey ? "text" : "password"}
            autoComplete="off"
            spellCheck={false}
            placeholder={hosted ? t("settings.ai.apiKeyRequired") : t("settings.ai.apiKeyPlaceholder")}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            onBlur={() => (apiKey.trim() || null) !== s.ai.apiKey && set((d) => void (d.ai.apiKey = apiKey.trim() || null))}
            onKeyDown={(e) => e.key === "Enter" && set((d) => void (d.ai.apiKey = apiKey.trim() || null))}
          />
          <button type="button" className="icon-btn" aria-label={showKey ? t("settings.ai.hideKey") : t("settings.ai.showKey")} onClick={() => setShowKey((v) => !v)}>
            {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
          </button>
        </Row>
        <Row label={t("settings.ai.status")}>
          {testing ? (
            <span className="badge">
              <span className="spinner" style={{ width: 10, height: 10 }} /> {t("status.checking")}
            </span>
          ) : status ? (
            <span className="badge ok">
              <span className="dot ok" /> {t("status.connected")} · {t("settings.ai.latency", { ms: status.latencyMs })}
            </span>
          ) : needsKey ? (
            <span className="badge">{t("settings.ai.apiKeyNeeded")}</span>
          ) : (
            <span className="badge err">
              <span className="dot err" /> {t("status.disconnected")}
            </span>
          )}
          <button className="btn btn-sm" onClick={() => void test()} disabled={needsKey}>
            {t("settings.ai.testConnection")}
          </button>
          <button className="btn btn-sm" onClick={() => void refresh(true)} disabled={loading || needsKey}>
            <RefreshCw size={12} /> {t("settings.ai.refreshModels")}
          </button>
        </Row>
        {testError && (
          <div style={{ paddingBottom: 12 }}>
            <ErrorNotice error={testError} />
          </div>
        )}
      </Card>

      <Card title={t("settings.ai.model")}>
        {error && !testError && (
          <div style={{ padding: "10px 0" }}>
            <ErrorNotice error={error} />
          </div>
        )}
        {hosted && models.some((m) => m.free) && (
          <Row label={t("settings.ai.freeOnly")} hint={t("settings.ai.freeOnlyHint")} htmlFor="sw-free-only">
            <Switch id="sw-free-only" label={t("settings.ai.freeOnly")} checked={s.ai.freeModelsOnly} onChange={(v) => set((d) => void (d.ai.freeModelsOnly = v))} />
          </Row>
        )}
        {hosted && models.length > 8 && (
          <div style={{ padding: "10px 0" }}>
            <input className="input" aria-label={t("settings.ai.filterModels")} placeholder={t("settings.ai.filterModels")} value={modelFilter} onChange={(e) => setModelFilter(e.target.value)} />
          </div>
        )}
        <div className="model-list" role="radiogroup" aria-label={t("settings.ai.model")}>
          {!hosted && (
            <label className="model-item">
              <input type="radio" name="model" checked={s.ai.modelMode === "auto"} onChange={() => set((d) => void (d.ai.modelMode = "auto"))} />
              <div>
                <div className="model-name">{t("settings.ai.automatic")}</div>
                {auto && (
                  <div className="model-meta">
                    <span>{t("settings.ai.autoPicked", { model: modelLabel(auto.modelId, s.ai.modelAliases, 48) })}</span>
                    {auto.reasons.map((r) => (
                      <span key={r} className="badge">
                        {t(`settings.ai.reasons.${r}`)}
                      </span>
                    ))}
                  </div>
                )}
              </div>
              <span />
            </label>
          )}
          {chatModels.length === 0 && !loading && <p className="row-hint">{hosted ? t("settings.ai.noModelsHosted") : t("settings.ai.noModels")}</p>}
          {chatModels.map((m) => {
            const hidden = s.ai.hiddenModels.includes(m.id);
            return (
            <label className={`model-item${hidden ? " hidden-model" : ""}`} key={m.id}>
              <input
                type="radio"
                name="model"
                checked={s.ai.modelMode === "manual" && s.ai.model === m.id}
                onChange={() =>
                  set((d) => {
                    d.ai.modelMode = "manual";
                    d.ai.model = m.id;
                  })
                }
              />
              <div style={{ minWidth: 0 }}>
                <div className="model-name" title={m.id}>
                  {modelLabel(m.id, s.ai.modelAliases, 48)}
                  {s.ai.modelAliases[m.id] && <span style={{ color: "var(--text-faint)", fontWeight: 400 }}> · {m.id}</span>}
                </div>
                <div className="model-meta">
                  <span className="model-provider">{provider.label}</span>
                  {m.params && <span>{m.params}</span>}
                  {m.quantization && <span>{m.quantization}</span>}
                  {m.sizeBytes ? <span>{formatBytes(m.sizeBytes)}</span> : null}
                  {m.maxContextLength ? <span>{Math.round(m.maxContextLength / 1000)}k ctx</span> : null}
                  {m.free && <span className="badge ok">{t("settings.ai.free")}</span>}
                  {m.loaded && <span className="badge ok">{t("settings.ai.loaded")}</span>}
                  {m.toolUse && <span className="badge accent">{t("settings.ai.toolUse")}</span>}
                  {m.vision && <span className="badge">{t("settings.ai.vision")}</span>}
                </div>
              </div>
              <input
                className="input alias-input"
                aria-label={`${t("settings.ai.shortName")}: ${m.id}`}
                placeholder={t("settings.ai.shortName")}
                title={t("settings.ai.shortNameHint")}
                defaultValue={s.ai.modelAliases[m.id] ?? ""}
                maxLength={40}
                onClick={(e) => e.preventDefault()}
                onBlur={(e) => {
                  const v = e.target.value.trim();
                  if ((s.ai.modelAliases[m.id] ?? "") === v) return;
                  set((d) => {
                    if (v) d.ai.modelAliases[m.id] = v;
                    else delete d.ai.modelAliases[m.id];
                  });
                }}
                onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
              />
              <button
                type="button"
                className="icon-btn"
                aria-pressed={!hidden}
                aria-label={`${hidden ? t("settings.ai.showInChat") : t("settings.ai.hideInChat")}: ${m.id}`}
                title={hidden ? t("settings.ai.showInChat") : t("settings.ai.hideInChat")}
                onClick={(e) => {
                  // Inside the row's label: do not select the model too.
                  e.preventDefault();
                  set((d) => {
                    d.ai.hiddenModels = hidden ? d.ai.hiddenModels.filter((x) => x !== m.id) : [...d.ai.hiddenModels, m.id];
                  });
                }}
              >
                {hidden ? <EyeOff size={15} /> : <Eye size={15} />}
              </button>
            </label>
            );
          })}
        </div>
      </Card>

      <Card title={t("app.advanced")}>
        <Row label={t("settings.ai.temperature")} htmlFor="rng-temp">
          <input id="rng-temp" type="range" min={0} max={2} step={0.05} value={s.ai.temperature} onChange={(e) => set((d) => void (d.ai.temperature = Number(e.target.value)))} style={{ maxWidth: 260 }} />
          <span className="range-value">{s.ai.temperature.toFixed(2)}</span>
        </Row>
        <Row label={t("settings.ai.contextLength")} hint={t("settings.ai.contextHint")} htmlFor="in-ctx">
          <input id="in-ctx" className="input" type="number" min={512} step={512} style={{ maxWidth: 160 }} defaultValue={s.ai.contextLength ?? ""} onBlur={(e) => set((d) => void (d.ai.contextLength = numberOrNull(e.target.value)))} />
        </Row>
        <Row label={t("settings.ai.maxTokens")} hint={t("settings.ai.maxTokensHint")} htmlFor="in-max">
          <input id="in-max" className="input" type="number" min={16} step={64} style={{ maxWidth: 160 }} defaultValue={s.ai.maxTokens ?? ""} onBlur={(e) => set((d) => void (d.ai.maxTokens = numberOrNull(e.target.value)))} />
        </Row>
        <Row label={t("settings.ai.pasteAsFile")} hint={t("settings.ai.pasteAsFileHint")} htmlFor="in-paste">
          <input
            id="in-paste"
            className="input"
            type="number"
            min={200}
            max={200000}
            step={500}
            style={{ maxWidth: 160 }}
            defaultValue={s.ai.pasteAsFileChars || ""}
            onBlur={(e) => set((d) => void (d.ai.pasteAsFileChars = numberOrNull(e.target.value) ?? 0))}
          />
        </Row>
        <Row label={t("settings.ai.timeout")} htmlFor="in-timeout">
          <input id="in-timeout" className="input" type="number" min={10} max={3600} style={{ maxWidth: 160 }} defaultValue={s.ai.requestTimeoutSecs} onBlur={(e) => set((d) => void (d.ai.requestTimeoutSecs = Number(e.target.value) || 300))} />
        </Row>
        <Row label={t("settings.ai.streaming")} htmlFor="sw-stream">
          <Switch id="sw-stream" label={t("settings.ai.streaming")} checked={s.ai.streaming} onChange={(v) => set((d) => void (d.ai.streaming = v))} />
        </Row>
        <Row label={t("settings.ai.showReasoning")} htmlFor="sw-reason">
          <Switch id="sw-reason" label={t("settings.ai.showReasoning")} checked={s.ai.showReasoning} onChange={(v) => set((d) => void (d.ai.showReasoning = v))} />
        </Row>
        <Row label={t("settings.ai.systemPrompt")} hint={t("settings.ai.systemPromptHint")} htmlFor="ta-sys" stack>
          <textarea id="ta-sys" className="textarea" rows={4} dir="auto" defaultValue={s.ai.systemPrompt} onBlur={(e) => set((d) => void (d.ai.systemPrompt = e.target.value))} />
        </Row>
      </Card>
    </>
  );
}
