#!/bin/bash
# Debug-Script: Startet QEMU mit GDB Server

set -e

IMG="floppy.img"

if [ ! -f "$IMG" ]; then
    echo "Image $IMG nicht gefunden. Führe zuerst ./build.sh aus."
    exit 1
fi

echo "=== Starte OSM in QEMU (GDB Debug Mode) ==="
echo "In anderem Terminal: gdb target/i686-unknown-none-gnu/release/osm_kernel"
echo "  (gdb) target remote :1234"
echo "  (gdb) break kernel_main"
echo "  (gdb) continue"
echo ""

qemu-system-i386 \
    -fda "$IMG" \
    -boot a \
    -m 32M \
    -serial stdio \
    -display gtk \
    -no-reboot \
    -s -S \
    -d int,cpu_reset \
    -D qemu.log