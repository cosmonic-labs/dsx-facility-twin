#!/usr/bin/env bash
# Keep a port-forward from <VM IP>:PORT to Control's ingress running in the background, so the
# control panel opens from the Mac at http://<vm-ip>:PORT. Restarts the forward if it drops.
set -euo pipefail
PORT="${1:-8088}"
pkill -f "port-forward.*svc/ingress $PORT:80" 2>/dev/null || true
pkill -f "twin-expose-loop" 2>/dev/null || true
nohup bash -c "exec -a twin-expose-loop bash -c 'while true; do kubectl --context kind-dsx-exchange -n cosmonic-system port-forward --address 0.0.0.0 svc/ingress $PORT:80; sleep 2; done'" >/tmp/twin-expose.log 2>&1 &
sleep 3
curl -s -o /dev/null -w "control panel: http://$(hostname -I | awk '{print $1}'):$PORT  (HTTP %{http_code})\n" -H "Host: twin.dsx.lab" localhost:$PORT/ || true
