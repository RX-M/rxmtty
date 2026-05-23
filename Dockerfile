# Build
FROM docker.io/ubuntu:24.04 AS builder

RUN apt-get update -y && apt install build-essential curl git -y
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN git clone https://github.com/RX-M/rxmtty
RUN . "$HOME/.cargo/env" && cargo build --manifest-path ./rxmtty/Cargo.toml --release

# Container image for execution
FROM scratch
LABEL org.opencontainers.image.ref.name=rxmtty
LABEL org.opencontainers.image.version=1.0.0
LABEL org.opencontainers.image.authors=rx-m
LABEL org.opencontainers.image.url=https://rx-m.com
COPY --from=builder /rxmtty/target/release/rxmtty .
CMD ["./rxmtty"]
