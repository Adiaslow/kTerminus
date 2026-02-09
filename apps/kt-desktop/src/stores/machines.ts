import { create } from "zustand";
import type { Machine } from "../types";

interface MachinesState {
  machines: Machine[];
  selectedMachineId: string | null;

  // Actions
  setMachines: (machines: Machine[]) => void;
  addMachine: (machine: Machine) => void;
  updateMachine: (id: string, updates: Partial<Machine>) => void;
  removeMachine: (id: string) => void;
  selectMachine: (id: string | null) => void;
}

export const useMachinesStore = create<MachinesState>((set) => ({
  machines: [],
  selectedMachineId: null,

  setMachines: (machines) => set({ machines }),

  addMachine: (machine) =>
    set((state) => {
      // Prevent duplicate machines by checking ID
      if (state.machines.some((m) => m.id === machine.id)) {
        console.warn(`[machines] Attempted to add duplicate machine with id: ${machine.id}`);
        return state; // Return unchanged state
      }
      return {
        machines: [...state.machines, machine],
      };
    }),

  updateMachine: (id, updates) =>
    set((state) => ({
      machines: state.machines.map((m) =>
        m.id === id ? { ...m, ...updates } : m
      ),
    })),

  removeMachine: (id) =>
    set((state) => ({
      machines: state.machines.filter((m) => m.id !== id),
      selectedMachineId:
        state.selectedMachineId === id ? null : state.selectedMachineId,
    })),

  selectMachine: (id) => set({ selectedMachineId: id }),
}));
