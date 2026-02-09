/**
 * Format uptime in seconds to a human-readable string.
 * @param secs - Total seconds of uptime
 * @returns Formatted string like "3d 5h", "5h 30m", or "45m"
 */
export function formatUptime(secs: number): string {
  const days = Math.floor(secs / 86400);
  const hours = Math.floor((secs % 86400) / 3600);
  const mins = Math.floor((secs % 3600) / 60);

  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${mins}m`;
  return `${mins}m`;
}
