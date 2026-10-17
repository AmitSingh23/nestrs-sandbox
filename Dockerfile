FROM rust:1-slim-bookworm AS builder
WORKDIR /app

# Build dependencies first so they're cached between source changes.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/nestrs-app /usr/local/bin/nestrs-app

ENV HOST=0.0.0.0 \
    PORT=3000
EXPOSE 3000
CMD ["nestrs-app"]
