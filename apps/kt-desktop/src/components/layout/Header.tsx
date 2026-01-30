import { useAppStore, type ViewMode } from "../../stores/app";
import { clsx } from "clsx";

const viewModes: { id: ViewMode; label: string; icon: string }[] = [
  { id: "terminals", label: "Terminals", icon: "⌨" },
  { id: "topology", label: "Topology", icon: "🔗" },
  { id: "health", label: "Health", icon: "💓" },
  { id: "logs", label: "Logs", icon: "📋" },
];

export function Header() {
  const viewMode = useAppStore((s) => s.viewMode);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const showSidebar = useAppStore((s) => s.showSidebar);
  const isConnected = useAppStore((s) => s.isConnected);
  const status = useAppStore((s) => s.orchestratorStatus);

  return (
    <header className="h-10 flex items-center bg-sidebar-bg border-b border-sidebar-active px-2 gap-2">
      {/* Sidebar toggle */}
      <button
        onClick={toggleSidebar}
        className="p-1.5 rounded hover:bg-sidebar-hover transition-colors"
        title={showSidebar ? "Hide sidebar" : "Show sidebar"}
      >
        <svg
          className="w-4 h-4"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M4 6h16M4 12h16M4 18h16"
          />
        </svg>
      </button>

      {/* Logo */}
      <div className="flex items-center gap-2 px-2">
        <span className="text-terminal-blue font-bold">k-Terminus</span>
      </div>

      {/* Divider */}
      <div className="w-px h-5 bg-sidebar-active" />

      {/* View mode tabs */}
      <nav className="flex items-center gap-1">
        {viewModes.map((mode) => (
          <button
            key={mode.id}
            onClick={() => setViewMode(mode.id)}
            className={clsx(
              "px-3 py-1 text-sm rounded transition-colors",
              viewMode === mode.id
                ? "bg-sidebar-active text-terminal-fg"
                : "text-terminal-fg/60 hover:text-terminal-fg hover:bg-sidebar-hover"
            )}
          >
            <span className="mr-1.5">{mode.icon}</span>
            {mode.label}
          </button>
        ))}
      </nav>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Connection status */}
      <div className="flex items-center gap-2 px-2 text-sm">
        <div
          className={clsx(
            "w-2 h-2 rounded-full",
            isConnected ? "bg-terminal-green" : "bg-terminal-red"
          )}
        />
        <span className="text-terminal-fg/60">
          {isConnected
            ? `${status?.machineCount ?? 0} machines`
            : "Disconnected"}
        </span>
      </div>
    </header>
  );
}
