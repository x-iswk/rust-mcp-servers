FROM rust:1.97-slim AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:trixie-slim

COPY --from=builder \
  /app/target/release/db-schema-mcp \
  /usr/local/bin/db-schema-mcp

EXPOSE 8081

CMD ["db-schema-mcp"]
