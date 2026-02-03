import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Machine,
  Session,
  OrchestratorStatus,
  MachineEvent,
  SessionEvent,
  TerminalOutputEvent,
} from "../types";

// Orchestrator commands
export async function getStatus(): Promise<OrchestratorStatus> {
  return invoke("get_status");
}

export async function startOrchestrator(): Promise<void> {
  return invoke("start_orchestrator");
}

export async function stopOrchestrator(): Promise<void> {
  return invoke("stop_orchestrator");
}

// Machine commands
export async function listMachines(): Promise<Machine[]> {
  return invoke("list_machines");
}

export async function getMachine(id: string): Promise<Machine> {
  return invoke("get_machine", { id });
}

export async function disconnectMachine(id: string): Promise<void> {
  return invoke("disconnect_machine", { id });
}

// Session commands
export async function listSessions(machineId?: string): Promise<Session[]> {
  return invoke("list_sessions", { machineId });
}

export async function createSession(machineId: string, shell?: string): Promise<Session> {
  return invoke("create_session", { machineId, shell });
}

export async function killSession(sessionId: string, force: boolean = false): Promise<void> {
  return invoke("kill_session", { sessionId, force });
}

// Terminal I/O commands
export async function terminalWrite(sessionId: string, data: Uint8Array): Promise<void> {
  return invoke("terminal_write", { sessionId, data: Array.from(data) });
}

export async function terminalResize(sessionId: string, cols: number, rows: number): Promise<void> {
  return invoke("terminal_resize", { sessionId, cols, rows });
}

export async function terminalClose(sessionId: string): Promise<void> {
  return invoke("terminal_close", { sessionId });
}

// Session subscription commands
export async function subscribeSession(sessionId: string): Promise<void> {
  return invoke("subscribe_session", { sessionId });
}

export async function unsubscribeSession(sessionId: string): Promise<void> {
  return invoke("unsubscribe_session", { sessionId });
}

// Event listeners
export function onMachineEvent(callback: (event: MachineEvent) => void): Promise<UnlistenFn> {
  return listen<MachineEvent>("machine-event", (event) => callback(event.payload));
}

export function onSessionEvent(callback: (event: SessionEvent) => void): Promise<UnlistenFn> {
  return listen<SessionEvent>("session-event", (event) => callback(event.payload));
}

export function onTerminalOutput(
  sessionId: string,
  callback: (data: Uint8Array) => void
): Promise<UnlistenFn> {
  return listen<TerminalOutputEvent>(`terminal-output:${sessionId}`, (event) => {
    // Data comes as a JSON array of numbers from Rust Vec<u8>, convert to Uint8Array
    const data = event.payload.data;
    const bytes = Array.isArray(data) ? new Uint8Array(data) : data;
    callback(bytes);
  });
}

export function onOrchestratorStatus(
  callback: (status: OrchestratorStatus) => void
): Promise<UnlistenFn> {
  return listen<OrchestratorStatus>("orchestrator-status", (event) =>
    callback(event.payload)
  );
}

// Utility to convert string to Uint8Array for terminal input
export function stringToBytes(str: string): Uint8Array {
  return new TextEncoder().encode(str);
}

// Utility to convert Uint8Array to string for terminal output
export function bytesToString(bytes: Uint8Array): string {
  return new TextDecoder().decode(bytes);
}
