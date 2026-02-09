// @refresh reset
import { create } from "zustand";
import type { TerminalTab, Session } from "../types";

interface TerminalsState {
  tabs: TerminalTab[];
  activeTabId: string | null;
  sessions: Map<string, Session>;

  // Actions
  addTab: (tab: TerminalTab) => void;
  removeTab: (id: string) => void;
  setActiveTab: (id: string | null) => void;
  updateTabTitle: (id: string, title: string) => void;
  addSession: (session: Session) => void;
  removeSession: (sessionId: string) => void;
}

export const useTerminalsStore = create<TerminalsState>((set) => ({
  tabs: [],
  activeTabId: null,
  sessions: new Map(),

  addTab: (tab) =>
    set((state) => {
      console.info("[terminals] addTab:", tab.id, new Error().stack?.split('\n').slice(0, 5).join('\n'));
      // Prevent duplicate tabs by checking ID
      if (state.tabs.some((t) => t.id === tab.id)) {
        console.warn(`[terminals] Attempted to add duplicate tab with id: ${tab.id}`);
        return state; // Return unchanged state
      }
      // Also check for duplicate sessionId to prevent orphaned tabs
      if (state.tabs.some((t) => t.sessionId === tab.sessionId)) {
        console.warn(`[terminals] Attempted to add tab with duplicate sessionId: ${tab.sessionId}`);
        return state; // Return unchanged state
      }
      return {
        tabs: [...state.tabs, tab],
        activeTabId: tab.id,
      };
    }),

  removeTab: (id) =>
    set((state) => {
      const newTabs = state.tabs.filter((t) => t.id !== id);
      let newActiveId = state.activeTabId;

      if (state.activeTabId === id) {
        // Activate the previous tab, or the next one, or null
        const index = state.tabs.findIndex((t) => t.id === id);
        if (newTabs.length > 0) {
          const newIndex = Math.min(index, newTabs.length - 1);
          newActiveId = newTabs[newIndex].id;
        } else {
          newActiveId = null;
        }
      }

      return { tabs: newTabs, activeTabId: newActiveId };
    }),

  setActiveTab: (id) =>
    set(() => ({
      activeTabId: id,
    })),

  updateTabTitle: (id, title) =>
    set((state) => ({
      tabs: state.tabs.map((t) => (t.id === id ? { ...t, title } : t)),
    })),

  addSession: (session) =>
    set((state) => {
      // Warn if session already exists (may be a reconnection, which is fine)
      if (state.sessions.has(session.id)) {
        console.warn(`[terminals] Session with id already exists, updating: ${session.id}`);
      }
      const newSessions = new Map(state.sessions);
      newSessions.set(session.id, session);
      return { sessions: newSessions };
    }),

  removeSession: (sessionId) =>
    set((state) => {
      const newSessions = new Map(state.sessions);
      newSessions.delete(sessionId);
      return { sessions: newSessions };
    }),
}));

// Reset terminals on HMR to prevent stale state
// Also reset layout since it depends on terminals
if (import.meta.hot) {
  import.meta.hot.accept(async () => {
    // Reset terminals store
    useTerminalsStore.setState({
      tabs: [],
      activeTabId: null,
      sessions: new Map(),
    });
    // Also reset layout store to prevent stale pane references
    const { useLayoutStore } = await import("./layout");
    useLayoutStore.getState().resetLayout();
  });
}
