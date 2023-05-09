# Tellor

[![Check Set-Up & Build](https://github.com/tellor-io/tellor-pallet/actions/workflows/check.yml/badge.svg?branch=main)](https://github.com/tellor-io/tellor-pallet/actions/workflows/check.yml)
[![Run Tests](https://github.com/tellor-io/tellor-pallet/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/tellor-io/tellor-pallet/actions/workflows/test.yml)
[![Discord Chat](https://img.shields.io/discord/461602746336935936)](https://discord.gg/tellor)
[![Twitter Follow](https://img.shields.io/twitter/follow/wearetellor?style=social)](https://twitter.com/WeAreTellor)

## Setup Environment & Run Tests
### Option 1: Run tests using local environment
- install Rust as per https://docs.substrate.io/install/
- install `cargo-nextest` to local environment:
```shell
cargo install cargo-nextest --locked
```
- run the `pallet` tests using the command:
```shell
cargo nextest run --all --release
```
- run the `runtime-api` tests using the command:
```shell
cd ./runtime-api/ && cargo nextest run --all --release
```
### Option 2: Docker
Run tests in Docker container:
- [Install Docker](https://docs.docker.com/get-docker/)
- Allocate at least 8GB of RAM to Docker, 3GB swap space, or you'll get out of memory errors
- Build and run the `tellor-pallet-tests` image defined in `Dockerfile` using the command:
```shell
docker build -t tellor-pallet-tests . && docker run --rm tellor-pallet-tests
```
- Build and run the `tellor-runtime-api-tests` image defined in `Dockerfile.runtime-api` using the command:
```shell
docker build -f Dockerfile.runtime-api -t tellor-runtime-api-tests . && docker run --rm tellor-runtime-api-tests
```