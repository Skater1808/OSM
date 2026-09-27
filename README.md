# OSM - Minimaler x86 Kernel in Rust + NASM Bootloader

Ein komplettes, bootbares x86 OS-Projekt mit:
- **Bootloader** in NASM Assembly (16-bit Real Mode → 32-bit Protected Mode)
- **Kernel** in Rust (`no_std`, `no_main`) mit VGA-Textausgabe
- **Build-Scripts** für automatisierte Kompilierung und QEMU-Start

---

## 📁 Projektstruktur

```
OSM/
├── boot.asm           # NASM Bootloader (512 Bytes, MBR)
├── linker.ld          # Linker Script (Kernel @ 0x10000)
├── Cargo.toml         # Rust Projekt Konfiguration
├── .cargo/config.toml # Linker Flags für bare-metal
├── build.sh           # Build Script (Kernel + Bootloader + Image)
├── run.sh             # QEMU Start (Release)
├── debug.sh           # QEMU + GDB Server (Port 1234)
├── src/
│   └── main.rs        # Kernel Entry Point + VGA Writer
└── README.md          # Diese Datei
```

---

## 🔧 Installation der Voraussetzungen

### Arch Linux / Manjaro
```bash
sudo pacman -S nasm qemu-base rustup binutils i686-linux-gnu-binutils
rustup default stable
rustup target add i686-unknown-none-gnu
```

### Ubuntu / Debian
```bash
sudo apt update
sudo apt install nasm qemu-system-x86 rustup binutils gcc-multilib g++-multilib
rustup default stable
rustup target add i686-unknown-none-gnu
```

### Fedora
```bash
sudo dnf install nasm qemu-system-x86 rustup binutils glibc-devel.i686
rustup default stable
rustup target add i686-unknown-none-gnu
```

### macOS (mit Homebrew)
```bash
brew install nasm qemu rustup x86_64-elf-binutils
rustup default stable
rustup target add i686-unknown-none-gnu
# Hinweis: i686-linux-gnu-ld heißt hier i386-elf-ld
# .cargo/config.toml anpassen!
```

---

## 🏗️ Build & Run

### Einmaliger Build
```bash
cd OSM
./build.sh
```

Erzeugt:
- `boot.bin`      - 512 Byte Bootloader
- `kernel.bin`    - Kernel Flat Binary
- `floppy.img`    - 1.44MB bootbares Disk Image

### In QEMU starten (Release)
```bash
./run.sh
```

### Mit GDB Debuggen
Terminal 1:
```bash
./debug.sh
```

Terminal 2:
```bash
gdb target/i686-unknown-none-gnu/release/osm_kernel
(gdb) target remote :1234
(gdb) break kernel_main
(gdb) continue
(gdb) layout src
```

---

## 📖 Code-Erklärung

### Bootloader (`boot.asm`)
1. **Real Mode (16-bit)**: BIOS-Interrupts nutzbar
2. **Lädt Kernel** von Sektor 2+ (LBA) nach `0x10000` (1 MB)
3. **GDT setzen** und **Protected Mode** aktivieren (CR0.PE=1)
4. **Far Jump** zu 32-bit Code Segment
5. **Stack** einrichten und **Kernel aufrufen** (`call 0x10000`)

### Kernel (`src/main.rs`)
- **`no_std`**: Keine Rust Standard Library
- **`no_main`**: Eigenes `_start` Symbol
- **VGA Writer**: Schreibt direkt in `0xB8000` (Textspeicher)
- **Panic Handler**: Zeigt Fehlerinfos auf Screen
- **`kernel_main`**: Endlosschleife mit `hlt` (CPU spart Strom)

### Linker Script (`linker.ld`)
- Kernel Base Address: **0x10000** (1 MB)
- Sections: `.text`, `.rodata`, `.data`, `.bss`
- Entry Point: `_start` (aus Assembly in `main.rs`)

---

## 🎯 Nützliche QEMU Optionen

| Option | Zweck |
|--------|-------|
| `-serial stdio` | Serielle Ausgabe ins Terminal |
| `-display gtk` | GTK Fenster (oder `sdl`, `cocoa`, `vnc=:1`, `none`) |
| `-d int,cpu_reset` | Debug: Interrupts & Resets loggen |
| `-D qemu.log` | Log in Datei schreiben |
| `-no-reboot` | Bei Triple Fault nicht neustarten |
| `-s -S` | GDB Server auf :1234, CPU anhalten |

---

## 🐛 Troubleshooting

### "i686-linux-gnu-ld: not found"
```bash
# Ubuntu/Debian
sudo apt install binutils-i686-linux-gnu

# Arch
sudo pacman -S i686-linux-gnu-binutils

# Fedora
sudo dnf install binutils-i686-linux-gnu
```

### "target i686-unknown-none-gnu not found"
```bash
rustup target add i686-unknown-none-gnu
```

### Kernel zu groß für 16 Sektoren (8 KB)
`boot.asm` anpassen: `mov al, 32` (16 KB) oder mehr.
`build.sh`: `dd ... seek=1` bleibt gleich.

### Schwarzer Screen / Keine Ausgabe
- QEMU Log prüfen: `cat qemu.log`
- Serial Output: `-serial stdio` zeigt Panics
- VGA Adresse prüfen: `0xB8000` ist Standard für Textmodus

---

## 🚀 Erweiterungsideen

- [ ] IDT + Interrupt Handler (Timer, Tastatur)
- [ ] Paging + Heap Allocator (`linked_list_allocator`)
- [ ] PS/2 Tastaturtreiber
- [ ] Serial Logging (COM1: 0x3F8)
- [ ] Multiboot2 Support (GRUB booten)
- [ ] Userspace + Syscalls

---

## 📚 Weiterführende Ressourcen

- [OSDev Wiki](https://wiki.osdev.org/)
- [Writing an OS in Rust](https://os.phil-opp.com/)
- [NASM Tutorial](https://cs.lmu.edu/~ray/notes/nasmtutorial/)
- [Rust Embedded Book](https://docs.rust-embedded.org/book/)
- [Intel SDM Vol. 3](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html)

---

## 📄 Lizenz

MIT / Public Domain - Frei für Lernzwecke und Experimente.