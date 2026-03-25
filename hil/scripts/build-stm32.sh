#!/usr/bin/env bash
# Build board-support-stm32 firmware for multiple chip targets.
#
# Usage:
#   scripts/build-stm32.sh [OUTPUT_DIR]
#
# Iterates over the CHIPS array, builds each variant with the correct
# feature flag, and copies the resulting ELF to OUTPUT_DIR.

set -euo pipefail

CHIPS=(
    stm32f401cc
    stm32f411ce
    stm32h743vi
)

OUTPUT_DIR="${1:-firmware-out}"
TARGET="thumbv7em-none-eabihf"

mkdir -p "$OUTPUT_DIR"

for chip in "${CHIPS[@]}"; do
    echo "Building board-support-stm32 for $chip..."
    cargo build \
        -p board-support-stm32 \
        --target "$TARGET" \
        --release \
        --no-default-features \
        --features "$chip"

    src="target/$TARGET/release/board-support-stm32"
    cp "$src" "$OUTPUT_DIR/board-support-$chip.elf"
    echo "  -> $OUTPUT_DIR/board-support-$chip.elf"
done

echo "All firmware builds complete in $OUTPUT_DIR/"
