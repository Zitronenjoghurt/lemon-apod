FROM node:24-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
COPY Cargo.toml /Cargo.toml
RUN npm run build


FROM rust:1.96-slim-trixie AS rust
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY core core
COPY archiver archiver
COPY api api
COPY baseline baseline

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked \
    && mkdir -p /out \
    && cp target/release/apod-archiver target/release/apod-api /out/

FROM debian:trixie-slim AS prep
RUN useradd --system --uid 10001 --no-create-home --home-dir /nonexistent apod \
    && install -d -o apod -g apod /skel/data

FROM gcr.io/distroless/cc-debian13 AS runtime
COPY --from=prep /etc/passwd /etc/passwd
COPY --from=prep /etc/group /etc/group
COPY --from=prep --chown=10001:10001 /skel/data /data
USER 10001:10001
ENV APOD_DATA_DIR=/data
WORKDIR /app

FROM runtime AS archiver
COPY --from=rust /out/apod-archiver /usr/local/bin/apod-archiver
COPY baseline /app/baseline
ENTRYPOINT ["apod-archiver"]
CMD ["run"]

FROM runtime AS api
COPY --from=rust /out/apod-api /usr/local/bin/apod-api
COPY --from=web /web/dist /app/static
ENV APOD_STATIC_DIR=/app/static
EXPOSE 51995
ENTRYPOINT ["apod-api"]
