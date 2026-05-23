# Build
FROM rust:1.87 AS builder

RUN apt-get update && apt-get install -y --no-install-recommends musl-tools
RUN rustup target add x86_64-unknown-linux-musl
RUN git clone https://github.com/RX-M/rxmtty
RUN cargo build --release --target x86_64-unknown-linux-musl --manifest-path ./rxmtty/Cargo.toml


# Container image for execution
FROM alpine:3.20

RUN apk add --no-cache openssh-client ca-certificates  # The ssh client must be available for rxmtty to run as a child proc
LABEL org.opencontainers.image.ref.name=rxmtty
LABEL org.opencontainers.image.version=1.0.0
LABEL org.opencontainers.image.authors=rx-m
LABEL org.opencontainers.image.url=https://rx-m.com
COPY --from=builder /rxmtty/target/x86_64-unknown-linux-musl/release/rxmtty /rxmtty
ENTRYPOINT ["/rxmtty"]
