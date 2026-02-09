import { useMemo, useCallback, useEffect } from "react";
import {
  ReactFlow,
  type Node,
  type Edge,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  useReactFlow,
  ReactFlowProvider,
  BackgroundVariant,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { useMachinesStore } from "../../stores/machines";
import { useAppStore } from "../../stores/app";
import { MachineNode } from "./MachineNode";
import { OrchestratorNode } from "./OrchestratorNode";
import { topologyColors as colors } from "../../lib/theme";
import type { Machine, OrchestratorStatus } from "../../types";

// Type-safe node data types for ReactFlow
type MachineNodeData = { machine: Machine };
type OrchestratorNodeData = { status: OrchestratorStatus };

// Union type for all node data
type TopologyNodeData = MachineNodeData | OrchestratorNodeData;

// Type guard to check if node data is a machine node
function isMachineNodeData(data: TopologyNodeData): data is MachineNodeData {
  return "machine" in data;
}

const nodeTypes = {
  machine: MachineNode,
  orchestrator: OrchestratorNode,
};

// Node dimensions for layout calculation (must match actual rendered sizes)
const NODE_WIDTH = 180;
const NODE_HEIGHT = 120;
const ORCHESTRATOR_WIDTH = 220;
const ORCHESTRATOR_HEIGHT = 320; // Includes pairing code section
const HORIZONTAL_SPACING = 80;
const VERTICAL_SPACING = 60;

/**
 * Simple hierarchical layout - orchestrator at top center, machines below in a row
 */
function applyLayout(nodes: Node[]): Node[] {
  const orchestratorNode = nodes.find(n => n.type === "orchestrator");
  const machineNodes = nodes.filter(n => n.type === "machine");

  const result: Node[] = [];

  // Calculate total width needed for machine row
  const machineRowWidth = machineNodes.length > 0
    ? machineNodes.length * NODE_WIDTH + (machineNodes.length - 1) * HORIZONTAL_SPACING
    : 0;

  // Use the wider of orchestrator or machine row for centering
  const totalWidth = Math.max(machineRowWidth, ORCHESTRATOR_WIDTH);

  // Position orchestrator at top center
  if (orchestratorNode) {
    result.push({
      ...orchestratorNode,
      position: {
        x: totalWidth / 2 - ORCHESTRATOR_WIDTH / 2,
        y: 0,
      },
      width: ORCHESTRATOR_WIDTH,
      height: ORCHESTRATOR_HEIGHT,
    });
  }

  // Position machines in a centered row below orchestrator
  const machineY = ORCHESTRATOR_HEIGHT + VERTICAL_SPACING;
  const machineStartX = (totalWidth - machineRowWidth) / 2;

  machineNodes.forEach((node, index) => {
    result.push({
      ...node,
      position: {
        x: machineStartX + index * (NODE_WIDTH + HORIZONTAL_SPACING),
        y: machineY,
      },
      width: NODE_WIDTH,
      height: NODE_HEIGHT,
    });
  });

  return result;
}

function TopologyViewInner() {
  const machines = useMachinesStore((s) => s.machines);
  const status = useAppStore((s) => s.orchestratorStatus);
  const { fitView } = useReactFlow();

  // Generate nodes and edges from machines
  const { rawNodes, rawEdges } = useMemo(() => {
    const nodes: Node[] = [];
    const edges: Edge[] = [];

    const defaultStatus: OrchestratorStatus = {
      running: false,
      uptimeSecs: 0,
      machineCount: 0,
      sessionCount: 0,
      version: "unknown",
    };

    // Orchestrator node (will be positioned by layout)
    nodes.push({
      id: "orchestrator",
      type: "orchestrator",
      position: { x: 0, y: 0 },
      data: { status: status || defaultStatus },
    });

    // Machine nodes
    machines.forEach((machine: Machine) => {
      nodes.push({
        id: machine.id,
        type: "machine",
        position: { x: 0, y: 0 },
        data: { machine },
      });

      // Edge from orchestrator to machine
      const edgeColor = machine.status === "connected"
        ? colors.sage
        : machine.status === "connecting"
          ? colors.ochre
          : colors.terracottaDim;

      edges.push({
        id: `edge-${machine.id}`,
        source: "orchestrator",
        target: machine.id,
        animated: machine.status === "connected",
        style: {
          stroke: edgeColor,
          strokeWidth: 1.5,
        },
      });
    });

    return { rawNodes: nodes, rawEdges: edges };
  }, [machines, status]);

  // Apply layout to nodes
  const layoutedNodes = useMemo(() => {
    return applyLayout(rawNodes);
  }, [rawNodes]);

  const [nodes, setNodes, onNodesChange] = useNodesState(layoutedNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(rawEdges);

  // Update nodes when machines change and fit view
  useEffect(() => {
    setNodes(layoutedNodes);
    setEdges(rawEdges);
  }, [layoutedNodes, rawEdges, setNodes, setEdges]);

  // Fit view after nodes are set (separate effect to ensure nodes are rendered)
  useEffect(() => {
    if (nodes.length > 0) {
      const timer = setTimeout(() => {
        fitView({ padding: 0.4, duration: 200 });
      }, 100);
      return () => clearTimeout(timer);
    }
  }, [nodes.length, fitView]);

  const onNodeClick = useCallback((_event: React.MouseEvent, node: Node) => {
    if (node.type === "machine") {
      useMachinesStore.getState().selectMachine(node.id);
    }
  }, []);

  // Fit view when ReactFlow initializes
  const onInit = useCallback(() => {
    setTimeout(() => {
      fitView({ padding: 0.4, duration: 200 });
    }, 50);
  }, [fitView]);

  return (
    <div className="h-full w-full bg-bg-void">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={onNodeClick}
        onInit={onInit}
        nodeTypes={nodeTypes}
        fitView
        fitViewOptions={{ padding: 0.4 }}
        minZoom={0.3}
        maxZoom={2}
        defaultEdgeOptions={{
          type: "smoothstep",
        }}
        proOptions={{ hideAttribution: true }}
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={28}
          size={0.8}
          color={colors.borderFaint}
        />
        <Controls
          showZoom={true}
          showFitView={true}
          showInteractive={false}
        />
        <MiniMap
          nodeColor={(node) => {
            if (node.type === "orchestrator") return colors.mauve;
            // Use type guard for type-safe access to node data
            const data = node.data as TopologyNodeData;
            if (isMachineNodeData(data)) {
              if (data.machine.status === "connected") return colors.sage;
              if (data.machine.status === "connecting") return colors.ochre;
            }
            return colors.terracottaDim;
          }}
          maskColor="rgba(21, 18, 26, 0.8)"
          style={{
            backgroundColor: colors.bgSurface,
            border: `1px solid ${colors.borderFaint}`,
            borderRadius: '3px',
          }}
        />
      </ReactFlow>
    </div>
  );
}

// Wrap with ReactFlowProvider to access useReactFlow
export function TopologyView() {
  return (
    <ReactFlowProvider>
      <TopologyViewInner />
    </ReactFlowProvider>
  );
}
