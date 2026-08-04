# syntax=docker/dockerfile:1@sha256:87999aa3d42bdc6bea60565083ee17e86d1f3339802f543c0d03998580f9cb89

# Ticket 048 only: matched source-built A/B probes. This file is deliberately
# separate from the production Dockerfile and must never publish an image.
FROM rust:1.93.1-trixie@sha256:ecbe59a8408895edd02d9ef422504b8501dd9fa1526de27a45b73406d734d659 AS builder

ARG ONNXRUNTIME_COMMIT=da9b5e364c465de65c49d91e696cd6485270757f
ARG ONNXRUNTIME_SOURCE_SHA256=9616cbdbbfcb1420b3261cd280a047d74ab0a249825e577b0e2dd310e22f6b83
ARG CPUINFO_BASELINE_COMMIT=4628dc060ce4e82345dc166bbac875609db4ff69
ARG CPUINFO_BASELINE_SHA1=e58d4b47c16a982111c897e669ae4f1821a393d7
ARG CPUINFO_BASELINE_SHA256=2ed3ebc6c2656cc0aafc7af319e5cb0f97cc9b415eae180f566def84f1ca6a29
ARG CPUINFO_PATCHED_COMMIT=0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6
ARG CPUINFO_PATCHED_SHA1=44419f8b0fda75bb0d2fbe3dd0629493c98ad905
ARG CPUINFO_PATCHED_SHA256=d40ed2e6134c6b8103ac7916de080a67964f48a8f61c5e72ecca18e9c492f884

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        binutils ca-certificates cmake curl diffutils git ninja-build \
        python3 python3-packaging unzip \
    && test "$(uname -m)" = aarch64 \
    && (set -- $(git ls-remote https://github.com/microsoft/onnxruntime.git refs/tags/v1.28.0); \
        test "$#" = 2; \
        test "$1" = "${ONNXRUNTIME_COMMIT}"; \
        test "$2" = refs/tags/v1.28.0)

WORKDIR /probe
COPY maintainers/ticket-048/expected-cpuinfo.patch /probe/expected-cpuinfo.patch

RUN curl --fail --location --proto '=https' --tlsv1.2 \
        --output /tmp/onnxruntime.tar.gz \
        https://github.com/microsoft/onnxruntime/archive/refs/tags/v1.28.0.tar.gz \
    && echo "${ONNXRUNTIME_SOURCE_SHA256}  /tmp/onnxruntime.tar.gz" | sha256sum --check --strict \
    && mkdir -p /probe/ort-baseline \
    && tar -xzf /tmp/onnxruntime.tar.gz -C /probe/ort-baseline --strip-components=1 \
    && test "$(grep -Fc "pytorch_cpuinfo;https://github.com/pytorch/cpuinfo/archive/${CPUINFO_BASELINE_COMMIT}.zip;${CPUINFO_BASELINE_SHA1}" /probe/ort-baseline/cmake/deps.txt)" = 1 \
    && cp -a /probe/ort-baseline /probe/ort-patched \
    && sed -i "s#pytorch_cpuinfo;https://github.com/pytorch/cpuinfo/archive/${CPUINFO_BASELINE_COMMIT}.zip;${CPUINFO_BASELINE_SHA1}#pytorch_cpuinfo;https://github.com/pytorch/cpuinfo/archive/${CPUINFO_PATCHED_COMMIT}.zip;${CPUINFO_PATCHED_SHA1}#" /probe/ort-patched/cmake/deps.txt \
    && test "$(grep -Fc "pytorch_cpuinfo;https://github.com/pytorch/cpuinfo/archive/${CPUINFO_PATCHED_COMMIT}.zip;${CPUINFO_PATCHED_SHA1}" /probe/ort-patched/cmake/deps.txt)" = 1 \
    && diff -qr --exclude=deps.txt /probe/ort-baseline /probe/ort-patched \
    && (diff -u --label a/cmake/deps.txt --label b/cmake/deps.txt \
        /probe/ort-baseline/cmake/deps.txt /probe/ort-patched/cmake/deps.txt \
        > /tmp/actual-cpuinfo.patch; test "$?" = 1) \
    && cmp /probe/expected-cpuinfo.patch /tmp/actual-cpuinfo.patch

ARG CPUINFO_VARIANT
RUN case "${CPUINFO_VARIANT}" in baseline|patched) ;; *) exit 64 ;; esac \
    && case "${CPUINFO_VARIANT}" in \
        baseline) selected_commit="${CPUINFO_BASELINE_COMMIT}"; selected_sha1="${CPUINFO_BASELINE_SHA1}"; selected_sha256="${CPUINFO_BASELINE_SHA256}" ;; \
        patched) selected_commit="${CPUINFO_PATCHED_COMMIT}"; selected_sha1="${CPUINFO_PATCHED_SHA1}"; selected_sha256="${CPUINFO_PATCHED_SHA256}" ;; \
       esac \
    && curl --fail --location --proto '=https' --tlsv1.2 \
        --output /tmp/cpuinfo.zip \
        "https://github.com/pytorch/cpuinfo/archive/${selected_commit}.zip" \
    && echo "${selected_sha1}  /tmp/cpuinfo.zip" | sha1sum --check --strict \
    && echo "${selected_sha256}  /tmp/cpuinfo.zip" | sha256sum --check --strict \
    && mkdir -p /tmp/cpuinfo-extract \
    && unzip -q /tmp/cpuinfo.zip -d /tmp/cpuinfo-extract \
    && test -d "/tmp/cpuinfo-extract/cpuinfo-${selected_commit}" \
    && test "$(find /tmp/cpuinfo-extract -mindepth 1 -maxdepth 1 -type d | wc -l)" = 1 \
    && cp -a "/tmp/cpuinfo-extract/cpuinfo-${selected_commit}" /opt/cpuinfo \
    && printf 'commit=%s\narchive_sha1=%s\narchive_sha256=%s\n' \
        "${selected_commit}" "${selected_sha1}" "${selected_sha256}" \
        > /opt/cpuinfo/.pangopup-source-identity \
    && cp -a "/probe/ort-${CPUINFO_VARIANT}" /opt/onnxruntime

RUN cd /opt/onnxruntime \
    && ./build.sh \
        --build --update --config Release --parallel \
        --cmake_generator Ninja \
        --compile_no_warning_as_error \
        --skip_submodule_sync --skip_tests \
        --cmake_extra_defines \
            onnxruntime_BUILD_SHARED_LIB=OFF \
            onnxruntime_ENABLE_CPUINFO=ON \
            FETCHCONTENT_SOURCE_DIR_PYTORCH_CPUINFO=/opt/cpuinfo \
    && test -f /opt/onnxruntime/build/Linux/Release/libonnxruntime_common.a \
    && test -f /opt/onnxruntime/build/Linux/Release/_deps/pytorch_cpuinfo-build/libcpuinfo.a \
    && grep -F 'FETCHCONTENT_SOURCE_DIR_PYTORCH_CPUINFO' /opt/onnxruntime/build/Linux/Release/CMakeCache.txt \
        | grep -F '=/opt/cpuinfo' \
    && nm -g --defined-only /opt/onnxruntime/build/Linux/Release/_deps/pytorch_cpuinfo-build/libcpuinfo.a \
        | grep -F 'cpuinfo_initialize'

WORKDIR /source
COPY Cargo.toml Cargo.lock ./
COPY LICENSE NOTICE ./
COPY assets/notices ./assets/notices
COPY release-profiles ./release-profiles
COPY crates ./crates

# Keep the production manifests untouched. The candidate copy explicitly drops
# download-binaries, so ORT_LIB_LOCATION is the only possible native runtime.
RUN sed -i 's/=2.0.0-rc.12/=2.0.0-rc.13/' crates/pangopup-model/Cargo.toml \
    && sed -i 's/"api-24"/"api-27"/' crates/pangopup-model/Cargo.toml \
    && sed -i '/"download-binaries",/d' crates/pangopup-model/Cargo.toml \
    && cargo update -p ort@2.0.0-rc.12 --precise 2.0.0-rc.13 \
    && cargo tree --locked -e features -i ort-sys > /tmp/ort-features.txt \
    && grep -F 'ort-sys v2.0.0-rc.13' /tmp/ort-features.txt \
    && ! grep -F 'download-binaries' /tmp/ort-features.txt \
    && ORT_LIB_LOCATION=/opt/onnxruntime/build/Linux \
       RUSTFLAGS='-C link-arg=-Wl,-Map=/tmp/pangopup.link.map' \
       cargo build --locked --release --package pangopup-cli \
    && grep -F '/opt/onnxruntime/build/Linux/Release/_deps/pytorch_cpuinfo-build/libcpuinfo.a' /tmp/pangopup.link.map \
    && grep -F '/opt/onnxruntime/build/Linux/Release/libonnxruntime_common.a' /tmp/pangopup.link.map \
    && nm target/release/pangopup | grep -F 'cpuinfo_initialize' \
    && ! readelf -d target/release/pangopup | grep -F 'onnxruntime' \
    && find /opt/onnxruntime/build/Linux/Release -type f -name '*.a' -print0 \
        | sort -z | xargs -0 sha256sum > /tmp/ort-static-libraries.sha256 \
    && sha256sum /tmp/ort-static-libraries.sha256 > /tmp/ort-static-manifest.sha256 \
    && sha256sum /opt/onnxruntime/build/Linux/Release/_deps/pytorch_cpuinfo-build/libcpuinfo.a \
        > /tmp/cpuinfo-library.sha256 \
    && { rustc --version; cargo --version; cc --version | head -1; cmake --version | head -1; ninja --version; python3 --version; } \
        > /tmp/build-versions.txt \
    && strip target/release/pangopup \
    && install -D -m 0755 target/release/pangopup /out/usr/local/bin/pangopup \
    && install -D -m 0444 LICENSE /out/usr/share/licenses/pangopup/LICENSE \
    && install -D -m 0444 NOTICE /out/usr/share/doc/pangopup/NOTICE \
    && install -D -m 0444 /opt/onnxruntime/LICENSE /out/usr/share/licenses/onnxruntime/LICENSE \
    && install -D -m 0444 /opt/onnxruntime/ThirdPartyNotices.txt /out/usr/share/licenses/onnxruntime/ThirdPartyNotices.txt \
    && install -D -m 0444 /opt/cpuinfo/LICENSE /out/usr/share/licenses/cpuinfo/LICENSE \
    && install -D -m 0444 /opt/cpuinfo/.pangopup-source-identity /out/usr/share/doc/pangopup/ticket-048-cpuinfo-source.txt \
    && install -D -m 0444 /tmp/build-versions.txt /out/usr/share/doc/pangopup/ticket-048-build-versions.txt \
    && install -D -m 0444 /tmp/ort-features.txt /out/usr/share/doc/pangopup/ticket-048-ort-features.txt \
    && install -D -m 0444 /tmp/ort-static-manifest.sha256 /out/usr/share/doc/pangopup/ticket-048-ort-static-manifest.sha256 \
    && install -D -m 0444 /tmp/cpuinfo-library.sha256 /out/usr/share/doc/pangopup/ticket-048-cpuinfo-library.sha256

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:d97bc0a941b8d4be647dc0ee75b264ddbb772f1ac5ba690a4309c00723b23775 AS probe

ARG PANGOPUP_REVISION
ARG CPUINFO_VARIANT
ARG ONNXRUNTIME_COMMIT=da9b5e364c465de65c49d91e696cd6485270757f
ARG ONNXRUNTIME_SOURCE_SHA256=9616cbdbbfcb1420b3261cd280a047d74ab0a249825e577b0e2dd310e22f6b83
ARG CPUINFO_BASELINE_COMMIT=4628dc060ce4e82345dc166bbac875609db4ff69
ARG CPUINFO_BASELINE_SHA256=2ed3ebc6c2656cc0aafc7af319e5cb0f97cc9b415eae180f566def84f1ca6a29
ARG CPUINFO_PATCHED_COMMIT=0c0ab15cb0a8bafbbf71c2ae6f84128a4c2a8da6
ARG CPUINFO_PATCHED_SHA256=d40ed2e6134c6b8103ac7916de080a67964f48a8f61c5e72ecca18e9c492f884

LABEL org.opencontainers.image.title="Pangopup Ticket 048 probe" \
      org.opencontainers.image.source="https://github.com/genomoncology/pangopup" \
      org.opencontainers.image.revision="${PANGOPUP_REVISION}" \
      org.opencontainers.image.version="0.1.0-ticket-048" \
      org.opencontainers.image.licenses="GPL-3.0-only AND MIT" \
      org.pangopup.probe.cpuinfo-variant="${CPUINFO_VARIANT}" \
      org.pangopup.probe.onnxruntime-commit="${ONNXRUNTIME_COMMIT}" \
      org.pangopup.probe.onnxruntime-source-sha256="${ONNXRUNTIME_SOURCE_SHA256}" \
      org.pangopup.probe.cpuinfo-baseline-commit="${CPUINFO_BASELINE_COMMIT}" \
      org.pangopup.probe.cpuinfo-baseline-sha256="${CPUINFO_BASELINE_SHA256}" \
      org.pangopup.probe.cpuinfo-patched-commit="${CPUINFO_PATCHED_COMMIT}" \
      org.pangopup.probe.cpuinfo-patched-sha256="${CPUINFO_PATCHED_SHA256}"

COPY --from=builder /out/ /
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/pangopup"]
