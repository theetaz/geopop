FROM --platform=$BUILDPLATFORM rust:bookworm AS builder
ARG TARGETARCH
RUN dpkg --add-architecture arm64 && \
    apt-get update && \
    if [ "$TARGETARCH" = "arm64" ]; then \
      apt-get install -y gcc-aarch64-linux-gnu libssl-dev:arm64 pkg-config && \
      rustup target add aarch64-unknown-linux-gnu; \
    else \
      apt-get install -y libssl-dev pkg-config; \
    fi
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
ENV PKG_CONFIG_SYSROOT_DIR=/usr/aarch64-linux-gnu
ENV PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig
ENV OPENSSL_DIR=/usr/aarch64-linux-gnu
WORKDIR /app
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    if [ "$TARGETARCH" = "arm64" ]; then \
      cargo build --release --target aarch64-unknown-linux-gnu; \
    else \
      cargo build --release; \
    fi && rm -rf src
COPY src/ src/
RUN touch src/main.rs && \
    if [ "$TARGETARCH" = "arm64" ]; then \
      cargo build --release --target aarch64-unknown-linux-gnu && \
      cp target/aarch64-unknown-linux-gnu/release/geopop-api target/release/geopop-api; \
    else \
      cargo build --release; \
    fi

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /app/target/release/geopop-api /usr/local/bin/geopop-api
EXPOSE 8080
CMD ["geopop-api"]
