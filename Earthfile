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
	ARG release="n"
	RUN cargo build $(test "$release" = "y" && echo "--release")
	SAVE ARTIFACT target/*/photosort bin
	SAVE ARTIFACT target/ AS LOCAL .

build-release:
	BUILD +build --release="y"

build-image:
	FROM docker.io/library/debian:bullseye
	COPY scripts/tags.sh /usr/local/bin/tags
	COPY +build/bin /usr/local/bin/photosort
	ENTRYPOINT ["/usr/local/bin/photosort"]
	CMD ["--help"]
	ARG prefix="negrel"
	ARG tag="dev"
	FOR t IN $(tags $tag) 
		SAVE IMAGE --push "$prefix/photosort:$t"
	END
