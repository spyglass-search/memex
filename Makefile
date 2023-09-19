.PHONY: build build-all fmt clippy docker-build setup-examples
.default: build

GIT_HASH ?= $(shell git rev-parse --short HEAD)

build:
	cargo build -p memex

build-all:
	cargo build --all

fmt:
	cargo fmt --all

clippy: fmt
	cargo clippy --all

docker-build:
	docker build \
		--build-arg GIT_HASH=$(GIT_HASH) \
		-f Dockerfile \
		-t getspyglass/memex:latest .

setup-examples:
	mkdir -p resources/models/Wizard-Vicuna
ifeq (,$(wildcard ./resources/models/Wizard-Vicuna/Wizard-Vicuna-7B-Uncensored.ggmlv3.q4_0.bin))
	wget -P resources/models/Wizard-Vicuna https://huggingface.co/TheBloke/Wizard-Vicuna-7B-Uncensored-GGML/resolve/main/Wizard-Vicuna-7B-Uncensored.ggmlv3.q4_0.bin
else
	@echo "-> Skipping model download, Wizard-Vicuna-7B-Uncensored.ggmlv3.q4_0.bin exists"
endif