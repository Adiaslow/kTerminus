import { useAppStore } from "../../stores/app";
import { TerminalView } from "../terminal/TerminalView";
import { TopologyView } from "../topology/TopologyView";
import { HealthView } from "./HealthView";
import { LogsView } from "./LogsView";

export function MainContent() {
  const viewMode = useAppStore((s) => s.viewMode);

  switch (viewMode) {
    case "terminals":
      return <TerminalView />;
    case "topology":
      return <TopologyView />;
    case "health":
      return <HealthView />;
    case "logs":
      return <LogsView />;
    default:
      return <TerminalView />;
  }
}
