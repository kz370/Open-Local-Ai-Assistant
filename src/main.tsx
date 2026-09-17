import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { useChat } from "./app/chatStore";
import { ipc } from "./app/ipc";
import { useSettings } from "./app/settingsStore";
import { useVoice } from "./app/voiceStore";
import { ErrorBoundary } from "./components/common/ErrorBoundary";
import { Bubble } from "./pages/Bubble/Bubble";
import { ChatApp } from "./pages/Chat/ChatApp";
import { Overlay } from "./pages/Overlay/Overlay";
import { SettingsApp } from "./pages/Settings/SettingsApp";
import { SetupWizard } from "./pages/Setup/SetupWizard";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/chat.css";
import "./styles/settings.css";

type Route = "chat" | "settings" | "overlay" | "bubble";

function routeFromHash(): Route {
  const h = location.hash;
  if (h.startsWith("#/settings")) return "settings";
  if (h.startsWith("#/overlay")) return "overlay";
  if (h.startsWith("#/bubble")) return "bubble";
  return "chat";
}

// Disable the webview's default context menu outside editable fields.
document.addEventListener("contextmenu", (e) => {
  const el = e.target as HTMLElement;
  if (!el.closest("input, textarea, .messages, .log-view")) e.preventDefault();
});

function Root() {
  const [route] = useState(routeFromHash);
  const settings = useSettings((s) => s.settings);
  const [ready, setReady] = useState(false);
  const [startupError, setStartupError] = useState<string | null>(null);

  useEffect(() => {
    const cleanups: (() => void)[] = [];
    (async () => {
      const s = await useSettings.getState().load();
      cleanups.push(await useSettings.getState().subscribe());
      if (route === "chat") {
        cleanups.push(await useVoice.getState().subscribe());
        if (s.lastConversationId) {
          await useChat.getState().loadConversation(s.lastConversationId).catch(() => undefined);
        }
      }
      setReady(true);
      if (route === "chat") void ipc.appReady();
    })().catch((e: unknown) => {
      console.error("startup failed", e);
      setStartupError(e instanceof Error ? e.message : JSON.stringify(e));
      setReady(true);
    });
    return () => cleanups.forEach((c) => c());
  }, [route]);

  if (startupError || (ready && !settings)) {
    return (
      <div style={{ padding: 20, userSelect: "text" }}>
        <h1 style={{ fontSize: 16, margin: "0 0 6px" }}>Local Assistant could not start.</h1>
        <p style={{ fontSize: 13, color: "var(--text-muted)" }}>{startupError ?? "Settings could not be loaded."}</p>
        <button className="btn" onClick={() => location.reload()}>
          Retry
        </button>
      </div>
    );
  }
  if (!ready || !settings) return null;
  const screen =
    route === "overlay" ? (
      <Overlay />
    ) : route === "bubble" ? (
      <Bubble />
    ) : route === "settings" ? (
      <SettingsApp />
    ) : !settings.general.firstRunComplete ? (
      <SetupWizard onDone={() => void useSettings.getState().load()} />
    ) : (
      <ChatApp />
    );
  return <ErrorBoundary where={route}>{screen}</ErrorBoundary>;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
