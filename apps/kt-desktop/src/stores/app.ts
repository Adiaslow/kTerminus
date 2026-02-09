import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { OrchestratorStatus } from "../types";

export type ViewMode = "terminals" | "topology" | "health" | "logs";
export type SidebarSection = "machines" | "sessions";

interface AppState {
  // UI state (persisted)
  viewMode: ViewMode;
  sidebarSection: SidebarSection;
  sidebarWidth: number;
  showSidebar: boolean;

  // Orchestrator state (not persisted)
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

/**
 * Validate persisted app state to handle corrupted localStorage
 */
function validatePersistedState(data: unknown): Partial<AppState> | null {
  if (!data || typeof data !== "object") return null;

  const obj = data as Record<string, unknown>;
  const result: Partial<AppState> = {};

  // Validate viewMode
  const validViewModes: ViewMode[] = ["terminals", "topology", "health", "logs"];
  if (typeof obj.viewMode === "string" && validViewModes.includes(obj.viewMode as ViewMode)) {
    result.viewMode = obj.viewMode as ViewMode;
  }

  // Validate sidebarSection
  const validSections: SidebarSection[] = ["machines", "sessions"];
  if (typeof obj.sidebarSection === "string" && validSections.includes(obj.sidebarSection as SidebarSection)) {
    result.sidebarSection = obj.sidebarSection as SidebarSection;
  }

  // Validate sidebarWidth (number between 180 and 400)
  if (typeof obj.sidebarWidth === "number" && obj.sidebarWidth >= 180 && obj.sidebarWidth <= 400) {
    result.sidebarWidth = obj.sidebarWidth;
  }

  // Validate showSidebar
  if (typeof obj.showSidebar === "boolean") {
    result.showSidebar = obj.showSidebar;
  }

  return result;
}

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
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
    }),
    {
      name: "kt-app",
      // Only persist UI preferences, not runtime state
      partialize: (state) => ({
        viewMode: state.viewMode,
        sidebarSection: state.sidebarSection,
        sidebarWidth: state.sidebarWidth,
        showSidebar: state.showSidebar,
      }),
      // Validate persisted data before merging
      merge: (persistedState, currentState) => {
        const validated = validatePersistedState(persistedState);
        if (!validated) {
          console.warn("App persistence: invalid data detected, using defaults");
          return currentState;
        }
        return {
          ...currentState,
          ...validated,
        };
      },
    }
  )
);
