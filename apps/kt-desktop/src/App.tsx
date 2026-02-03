import { useEffect } from "react";
import { Layout } from "./components/layout/Layout";
import { useAppStore } from "./stores/app";
import { useMachinesStore } from "./stores/machines";
import { useTerminalsStore } from "./stores/terminals";
import * as tauri from "./lib/tauri";

function App() {
  const setOrchestratorStatus = useAppStore((s) => s.setOrchestratorStatus);
  const setConnected = useAppStore((s) => s.setConnected);
  const setMachines = useMachinesStore((s) => s.setMachines);
  const addMachine = useMachinesStore((s) => s.addMachine);
  const updateMachine = useMachinesStore((s) => s.updateMachine);
  const removeMachine = useMachinesStore((s) => s.removeMachine);
  const tabs = useTerminalsStore((s) => s.tabs);
  const removeTab = useTerminalsStore((s) => s.removeTab);
  const removeSession = useTerminalsStore((s) => s.removeSession);

  useEffect(() => {
    let isMounted = true;

    // Initial data fetch
    const fetchInitialData = async () => {
      try {
        const status = await tauri.getStatus();
        if (!isMounted) return;
        setOrchestratorStatus(status);
        setConnected(status.running);

        if (status.running) {
          const machines = await tauri.listMachines();
          if (!isMounted) return;
          setMachines(machines);
        }
      } catch (err) {
        if (!isMounted) return;
        console.error("Failed to fetch initial data:", err);
        setConnected(false);
      }
    };

    fetchInitialData();

    // Set up event listeners with proper cleanup tracking
    const unlisteners: (() => void)[] = [];
    const pendingListeners: Promise<() => void>[] = [];

    pendingListeners.push(
      tauri.onMachineEvent((event) => {
        if (!isMounted) return;
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
      }).then((unlisten) => {
        if (isMounted) {
          unlisteners.push(unlisten);
        } else {
          unlisten();
        }
        return unlisten;
      })
    );

    pendingListeners.push(
      tauri.onOrchestratorStatus((status) => {
        if (!isMounted) return;
        setOrchestratorStatus(status);
        setConnected(status.running);
      }).then((unlisten) => {
        if (isMounted) {
          unlisteners.push(unlisten);
        } else {
          unlisten();
        }
        return unlisten;
      })
    );

    pendingListeners.push(
      tauri.onSessionEvent((event) => {
        if (!isMounted) return;
        switch (event.type) {
          case "closed":
            // Find and remove the tab for this session
            if (event.sessionId) {
              const tab = tabs.find((t) => t.sessionId === event.sessionId);
              if (tab) {
                removeTab(tab.id);
              }
              removeSession(event.sessionId);
            }
            break;
        }
      }).then((unlisten) => {
        if (isMounted) {
          unlisteners.push(unlisten);
        } else {
          unlisten();
        }
        return unlisten;
      })
    );

    // Cleanup event listeners
    return () => {
      isMounted = false;
      // Clean up already-resolved listeners
      unlisteners.forEach((unlisten) => unlisten());
      // Clean up any that resolve after unmount (handled in .then above)
    };
  }, [
    setOrchestratorStatus,
    setConnected,
    setMachines,
    addMachine,
    updateMachine,
    removeMachine,
    tabs,
    removeTab,
    removeSession,
  ]);

  return <Layout />;
}

export default App;
