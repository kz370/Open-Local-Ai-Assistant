import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { useChat } from "./app/chatStore";
import { ipc } from "./app/ipc";
import { useSettings } from "./app/settingsStore";
import { useVoice } from "./app/voiceStore";
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
    })();
    return () => cleanups.forEach((c) => c());
  }, [route]);

  if (!ready || !settings) return null;
  if (route === "overlay") return <Overlay />;
  if (route === "bubble") return <Bubble />;
  if (route === "settings") return <SettingsApp />;
  if (!settings.general.firstRunComplete) return <SetupWizard onDone={() => void useSettings.getState().load()} />;
  return <ChatApp />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
