import { create } from "zustand";
import * as tauri from "../lib/tauri";
import { useMachinesStore } from "./machines";
import { useTerminalsStore } from "./terminals";
import type { Machine, Session } from "../types";

/**
 * State snapshot returned from the orchestrator.
 * Contains the current epoch, sequence number, and all state.
 */
export interface StateSnapshot {
  epochId: string;
  currentSeq: number;
  machines: Machine[];
  sessions: Session[];
}

/**
 * Event envelope wrapping IPC events with sequence metadata.
 * Used for ordering and gap detection.
 */
export interface IpcEventEnvelope {
  seq: number;
  timestamp: number;
  event: unknown;
  sessionSeq?: number;
}

interface SyncState {
  /** Last processed sequence number */
  lastSeq: number;
  /** Current epoch ID (changes on orchestrator restart) */
  epochId: string | null;
  /** Whether reconciliation is in progress */
  isReconciling: boolean;

  // Actions
  setLastSeq: (seq: number) => void;
  setEpochId: (epochId: string) => void;
  reconcile: () => Promise<void>;
  handleEvent: (envelope: IpcEventEnvelope) => void;
  resetAll: () => void;
}

export const useSyncStore = create<SyncState>((set, get) => ({
  lastSeq: 0,
  epochId: null,
  isReconciling: false,

  setLastSeq: (seq) => set({ lastSeq: seq }),
  setEpochId: (epochId) => set({ epochId }),

  reconcile: async () => {
    // Prevent concurrent reconciliation
    if (get().isReconciling) return;
    set({ isReconciling: true });

    try {
      const snapshot = await tauri.getStateSnapshot();

      // Check for epoch change (orchestrator restart)
      const currentEpochId = get().epochId;
      if (currentEpochId && currentEpochId !== snapshot.epochId) {
        console.warn("[sync] Epoch changed, resetting all state");
        // Reset terminals as sessions don't persist across restarts
        useTerminalsStore.setState({
          tabs: [],
          activeTabId: null,
          sessions: new Map(),
        });
      }

      // Update machines from snapshot
      useMachinesStore.getState().setMachines(snapshot.machines);

      // Prune stale sessions - remove tabs for sessions that no longer exist
      const localTabs = useTerminalsStore.getState().tabs;
      const remoteSessionIds = new Set(snapshot.sessions.map((s) => s.id));

      for (const tab of localTabs) {
        if (!remoteSessionIds.has(tab.sessionId)) {
          console.info(`[sync] Pruning stale tab for session: ${tab.sessionId}`);
          useTerminalsStore.getState().removeTab(tab.id);
          useTerminalsStore.getState().removeSession(tab.sessionId);
        }
      }

      // Update sync state
      set({
        lastSeq: snapshot.currentSeq,
        epochId: snapshot.epochId,
      });

      console.info(`[sync] Reconciled: epoch=${snapshot.epochId}, seq=${snapshot.currentSeq}`);
    } catch (err) {
      console.error("[sync] Reconciliation failed:", err);
    } finally {
      set({ isReconciling: false });
    }
  },

  handleEvent: (envelope) => {
    const { lastSeq } = get();

    // Gap detection: if we receive a sequence number greater than expected,
    // we may have missed events
    if (envelope.seq > lastSeq + 1 && lastSeq > 0) {
      console.warn(
        `[sync] Sequence gap detected: expected ${lastSeq + 1}, got ${envelope.seq}`
      );
      // Trigger reconciliation to get back in sync
      get().reconcile();
      return;
    }

    // Update sequence number
    set({ lastSeq: envelope.seq });

    // Forward event to appropriate stores based on event type
    // The actual event handling is done by existing listeners in App.tsx
    // This method primarily handles sequence tracking and gap detection
    //
    // TODO: In a future iteration, centralize all event handling here
    // by parsing envelope.event and dispatching to the appropriate store actions
  },

  resetAll: () => {
    set({
      lastSeq: 0,
      epochId: null,
      isReconciling: false,
    });
  },
}));
