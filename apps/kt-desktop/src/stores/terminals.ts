import { create } from "zustand";
import type { TerminalTab, Session } from "../types";

interface TerminalsState {
  tabs: TerminalTab[];
  activeTabId: string | null;
  sessions: Map<string, Session>;

  // Actions
  addTab: (tab: TerminalTab) => void;
  removeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  updateTabTitle: (id: string, title: string) => void;
  addSession: (session: Session) => void;
  removeSession: (sessionId: string) => void;
}

export const useTerminalsStore = create<TerminalsState>((set) => ({
  tabs: [],
  activeTabId: null,
  sessions: new Map(),

  addTab: (tab) =>
    set((state) => ({
      tabs: [...state.tabs.map((t) => ({ ...t, active: false })), { ...tab, active: true }],
      activeTabId: tab.id,
    })),

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
          newTabs[newIndex].active = true;
        } else {
          newActiveId = null;
        }
      }

      return { tabs: newTabs, activeTabId: newActiveId };
    }),

  setActiveTab: (id) =>
    set((state) => ({
      tabs: state.tabs.map((t) => ({ ...t, active: t.id === id })),
      activeTabId: id,
    })),

  updateTabTitle: (id, title) =>
    set((state) => ({
      tabs: state.tabs.map((t) => (t.id === id ? { ...t, title } : t)),
    })),

  addSession: (session) =>
    set((state) => {
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
