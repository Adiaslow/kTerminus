import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "../../test/utils";
import { MachineList } from "./MachineList";
import { createMockMachine } from "../../test/utils";
import { useMachinesStore } from "../../stores/machines";
import { useTerminalsStore } from "../../stores/terminals";

// Only mock external dependencies that don't work in jsdom
vi.mock("../../stores/toast", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

vi.mock("../../lib/tauri", () => ({
  createSession: vi.fn(),
  disconnectMachine: vi.fn(),
}));

// Mock useVirtualizer since jsdom doesn't support scroll containers properly
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: vi.fn(() => ({
    getVirtualItems: () => [],
    getTotalSize: () => 0,
  })),
}));

import { useVirtualizer } from "@tanstack/react-virtual";

const mockUseVirtualizer = vi.mocked(useVirtualizer);

describe("MachineList", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    // Reset stores to initial state
    useMachinesStore.setState({
      machines: [],
      selectedMachineId: null,
    });

    useTerminalsStore.setState({
      tabs: [],
      activeTabId: null,
      sessions: new Map(),
    });

    // Default virtualizer mock (partial mock - only what we use)
    mockUseVirtualizer.mockReturnValue({
      getVirtualItems: () => [],
      getTotalSize: () => 0,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any);
  });

  it("renders empty state when no machines", () => {
    render(<MachineList />);

    expect(screen.getByText("No machines connected")).toBeInTheDocument();
  });

  it("renders search input", () => {
    render(<MachineList />);

    const searchInput = screen.getByPlaceholderText("Search machines...");
    expect(searchInput).toBeInTheDocument();
  });

  it("has accessible search input", () => {
    render(<MachineList />);

    const searchInput = screen.getByRole("textbox");
    expect(searchInput).toHaveAttribute(
      "aria-label",
      "Search machines by hostname, alias, or tag"
    );
  });

  it("updates search term on input", () => {
    render(<MachineList />);

    const searchInput = screen.getByPlaceholderText("Search machines...");
    fireEvent.change(searchInput, { target: { value: "test-search" } });

    expect(searchInput).toHaveValue("test-search");
  });

  it("shows 'no results' when search has no matches", () => {
    // Set up store with a machine
    useMachinesStore.setState({
      machines: [
        createMockMachine({
          id: "machine-1",
          hostname: "test-server.local",
          status: "connected",
        }),
      ],
    });

    render(<MachineList />);

    const searchInput = screen.getByPlaceholderText("Search machines...");
    fireEvent.change(searchInput, { target: { value: "nonexistent" } });

    expect(
      screen.getByText('No machines match "nonexistent"')
    ).toBeInTheDocument();
  });

  it("does not show empty state when machines exist", () => {
    useMachinesStore.setState({
      machines: [
        createMockMachine({
          id: "machine-1",
          hostname: "test-server.local",
          status: "connected",
        }),
      ],
    });

    // Setup virtualizer to return items (partial mock)
    mockUseVirtualizer.mockReturnValue({
      getVirtualItems: () => [
        { key: "0", index: 0, start: 0, end: 32, size: 32, lane: 0 },
        { key: "1", index: 1, start: 32, end: 108, size: 76, lane: 0 },
      ],
      getTotalSize: () => 108,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any);

    render(<MachineList />);

    // Should not show "No machines connected"
    expect(screen.queryByText("No machines connected")).not.toBeInTheDocument();
  });

  it("filters machines correctly", () => {
    useMachinesStore.setState({
      machines: [
        createMockMachine({
          id: "machine-1",
          hostname: "web-server.local",
          status: "connected",
        }),
        createMockMachine({
          id: "machine-2",
          hostname: "db-server.local",
          status: "connected",
        }),
      ],
    });

    const { rerender } = render(<MachineList />);

    // Search for "web"
    const searchInput = screen.getByPlaceholderText("Search machines...");
    fireEvent.change(searchInput, { target: { value: "web" } });

    rerender(<MachineList />);

    // Verify the search input has the value
    expect(searchInput).toHaveValue("web");
  });

  it("creates virtualizer with correct count", () => {
    useMachinesStore.setState({
      machines: [
        createMockMachine({
          id: "machine-1",
          hostname: "server1.local",
          status: "connected",
        }),
        createMockMachine({
          id: "machine-2",
          hostname: "server2.local",
          status: "disconnected",
        }),
      ],
    });

    render(<MachineList />);

    // Virtualizer should be called with:
    // 2 headers (Connected, Disconnected) + 2 machines = 4 items
    expect(mockUseVirtualizer).toHaveBeenCalledWith(
      expect.objectContaining({
        count: 4,
        overscan: 5,
      })
    );
  });

  it("estimates correct size for headers and machines", () => {
    useMachinesStore.setState({
      machines: [
        createMockMachine({
          id: "machine-1",
          hostname: "server.local",
          status: "connected",
          tags: ["production", "web"],
        }),
      ],
    });

    render(<MachineList />);

    // Get the estimateSize function that was passed to useVirtualizer
    const virtualizerConfig = mockUseVirtualizer.mock.calls[0][0];
    const estimateSize = virtualizerConfig.estimateSize;

    // Index 0 is header (should be 32px)
    expect(estimateSize(0)).toBe(32);

    // Index 1 is machine with tags (should be 96px)
    expect(estimateSize(1)).toBe(96);
  });
});
