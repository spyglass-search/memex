.PHONY: docker-build
.default: build

build:
	cargo build -p memex

fmt:
	cargo fmt --all

clippy: fmt
	cargo clippy --all

docker-build:
	docker build -f Dockerfile -t spyglass-search/memex:latest .