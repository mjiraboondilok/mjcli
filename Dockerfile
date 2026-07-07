# syntax=docker/dockerfile:1.7
FROM rust:1-alpine AS builder
ARG TARGETPLATFORM
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY mj ./mj
COPY README.md ./
WORKDIR /src/mj
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry \
    --mount=type=cache,target=/src/mj/target,id=cargo-target-${TARGETPLATFORM},sharing=locked \
    cargo build --release --locked && \
    cp target/release/mj /mj

FROM scratch
COPY --from=builder /mj /usr/local/bin/mj
ENTRYPOINT ["/usr/local/bin/mj"]
