import { useEffect, useState } from "react";
import { RefreshCw, Search } from "lucide-react";
import { ipc, toAppError } from "../../../app/ipc";
import { useSettings } from "../../../app/settingsStore";
import { t } from "../../../app/strings";
import type { PublicSearxInstance } from "../../../app/types";
import { Dialog, Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";

const shortUrl = (url: string) => url.replace(/^https?:\/\//, "").replace(/\/$/, "");

/** Picks a public SearXNG instance from the searx.space list. */
function InstancePicker({ current, onPick, onClose }: { current: string; onPick: (url: string) => void; onClose: () => void }) {
  const [instances, setInstances] = useState<PublicSearxInstance[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState(current);

  const load = async (refresh: boolean) => {
    setLoading(true);
    setError(null);
    try {
      setInstances(await ipc.searchPublicInstances(refresh));
    } catch (e) {
      setError(toAppError(e).detail);
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => {
    void load(false);
  }, []);

  const shown = (instances ?? []).filter((i) => i.url.toLowerCase().includes(filter.trim().toLowerCase()));

  return (
    <Dialog
      title={t("settings.search.pickTitle")}
      onClose={onClose}
      actions={
        <>
          <button className="btn" onClick={onClose}>
            {t("app.cancel")}
          </button>
          <button
            className="btn btn-primary"
            onClick={() => {
              onPick(selected);
              onClose();
            }}
          >
            {t("settings.search.pickUse")}
          </button>
        </>
      }
    >
      <p className="row-hint">{t("settings.search.pickHint")}</p>
      <div style={{ display: "flex", gap: 6, alignItems: "center", margin: "10px 0" }}>
        <input
          className="input"
          placeholder={t("settings.search.pickFilter")}
          aria-label={t("settings.search.pickFilter")}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          spellCheck={false}
        />
        <button className="btn btn-sm" disabled={loading} onClick={() => void load(true)}>
          <RefreshCw size={13} className={loading ? "spin" : undefined} /> {t("settings.search.refresh")}
        </button>
      </div>
      {error && (
        <div className="notice err" role="alert">
          {t("settings.search.listError")} {error}
        </div>
      )}
      <div className="radio-list" role="radiogroup" aria-label={t("settings.search.publicInstance")} style={{ maxHeight: 320, overflowY: "auto" }}>
        <label>
          <input type="radio" name="searx-instance" checked={selected === ""} onChange={() => setSelected("")} />
          <span>
            <strong>{t("settings.search.auto")}</strong>
            <span className="server-sub" style={{ display: "block" }}>
              {t("settings.search.autoHint")}
            </span>
          </span>
        </label>
        {/* A saved pick the fresh list no longer has stays selectable. */}
        {current && instances && !instances.some((i) => i.url === current) && (
          <label>
            <input type="radio" name="searx-instance" checked={selected === current} onChange={() => setSelected(current)} />
            <span>
              <strong>{shortUrl(current)}</strong>
              <span className="server-sub" style={{ display: "block" }}>
                {t("settings.search.notListed")}
              </span>
            </span>
          </label>
        )}
        {loading && !instances && <p className="row-hint">{t("settings.search.loading")}</p>}
        {shown.map((i) => (
          <label key={i.url}>
            <input type="radio" name="searx-instance" checked={selected === i.url} onChange={() => setSelected(i.url)} />
            <span>
              <strong>{shortUrl(i.url)}</strong>
              <span className="server-sub" style={{ display: "block" }}>
                {t("settings.search.instanceStats", {
                  success: Math.round(i.searchSuccess),
                  time: i.searchTime != null ? `${i.searchTime.toFixed(1)}s` : "–",
                })}
                {i.version ? ` · ${i.version}` : ""}
              </span>
            </span>
          </label>
        ))}
      </div>
    </Dialog>
  );
}

export function WebSearchSection() {
  const search = useSettings((s) => s.settings?.search);
  const setSettings = useSettings((s) => s.update);
  const [picking, setPicking] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; text: string } | null>(null);

  const ddgOn = search?.enabled ?? true;
  const searxngOn = search?.searxngEnabled ?? true;
  const source = search?.searxngSource ?? "local";
  const publicUrl = search?.searxngPublicUrl ?? "";

  const runTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const r = await ipc.searchTest();
      setTestResult({ ok: true, text: t("settings.search.testOk", { count: r.count, engine: r.engine }) });
    } catch (e) {
      setTestResult({ ok: false, text: toAppError(e).detail });
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <SectionHeader title={t("settings.sections.search")} intro={t("settings.search.intro")} />
      <Card title={t("settings.search.engines")}>
        <Row label={t("settings.search.searxng")} hint={t("settings.search.searxngHint")} htmlFor="sw-searxng">
          <Switch id="sw-searxng" label={t("settings.search.searxng")} checked={searxngOn} onChange={(v) => setSettings((x) => void (x.search.searxngEnabled = v))} />
        </Row>
        <Row label={t("settings.search.duckduckgo")} hint={t("settings.search.duckduckgoHint")} htmlFor="sw-ddg">
          <Switch id="sw-ddg" label={t("settings.search.duckduckgo")} checked={ddgOn} onChange={(v) => setSettings((x) => void (x.search.enabled = v))} />
        </Row>
        {searxngOn && ddgOn && (
          <Row label={t("settings.search.primary")} hint={t("settings.search.primaryHint")} htmlFor="sel-primary">
            <select
              id="sel-primary"
              className="select"
              value={search?.primary ?? "searxng"}
              onChange={(e) => setSettings((x) => void (x.search.primary = e.target.value as "searxng" | "duckduckgo"))}
            >
              <option value="searxng">{t("settings.search.searxng")}</option>
              <option value="duckduckgo">{t("settings.search.duckduckgo")}</option>
            </select>
          </Row>
        )}
        <Row label={t("settings.search.results")} htmlFor="sel-search-results">
          <select
            id="sel-search-results"
            className="select"
            value={String(search?.maxResults ?? 5)}
            disabled={!searxngOn && !ddgOn}
            onChange={(e) => setSettings((x) => void (x.search.maxResults = Number(e.target.value)))}
          >
            {[3, 5, 8, 10].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </Row>
        {!searxngOn && !ddgOn && (
          <div className="notice warn" role="status" style={{ marginTop: 8 }}>
            {t("settings.search.allOff")}
          </div>
        )}
      </Card>

      {searxngOn && (
        <Card title={t("settings.search.searxng")}>
          <Row label={t("settings.search.source")} htmlFor="sel-searxng-source">
            <select
              id="sel-searxng-source"
              className="select"
              value={source}
              onChange={(e) => setSettings((x) => void (x.search.searxngSource = e.target.value as "local" | "public"))}
            >
              <option value="local">{t("settings.search.sourceLocal")}</option>
              <option value="public">{t("settings.search.sourcePublic")}</option>
            </select>
          </Row>
          {source === "local" ? (
            <Row label={t("settings.search.address")} hint={t("settings.search.addressHint")} htmlFor="in-searxng">
              <input
                id="in-searxng"
                className="input"
                placeholder="http://localhost:8080"
                value={search?.searxngUrl ?? ""}
                onChange={(e) => setSettings((x) => void (x.search.searxngUrl = e.target.value.trim()))}
                spellCheck={false}
              />
            </Row>
          ) : (
            <Row label={t("settings.search.publicInstance")} hint={publicUrl ? shortUrl(publicUrl) : t("settings.search.autoHint")}>
              <button className="btn btn-sm" onClick={() => setPicking(true)}>
                {publicUrl ? t("settings.search.change") : t("settings.search.choose")}
              </button>
            </Row>
          )}
        </Card>
      )}

      <Card title={t("settings.search.test")}>
        <Row label={t("settings.search.test")} hint={t("settings.search.testHint")}>
          <button className="btn btn-sm" disabled={(!searxngOn && !ddgOn) || testing} onClick={() => void runTest()}>
            <Search size={13} /> {testing ? t("settings.search.testing") : t("settings.search.test")}
          </button>
        </Row>
        {testResult && (
          <div className={`notice ${testResult.ok ? "info" : "err"}`} role="status" style={{ marginTop: 8 }}>
            {testResult.text}
          </div>
        )}
      </Card>

      {picking && (
        <InstancePicker current={publicUrl} onPick={(url) => void setSettings((x) => void (x.search.searxngPublicUrl = url))} onClose={() => setPicking(false)} />
      )}
    </>
  );
}
