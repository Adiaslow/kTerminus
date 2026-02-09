import { useAppStore } from "../../stores/app";
import { useMachinesStore } from "../../stores/machines";
import { formatUptime } from "../../lib/utils";

export function HealthView() {
  const status = useAppStore((s) => s.orchestratorStatus);
  const machines = useMachinesStore((s) => s.machines);

  return (
    <div className="h-full p-6 overflow-auto">
      <h1 className="text-xl font-semibold mb-6">System Health</h1>

      {/* Orchestrator Status */}
      <section className="mb-8">
        <h2 className="text-lg font-medium mb-4 text-text-secondary">
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
        <h2 className="text-lg font-medium mb-4 text-text-secondary">
          Machines ({machines.length})
        </h2>
        <div className="space-y-3">
          {machines.length === 0 ? (
            <p className="text-text-muted">No machines connected</p>
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
    good: "text-sage",
    bad: "text-terracotta",
    warn: "text-ochre",
  };

  return (
    <div className="bg-bg-surface rounded p-4 border-2 border-border-faint">
      <div className="text-sm text-text-muted mb-1">{label}</div>
      <div
        className={`text-lg font-medium ${status ? statusColors[status] : "text-text-primary"}`}
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
    <div className="bg-bg-surface rounded p-4 border-2 border-border-faint flex items-center gap-4">
      <div
        className={`w-3 h-3 rounded-full ${
          machine.status === "connected"
            ? "bg-sage"
            : machine.status === "connecting"
            ? "bg-ochre animate-breathe"
            : "bg-terracotta-dim"
        }`}
      />
      <div className="flex-1">
        <div className="font-medium text-text-primary">{machine.hostname}</div>
        <div className="text-sm text-text-muted">
          {machine.os} • {machine.sessionCount} sessions
        </div>
      </div>
      <div className="text-sm text-text-ghost">{machine.id.slice(0, 8)}</div>
    </div>
  );
}
