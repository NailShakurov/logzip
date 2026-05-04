FROM rust:1.87-slim AS builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release --bin logzip

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/logzip /usr/local/bin/logzip
EXPOSE 8080
ENTRYPOINT ["logzip"]
CMD ["http"]
