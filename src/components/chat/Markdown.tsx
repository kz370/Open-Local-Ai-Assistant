import { memo, useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { Check, Copy } from "lucide-react";
import { openExternal } from "../common/controls";
import { t } from "../../app/strings";

function CodeBlock({ lang, code }: { lang: string; code: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="codeblock" dir="ltr">
      <div className="codeblock-head">
        <span>{lang || "code"}</span>
        <button
          className="icon-btn"
          style={{ width: 24, height: 24 }}
          aria-label={copied ? t("app.copied") : t("app.copy")}
          title={copied ? t("app.copied") : t("app.copy")}
          onClick={() => {
            void navigator.clipboard.writeText(code);
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          }}
        >
          {copied ? <Check size={13} /> : <Copy size={13} />}
        </button>
      </div>
      <pre>
        <code>{code}</code>
      </pre>
    </div>
  );
}

const components: Components = {
  // Model output is untrusted: links open in the system browser only for http(s).
  a: ({ href, children }) => (
    <a
      href={href}
      dir="ltr"
      onClick={(e) => {
        e.preventDefault();
        if (href) openExternal(href);
      }}
      title={href}
    >
      {children}
    </a>
  ),
  pre: ({ children }) => <>{children}</>,
  code: ({ className, children }) => {
    const text = String(children ?? "");
    const lang = /language-(\w+)/.exec(className ?? "")?.[1] ?? "";
    const isBlock = !!className || text.includes("\n");
    if (isBlock) return <CodeBlock lang={lang} code={text.replace(/\n$/, "")} />;
    return <code dir="ltr">{children}</code>;
  },
  img: ({ alt }) => <span>{alt}</span>,
  p: ({ children }) => <p dir="auto">{children}</p>,
  li: ({ children }) => <li dir="auto">{children}</li>,
};

export const Markdown = memo(function Markdown({ text }: { text: string }) {
  return (
    <div className="markdown">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components} skipHtml>
        {text}
      </ReactMarkdown>
    </div>
  );
});
