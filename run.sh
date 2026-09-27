#!/bin/bash
# QEMU Run-Script für OSM

set -e

IMG="floppy.img"

if [ ! -f "$IMG" ]; then
    echo "Image $IMG nicht gefunden. Führe zuerst ./build.sh aus."
    exit 1
fi

echo "=== Starte OSM in QEMU (nographic - Terminal Mode) ==="
echo "Beenden mit: Ctrl+A dann X"
echo ""

# -nographic: Kein grafisches Fenster, Serial Port auf stdio (Eingabe + Ausgabe)
# Das ermöglicht Tastatureingabe direkt im Terminal
qemu-system-i386 \
    -fda "$IMG" \
    -boot a \
    -m 32M \
    -nographic \
    -no-reboot \
    -d int,cpu_reset \
    -D qemu.log