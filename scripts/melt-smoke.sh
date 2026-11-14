#!/bin/sh
# Renders each example with melt using the freshly built module.
# Run from the repo root after `cmake --build build`.
set -eu

out_dir=build/smoke
repo_dir=$out_dir/mlt-repository
rm -rf "$out_dir"
mkdir -p "$repo_dir"

# melt only loads modules from one folder, so link the system modules and ours together.
ln -s "$(pkg-config --variable=moduledir mlt-framework-7)"/* "$repo_dir"/
ln -s "$PWD/build/libmltanicode.so" "$repo_dir"/
export MLT_REPOSITORY="$PWD/$repo_dir"

melt -query producers | grep -q -- "- anicode" || { echo "anicode producer not found"; exit 1; }

for scene in examples/*.mlt; do
    name=$(basename "$scene" .mlt)
    melt "$scene" out=29 -consumer avformat:"$out_dir/$name.mov" vcodec=prores_ks
    test -s "$out_dir/$name.mov" || { echo "$name: no output"; exit 1; }
    echo "$name: ok"
done
