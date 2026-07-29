# set shell := ["nu", "-c"]

run:
    cargo run --package wind --bin wind -- -f config.toml
test:
    cargo test -- --ignored

fast-release:
    cross build --profile fast-release

server:
  cargo run --package tuic-server --bin tuic-server -- -c ./config.toml