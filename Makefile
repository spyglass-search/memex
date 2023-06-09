.PHONY: docker-build
.default: build

build:
	cargo build -p memex

fmt:
	cargo fmt --all

clippy: fmt
	cargo clippy --all

docker-build:
	docker build \
		--build-arg GIT_HASH=$(git rev-parse --short HEAD) \
		-f Dockerfile \
		-t spyglass-search/memex:latest .