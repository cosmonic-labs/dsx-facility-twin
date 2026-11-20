import { useEffect, useState } from "react";

export interface Route { page: string; params: URLSearchParams }

function parse(): Route {
  const h = window.location.hash.replace(/^#\/?/, "");
  const [page, qs] = h.split("?");
  return { page: page || "overview", params: new URLSearchParams(qs ?? "") };
}

export function useRoute(): Route {
  const [r, setR] = useState(parse());
  useEffect(() => {
    const f = () => setR(parse());
    window.addEventListener("hashchange", f);
    return () => window.removeEventListener("hashchange", f);
  }, []);
  return r;
}

export function go(page: string, params?: Record<string, string | undefined>) {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params ?? {})) if (v) p.set(k, v);
  const s = p.toString();
  window.location.hash = `#/${page}${s ? "?" + s : ""}`;
}
