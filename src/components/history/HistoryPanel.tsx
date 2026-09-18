import { useEffect, useMemo, useRef, useState } from "react";
import { Download, Pencil, Search, Trash2, Upload, X } from "lucide-react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useChat } from "../../app/chatStore";
import { ipc, toAppError } from "../../app/ipc";
import { t } from "../../app/strings";
import type { AppErrorPayload, Conversation, SearchHit } from "../../app/types";
import { Dialog, ErrorNotice } from "../common/controls";

type ExportFormat = "json" | "markdown" | "txt";
const EXT: Record<ExportFormat, string> = { json: "json", markdown: "md", txt: "txt" };

function isToday(iso: string) {
  const d = new Date(iso);
  const n = new Date();
  return d.getFullYear() === n.getFullYear() && d.getMonth() === n.getMonth() && d.getDate() === n.getDate();
}

export function HistoryPanel({ onClose }: { onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [renaming, setRenaming] = useState<Conversation | null>(null);
  const [deleting, setDeleting] = useState<Conversation | null>(null);
  const [exporting, setExporting] = useState<string[] | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const activeId = useChat((s) => s.conversationId);
  const load = useChat((s) => s.loadConversation);
  const newConversation = useChat((s) => s.newConversation);
  const searchRef = useRef<HTMLInputElement>(null);

  const refresh = async (q = query) => {
    try {
      setHits(await ipc.convSearch(q));
    } catch (e) {
      setError(toAppError(e));
    }
  };

  useEffect(() => {
    searchRef.current?.focus();
  }, []);

  useEffect(() => {
    const id = setTimeout(() => void refresh(query), 150);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !renaming && !deleting && !exporting && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, renaming, deleting, exporting]);

  const groups = useMemo(() => {
    if (query.trim()) return [{ label: "", items: hits }];
    return [
      { label: t("history.today"), items: hits.filter((h) => isToday(h.conversation.updatedAt)) },
      { label: t("history.earlier"), items: hits.filter((h) => !isToday(h.conversation.updatedAt)) },
    ].filter((g) => g.items.length);
  }, [hits, query]);

  const doExport = async (format: ExportFormat) => {
    const ids = exporting ?? [];
    setExporting(null);
    const single = ids.length === 1 ? hits.find((h) => h.conversation.id === ids[0])?.conversation.title : null;
    const base = (single ?? "conversations").replace(/[\\/:*?"<>|]+/g, " ").trim().slice(0, 60) || "conversation";
    const path = await saveDialog({ defaultPath: `${base}.${EXT[format]}`, filters: [{ name: format.toUpperCase(), extensions: [EXT[format]] }] });
    if (!path) return;
    try {
      await ipc.convExport(ids, format, path);
    } catch (e) {
      setError(toAppError(e));
    }
  };

  const doImport = async () => {
    const path = await openDialog({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path || Array.isArray(path)) return;
    try {
      const ids = await ipc.convImport(path);
      setNotice(t("history.imported", { count: ids.length }));
      await refresh();
    } catch (e) {
      setError(toAppError(e));
    }
  };

  return (
    <section className="history" aria-label={t("history.title")}>
      <div className="history-head">
        <Search size={15} aria-hidden style={{ color: "var(--text-faint)", flex: "none" }} />
        <input ref={searchRef} className="input" type="search" placeholder={t("history.search")} aria-label={t("history.search")} value={query} onChange={(e) => setQuery(e.target.value)} dir="auto" />
        <button className="icon-btn" aria-label={t("app.close")} title={t("app.close")} onClick={onClose}>
          <X size={16} />
        </button>
      </div>
      {error && (
        <div style={{ padding: "8px 10px 0" }}>
          <ErrorNotice error={error} actions={<button className="btn btn-sm" onClick={() => setError(null)}>{t("app.close")}</button>} />
        </div>
      )}
      {notice && (
        <div style={{ padding: "8px 10px 0" }}>
          <div className="notice info" role="status">
            {notice}
          </div>
        </div>
      )}
      <div className="history-list">
        {hits.length === 0 && <p style={{ color: "var(--text-muted)", textAlign: "center", fontSize: "var(--text-sm)" }}>{query ? t("history.noResults") : t("history.empty")}</p>}
        {groups.map((g) => (
          <div key={g.label || "results"}>
            {g.label && <div className="history-group">{g.label}</div>}
            {g.items.map(({ conversation: c, snippet }) => (
              <div key={c.id} className={`history-item${c.id === activeId ? " active" : ""}`}>
                <button
                  className="history-open"
                  onClick={() => {
                    void load(c.id);
                    onClose();
                  }}
                >
                  <span className="title" dir="auto">
                    {c.title}
                  </span>
                  {snippet && (
                    <span className="snippet" dir="auto">
                      {snippet.replace(/\[|\]/g, "")}
                    </span>
                  )}
                </button>
                <button className="icon-btn" aria-label={`${t("app.rename")}: ${c.title}`} title={t("app.rename")} onClick={() => setRenaming(c)}>
                  <Pencil size={13} />
                </button>
                <button className="icon-btn" aria-label={`${t("app.export")}: ${c.title}`} title={t("app.export")} onClick={() => setExporting([c.id])}>
                  <Download size={13} />
                </button>
                <button className="icon-btn danger" aria-label={`${t("app.delete")}: ${c.title}`} title={t("app.delete")} onClick={() => setDeleting(c)}>
                  <Trash2 size={13} />
                </button>
              </div>
            ))}
          </div>
        ))}
      </div>
      <div className="history-foot">
        <button className="btn btn-sm" onClick={() => void doImport()}>
          <Upload size={13} /> {t("app.import")}
        </button>
        <button className="btn btn-sm" disabled={!hits.length} onClick={() => setExporting(hits.map((h) => h.conversation.id))}>
          <Download size={13} /> {t("app.export")}
        </button>
      </div>

      {renaming && <RenameDialog conversation={renaming} onDone={async () => { setRenaming(null); await refresh(); }} />}
      {deleting && (
        <Dialog
          title={t("app.delete")}
          onClose={() => setDeleting(null)}
          actions={
            <>
              <button className="btn" onClick={() => setDeleting(null)}>
                {t("app.cancel")}
              </button>
              <button
                className="btn btn-primary"
                onClick={async () => {
                  const id = deleting.id;
                  setDeleting(null);
                  await ipc.convDelete(id);
                  // The last one is gone: start fresh instead of showing an empty list.
                  if ((await ipc.convList(1)).length === 0) {
                    newConversation();
                    onClose();
                    return;
                  }
                  if (id === activeId) newConversation();
                  await refresh();
                }}
              >
                {t("app.delete")}
              </button>
            </>
          }
        >
          <p dir="auto">{t("history.deleteConfirm", { title: deleting.title })}</p>
        </Dialog>
      )}
      {exporting && (
        <Dialog title={t("history.exportAs")} onClose={() => setExporting(null)} actions={<button className="btn" onClick={() => setExporting(null)}>{t("app.cancel")}</button>}>
          <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
            <button className="btn" onClick={() => void doExport("json")}>JSON</button>
            <button className="btn" onClick={() => void doExport("markdown")}>Markdown</button>
            <button className="btn" onClick={() => void doExport("txt")}>TXT</button>
          </div>
        </Dialog>
      )}
    </section>
  );
}

function RenameDialog({ conversation, onDone }: { conversation: Conversation; onDone: () => void }) {
  const [title, setTitle] = useState(conversation.title);
  const [error, setError] = useState<AppErrorPayload | null>(null);
  const save = async () => {
    try {
      await ipc.convRename(conversation.id, title);
      onDone();
    } catch (e) {
      setError(toAppError(e));
    }
  };
  return (
    <Dialog
      title={t("app.rename")}
      onClose={onDone}
      actions={
        <>
          <button className="btn" onClick={onDone}>
            {t("app.cancel")}
          </button>
          <button className="btn btn-primary" onClick={() => void save()} disabled={!title.trim()}>
            {t("app.save")}
          </button>
        </>
      }
    >
      <label className="sr-only" htmlFor="rename-input">
        {t("history.renamePrompt")}
      </label>
      <input id="rename-input" className="input" value={title} dir="auto" onChange={(e) => setTitle(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void save()} />
      {error && <ErrorNotice error={error} />}
    </Dialog>
  );
}
