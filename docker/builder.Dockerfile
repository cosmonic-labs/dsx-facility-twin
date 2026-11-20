# Build image for the Wasm components and the UI. Built once in the VM: make builder
FROM rust:1.95-slim
RUN rustup target add wasm32-wasip2 \
 && apt-get update && apt-get install -y --no-install-recommends ca-certificates curl xz-utils \
 && curl -fsSL https://nodejs.org/dist/v22.21.0/node-v22.21.0-linux-arm64.tar.xz | tar -xJ -C /usr/local --strip-components=1 \
 && rm -rf /var/lib/apt/lists/*
