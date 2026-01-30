import { useAppStore } from "../../stores/app";
import { MachineList } from "./MachineList";
import { SessionList } from "./SessionList";
import { clsx } from "clsx";

export function Sidebar() {
  const section = useAppStore((s) => s.sidebarSection);
  const setSection = useAppStore((s) => s.setSidebarSection);

  return (
    <div className="h-full flex flex-col bg-sidebar-bg border-r border-sidebar-active">
      {/* Section tabs */}
      <div className="flex border-b border-sidebar-active">
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
        "flex-1 px-3 py-2 text-sm font-medium transition-colors",
        active
          ? "bg-sidebar-active text-terminal-fg"
          : "text-terminal-fg/60 hover:text-terminal-fg hover:bg-sidebar-hover"
      )}
    >
      {label}
    </button>
  );
}
