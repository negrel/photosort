FROM docker.io/library/rust:1.64 AS base

WORKDIR /usr/src/photosort

COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo fetch --locked

COPY src/ src/

RUN cargo build --release

FROM docker.io/bitnami/minideb:bullseye
COPY --from=base /usr/src/photosort/target/release/photosort /usr/local/bin/photosort

ENTRYPOINT /usr/local/bin/photosort
CMD "--help"

