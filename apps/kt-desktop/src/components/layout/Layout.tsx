import { useAppStore } from "../../stores/app";
import { Header } from "./Header";
import { Sidebar } from "../sidebar/Sidebar";
import { MainContent } from "./MainContent";
import { clsx } from "clsx";

export function Layout() {
  const showSidebar = useAppStore((s) => s.showSidebar);
  const sidebarWidth = useAppStore((s) => s.sidebarWidth);

  return (
    <div className="h-screen flex flex-col bg-terminal-bg">
      {/* Header */}
      <Header />

      {/* Main area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar */}
        <div
          className={clsx(
            "flex-shrink-0 transition-all duration-200",
            showSidebar ? "opacity-100" : "opacity-0 w-0"
          )}
          style={{ width: showSidebar ? sidebarWidth : 0 }}
        >
          <Sidebar />
        </div>

        {/* Resize handle */}
        {showSidebar && <ResizeHandle />}

        {/* Main content */}
        <div className="flex-1 overflow-hidden">
          <MainContent />
        </div>
      </div>
    </div>
  );
}

function ResizeHandle() {
  const setSidebarWidth = useAppStore((s) => s.setSidebarWidth);
  const sidebarWidth = useAppStore((s) => s.sidebarWidth);

  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = sidebarWidth;

    const handleMouseMove = (e: MouseEvent) => {
      const delta = e.clientX - startX;
      setSidebarWidth(startWidth + delta);
    };

    const handleMouseUp = () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
      document.body.classList.remove("no-select");
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    document.body.style.cursor = "col-resize";
    document.body.classList.add("no-select");
  };

  return (
    <div
      className="w-1 cursor-col-resize hover:bg-terminal-blue/30 transition-colors"
      onMouseDown={handleMouseDown}
    />
  );
}
