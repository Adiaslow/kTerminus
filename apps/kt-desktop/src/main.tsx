// Startup guard: prevent click event replay from bfcache/session restore
// Safari/WebKit can replay click events from before the page was unloaded
// This flag is checked by click handlers to ignore stale replayed events
declare global {
  interface Window {
    __appReady?: boolean;
  }
}
window.__appReady = false;
const startTime = Date.now();
console.info("[main] App starting at", startTime);
// Mark app as ready after a delay to let any replayed events fire first
setTimeout(() => {
  window.__appReady = true;
  console.info("[main] App ready at", Date.now(), "delta:", Date.now() - startTime);
}, 500);

// Clear stale zustand state before React renders
import { useTerminalsStore } from "./stores/terminals";
import { useLayoutStore } from "./stores/layout";

// Clear any stale store state (from HMR or browser session restore)
const terminalState = useTerminalsStore.getState();
const layoutState = useLayoutStore.getState();
if (terminalState.tabs.length > 0 || layoutState.layout.root !== null || terminalState.activeTabId !== null) {
  useTerminalsStore.setState({
    tabs: [],
    activeTabId: null,
    sessions: new Map(),
  });
  useLayoutStore.getState().resetLayout();
}

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>
);
