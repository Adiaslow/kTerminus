import { useAppStore } from "../../stores/app";
import { useMachinesStore } from "../../stores/machines";

export function HealthView() {
  const status = useAppStore((s) => s.orchestratorStatus);
  const machines = useMachinesStore((s) => s.machines);

  const formatUptime = (secs: number) => {
    const days = Math.floor(secs / 86400);
    const hours = Math.floor((secs % 86400) / 3600);
    const mins = Math.floor((secs % 3600) / 60);

    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${mins}m`;
    return `${mins}m`;
  };

  return (
    <div className="h-full p-6 overflow-auto">
      <h1 className="text-xl font-semibold mb-6">System Health</h1>

      {/* Orchestrator Status */}
      <section className="mb-8">
        <h2 className="text-lg font-medium mb-4 text-terminal-fg/80">
          Orchestrator
        </h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <MetricCard
            label="Status"
            value={status?.running ? "Running" : "Stopped"}
            status={status?.running ? "good" : "bad"}
          />
          <MetricCard
            label="Uptime"
            value={status ? formatUptime(status.uptimeSecs) : "-"}
          />
          <MetricCard
            label="Version"
            value={status?.version ?? "-"}
          />
          <MetricCard
            label="Sessions"
            value={status?.sessionCount.toString() ?? "0"}
          />
        </div>
      </section>

      {/* Machine Health */}
      <section>
        <h2 className="text-lg font-medium mb-4 text-terminal-fg/80">
          Machines ({machines.length})
        </h2>
        <div className="space-y-3">
          {machines.length === 0 ? (
            <p className="text-terminal-fg/50">No machines connected</p>
          ) : (
            machines.map((machine) => (
              <MachineHealthCard key={machine.id} machine={machine} />
            ))
          )}
        </div>
      </section>
    </div>
  );
}

function MetricCard({
  label,
  value,
  status,
}: {
  label: string;
  value: string;
  status?: "good" | "bad" | "warn";
}) {
  const statusColors = {
    good: "text-terminal-green",
    bad: "text-terminal-red",
    warn: "text-terminal-yellow",
  };

  return (
    <div className="bg-sidebar-bg rounded-lg p-4 border border-sidebar-active">
      <div className="text-sm text-terminal-fg/60 mb-1">{label}</div>
      <div
        className={`text-lg font-medium ${status ? statusColors[status] : ""}`}
      >
        {value}
      </div>
    </div>
  );
}

function MachineHealthCard({
  machine,
}: {
  machine: { id: string; hostname: string; os: string; status: string; sessionCount: number };
}) {
  return (
    <div className="bg-sidebar-bg rounded-lg p-4 border border-sidebar-active flex items-center gap-4">
      <div
        className={`w-3 h-3 rounded-full ${
          machine.status === "connected"
            ? "bg-terminal-green"
            : machine.status === "connecting"
            ? "bg-terminal-yellow animate-pulse"
            : "bg-terminal-red"
        }`}
      />
      <div className="flex-1">
        <div className="font-medium">{machine.hostname}</div>
        <div className="text-sm text-terminal-fg/60">
          {machine.os} • {machine.sessionCount} sessions
        </div>
      </div>
      <div className="text-sm text-terminal-fg/50">{machine.id.slice(0, 8)}</div>
    </div>
  );
}
