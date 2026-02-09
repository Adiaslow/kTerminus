import { useEffect, useRef, useMemo } from "react";
import { Layout } from "./components/layout/Layout";
import { ToastContainer } from "./components/Toast";
import { useAppStore } from "./stores/app";
import { useMachinesStore } from "./stores/machines";
import { useTerminalsStore } from "./stores/terminals";
import { useSyncStore } from "./stores/sync";
import { toast } from "./stores/toast";
import * as tauri from "./lib/tauri";

/**
 * Custom hook to get stable store action references.
 *
 * This pattern extracts store actions once via getState() and memoizes them.
 * Actions in Zustand stores are stable references that never change, so this
 * is safe and avoids the anti-pattern of calling getState() at module level.
 */
function useStoreActions() {
  return useMemo(() => ({
    // App store actions
    setOrchestratorStatus: useAppStore.getState().setOrchestratorStatus,
    setConnected: useAppStore.getState().setConnected,
    // Machines store actions
    setMachines: useMachinesStore.getState().setMachines,
    addMachine: useMachinesStore.getState().addMachine,
    updateMachine: useMachinesStore.getState().updateMachine,
    removeMachine: useMachinesStore.getState().removeMachine,
    // Terminals store actions
    removeTab: useTerminalsStore.getState().removeTab,
    removeSession: useTerminalsStore.getState().removeSession,
    // Sync store actions
    setLastSeq: useSyncStore.getState().setLastSeq,
    setEpochId: useSyncStore.getState().setEpochId,
    reconcile: useSyncStore.getState().reconcile,
  }), []);
}

function App() {
  // Get stable action references from stores via custom hook
  const {
    setOrchestratorStatus,
    setConnected,
    setMachines,
    addMachine,
    updateMachine,
    removeMachine,
    removeTab,
    removeSession,
    setLastSeq,
    setEpochId,
    reconcile,
  } = useStoreActions();

  // Use ref to access current tabs in event handlers without causing re-renders
  const tabsRef = useRef(useTerminalsStore.getState().tabs);

  // Subscribe to tabs changes and update ref
  useEffect(() => {
    return useTerminalsStore.subscribe((state) => {
      tabsRef.current = state.tabs;
    });
  }, []);

  useEffect(() => {
    // Clear any stale tabs/sessions from HMR on startup
    // Sessions don't persist on the backend, so any existing tabs are stale
    const currentTabs = useTerminalsStore.getState().tabs;
    if (currentTabs.length > 0) {
      console.info("[App] Clearing stale tabs/sessions on startup");
      useTerminalsStore.setState({
        tabs: [],
        activeTabId: null,
        sessions: new Map(),
      });
    }

    // Use AbortController for cleaner async cleanup
    const abortController = new AbortController();
    const { signal } = abortController;

    // Track unlisteners for cleanup
    const unlisteners: (() => void)[] = [];

    // Helper to safely add unlistener only if not aborted
    const registerUnlistener = (unlisten: () => void) => {
      if (signal.aborted) {
        // Already unmounted, clean up immediately
        unlisten();
      } else {
        unlisteners.push(unlisten);
      }
    };

    // Initial data fetch with state snapshot synchronization
    const fetchInitialData = async () => {
      try {
        const status = await tauri.getStatus();
        if (signal.aborted) return;
        setOrchestratorStatus(status);
        setConnected(status.running);

        if (status.running) {
          // Fetch state snapshot for synchronized initialization
          try {
            const snapshot = await tauri.getStateSnapshot();
            if (signal.aborted) return;

            // Initialize sync store with epoch and sequence
            setEpochId(snapshot.epochId);
            setLastSeq(snapshot.currentSeq);

            // Set machines from snapshot
            setMachines(snapshot.machines);

            console.info(
              `[App] Initialized from snapshot: epoch=${snapshot.epochId}, seq=${snapshot.currentSeq}, machines=${snapshot.machines.length}`
            );
          } catch (snapshotErr) {
            // Fallback to legacy listMachines if snapshot not available
            console.warn("[App] State snapshot not available, falling back to listMachines:", snapshotErr);
            const machines = await tauri.listMachines();
            if (signal.aborted) return;
            setMachines(machines);
          }
        }
      } catch (err) {
        if (signal.aborted) return;
        console.error("Failed to fetch initial data:", err);
        setConnected(false);
        toast.error("Failed to connect to orchestrator. Please ensure it is running.");
      }
    };

    fetchInitialData();

    // Set up event listeners
    tauri.onMachineEvent((event) => {
      if (signal.aborted) return;
      switch (event.type) {
        case "connected":
          if (event.machine) addMachine(event.machine);
          break;
        case "disconnected":
          if (event.machineId) removeMachine(event.machineId);
          break;
        case "updated":
          if (event.machine) updateMachine(event.machine.id, event.machine);
          break;
      }
    }).then(registerUnlistener);

    tauri.onOrchestratorStatus((status) => {
      if (signal.aborted) return;
      setOrchestratorStatus(status);
      setConnected(status.running);
    }).then(registerUnlistener);

    tauri.onSessionEvent((event) => {
      if (signal.aborted) return;
      switch (event.type) {
        case "closed":
          // Find and remove the tab for this session
          if (event.sessionId) {
            const tab = tabsRef.current.find((t) => t.sessionId === event.sessionId);
            if (tab) {
              removeTab(tab.id);
            }
            removeSession(event.sessionId);
          }
          break;
      }
    }).then(registerUnlistener);

    // Cleanup event listeners
    return () => {
      abortController.abort();
      // Clean up all registered listeners
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [
    // Actions are stable references from useStoreActions() hook, tabs accessed via ref
    setOrchestratorStatus,
    setConnected,
    setMachines,
    addMachine,
    updateMachine,
    removeMachine,
    removeTab,
    removeSession,
    setLastSeq,
    setEpochId,
    reconcile,
  ]);

  return (
    <>
      <Layout />
      <ToastContainer />
    </>
  );
}

export default App;
