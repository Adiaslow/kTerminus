import { create } from "zustand";
import type { OrchestratorStatus } from "../types";

export type ViewMode = "terminals" | "topology" | "health" | "logs";
export type SidebarSection = "machines" | "sessions";

interface AppState {
  // UI state
  viewMode: ViewMode;
  sidebarSection: SidebarSection;
  sidebarWidth: number;
  showSidebar: boolean;

  // Orchestrator state
  orchestratorStatus: OrchestratorStatus | null;
  isConnected: boolean;

  // Actions
  setViewMode: (mode: ViewMode) => void;
  setSidebarSection: (section: SidebarSection) => void;
  setSidebarWidth: (width: number) => void;
  toggleSidebar: () => void;
  setOrchestratorStatus: (status: OrchestratorStatus | null) => void;
  setConnected: (connected: boolean) => void;
}

export const useAppStore = create<AppState>((set) => ({
  viewMode: "terminals",
  sidebarSection: "machines",
  sidebarWidth: 240,
  showSidebar: true,
  orchestratorStatus: null,
  isConnected: false,

  setViewMode: (mode) => set({ viewMode: mode }),
  setSidebarSection: (section) => set({ sidebarSection: section }),
  setSidebarWidth: (width) => set({ sidebarWidth: Math.max(180, Math.min(400, width)) }),
  toggleSidebar: () => set((state) => ({ showSidebar: !state.showSidebar })),
  setOrchestratorStatus: (status) => set({ orchestratorStatus: status }),
  setConnected: (connected) => set({ isConnected: connected }),
}));
