#!/bin/bash
# QEMU Run-Script mit grafischem Display (VNC)

set -e

IMG="floppy.img"

if [ ! -f "$IMG" ]; then
    echo "Image $IMG nicht gefunden. Führe zuerst ./build.sh aus."
    exit 1
fi

echo "=== Starte OSM in QEMU (VNC Display) ==="
echo "Verbinden mit: vncviewer :0"
echo "Oder im Browser: http://localhost:6080/vnc.html (wenn websockify läuft)"
echo ""

# VNC Display auf Port 5900 (:0)
qemu-system-i386 \
    -fda "$IMG" \
    -boot a \
    -m 32M \
    -serial stdio \
    -vnc :0 \
    -no-reboot \
    -d int,cpu_reset \
    -D qemu.log