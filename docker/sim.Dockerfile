# twin-sim: the native simulator and scenario API (demo harness).
FROM rust:1.95-slim AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY sim sim
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/src/target \
    cargo build --release -p twin-sim && cp target/release/twin-sim /twin-sim

FROM debian:bookworm-slim
COPY --from=build /twin-sim /usr/local/bin/twin-sim
COPY profiles /profiles
ENV PROFILE_DIR=/profiles/demo-dsx1 HTTP_ADDR=0.0.0.0:8090
EXPOSE 8090
ENTRYPOINT ["/usr/local/bin/twin-sim"]
