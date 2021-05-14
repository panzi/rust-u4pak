#!/usr/bin/bash

set -e

SELF=$(readlink -f "$0")
DIR=$(dirname "$SELF")

cd "$DIR"

release_id=$(git describe --tags || git rev-parse HEAD)
release_dir="$DIR/release"

if [[ -e "$release_dir" ]]; then
    rm -r "$release_dir"
fi

if [[ -e "$DIR/release-$release_id.zip" ]]; then
    rm "$DIR/release-$release_id.zip"
fi

mkdir -p "$release_dir"

cargo build

{
    "./target/debug/u4pak" help

    for cmd in help check info list unpack pack mount; do
        printf '=%.0s' {1..120}; echo
        "./target/debug/u4pak" help $cmd
    done
} > "$release_dir/Help.txt"

pandoc -f markdown -t html5 -o "$release_dir/README.html" README.md
cp LICENSE.txt "$release_dir"

for target in x86_64-unknown-linux-gnu x86_64-pc-windows-gnu; do
    cargo clean --release --target="$target"
    cargo build --release --target="$target"
    mkdir "$release_dir/$target"
    if [[ "$target" =~ .*windows.* ]]; then
        cp "./target/$target/release/u4pak.exe" "$release_dir/$target"
    else
        cp "./target/$target/release/u4pak" "$release_dir/$target"
    fi
done

pushd "$release_dir"
zip -9r "$DIR/release-$release_id.zip" .
popd
