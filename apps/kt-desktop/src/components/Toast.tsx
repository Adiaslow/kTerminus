import { useToastStore, type ToastType } from "../stores/toast";
import { clsx } from "clsx";
import { iconPaths, XIcon } from "./Icons";

const toastStyles: Record<ToastType, { bg: string; border: string; text: string; iconPath: string }> = {
  success: {
    bg: "bg-sage/10",
    border: "border-sage/30",
    text: "text-sage",
    iconPath: iconPaths.success,
  },
  error: {
    bg: "bg-terracotta/10",
    border: "border-terracotta/30",
    text: "text-terracotta",
    iconPath: iconPaths.error,
  },
  warning: {
    bg: "bg-ochre/10",
    border: "border-ochre/30",
    text: "text-ochre",
    iconPath: iconPaths.warning,
  },
  info: {
    bg: "bg-mauve/10",
    border: "border-mauve/30",
    text: "text-mauve",
    iconPath: iconPaths.info,
  },
};

export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts);
  const removeToast = useToastStore((s) => s.removeToast);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
      {toasts.map((toast) => {
        const style = toastStyles[toast.type];
        return (
          <div
            key={toast.id}
            className={clsx(
              "flex items-start gap-3 px-4 py-3 rounded-zen border shadow-lg animate-in slide-in-from-right-5 fade-in duration-200",
              style.bg,
              style.border
            )}
            role="alert"
          >
            {/* Icon */}
            <svg
              className={clsx("w-5 h-5 flex-shrink-0 mt-0.5", style.text)}
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              strokeLinecap="round"
              strokeLinejoin="round"
              viewBox="0 0 24 24"
            >
              <path d={style.iconPath} />
            </svg>

            {/* Message */}
            <p className="flex-1 text-sm text-text-primary">{toast.message}</p>

            {/* Close button */}
            <button
              onClick={() => removeToast(toast.id)}
              className="flex-shrink-0 p-0.5 rounded hover:bg-bg-hover transition-colors text-text-ghost hover:text-text-muted"
              aria-label="Dismiss notification"
            >
              <XIcon className="w-4 h-4" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
