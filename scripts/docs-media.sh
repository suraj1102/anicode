#!/bin/sh
# Renders the scenes shown in the docs to files the mdBook pages embed: a
# video for an animated scene, a PNG for a still one. Covers the examples in
# docs/src/examples and the gallery scenes in examples/.
# Run from the repo root, then `mdbook serve docs --open`.
set -eu

media=docs/src/media
frames=build/docs-frames

# Reads `key = value` from a TOML file. Only used for keys that appear once.
setting() {
    sed -n "s/^$1 = //p" "$2" | tr -d '"#'
}

fps=$(setting fps config/anicode.toml)
width=$(setting width config/docs.toml)
background=$(setting background config/docs.toml)

# Scenes have a transparent background, so fill one in under each frame.
filter="split[a][b];[a]drawbox=color=0x$background:thickness=fill[bg];[bg][b]overlay,scale=$width:-2,format=yuv420p"

render() {
    scene=$1
    name=$2
    rm -rf "$frames"
    target/release/anicode render "$scene" --out "$frames" >/dev/null
    rm -f "$media/$name.png" "$media/$name.mp4"
    if [ "$(ls "$frames" | wc -l)" -eq 1 ]; then
        ffmpeg -loglevel error -i "$frames/frame_00000.png" -vf "$filter" "$media/$name.png"
    else
        ffmpeg -loglevel error -framerate "$fps" -i "$frames/frame_%05d.png" -vf "$filter" \
            -c:v libx264 -crf 23 -movflags +faststart "$media/$name.mp4"
    fi
    echo "$name"
}

cargo build --release -q -p anicode-cli
mkdir -p "$media"
for scene in docs/src/examples/*.lua; do
    render "$scene" "$(basename "$scene" .lua)"
done
for scene in examples/*.lua; do
    render "$scene" "gallery-$(basename "$scene" .lua)"
done
rm -rf "$frames"
