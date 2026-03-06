FROM --platform=$BUILDPLATFORM rust:bookworm AS builder
ARG TARGETARCH
RUN if [ "$TARGETARCH" = "arm64" ]; then \
      apt-get update && apt-get install -y gcc-aarch64-linux-gnu && \
      rustup target add aarch64-unknown-linux-gnu; \
    fi
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
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
