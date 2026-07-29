ARG CROSS_BASE_IMAGE=scratch
ARG DEBIAN_FRONTEND=noninteractive

FROM $CROSS_BASE_IMAGE

RUN apt-get update && \
    apt-get install -y --no-install-recommends clang llvm && \
    rm -rf /var/lib/apt/lists/*

ENV CC=clang \
    CXX=clang++ \
    AR=llvm-ar
