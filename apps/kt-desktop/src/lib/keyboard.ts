/**
 * Keyboard shortcut utilities
 *
 * Provides cross-platform keyboard shortcut detection:
 * - Mac: Cmd+key (matches iTerm2)
 * - Windows/Linux: Ctrl+Shift+key (avoids terminal conflicts like Ctrl+C, Ctrl+D)
 */

/**
 * Check if the current platform is macOS
 */
export function isMac(): boolean {
  return navigator.platform.includes("Mac");
}

/**
 * Check if a keyboard event is an app shortcut
 *
 * App shortcuts use:
 * - Mac: Cmd (without Ctrl)
 * - Windows/Linux: Ctrl+Shift
 *
 * This avoids conflicts with terminal shortcuts (Ctrl+C, Ctrl+D, etc.)
 */
export function isAppShortcut(e: KeyboardEvent): boolean {
  if (isMac()) {
    return e.metaKey && !e.ctrlKey;
  }
  return e.ctrlKey && e.shiftKey;
}

/**
 * Get the normalized key for a keyboard event (lowercase)
 */
export function getShortcutKey(e: KeyboardEvent): string {
  return e.key.toLowerCase();
}

/**
 * Check if a specific shortcut key combo is pressed
 *
 * @param e - The keyboard event
 * @param key - The key to check (case-insensitive)
 * @param requireShift - If true, also requires Shift key
 * @param requireAlt - If true, also requires Alt key (for Win/Linux vertical split)
 */
export function isShortcutPressed(
  e: KeyboardEvent,
  key: string,
  requireShift = false,
  requireAlt = false
): boolean {
  if (!isAppShortcut(e)) return false;

  const normalizedKey = getShortcutKey(e);
  if (normalizedKey !== key.toLowerCase()) return false;

  if (requireShift && !e.shiftKey) return false;
  if (requireAlt && !e.altKey) return false;

  return true;
}

/**
 * List of app shortcut keys that should pass through xterm.js
 * (not be handled by the terminal)
 */
export const APP_SHORTCUT_KEYS = ["d", "w", "t", "n", "]", "["] as const;

/**
 * Check if an event is an app shortcut that should pass through the terminal
 *
 * Used by xterm's attachCustomKeyEventHandler to determine if the terminal
 * should handle the key event or let it bubble up to the app.
 */
export function shouldPassThroughTerminal(e: KeyboardEvent): boolean {
  if (!isAppShortcut(e)) return false;

  const key = getShortcutKey(e);
  return APP_SHORTCUT_KEYS.includes(key as (typeof APP_SHORTCUT_KEYS)[number]);
}
