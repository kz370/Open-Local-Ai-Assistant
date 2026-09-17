import { ExternalLink } from "lucide-react";
import { openExternal } from "../common/controls";
import { t } from "../../app/strings";
import type { Source } from "../../app/types";

function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

/** Renders only sources that were actually returned by tools. */
export function Sources({ sources }: { sources: Source[] }) {
  if (!sources.length) return null;
  return (
    <div className="sources">
      <div className="sources-title">{t("chat.sources")}</div>
      <div className="source-list">
        {sources.map((s, i) => (
          <a
            key={s.url}
            className="source-chip"
            href={s.url}
            title={s.url}
            onClick={(e) => {
              e.preventDefault();
              openExternal(s.url);
            }}
          >
            <span className="source-num">{i + 1}</span>
            <span dir="auto">{s.title || hostOf(s.url)}</span>
            <ExternalLink size={11} aria-hidden />
          </a>
        ))}
      </div>
    </div>
  );
}
