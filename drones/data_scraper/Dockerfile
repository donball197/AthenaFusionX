FROM docker.io/library/rust:alpine AS builder
# Added openssl-dev and pkgconfig to fix the build error
RUN apk add --no-cache musl-dev openssl-dev pkgconfig
WORKDIR /app
COPY . .
RUN rm -f Cargo.lock && cargo build --release

FROM docker.io/library/alpine:latest
# We need openssl in the final image too for it to run
RUN apk add --no-cache openssl
WORKDIR /
COPY --from=builder /app/target/release/data-scraper /data_scraper
EXPOSE 5000
CMD ["/data_scraper"]
