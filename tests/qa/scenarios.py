"""Run every failure scenario end to end against a live twin and report what the twin showed.

Usage (inside the VM):  python3 tests/qa/scenarios.py [scenario-id ...]
Talks to the control panel API on 127.0.0.1:8088 with the lab's demo login, drives each
scenario stage by stage through the simulator proxy, and records the target's state, the
spread of states, customers affected, warnings and augurs predictions after each stage.
Writes tests/qa/report.json and tests/qa/report.md.
"""
import http.cookiejar, json, os, sys, time, urllib.request

BASE = os.environ.get("TWIN_URL", "http://127.0.0.1:8088")
SPEED = float(os.environ.get("QA_SPEED", "120"))
jar = http.cookiejar.CookieJar()
op = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))

def call(path, body=None):
    req = urllib.request.Request(BASE + path, data=None if body is None else json.dumps(body).encode(), headers={"content-type": "application/json"}, method="POST" if body is not None else "GET")
    # GETs retry; POSTs (next stage, start, cancel) are not idempotent, so they are tried once.
    for _ in range(5 if body is None else 1):
        try:
            with op.open(req, timeout=60) as r:
                return json.loads(r.read() or b"null")
        except Exception as e:
            err = e
            time.sleep(3)
    raise err

def wait_ticks(minutes):
    # one tick = 10 s real = 10*SPEED sim seconds; give ingest/impact/predict two extra ticks
    sim_s = minutes * 60
    time.sleep(max(22, sim_s / SPEED + 16))

RANK = {"green": 0, "unknown": 1, "yellow": 2, "orange": 3, "red": 4}
TOPO = None

def held(target):
    """A rack or row target is a container: judge it with what it holds (NVLink domain, trays, racks, CDUs)."""
    global TOPO
    TOPO = TOPO or call("/api/topology")
    kinds = {a["id"]: a["kind"] for a in TOPO["assets"]}
    if kinds.get(target) not in ("rack", "row"):
        return [target]
    return [target] + [a["id"] for a in TOPO["assets"] if a.get("parent") == target]

def observe(target):
    s = call("/api/snapshot")
    p = call("/api/predictions")
    ids = held(target)
    states = [s["assets"].get(i, {"tlp": "green", "reason": ""}) for i in ids]
    a = max(states, key=lambda x: RANK.get(x.get("tlp"), 0))
    caused = [k for k, v in s["assets"].items() if set(ids) & set(v.get("causes", [])) and k not in ids]
    return {
        "target": a.get("tlp"), "reason": a.get("reason", ""), "eta": a.get("eta_min"),
        "unknown": sum(1 for v in s["assets"].values() if v.get("tlp") == "unknown"),
        "by_tlp": {k: v for k, v in s["kpis"]["assets_by_tlp"].items() if k != "green"},
        "caused": len(caused),
        "customers": {c["id"]: c["tlp"] for c in s["customers"] if c["tlp"] != "green"},
        "warnings": [w["title"] for w in s["warnings"][:5]],
        "predictions": [(x["point"], x["kind"], round(x["eta_min"])) for x in p["items"][:5]],
        "fitted": p.get("fitted"), "fit_ms": p.get("took_ms"),
    }

call("/api/login", {"username": "admin", "password": os.environ.get("TWIN_PASSWORD", "admin")})
call("/api/sim/reset", {})
for r in call("/api/sim/runs"):  # stop anything left running (a demo someone forgot, an earlier QA)
    if not r["done"]:
        print(f"cancelling leftover run #{r['id']} {r['scenario']}", flush=True)
        call(f"/api/sim/runs/{r['id']}/cancel", {})
call("/api/sim/clock", {"speed": SPEED, "paused": False})
cat = call("/api/sim/scenarios")
only = set(sys.argv[1:])
report = []
for d in cat:
    if only and d["id"] not in only:
        continue
    print(f"=== {d['name']} ({d['default_target']})", flush=True)
    time.sleep(15)  # let the previous restore settle
    base = observe(d["default_target"])
    run = call("/api/sim/runs", {"scenario": d["id"], "auto": False})
    rows = [{"stage": "baseline", "kind": "-", **base}]
    for i, st in enumerate(d["stages"]):
        if i > 0:
            call(f"/api/sim/runs/{run['id']}/next", {})
        wait_ticks(st["minutes"])
        o = observe(d["default_target"])
        live = next((x for x in call("/api/sim/runs") if x["id"] == run["id"]), {})
        o["sim_stage"] = live.get("stage")
        if live.get("stage") != i:
            print(f"  !! simulator is on stage {live.get('stage')} but QA expected {i}", flush=True)
        rows.append({"stage": st["name"], "kind": st["kind"], "expect": st["expect"], **o})
        print(f"  [{st['kind']}] {st['name']}: target={o['target']} caused={o['caused']} cust={o['customers']} preds={o['predictions'][:2]}", flush=True)
    call(f"/api/sim/runs/{run['id']}/cancel", {})
    time.sleep(40)
    after = observe(d["default_target"])
    rows.append({"stage": "after restore", "kind": "-", **after})
    print(f"  after: target={after['target']} by_tlp={after['by_tlp']}", flush=True)
    report.append({"id": d["id"], "name": d["name"], "target": d["default_target"], "rows": rows})
    json.dump(report, open(os.path.join(os.path.dirname(__file__), "report.json"), "w"), indent=1)

with open(os.path.join(os.path.dirname(__file__), "report.md"), "w") as f:
    f.write("# Scenario QA report\n\n")
    for r in report:
        f.write(f"## {r['name']} — `{r['target']}`\n\n| Stage | Kind | Target | Downstream | Customers | Predictions |\n|---|---|---|---|---|---|\n")
        for x in r["rows"]:
            f.write(f"| {x['stage']} | {x['kind']} | {x['target']} | {x['caused']} | {', '.join(f'{k}:{v}' for k, v in x['customers'].items())} | {'; '.join(f'{p[0]} {p[1]} {p[2]}m' for p in x['predictions'][:3])} |\n")
        f.write("\n")
call("/api/sim/clock", {"speed": 30})
print("done")
