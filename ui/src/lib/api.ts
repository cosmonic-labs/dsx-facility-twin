import { useEffect, useRef, useState } from "react";
import type { Clock, PredictionSet, RunView, ScenarioDef, Snapshot, Topology } from "./types.ts";

export class Unauthorized extends Error {}

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const r = await fetch(path, { credentials: "same-origin", ...init, headers: { "content-type": "application/json", ...(init?.headers ?? {}) } });
  if (r.status === 401) throw new Unauthorized();
  const text = await r.text();
  let body: any = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    // Not JSON: an ingress or proxy answered instead of the twin (e.g. Envoy's "no healthy
    // upstream" while a workload restarts). Say so plainly instead of a parse error.
    throw new Error(r.status >= 500 ? `The twin is restarting or unreachable (${r.status}: ${text.slice(0, 60)}). Retrying…` : `Unexpected response (${r.status})`);
  }
  if (!r.ok) throw new Error(body?.error ?? `${r.status} ${r.statusText}`);
  return body as T;
}
export const post = <T,>(path: string, body?: unknown) => api<T>(path, { method: "POST", body: JSON.stringify(body ?? {}) });

/** Poll `path` every `ms`. Returns the latest value (null until the first answer) and the last error. */
export function usePoll<T>(path: string | null, ms: number, onUnauthorized?: () => void): [T | null, string | null, () => void] {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const cb = useRef(onUnauthorized);
  cb.current = onUnauthorized;
  useEffect(() => {
    if (!path) return;
    let alive = true;
    const run = () =>
      api<T>(path)
        .then((d) => alive && (setData(d), setError(null)))
        .catch((e) => {
          if (!alive) return;
          if (e instanceof Unauthorized) cb.current?.();
          else setError(String(e.message ?? e));
        });
    run();
    const t = setInterval(run, ms);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [path, ms, tick]);
  return [data, error, () => setTick((x) => x + 1)];
}

export interface Live {
  topo: Topology | null;
  snap: Snapshot | null;
  preds: PredictionSet | null;
  clock: Clock | null;
  runs: RunView[] | null;
  scenarios: ScenarioDef[] | null;
  error: string | null;
  refreshRuns: () => void;
}
