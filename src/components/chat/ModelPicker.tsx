import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown, Cpu, RefreshCw, Sparkles } from "lucide-react";
import { ipc } from "../../app/ipc";
import { useSettings } from "../../app/settingsStore";
import { formatBytes, modelLabel, t } from "../../app/strings";
import type { ModelInfo } from "../../app/types";

/** Model dropdown in the composer toolbar (Automatic + LM Studio models). */
export function ModelPicker({ autoModel }: { autoModel: string | null }) {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const [open, setOpen] = useState(false);
  const [models, setModels] = useState<ModelInfo[] | null>(null);
  const [loading, setLoading] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const aliases = settings?.ai.modelAliases;
  const manual = settings?.ai.modelMode === "manual" ? settings.ai.model : null;
  const label = manual ? modelLabel(manual, aliases) : autoModel ? `${t("chat.modelAuto")} · ${modelLabel(autoModel, aliases, 18)}` : t("chat.modelAuto");

  const load = async (refresh: boolean) => {
    setLoading(true);
    try {
      setModels((await ipc.lmstudioModels(refresh)).filter((m) => m.kind !== "embedding"));
    } catch {
      setModels([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!open) return;
    void load(false);
    const onDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        setOpen(false);
      }
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    setTimeout(() => listRef.current?.querySelector<HTMLElement>("[aria-checked='true'], [role='menuitemradio']")?.focus(), 0);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [open]);

  const choose = (id: string | null) => {
    void update((d) => {
      if (id) {
        d.ai.modelMode = "manual";
        d.ai.model = id;
      } else {
        d.ai.modelMode = "auto";
      }
    });
    setOpen(false);
  };

  const onListKey = (e: React.KeyboardEvent) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const items = Array.from(listRef.current?.querySelectorAll<HTMLElement>("[role='menuitemradio']") ?? []);
    const i = items.indexOf(document.activeElement as HTMLElement);
    items[(i + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length]?.focus();
  };

  return (
    <div className="picker" ref={rootRef}>
      <button type="button" className="chip" aria-haspopup="menu" aria-expanded={open} title={manual ?? autoModel ?? t("chat.modelPicker")} onClick={() => setOpen((v) => !v)}>
        {manual ? <Cpu size={13} aria-hidden /> : <Sparkles size={13} aria-hidden />}
        <span className="chip-label">{label}</span>
        <ChevronDown size={13} aria-hidden className={open ? "flip" : ""} />
      </button>
      {open && (
        <div className="picker-menu" role="menu" aria-label={t("chat.modelPicker")} ref={listRef} onKeyDown={onListKey}>
          <div className="picker-head">
            <span>{t("chat.modelPicker")}</span>
            <button type="button" className="icon-btn" aria-label={t("chat.modelsRefresh")} title={t("chat.modelsRefresh")} onClick={() => void load(true)}>
              <RefreshCw size={13} className={loading ? "spin" : ""} />
            </button>
          </div>
          <button type="button" role="menuitemradio" aria-checked={!manual} className="picker-item" onClick={() => choose(null)}>
            <Sparkles size={15} aria-hidden className="picker-icon" />
            <span className="picker-text">
              <span className="picker-name">{t("chat.modelAuto")}</span>
              <span className="picker-sub">{autoModel ? t("chat.modelCurrently", { model: modelLabel(autoModel, aliases, 32) }) : t("chat.modelAutoHint")}</span>
            </span>
            {!manual && <Check size={15} aria-hidden className="picker-check" />}
          </button>
          <div className="picker-sep" />
          <div className="picker-scroll">
            {models === null && <div className="picker-empty"><span className="spinner" /></div>}
            {models?.length === 0 && <div className="picker-empty">{t("chat.modelsEmpty")}</div>}
            {models?.map((m) => (
              <button type="button" role="menuitemradio" aria-checked={manual === m.id} key={m.id} className="picker-item" onClick={() => choose(m.id)} title={m.id}>
                <span className={`picker-dot${m.loaded ? " on" : ""}`} aria-hidden />
                <span className="picker-text">
                  <span className="picker-name">{modelLabel(m.id, aliases, 34)}</span>
                  <span className="picker-sub">
                    {[m.params, m.quantization, m.sizeBytes ? formatBytes(m.sizeBytes) : null, m.loaded ? t("settings.ai.loaded") : null].filter(Boolean).join(" · ")}
                  </span>
                </span>
                {m.toolUse && <span className="badge accent">{t("settings.ai.toolUse")}</span>}
                {manual === m.id && <Check size={15} aria-hidden className="picker-check" />}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
