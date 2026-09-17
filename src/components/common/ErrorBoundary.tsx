import { Component, type ErrorInfo, type ReactNode } from "react";

interface State {
  error: Error | null;
  info: string;
}

/** Shows the failure instead of a blank window when a screen crashes. */
export class ErrorBoundary extends Component<{ children: ReactNode; where: string }, State> {
  state: State = { error: null, info: "" };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Also lands in the dev console and the webview console.
    console.error(`[${this.props.where}] render failed`, error, info.componentStack);
    this.setState({ info: info.componentStack ?? "" });
  }

  render() {
    const { error, info } = this.state;
    if (!error) return this.props.children;
    return (
      <div style={{ padding: 20, overflow: "auto", height: "100%", userSelect: "text" }}>
        <h1 style={{ fontSize: 16, margin: "0 0 6px" }}>Something went wrong in {this.props.where}.</h1>
        <p style={{ color: "var(--text-muted)", fontSize: 13, margin: "0 0 12px" }}>Reload the window, or report the details below.</p>
        <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
          <button className="btn" onClick={() => location.reload()}>
            Reload
          </button>
          <button className="btn" onClick={() => void navigator.clipboard.writeText(`${error.message}\n${error.stack ?? ""}\n${info}`)}>
            Copy details
          </button>
        </div>
        <pre style={{ whiteSpace: "pre-wrap", fontSize: 12, fontFamily: "var(--font-mono)", background: "var(--code-bg)", padding: 10, borderRadius: 8 }}>
          {error.message}
          {"\n\n"}
          {error.stack}
          {info}
        </pre>
      </div>
    );
  }
}
