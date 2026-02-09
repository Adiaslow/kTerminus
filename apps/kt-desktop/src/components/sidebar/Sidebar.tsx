import { useAppStore } from "../../stores/app";
import { MachineList } from "./MachineList";
import { SessionList } from "./SessionList";
import { clsx } from "clsx";

export function Sidebar() {
  const section = useAppStore((s) => s.sidebarSection);
  const setSection = useAppStore((s) => s.setSidebarSection);

  return (
    <div className="h-full flex flex-col bg-bg-surface border-r border-border">
      {/* Section tabs */}
      <div className="flex border-b border-border-faint">
        <SectionTab
          label="Machines"
          active={section === "machines"}
          onClick={() => setSection("machines")}
        />
        <SectionTab
          label="Sessions"
          active={section === "sessions"}
          onClick={() => setSection("sessions")}
        />
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {section === "machines" ? <MachineList /> : <SessionList />}
      </div>
    </div>
  );
}

function SectionTab({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={clsx(
        "relative flex-1 py-3 text-[10px] font-semibold uppercase tracking-[1.5px] text-center transition-colors",
        active
          ? "text-text-secondary"
          : "text-text-ghost hover:text-text-muted"
      )}
    >
      {label}
      {/* Light slit under active tab */}
      {active && (
        <span className="absolute bottom-0 left-[30%] right-[30%] h-px bg-mauve opacity-50" />
      )}
    </button>
  );
}
