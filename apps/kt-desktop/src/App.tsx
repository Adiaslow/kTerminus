import { useEffect } from "react";
import { Layout } from "./components/layout/Layout";
import { useAppStore } from "./stores/app";
import { useMachinesStore } from "./stores/machines";
import * as tauri from "./lib/tauri";

function App() {
  const setOrchestratorStatus = useAppStore((s) => s.setOrchestratorStatus);
  const setConnected = useAppStore((s) => s.setConnected);
  const setMachines = useMachinesStore((s) => s.setMachines);
  const addMachine = useMachinesStore((s) => s.addMachine);
  const updateMachine = useMachinesStore((s) => s.updateMachine);
  const removeMachine = useMachinesStore((s) => s.removeMachine);

  useEffect(() => {
    // Initial data fetch
    const fetchInitialData = async () => {
      try {
        const status = await tauri.getStatus();
        setOrchestratorStatus(status);
        setConnected(status.running);

        if (status.running) {
          const machines = await tauri.listMachines();
          setMachines(machines);
        }
      } catch (err) {
        console.error("Failed to fetch initial data:", err);
        setConnected(false);
      }
    };

    fetchInitialData();

    // Set up event listeners
    const unlisteners: Promise<() => void>[] = [];

    unlisteners.push(
      tauri.onMachineEvent((event) => {
        switch (event.type) {
          case "connected":
            addMachine(event.machine);
            break;
          case "disconnected":
            removeMachine(event.machine.id);
            break;
          case "updated":
            updateMachine(event.machine.id, event.machine);
            break;
        }
      })
    );

    unlisteners.push(
      tauri.onOrchestratorStatus((status) => {
        setOrchestratorStatus(status);
        setConnected(status.running);
      })
    );

    // Cleanup event listeners
    return () => {
      unlisteners.forEach((p) => p.then((unlisten) => unlisten()));
    };
  }, [
    setOrchestratorStatus,
    setConnected,
    setMachines,
    addMachine,
    updateMachine,
    removeMachine,
  ]);

  return <Layout />;
}

export default App;
