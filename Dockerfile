# syntax=docker/dockerfile:1.7

FROM oven/bun:1 AS web
WORKDIR /src
COPY package.json bun.lock ./
COPY packages ./packages
COPY apps/web ./apps/web
RUN bun install --frozen-lockfile
RUN bun run build

FROM rust:bookworm AS server
ENV RUSTUP_TOOLCHAIN=stable
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY --from=web /src/apps/web/dist ./apps/web/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p nuvora-server \
    && install -D target/release/nuvora /out/nuvora

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg ca-certificates tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 nuvora \
    && useradd --system --uid 10001 --gid nuvora --home-dir /data --shell /usr/sbin/nologin nuvora \
    && mkdir -p /data /media \
    && chown nuvora:nuvora /data
COPY --from=server /out/nuvora /usr/local/bin/nuvora
ENV HOME=/data
USER nuvora
VOLUME /data
EXPOSE 8640
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s --retries=3 \
    CMD bash -c ':</dev/tcp/127.0.0.1/8640' || exit 1
ENTRYPOINT ["tini", "--", "nuvora"]
CMD ["--bind", "0.0.0.0:8640", "--data-dir", "/data"]
