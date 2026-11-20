import type { Tlp } from "./types.ts";

export const TLP: Record<Tlp, { fg: string; bg: string; line: string; label: string; short: string }> = {
  green: { fg: "#13734e", bg: "#e6f6ef", line: "#1fa971", label: "Normal", short: "OK" },
  unknown: { fg: "#5c6a74", bg: "#edf1f3", line: "#9aa7af", label: "No data", short: "NO DATA" },
  yellow: { fg: "#7d5a00", bg: "#fff6e0", line: "#e0a000", label: "On trend to be impacted", short: "AT RISK" },
  orange: { fg: "#9a3d0b", bg: "#fdeee4", line: "#f0712c", label: "Degraded", short: "DEGRADED" },
  red: { fg: "#b0353a", bg: "#fdeaea", line: "#e5484d", label: "Out", short: "OUT" },
};
export const TLP_ORDER: Tlp[] = ["red", "orange", "yellow", "unknown", "green"];
export const rank = (t: Tlp) => ({ green: 0, unknown: 1, yellow: 2, orange: 3, red: 4 })[t];
export const worst = (a: Tlp, b: Tlp): Tlp => (rank(a) >= rank(b) ? a : b);

export function fmtEta(m?: number): string {
  if (m === undefined || m === null) return "";
  if (m < 1) return "now";
  if (m < 90) return `~${Math.round(m)} min`;
  return `~${(m / 60).toFixed(1)} h`;
}
export const money = (n: number) => (n >= 1000 ? `$${(n / 1000).toFixed(1)}k` : `$${n.toFixed(0)}`);
