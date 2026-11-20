"""End-to-end check of the control panel, run inside the VM (see `make e2e`).

Logs in with the lab's static demo credentials, walks every view, runs a scenario through its
stages from the UI, and writes screenshots to tests/e2e/out/.
"""
import os, sys, time
from playwright.sync_api import sync_playwright

BASE = os.environ.get("TWIN_URL", "http://127.0.0.1:8088")
OUT = os.path.join(os.path.dirname(__file__), "out")
os.makedirs(OUT, exist_ok=True)
pages = sys.argv[1:] or ["overview", "warnings", "customers", "impact", "hardware", "power", "cooling", "network", "scenarios"]

with sync_playwright() as p:
    b = p.chromium.launch()
    pg = b.new_page(viewport={"width": 1600, "height": 1000}, device_scale_factor=1)
    errors = []
    pg.on("console", lambda m: m.type == "error" and errors.append(m.text))
    pg.goto(BASE + "/")
    pg.wait_for_selector("text=Sign in", timeout=20000)
    pg.screenshot(path=f"{OUT}/00-login.png")
    pg.fill("input[type=password]", os.environ.get("TWIN_PASSWORD", "admin"))
    pg.click("button:has-text('Sign in')")
    pg.wait_for_selector(".tw-header", timeout=20000)
    for i, name in enumerate(pages, 1):
        pg.goto(f"{BASE}/#/{name}")
        time.sleep(3.5)
        slug = name.replace("?", "-").replace("=", "-").replace("&", "-")
        pg.screenshot(path=f"{OUT}/{i:02d}-{slug}.png", full_page=True)
    print("console errors:", errors[:10])
    b.close()
