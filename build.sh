#!/bin/bash
# Build-Script für OSM Kernel + Bootloader

set -e  # Bei Fehler abbrechen

echo "=== OSM OS Build ==="

# 1. Kernel kompilieren (nutzt Custom Target Spec, braucht Nightly + build-std)
echo "[1/4] Kompiliere Rust Kernel..."
RUSTFLAGS="-C link-arg=-Tlinker.ld -C link-arg=-nostdlib -C link-arg=-static" \
cargo +nightly build -Zjson-target-spec -Zbuild-std=core,compiler_builtins --target i686-osm.json --release

# 2. Bootloader assemblieren
echo "[2/4] Assembliere Bootloader (NASM)..."
nasm -f bin boot.asm -o boot.bin

# 3. Kernel ELF zu flat binary konvertieren
echo "[3/4] Konvertiere Kernel zu Flat Binary..."
objcopy -O binary \
    target/i686-osm/release/osm_kernel \
    kernel.bin

# 4. Disk Image erstellen (1.44MB Floppy)
echo "[4/4] Erstelle Disk Image (floppy.img)..."
dd if=/dev/zero of=floppy.img bs=512 count=2880 2>/dev/null
dd if=boot.bin of=floppy.img conv=notrunc 2>/dev/null
dd if=kernel.bin of=floppy.img bs=512 seek=1 conv=notrunc 2>/dev/null

echo ""
echo "=== Build erfolgreich! ==="
echo "Dateien:"
echo "  boot.bin     - Bootloader (512 Bytes)"
echo "  kernel.bin   - Kernel Binary"
echo "  floppy.img   - Bootbares Disk Image"
echo ""
echo "Starten mit: ./run.sh"