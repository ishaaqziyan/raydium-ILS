# syntax=docker/dockerfile:1

FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Cache dependency compilation separately from source changes.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

# reqwest's rustls-tls feature bundles Mozilla's root store via
# webpki-roots at compile time, so the app itself needs no
# ca-certificates package to make HTTPS calls to Helius — but the
# Doppler CLI (installed below) does, to fetch secrets at boot.
FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl gnupg \
    && curl -Ls https://cli.doppler.com/install.sh | sh \
    && apt-get remove -y curl gnupg \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --system --create-home --shell /usr/sbin/nologin appuser
COPY --from=builder /app/target/release/raydium-il-backend ./raydium-il-backend
USER appuser

ENV RUST_LOG=info
EXPOSE 3001
CMD ["doppler", "run", "--", "./raydium-il-backend"]
