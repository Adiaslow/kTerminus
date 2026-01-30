import { useState } from "react";
import { clsx } from "clsx";

type LogLevel = "all" | "error" | "warn" | "info" | "debug";

interface LogEntry {
  timestamp: string;
  level: "error" | "warn" | "info" | "debug";
  message: string;
  source?: string;
}

// Mock data for demonstration
const mockLogs: LogEntry[] = [
  {
    timestamp: "2024-01-15T10:30:00Z",
    level: "info",
    message: "Orchestrator started on 0.0.0.0:2222",
    source: "orchestrator",
  },
  {
    timestamp: "2024-01-15T10:30:05Z",
    level: "info",
    message: "Agent connected: lab-gpu-01 (192.168.1.100)",
    source: "connection",
  },
  {
    timestamp: "2024-01-15T10:30:10Z",
    level: "debug",
    message: "Heartbeat received from lab-gpu-01",
    source: "heartbeat",
  },
  {
    timestamp: "2024-01-15T10:30:15Z",
    level: "info",
    message: "Session created: session-abc123 on lab-gpu-01",
    source: "session",
  },
  {
    timestamp: "2024-01-15T10:30:20Z",
    level: "warn",
    message: "High latency detected for lab-gpu-01 (150ms)",
    source: "health",
  },
];

export function LogsView() {
  const [filter, setFilter] = useState<LogLevel>("all");
  const [search, setSearch] = useState("");

  const filteredLogs = mockLogs.filter((log) => {
    if (filter !== "all" && log.level !== filter) return false;
    if (search && !log.message.toLowerCase().includes(search.toLowerCase()))
      return false;
    return true;
  });

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-4 p-3 border-b border-sidebar-active">
        {/* Level filter */}
        <div className="flex items-center gap-1">
          {(["all", "error", "warn", "info", "debug"] as LogLevel[]).map(
            (level) => (
              <button
                key={level}
                onClick={() => setFilter(level)}
                className={clsx(
                  "px-2 py-1 text-xs rounded capitalize",
                  filter === level
                    ? "bg-sidebar-active"
                    : "hover:bg-sidebar-hover"
                )}
              >
                {level}
              </button>
            )
          )}
        </div>

        {/* Search */}
        <input
          type="text"
          placeholder="Search logs..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="input flex-1 max-w-xs text-sm"
        />

        {/* Actions */}
        <button className="btn btn-secondary text-xs">Clear</button>
        <button className="btn btn-secondary text-xs">Export</button>
      </div>

      {/* Log entries */}
      <div className="flex-1 overflow-auto font-mono text-sm">
        {filteredLogs.length === 0 ? (
          <div className="p-4 text-terminal-fg/50">No logs to display</div>
        ) : (
          <table className="w-full">
            <tbody>
              {filteredLogs.map((log, i) => (
                <LogRow key={i} log={log} />
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

function LogRow({ log }: { log: LogEntry }) {
  const levelColors = {
    error: "text-terminal-red",
    warn: "text-terminal-yellow",
    info: "text-terminal-blue",
    debug: "text-terminal-fg/50",
  };

  const formatTime = (ts: string) => {
    const date = new Date(ts);
    return date.toLocaleTimeString();
  };

  return (
    <tr className="hover:bg-sidebar-hover/30 border-b border-sidebar-active/30">
      <td className="px-3 py-1.5 text-terminal-fg/50 whitespace-nowrap">
        {formatTime(log.timestamp)}
      </td>
      <td
        className={clsx(
          "px-3 py-1.5 uppercase text-xs font-medium whitespace-nowrap",
          levelColors[log.level]
        )}
      >
        {log.level}
      </td>
      <td className="px-3 py-1.5 text-terminal-fg/60 whitespace-nowrap">
        {log.source}
      </td>
      <td className="px-3 py-1.5">{log.message}</td>
    </tr>
  );
}
