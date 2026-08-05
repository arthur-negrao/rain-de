#/bin/bash

set -e

cargo build --release

RUST_LOG=info ./target/debug/rain-appd &

RUST_LOG=info ./target/debug/rain-shell
