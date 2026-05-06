FROM rust:1.80-slim-bookworm as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p nexus_node

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/nexus_node /usr/local/bin/node
EXPOSE 8080
CMD ["node"]
