import { useMemo, useCallback } from "react";
import {
  ReactFlow,
  type Node,
  type Edge,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  BackgroundVariant,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { useMachinesStore } from "../../stores/machines";
import { useAppStore } from "../../stores/app";
import { MachineNode } from "./MachineNode";
import { OrchestratorNode } from "./OrchestratorNode";
import type { Machine, OrchestratorStatus } from "../../types";

const nodeTypes = {
  machine: MachineNode,
  orchestrator: OrchestratorNode,
};

export function TopologyView() {
  const machines = useMachinesStore((s) => s.machines);
  const status = useAppStore((s) => s.orchestratorStatus);

  // Generate nodes and edges from machines
  const { initialNodes, initialEdges } = useMemo(() => {
    const nodes: Node[] = [];
    const edges: Edge[] = [];

    const defaultStatus: OrchestratorStatus = {
      running: false,
      uptimeSecs: 0,
      machineCount: 0,
      sessionCount: 0,
      version: "unknown",
    };

    // Orchestrator node in the center
    nodes.push({
      id: "orchestrator",
      type: "orchestrator",
      position: { x: 400, y: 200 },
      data: { status: status || defaultStatus },
    });

    // Position machines in a circle around orchestrator
    const radius = 250;
    const angleStep = (2 * Math.PI) / Math.max(machines.length, 1);

    machines.forEach((machine: Machine, index: number) => {
      const angle = index * angleStep - Math.PI / 2;
      const x = 400 + radius * Math.cos(angle);
      const y = 200 + radius * Math.sin(angle);

      nodes.push({
        id: machine.id,
        type: "machine",
        position: { x, y },
        data: { machine },
      });

      // Edge from orchestrator to machine
      edges.push({
        id: `edge-${machine.id}`,
        source: "orchestrator",
        target: machine.id,
        animated: machine.status === "connected",
        style: {
          stroke:
            machine.status === "connected"
              ? "#9ece6a"
              : machine.status === "connecting"
              ? "#e0af68"
              : "#f7768e",
          strokeWidth: 2,
        },
      });
    });

    return { initialNodes: nodes, initialEdges: edges };
  }, [machines, status]);

  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  // Update nodes when machines change
  useMemo(() => {
    setNodes(initialNodes);
    setEdges(initialEdges);
  }, [initialNodes, initialEdges, setNodes, setEdges]);

  const onNodeClick = useCallback((_event: React.MouseEvent, node: Node) => {
    if (node.type === "machine") {
      useMachinesStore.getState().selectMachine(node.id);
    }
  }, []);

  return (
    <div className="h-full w-full">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onNodeClick={onNodeClick}
        nodeTypes={nodeTypes}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        minZoom={0.5}
        maxZoom={2}
        defaultEdgeOptions={{
          type: "smoothstep",
        }}
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={20}
          size={1}
          color="#292e42"
        />
        <Controls
          showZoom={true}
          showFitView={true}
          showInteractive={false}
        />
        <MiniMap
          nodeColor={(node) => {
            if (node.type === "orchestrator") return "#7aa2f7";
            const data = node.data as { machine: Machine };
            if (data.machine.status === "connected") return "#9ece6a";
            if (data.machine.status === "connecting") return "#e0af68";
            return "#f7768e";
          }}
          maskColor="rgba(22, 22, 30, 0.8)"
          style={{
            backgroundColor: "#16161e",
            border: "1px solid #292e42",
          }}
        />
      </ReactFlow>
    </div>
  );
}
