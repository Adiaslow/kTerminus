/**
 * Header bar for each terminal pane
 *
 * Shows the tab title and provides split/close controls
 */

import { memo } from "react";
import clsx from "clsx";
import { XIcon } from "../Icons";
import type { TerminalTab } from "../../types";

interface PaneHeaderProps {
  tab: TerminalTab | undefined;
  isActive: boolean;
  onSplitHorizontal: () => void;
  onSplitVertical: () => void;
  onClose: () => void;
}

export const PaneHeader = memo(function PaneHeader({
  tab,
  isActive,
  onSplitHorizontal,
  onSplitVertical,
  onClose,
}: PaneHeaderProps) {
  return (
    <div
      className={clsx(
        "flex items-center justify-between px-3 py-1.5 border-b select-none",
        isActive
          ? "bg-bg-surface border-border"
          : "bg-bg-deep border-border-faint"
      )}
    >
      {/* Tab title */}
      <div className="flex items-center gap-2 min-w-0">
        <div
          className={clsx(
            "w-1.5 h-1.5 rounded-full flex-shrink-0",
            isActive ? "bg-sage" : "bg-text-ghost"
          )}
        />
        <span
          className={clsx(
            "text-xs font-medium truncate",
            isActive ? "text-text-secondary" : "text-text-muted"
          )}
        >
          {tab?.title || "Terminal"}
        </span>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1 flex-shrink-0">
        {/* Split horizontal (side by side) */}
        <button
          onClick={onSplitHorizontal}
          className="p-1 rounded hover:bg-bg-elevated text-text-muted hover:text-text-secondary transition-colors"
          title="Split Right (Cmd+D)"
        >
          <SplitHorizontalIcon className="w-3.5 h-3.5" />
        </button>

        {/* Split vertical (stacked) */}
        <button
          onClick={onSplitVertical}
          className="p-1 rounded hover:bg-bg-elevated text-text-muted hover:text-text-secondary transition-colors"
          title="Split Down (Cmd+Shift+D)"
        >
          <SplitVerticalIcon className="w-3.5 h-3.5" />
        </button>

        {/* Close */}
        <button
          onClick={onClose}
          className="p-1 rounded hover:bg-terracotta/20 text-text-muted hover:text-terracotta transition-colors"
          title="Close Pane (Cmd+W)"
        >
          <XIcon className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  );
});

// Simple split icons
function SplitHorizontalIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      className={className}
    >
      {/* Left panel */}
      <rect x="2" y="3" width="5" height="10" rx="1" />
      {/* Right panel */}
      <rect x="9" y="3" width="5" height="10" rx="1" />
    </svg>
  );
}

function SplitVerticalIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      className={className}
    >
      {/* Top panel */}
      <rect x="3" y="2" width="10" height="5" rx="1" />
      {/* Bottom panel */}
      <rect x="3" y="9" width="10" height="5" rx="1" />
    </svg>
  );
}
