// Machine types
export interface Machine {
  id: string;
  alias?: string;
  hostname: string;
  os: string;
  arch: string;
  status: MachineStatus;
  connectedAt?: string;
  lastHeartbeat?: string;
  sessionCount: number;
  tags?: string[];
}

export type MachineStatus = "connected" | "disconnected" | "connecting";

// Session types
export interface Session {
  id: string;
  machineId: string;
  shell?: string;
  createdAt: string;
  pid?: number;
}

// Terminal types
export interface TerminalTab {
  id: string;
  sessionId: string;
  machineId: string;
  title: string;
  active: boolean;
}

// Orchestrator status
export interface OrchestratorStatus {
  running: boolean;
  uptimeSecs: number;
  machineCount: number;
  sessionCount: number;
  version: string;
}

// IPC message types (for Tauri commands)
export interface CreateSessionParams {
  machineId: string;
  shell?: string;
}

export interface TerminalWriteParams {
  sessionId: string;
  data: Uint8Array;
}

export interface TerminalResizeParams {
  sessionId: string;
  cols: number;
  rows: number;
}

// Event types from Tauri
export interface MachineEvent {
  type: "connected" | "disconnected" | "updated";
  machine: Machine;
}

export interface SessionEvent {
  type: "created" | "closed";
  session: Session;
  exitCode?: number;
}

export interface TerminalOutputEvent {
  sessionId: string;
  data: Uint8Array;
}

// Topology node types for React Flow
export interface MachineNode {
  id: string;
  type: "machine";
  position: { x: number; y: number };
  data: Machine;
}

export interface OrchestratorNode {
  id: string;
  type: "orchestrator";
  position: { x: number; y: number };
  data: OrchestratorStatus;
}

export type TopologyNode = MachineNode | OrchestratorNode;
