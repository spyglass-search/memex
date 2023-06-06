.PHONY: docker-build
.default: build

build:
	cargo build -p sightglass

fmt:
	cargo fmt

clippy: fmt
	cargo clippy

docker-build:
	docker build -f Dockerfile -t spyglass-search/sightglass:latest .