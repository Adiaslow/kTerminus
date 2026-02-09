import { useState } from "react";
import { clsx } from "clsx";
import { DocumentIcon } from "../Icons";

type LogLevel = "all" | "error" | "warn" | "info" | "debug";

interface LogEntry {
  id: string; // Unique identifier for React key
  timestamp: string;
  level: "error" | "warn" | "info" | "debug";
  message: string;
  source?: string;
}

export function LogsView() {
  const [filter, setFilter] = useState<LogLevel>("all");
  const [search, setSearch] = useState("");

  // Log streaming from orchestrator is not yet implemented
  const logs: LogEntry[] = [];
  const isLogsAvailable = false;

  const filteredLogs = logs.filter((log) => {
    if (filter !== "all" && log.level !== filter) return false;
    if (search && !log.message.toLowerCase().includes(search.toLowerCase()))
      return false;
    return true;
  });

  return (
    <div className="h-full flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-4 p-3 border-b border-border-faint">
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
                    ? "bg-bg-elevated text-text-secondary"
                    : "text-text-ghost hover:bg-bg-hover hover:text-text-muted"
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
        <button
          className="btn btn-secondary text-xs disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={!isLogsAvailable || logs.length === 0}
          title={!isLogsAvailable ? "Log streaming coming soon" : "Clear all logs"}
          aria-label="Clear logs"
        >
          Clear
        </button>
        <button
          className="btn btn-secondary text-xs disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={!isLogsAvailable || logs.length === 0}
          title={!isLogsAvailable ? "Log streaming coming soon" : "Export logs to file"}
          aria-label="Export logs"
        >
          Export
        </button>
      </div>

      {/* Log entries */}
      <div className="flex-1 overflow-auto font-mono text-sm">
        {!isLogsAvailable ? (
          <div className="flex flex-col items-center justify-center h-full text-text-muted">
            <div className="w-16 h-16 mb-4 rounded-full bg-mauve/10 flex items-center justify-center">
              <DocumentIcon className="w-8 h-8 text-mauve" />
            </div>
            <div className="text-lg mb-2 font-medium">Log Streaming Coming Soon</div>
            <div className="text-sm text-text-ghost max-w-sm text-center">
              Real-time log streaming from the orchestrator and connected machines
              will be available in a future update.
            </div>
          </div>
        ) : filteredLogs.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-text-muted">
            <div className="text-lg mb-2">No logs yet</div>
            <div className="text-sm text-text-ghost">
              Logs will appear here when the orchestrator is running
            </div>
          </div>
        ) : (
          <table className="w-full">
            <tbody>
              {filteredLogs.map((log) => (
                <LogRow key={log.id} log={log} />
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
    error: "text-terracotta",
    warn: "text-ochre",
    info: "text-sage",
    debug: "text-text-muted",
  };

  const formatTime = (ts: string) => {
    const date = new Date(ts);
    return date.toLocaleTimeString();
  };

  return (
    <tr className="hover:bg-bg-hover/30 border-b border-border-faint/50">
      <td className="px-3 py-1.5 text-text-ghost whitespace-nowrap">
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
      <td className="px-3 py-1.5 text-text-muted whitespace-nowrap">
        {log.source}
      </td>
      <td className="px-3 py-1.5 text-text-secondary">{log.message}</td>
    </tr>
  );
}
