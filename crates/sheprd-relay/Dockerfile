# The relay as a static binary on a distroless image: no shell, no package
# manager, a non-root user. Build from the repository root, which the
# workspace's members and patches need:
#
#   docker buildx build -f crates/sheprd-relay/Dockerfile \
#     --platform linux/amd64,linux/arm64 -t sheprd-relay .
#
# Pin both base images by digest before publishing (docs/relay-plan.md).

ARG RUST_IMAGE=rust:1.99.0-alpine3.22

FROM ${RUST_IMAGE} AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p sheprd-relay \
 && cp target/release/sheprd-relay /sheprd-relay

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /sheprd-relay /usr/local/bin/sheprd-relay
# Inside a container the relay listens on every interface; the host decides
# what is published (deploy/relay/compose.yml keeps /metrics private).
ENV RELAY_LISTEN=0.0.0.0:8080 \
    RELAY_METRICS=0.0.0.0:9090 \
    RELAY_QUIC=0.0.0.0:8443
EXPOSE 8080 9090 8443/udp
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/sheprd-relay"]
