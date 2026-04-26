FROM rust:alpine as builder
RUN apk add --no-cache musl-dev gcc make sqlite-dev pkgconfig openssl-dev git
WORKDIR /app
COPY . .
# ⚡ Purge any leaked host configs that force GNU targeting
RUN rm -rf .cargo
RUN cargo build --release -j 1

FROM alpine:latest
RUN apk add --no-cache libgcc libstdc++ openssl sqlite-libs ca-certificates
WORKDIR /nexus
COPY --from=builder /app/target/release/nexus_singularity /nexus/nexus
COPY --from=builder /app/target/release/nexus_cli /nexus/nexus_cli
RUN mkdir -p .nexus/vault .nexus/reports
EXPOSE 3000
CMD ["./nexus"]
