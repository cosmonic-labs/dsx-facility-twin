# DSX facility twin — run inside the lab VM (see docs/lab.md).
# Uses the dsx-lab kind cluster (kind-dsx-exchange) with Cosmonic Control 0.15 installed.
SHELL := /bin/bash
ROOT  := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
KCTX  ?= kind-dsx-exchange
K     := kubectl --context $(KCTX)
HELM  ?= helm
TAG   := $(or $(TAG),$(shell date +%Y%m%d%H%M%S))
NATS  := nats -s nats://nexus.cosmonic-system.svc.cluster.local:4222
PORT  ?= 8088
export PATH := $(HOME)/.local/share/mise/installs/kubectl/v1.34.0:$(HOME)/.local/share/mise/installs/helm/v4.2.2/linux-arm64:$(shell ls -d $(HOME)/.local/share/mise/installs/kind/*/ 2>/dev/null | head -1):$(HOME)/.local/share/mise/shims:$(PATH)

.PHONY: e2e wasm-deploy help up builder ui wasm push sim-image hostgroup bucket deploy expose unexpose status logs reset down url
help: ## Show targets
	@grep -E '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  %-12s %s\n",$$1,$$2}'

up: builder ui wasm sim-image hostgroup bucket deploy expose ## Build everything and deploy (first run ~10 min)
	@echo; echo "Control panel: http://$$(hostname -I | awk '{print $$1}'):$(PORT)  (admin / admin)"

builder: ## Build the toolchain image (Rust wasm32-wasip2 + Node)
	docker build -q -t twin-builder:local -f docker/builder.Dockerfile docker

ui: ## Build the React control panel into ui/dist
	bash scripts/build-ui.sh

wasm: ## Compile the four components to wasm32-wasip2 (embeds ui/dist into twin-api)
	bash scripts/build-wasm.sh

push: ## Push dist/*.wasm to the kind registry as OCI artifacts
	bash scripts/push-wasm.sh $(TAG)

sim-image: ## Build the simulator image and load it into kind
	docker build -t twin-sim:$(TAG) -f docker/sim.Dockerfile .
	kind load docker-image twin-sim:$(TAG) --name dsx-exchange
	echo $(TAG) > .sim-tag

hostgroup: ## Install the `twin` hostgroup (wasmcloud:nats on Control's NATS, local registry)
	$(HELM) upgrade --install hostgroup-twin oci://ghcr.io/cosmonic/cosmonic-control-hostgroup --version 0.15.0 \
	  -n cosmonic-system -f deploy/hostgroup-twin.values.yaml --wait --timeout 5m

bucket: ## Create the twin KV bucket on Control's NATS (idempotent)
	$(K) create ns dsx-twin 2>/dev/null || true
	$(K) -n dsx-twin get pod nats-box >/dev/null 2>&1 || $(K) -n dsx-twin run nats-box --image=natsio/nats-box:0.18.0 --restart=Never --command -- sleep infinity
	$(K) -n dsx-twin wait --for=condition=Ready pod/nats-box --timeout=120s
	$(K) -n dsx-twin exec nats-box -- $(NATS) kv info twin >/dev/null 2>&1 || $(K) -n dsx-twin exec nats-box -- $(NATS) kv add twin --history=1 --max-value-size=1MiB

deploy: push ## Apply the workloads with a fresh tag
	sed "s/__TAG__/$(TAG)/g; s/__SIMTAG__/$$(cat .sim-tag)/g" deploy/twin.yaml | $(K) apply -f -
	$(K) -n dsx-twin rollout status deploy/twin-sim --timeout=180s
	$(K) -n dsx-twin wait --for=condition=Ready workloaddeployment --all --timeout=240s

expose: ## Serve the control panel on this VM's IP (port $(PORT))
	bash scripts/expose.sh $(PORT)

unexpose: ## Stop the port-forward
	-pkill -f "port-forward.*svc/ingress $(PORT):80"

status: ## Workloads, last snapshot and prediction stats
	$(K) -n dsx-twin get deploy,workloaddeployments
	@$(K) -n dsx-twin exec nats-box -- sh -c '$(NATS) kv get twin snapshot --raw 2>/dev/null | head -c 300; echo; $(NATS) kv get twin predictions --raw 2>/dev/null | head -c 200; echo'

logs: ## Host logs for the twin hostgroup and the simulator
	$(K) -n cosmonic-system logs deploy/hostgroup-twin --tail=40
	$(K) -n dsx-twin logs deploy/twin-sim --tail=20

reset: ## Stop every running scenario
	curl -s -X POST localhost:$(PORT)/api/sim/reset -H 'Host: twin.dsx.lab' -b 'twin_session=dsx-twin-demo-session'; echo

down: unexpose ## Remove the twin (keeps the lab)
	-$(K) delete ns dsx-twin
	-$(K) -n dsx-twin exec nats-box -- $(NATS) kv del twin -f
	-$(HELM) uninstall hostgroup-twin -n cosmonic-system

url: ## Print the control panel URL
	@echo "http://$$(hostname -I | awk '{print $$1}'):$(PORT)"

qa: ## Run every failure scenario end to end and write tests/qa/report.md
	python3 tests/qa/scenarios.py $(SCENARIOS)

e2e: ## Screenshot every view with headless Chromium (tests/e2e/out)
	docker run --rm --network host -v $(ROOT)/tests/e2e:/e2e -w /e2e mcr.microsoft.com/playwright/python:v1.56.0-noble bash -c 'pip install -q --break-system-packages playwright==1.56.0 >/dev/null 2>&1; python3 shots.py $(PAGES)'

wasm-deploy: wasm deploy ## Rebuild the components and roll them out
