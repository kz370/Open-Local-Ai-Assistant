import { memo, useEffect, useState } from "react";
import { File, FileText, ImageOff, X } from "lucide-react";
import { ipc } from "../../app/ipc";
import { t } from "../../app/strings";
import type { Attachment } from "../../app/types";

export function attachmentSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} B`;
}

/** Second line of a chip: what the model will actually get from this file. */
function detail(a: Attachment): string {
  if (a.note) return a.note;
  if (a.kind === "text") {
    return a.truncated
      ? t("attach.textTruncated", { chars: a.textChars.toLocaleString() })
      : t("attach.textChars", { chars: a.textChars.toLocaleString() });
  }
  return attachmentSize(a.sizeBytes);
}

/** Image preview, read back from the stored copy on demand. */
const Thumb = memo(function Thumb({ attachment }: { attachment: Attachment }) {
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let live = true;
    ipc
      .attachmentDataUrl(attachment.id, attachment.mime)
      .then((u) => live && setUrl(u))
      .catch(() => live && setFailed(true));
    return () => {
      live = false;
    };
  }, [attachment.id, attachment.mime]);

  if (failed) return <ImageOff size={16} aria-hidden />;
  return url ? <img src={url} alt={attachment.name} /> : <span className="attachment-thumb-empty" aria-hidden />;
});

interface Props {
  items: Attachment[];
  /** Omitted for sent messages, which can no longer be edited. */
  onRemove?: (id: string) => void;
}

export function AttachmentList({ items, onRemove }: Props) {
  if (items.length === 0) return null;
  return (
    <ul className="attachments" aria-label={t("attach.listLabel")}>
      {items.map((a) => (
        <li key={a.id} className={`attachment${a.kind === "image" ? " image" : ""}`} title={`${a.name} — ${detail(a)}`}>
          <span className="attachment-icon" aria-hidden>
            {a.kind === "image" ? <Thumb attachment={a} /> : a.kind === "text" ? <FileText size={16} /> : <File size={16} />}
          </span>
          <span className="attachment-text">
            <span className="attachment-name" dir="auto">
              {a.name}
            </span>
            <span className="attachment-detail">{detail(a)}</span>
          </span>
          {onRemove && (
            <button
              type="button"
              className="attachment-remove"
              aria-label={t("attach.remove", { name: a.name })}
              title={t("attach.remove", { name: a.name })}
              onClick={() => onRemove(a.id)}
            >
              <X size={13} />
            </button>
          )}
        </li>
      ))}
    </ul>
  );
}
