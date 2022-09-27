VERSION 0.6
FROM docker.io/library/rust:1.64
WORKDIR /usr/src/photosort
ARG CARGO_HOME="$PWD/.cargo"

fetch:
	COPY Cargo.toml .
	COPY Cargo.lock .
	RUN cargo fetch --locked

test:
	FROM +fetch
	COPY src/ src/
	RUN cargo test

build:
	FROM +fetch
	COPY src/ src/
	ARG release="y"
	RUN cargo build $(test "$release" = "y" && echo "--release")
	SAVE ARTIFACT target/release/photosort bin
	SAVE ARTIFACT target/ targe_dir AS LOCAL target/

build-debug:
	FROM +build --release="n"

build-local:
	ARG OUT="target/release/photosort"
	COPY +build/bin $OUT

build-image:
	FROM docker.io/library/debian:bullseye
	COPY +build/bin /usr/local/bin/photosort
	ENTRYPOINT ["/usr/local/bin/photosort"]
	CMD ["--help"]
	ARG prefix="negrel"
	ARG tag="latest"
	SAVE IMAGE $prefix/photosort:$tag

