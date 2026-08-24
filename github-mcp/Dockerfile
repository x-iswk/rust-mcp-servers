FROM rust:1.97-slim AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:trixie-slim

COPY --from=builder \
  /app/target/release/github-mcp \
  /usr/local/bin/github-mcp

CMD ["github-mcp"]
