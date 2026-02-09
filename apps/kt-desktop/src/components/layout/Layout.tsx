import React from "react";
import { useAppStore } from "../../stores/app";
import { Header } from "./Header";
import { Sidebar } from "../sidebar/Sidebar";
import { MainContent } from "./MainContent";
import { clsx } from "clsx";

export function Layout() {
  const showSidebar = useAppStore((s) => s.showSidebar);
  const sidebarWidth = useAppStore((s) => s.sidebarWidth);

  return (
    <div className="h-screen flex flex-col bg-bg-void text-text-primary">
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

  // Track active drag handlers for cleanup on unmount
  const dragHandlersRef = React.useRef<{
    move: ((e: MouseEvent) => void) | null;
    up: (() => void) | null;
  }>({ move: null, up: null });

  // Cleanup on unmount
  React.useEffect(() => {
    return () => {
      const { move, up } = dragHandlersRef.current;
      if (move) document.removeEventListener("mousemove", move);
      if (up) document.removeEventListener("mouseup", up);
      document.body.style.cursor = "";
      document.body.classList.remove("no-select");
    };
  }, []);

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
      dragHandlersRef.current = { move: null, up: null };
    };

    // Store refs for cleanup
    dragHandlersRef.current = { move: handleMouseMove, up: handleMouseUp };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    document.body.style.cursor = "col-resize";
    document.body.classList.add("no-select");
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize sidebar"
      tabIndex={0}
      className="w-1 cursor-col-resize hover:bg-mauve/30 transition-colors focus:bg-mauve/40 focus:outline-none"
      onMouseDown={handleMouseDown}
      onKeyDown={(e) => {
        // Allow keyboard resizing with arrow keys
        if (e.key === "ArrowLeft") {
          setSidebarWidth(sidebarWidth - 10);
        } else if (e.key === "ArrowRight") {
          setSidebarWidth(sidebarWidth + 10);
        }
      }}
    />
  );
}
