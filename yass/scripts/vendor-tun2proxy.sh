#!/bin/bash
set -x
set -e
PWD=$(dirname "${BASH_SOURCE[0]}")
cd $PWD/..

function patch_config_toml {
  cat >> $PWD/.cargo/config.toml << EOF

[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF
}

pushd third_party/tun2proxy
cargo install cargo-vendor-filterer
rm -rf vendor
#cargo vendor
cargo vendor-filterer --platform=i686-linux-android \
                      --platform=x86_64-linux-android \
                      --platform=armv7-linux-androideabi \
                      --platform=aarch64-linux-android \
                      --platform=aarch64-apple-ios \
                      --platform=x86_64-apple-ios \
                      --platform=aarch64-apple-ios-sim \
                      --platform=x86_64-unknown-linux-ohos \
                      --platform=armv7-unknown-linux-ohos \
                      --platform=aarch64-unknown-linux-ohos
grep vendor "$PWD/.cargo/config.toml" || patch_config_toml
popd
