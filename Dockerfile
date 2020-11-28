FROM rust:1-slim-stretch AS builder
RUN rustup install stable-x86_64-unknown-linux-gnu

RUN apt update && apt install -y libclang-dev

COPY . /sources
WORKDIR /sources
RUN cargo build --release
RUN chown nobody:nogroup /sources/target/release/bibin


FROM debian:stretch-slim
COPY --from=builder /sources/target/release/bibin /opt/bibin

WORKDIR /etc/secrets

USER nobody
EXPOSE 8000
ENTRYPOINT ["/opt/bibin"]
