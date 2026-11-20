import React, { useState } from "react";
import { BrandMark, Button, Input } from "@design";
import { post } from "../lib/api.ts";

export function Login({ onDone }: { onDone: () => void }) {
  const [u, setU] = useState("admin");
  const [p, setP] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      await post("/api/login", { username: u, password: p });
      onDone();
    } catch (x) {
      setErr((x as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="tw-login">
      <form onSubmit={submit} className="tw-login-card">
        <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 6 }}>
          <BrandMark size={30} color="var(--accent)" />
          <div>
            <div style={{ fontWeight: 800, fontSize: 18 }}>DSX Facility Twin</div>
            <div style={{ color: "var(--text-muted)", fontSize: 12.5 }}>Control panel · lab build</div>
          </div>
        </div>
        <p style={{ color: "var(--text-secondary)", fontSize: 13, margin: "8px 0 18px" }}>
          A digital twin of a simulated GB200 NVL72 hall: power, cooling, network and the customers on top, analyzed by Wasm components on Cosmonic Control.
        </p>
        <Input label="Username" value={u} onChange={(e) => setU(e.target.value)} block autoFocus />
        <div style={{ height: 12 }} />
        <Input label="Password" type="password" value={p} onChange={(e) => setP(e.target.value)} block />
        {err && <div className="tw-err" style={{ marginTop: 12 }}>{err}</div>}
        <div style={{ height: 18 }} />
        <Button type="submit" variant="primary" block loading={busy} icon="log-in">
          Sign in
        </Button>
        <div style={{ marginTop: 14, fontSize: 11.5, color: "var(--text-muted)" }}>Demo credentials: admin / admin. A static lab login, not authentication.</div>
      </form>
    </div>
  );
}
