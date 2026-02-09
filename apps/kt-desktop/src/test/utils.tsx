import { render, RenderOptions } from "@testing-library/react";
import { ReactElement } from "react";
import { Machine, Session } from "../types";

/**
 * Custom render function that wraps components with necessary providers
 */
function customRender(ui: ReactElement, options?: Omit<RenderOptions, "wrapper">) {
  return render(ui, { ...options });
}

// Re-export everything from testing-library
export * from "@testing-library/react";
export { customRender as render };

// Test data factories

export function createMockMachine(overrides: Partial<Machine> = {}): Machine {
  return {
    id: `machine-${Math.random().toString(36).substr(2, 9)}`,
    alias: "test-machine",
    hostname: "test.local",
    os: "linux",
    arch: "x86_64",
    status: "connected",
    sessionCount: 0,
    tags: [],
    ...overrides,
  };
}

export function createMockSession(overrides: Partial<Session> = {}): Session {
  return {
    id: `session-${Math.random().toString(36).substr(2, 9)}`,
    machineId: "machine-1",
    shell: "/bin/bash",
    createdAt: new Date().toISOString(),
    pid: 12345,
    ...overrides,
  };
}
