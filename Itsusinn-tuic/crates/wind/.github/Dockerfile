FROM alpine:latest

# Accept binary names and binary dir via build-args passed by reusable workflow
ARG BINARY_DIR=./docker-bins
ARG SERVER_BINARY_AMD64=wind-x86_64-linux-musl
ARG SERVER_BINARY_ARM64=wind-aarch64-linux-musl
ARG TARGETARCH

# Select correct binary based on TARGETARCH using BuildKit bind mount (no layer created)
RUN --mount=type=bind,source=${BINARY_DIR},target=/tmp/binaries \
    if [ "${TARGETARCH}" = "amd64" ]; then \
      cp /tmp/binaries/${SERVER_BINARY_AMD64} /usr/bin/wind; \
    elif [ "${TARGETARCH}" = "arm64" ]; then \
      cp /tmp/binaries/${SERVER_BINARY_ARM64} /usr/bin/wind; \
    else \
      echo "Unsupported TARGETARCH: ${TARGETARCH}"; exit 1; \
    fi && \
    chmod +x /usr/bin/wind

# Setup environment
RUN mkdir -p /etc/wind

WORKDIR /root
ENV IN_DOCKER=true

ENTRYPOINT [ "/usr/bin/wind" ]
CMD [ "-c", "/etc/wind/config.yaml" ]
