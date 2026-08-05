#/bin/bash

set -e

cargo build

RUST_LOG=debug ./target/debug/rain-appd &

RUST_LOG=debug ./target/debug/rain-shell
