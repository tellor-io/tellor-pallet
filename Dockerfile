FROM rust:slim as test
RUN cargo install cargo-nextest --locked && cargo install cargo-cache && cargo cache -r all

FROM test
WORKDIR /pallet
COPY . .
RUN cargo nextest run --all --release
ENTRYPOINT cargo nextest run --all --release
