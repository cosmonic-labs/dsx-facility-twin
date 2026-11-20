# Lab setup

Built and tested in the dsx-lab Parallels VM (Ubuntu ARM64, 16 GB, 6 vCPU) that already runs the
DSX Exchange kind cluster (`kind-dsx-exchange`) and Cosmonic Control 0.15.

```bash
cd ~/source/cosmonic-labs/dsx-facility-twin   # or the shared-folder path inside the VM
make up            # ~10 minutes the first time
make url
```

`make up` builds a toolchain image (Rust with wasm32-wasip2, Node), the UI, the four Wasm
components and the simulator image; installs the `twin` hostgroup; creates the `twin` KV bucket;
deploys; and keeps a port-forward from `<vm-ip>:8088` to Control's ingress (`make expose`).

Iterate: `make wasm-deploy` (components), `make sim-image deploy` (simulator), `make ui wasm-deploy`
(UI). `make down` removes the twin and leaves the lab alone.
