ARG BUILDPLATFORM

# ------------------------------------------------------------------------------
# PDFium Stage
# ------------------------------------------------------------------------------

FROM --platform=$BUILDPLATFORM alpine:3.21 AS pdfium

ARG TARGETARCH

RUN set -eux; \
    case "$TARGETARCH" in \
        amd64) asset="pdfium-linux-x64.tgz"; checksum="727145701ce5bb0344c389071b7aa8f1e15ae81a" ;; \
        arm64) asset="pdfium-linux-arm64.tgz"; checksum="c23942df3dc35f95a7bfa75fc52a480e77b5b0ca" ;; \
        *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    mkdir /pdfium; \
    wget -qO- "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/7763/${asset}" \
        | tar -xz -C /pdfium; \
    echo "${checksum}  /pdfium/lib/libpdfium.so" | sha1sum -c -; \
    rm -rf /pdfium/include /pdfium/PDFiumConfig.cmake /pdfium/VERSION /pdfium/args.gn

# ------------------------------------------------------------------------------
# kepubify Stage
# ------------------------------------------------------------------------------

FROM --platform=$BUILDPLATFORM alpine:3.21 AS kepubify

ARG TARGETARCH
ARG KEPUBIFY_VERSION=4.0.4

RUN set -eux; \
    case "$TARGETARCH" in \
        amd64) asset="kepubify-linux-64bit"; checksum="37d7628d26c5c906f607f24b36f781f306075e7073a6fe7820a751bb60431fc5" ;; \
        arm64) asset="kepubify-linux-arm64"; checksum="5a15b8f6f6a96216c69330601bca29638cfee50f7bf48712795cff88ae2d03a3" ;; \
        *) echo "Unsupported architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    wget -qO /kepubify "https://github.com/pgaskin/kepubify/releases/download/v${KEPUBIFY_VERSION}/${asset}"; \
    echo "${checksum}  /kepubify" | sha256sum -c -; \
    chmod 755 /kepubify

COPY docker/licenses/kepubify.txt /licenses/kepubify/LICENSE

# ------------------------------------------------------------------------------
# Frontend Build Stage
# ------------------------------------------------------------------------------

FROM --platform=$BUILDPLATFORM node:22.14.0-alpine3.21 AS frontend

WORKDIR /app

COPY package.json yarn.lock lerna.json .npmrc ./
COPY apps/web/package.json apps/web/
COPY packages/browser/package.json packages/browser/
COPY packages/client/package.json packages/client/
COPY packages/components/package.json packages/components/
COPY packages/graphql/package.json packages/graphql/
COPY packages/i18n/package.json packages/i18n/
COPY packages/sdk/package.json packages/sdk/

RUN --mount=type=cache,target=/usr/local/share/.cache/yarn,id=stump-yarn \
    HUSKY=0 yarn install --frozen-lockfile --network-timeout 300000

COPY tsconfig.json tsconfig.options.json ./
COPY apps/web apps/web
COPY apps/desktop/src apps/desktop/src
COPY packages packages
COPY crates/graphql/schema.graphql crates/graphql/

RUN yarn web build && \
    mv ./apps/web/dist/ ./build && \
    test -n "$(ls -A ./build)"

# ------------------------------------------------------------------------------
# Cargo Build Stage
# ------------------------------------------------------------------------------

FROM rust:1.92-bookworm AS builder

ARG TARGETARCH

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update && apt-get install -y --no-install-recommends cmake

# Cargo build for stump
WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY .cargo .cargo
COPY apps/server apps/server
COPY core core
COPY crates crates
COPY docker/build_server.sh docker/
COPY --from=pdfium /pdfium /opt/pdfium

ARG GIT_REV
ARG BUILD_CHANNEL

ENV GIT_REV=${GIT_REV}
ENV BUILD_CHANNEL=${BUILD_CHANNEL}

RUN --mount=type=cache,target=/usr/local/cargo/registry,id=stump-cargo-registry-${TARGETARCH} \
    --mount=type=cache,target=/usr/local/cargo/git,id=stump-cargo-git-${TARGETARCH} \
    --mount=type=cache,target=/app/target,id=stump-cargo-target-${TARGETARCH} \
    ./docker/build_server.sh && \
    cp ./target/release/stump_server ./stump_server && \
    strip --strip-unneeded ./stump_server /opt/pdfium/lib/libpdfium.so

# ------------------------------------------------------------------------------
# Final Stage
# ------------------------------------------------------------------------------

FROM gcr.io/distroless/cc-debian12:debug

ARG PORT=10801

LABEL org.opencontainers.image.title="Stump" \
      org.opencontainers.image.description="A free and open source comics, manga and digital book server with OPDS support" \
      org.opencontainers.image.source="https://github.com/stumpapp/stump" \
      org.opencontainers.image.url="https://stumpapp.dev" \
      org.opencontainers.image.documentation="https://stumpapp.dev/guides" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.vendor="stumpapp"

RUN [ "/busybox/ln", "-s", "/busybox/sh", "/bin/sh" ]
RUN ln -s /busybox/env /usr/bin/env

# TODO(distroless): Ensure locales aren't messed up
# RUN apt-get update && apt-get install -y locales-all && rm -rf /var/lib/apt/lists/*; \
#     mkdir -p config && mkdir -p data && mkdir -p app

COPY --from=builder /app/stump_server /app/stump
COPY --from=builder /opt/pdfium /opt/pdfium
COPY --from=kepubify /kepubify /usr/local/bin/kepubify
COPY --from=kepubify /licenses/kepubify/LICENSE /licenses/kepubify/LICENSE
COPY --from=frontend /app/build /app/client
COPY --chmod=755 docker/entrypoint.sh /entrypoint.sh

# Default Stump environment variables
ENV STUMP_CONFIG_DIR=/config \
    STUMP_CLIENT_DIR=/app/client \
    STUMP_PROFILE=release \
    STUMP_PORT=${PORT} \
    STUMP_IN_DOCKER=true \
    PDFIUM_PATH=/opt/pdfium/lib/libpdfium.so \
    API_VERSION=v1

WORKDIR /app

EXPOSE ${PORT}

ENTRYPOINT ["/entrypoint.sh"]
