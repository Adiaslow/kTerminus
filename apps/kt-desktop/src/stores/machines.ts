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
    set((state) => ({
      machines: [...state.machines, machine],
    })),

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
