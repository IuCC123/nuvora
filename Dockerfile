# syntax=docker/dockerfile:1.7

FROM oven/bun:1 AS web
WORKDIR /src
COPY package.json bun.lock ./
COPY packages ./packages
COPY apps/web ./apps/web
# A workspace too, for the desktop app's build tools; nothing of it ends up in the image.
COPY apps/desktop/package.json ./apps/desktop/
RUN bun install --frozen-lockfile
RUN bun run build

FROM rust:trixie AS server
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

FROM debian:trixie-slim
# FFmpeg brings the VA-API drivers for AMD; Intel's (18 MB) only exists for x86-64.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg ca-certificates tini \
    && if [ "$(dpkg --print-architecture)" = amd64 ]; then \
        apt-get install -y --no-install-recommends intel-media-va-driver; fi \
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
# No curl in the image: bash speaks just enough HTTP to read the status line.
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s --retries=3 \
    CMD bash -c 'exec 3<>/dev/tcp/127.0.0.1/8640 && printf "GET /api/health HTTP/1.0\r\nHost: localhost\r\n\r\n" >&3 && head -n 1 <&3 | grep -q " 200 "' || exit 1
ENTRYPOINT ["tini", "--", "nuvora"]
CMD ["--bind", "0.0.0.0:8640", "--data-dir", "/data"]
