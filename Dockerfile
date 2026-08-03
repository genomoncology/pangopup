# syntax=docker/dockerfile:1@sha256:87999aa3d42bdc6bea60565083ee17e86d1f3339802f543c0d03998580f9cb89

FROM rust:1.93.1-trixie@sha256:ecbe59a8408895edd02d9ef422504b8501dd9fa1526de27a45b73406d734d659 AS builder
WORKDIR /source
COPY Cargo.toml Cargo.lock ./
COPY LICENSE NOTICE ./
COPY assets/notices ./assets/notices
COPY release-profiles ./release-profiles
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/source/target,sharing=locked \
    case "$(uname -m)" in x86_64|aarch64) ;; *) exit 1 ;; esac \
    && cargo build --locked --release --package pangopup-cli \
    && strip target/release/pangopup \
    && install -D -m 0755 target/release/pangopup /out/usr/local/bin/pangopup \
    && install -D -m 0444 LICENSE /out/usr/share/licenses/pangopup/LICENSE \
    && install -D -m 0444 NOTICE /out/usr/share/doc/pangopup/NOTICE \
    && install -d -m 0700 -o 65532 -g 65532 /out/var/lib/pangopup \
    && install -d -m 0700 -o 65532 -g 65532 /out/var/cache/pangopup

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:d97bc0a941b8d4be647dc0ee75b264ddbb772f1ac5ba690a4309c00723b23775

ARG PANGOPUP_REVISION=unknown
ARG PANGOPUP_VERSION=unknown
LABEL org.opencontainers.image.title="Pangopup" \
      org.opencontainers.image.description="Fast Pangolin-compatible splice scoring service" \
      org.opencontainers.image.source="https://github.com/genomoncology/pangopup" \
      org.opencontainers.image.revision="${PANGOPUP_REVISION}" \
      org.opencontainers.image.version="${PANGOPUP_VERSION}" \
      org.opencontainers.image.licenses="GPL-3.0-only"

COPY --from=builder /out/ /

ENV PANGOPUP_DATA_DIR=/var/lib/pangopup \
    PANGOPUP_CACHE_DIR=/var/cache/pangopup \
    PANGOPUP_MODEL_CACHE=/var/cache/pangopup/model-results.sqlite3

USER 65532:65532
EXPOSE 8080
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/pangopup"]
CMD ["serve", "--listen", "0.0.0.0:8080"]
