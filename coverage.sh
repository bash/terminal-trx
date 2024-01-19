#!/usr/bin/env bash

export LLVM_PROFILE_FILE='target/coverage/your_name-%p-%m.profraw'
export RUSTFLAGS="-Cinstrument-coverage"
rm -rf target/coverage/
rm -rf target/debug/coverage/
cargo build
cargo test
grcov target/coverage -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage/
