// Minimaler Rust Kernel (no_std, no_main) - src/main.rs
#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_sym)]

use core::arch::global_asm;
use core::panic::PanicInfo;
use core::fmt::Write;

// Module
mod fs;

// Re-export FS functions
use fs::{fs_init, fs_mounted, fs_list_dir, fs_create_dir, fs_read_file, fs_write_file, fs_delete, fs_format};

// ---------------------------------------------------------
// VGA Textspeicher Konstanten
// ---------------------------------------------------------
const VGA_BUFFER: *mut u8 = 0xB8000 as *mut u8;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
const VGA_COLOR: u8 = 0x0F; // Weiß auf Schwarz
const VGA_COLOR_PROMPT: u8 = 0x0A; // Grün auf Schwarz
const VGA_COLOR_INPUT: u8 = 0x0E; // Gelb auf Schwarz

// COM1 Serial Port
const COM1: u16 = 0x3F8;

// PS/2 Keyboard Ports
const KBD_DATA_PORT: u16 = 0x60;
const KBD_STATUS_PORT: u16 = 0x64;
const KBD_CMD_PORT: u16 = 0x64;

// ---------------------------------------------------------
// Globale Writer-Struktur
// ---------------------------------------------------------
struct VgaWriter {
    column: usize,
    row: usize,
    color: u8,
    saved_color: u8,
}

static mut WRITER: VgaWriter = VgaWriter {
    column: 0,
    row: 0,
    color: VGA_COLOR,
    saved_color: VGA_COLOR,
};

impl VgaWriter {
    fn new() -> Self {
        VgaWriter {
            column: 0,
            row: 0,
            color: VGA_COLOR,
            saved_color: VGA_COLOR,
        }
    }

    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            8 => self.backspace(), // Backspace
            byte => {
                if self.column >= VGA_WIDTH {
                    self.new_line();
                }
                let offset = (self.row * VGA_WIDTH + self.column) * 2;
                unsafe {
                    *VGA_BUFFER.add(offset) = byte;
                    *VGA_BUFFER.add(offset + 1) = self.color;
                }
                self.column += 1;
            }
        }
        // Auch an Serial Port senden
        unsafe { serial_write_byte(byte); }
    }

    fn backspace(&mut self) {
        if self.column > 0 {
            self.column -= 1;
            let offset = (self.row * VGA_WIDTH + self.column) * 2;
            unsafe {
                *VGA_BUFFER.add(offset) = b' ';
                *VGA_BUFFER.add(offset + 1) = self.color;
            }
        } else if self.row > 0 {
            self.row -= 1;
            self.column = VGA_WIDTH - 1;
            let offset = (self.row * VGA_WIDTH + self.column) * 2;
            unsafe {
                *VGA_BUFFER.add(offset) = b' ';
                *VGA_BUFFER.add(offset + 1) = self.color;
            }
        }
    }

    fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
    }

    fn write_prompt(&mut self) {
        self.color = VGA_COLOR_PROMPT;
        self.write_string("\n[osm]> ");
        self.color = VGA_COLOR_INPUT;
        // Force serial flush
        unsafe { 
            for b in b"\n[osm]> " {
                serial_write_byte(*b);
            }
        }
    }

    fn new_line(&mut self) {
        self.column = 0;
        if self.row + 1 >= VGA_HEIGHT {
            self.scroll_up();
        } else {
            self.row += 1;
        }
    }

    fn scroll_up(&mut self) {
        unsafe {
            // Alle Zeilen eine nach oben schieben (manuell, ohne memmove)
            for row in 1..VGA_HEIGHT {
                let src = (row * VGA_WIDTH) * 2;
                let dst = ((row - 1) * VGA_WIDTH) * 2;
                for i in 0..VGA_WIDTH * 2 {
                    *VGA_BUFFER.add(dst + i) = *VGA_BUFFER.add(src + i);
                }
            }
            // Letzte Zeile leeren
            let last_line = (VGA_HEIGHT - 1) * VGA_WIDTH * 2;
            for i in 0..VGA_WIDTH * 2 {
                *VGA_BUFFER.add(last_line + i) = if i % 2 == 0 { b' ' } else { self.color };
            }
        }
        self.row = VGA_HEIGHT - 1;
    }

    fn clear_screen(&mut self) {
        unsafe {
            for i in 0..VGA_WIDTH * VGA_HEIGHT * 2 {
                *VGA_BUFFER.add(i) = if i % 2 == 0 { b' ' } else { self.color };
            }
        }
        self.column = 0;
        self.row = 0;
    }

    fn save_color(&mut self) {
        self.saved_color = self.color;
    }

    fn restore_color(&mut self) {
        self.color = self.saved_color;
    }
}

impl Write for VgaWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// ---------------------------------------------------------
// Panic Handler
// ---------------------------------------------------------
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        WRITER.color = 0x4F; // Rot auf Schwarz
        WRITER.write_string("\n\nKERNEL PANIC: ");
        if let Some(msg) = info.payload().downcast_ref::<&str>() {
            WRITER.write_string(msg);
        } else {
            WRITER.write_string("Unbekannter Fehler");
        }
        if let Some(loc) = info.location() {
            WRITER.write_string("\nBei: ");
            WRITER.write_string(loc.file());
            WRITER.write_string(":");
            write_u32(&mut WRITER, loc.line() as u32);
        }
    }
    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}

// Hilfsfunktion: u32 nach String (no_std)
fn write_u32(writer: &mut VgaWriter, mut n: u32) {
    if n == 0 {
        writer.write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        writer.write_byte(buf[i]);
    }
}

// ---------------------------------------------------------
// Input Buffer & Shell
// ---------------------------------------------------------
const INPUT_BUFFER_SIZE: usize = 256;

struct InputBuffer {
    buffer: [u8; INPUT_BUFFER_SIZE],
    len: usize,
    cursor: usize,
}

impl InputBuffer {
    const fn new() -> Self {
        InputBuffer {
            buffer: [0; INPUT_BUFFER_SIZE],
            len: 0,
            cursor: 0,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.cursor = 0;
    }

    fn push(&mut self, c: char) {
        if self.len < INPUT_BUFFER_SIZE - 1 && self.cursor <= self.len {
            // Shift right
            for i in (self.cursor..self.len).rev() {
                self.buffer[i + 1] = self.buffer[i];
            }
            self.buffer[self.cursor] = c as u8;
            self.len += 1;
            self.cursor += 1;
        }
    }

    fn backspace(&mut self) -> bool {
        if self.cursor > 0 {
            // Shift left
            for i in self.cursor..self.len {
                self.buffer[i - 1] = self.buffer[i];
            }
            self.len -= 1;
            self.cursor -= 1;
            self.buffer[self.len] = 0;
            true
        } else {
            false
        }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.buffer[..self.len]) }
    }
}

static mut INPUT_BUFFER: InputBuffer = InputBuffer::new();

// Shell Commands
fn cmd_help(writer: &mut VgaWriter) {
    writer.write_string("\nVerfuegbare Befehle:\n");
    writer.write_string("  help        - Zeigt diese Hilfe\n");
    writer.write_string("  clear       - Bildschirm loeschen\n");
    writer.write_string("  echo <text> - Text ausgeben\n");
    writer.write_string("  version     - Kernel Version anzeigen\n");
    writer.write_string("  reboot      - System neu starten\n");
    writer.write_string("  shutdown    - System herunterfahren (QEMU)\n");
    writer.write_string("  mem         - Speicherinfo\n");
    writer.write_string("  color       - Test Farben\n");
    writer.write_string("\nDateisystem:\n");
    writer.write_string("  ls [pfad]   - Verzeichnis auflisten\n");
    writer.write_string("  cat <datei> - Datei anzeigen\n");
    writer.write_string("  write <f> <t>- Datei schreiben\n");
    writer.write_string("  mkdir <dir> - Verzeichnis erstellen\n");
    writer.write_string("  rm <name>   - Datei/Verzeichnis löschen\n");
    writer.write_string("  fsck        - FS prüfen\n");
    writer.write_string("  format      - FS formatieren (ACHTUNG!)\n");
}

fn cmd_echo(writer: &mut VgaWriter, args: &str) {
    writer.write_string("\n");
    writer.write_string(args.trim());
    writer.write_byte(b'\n');
}

fn cmd_version(writer: &mut VgaWriter) {
    writer.write_string("\nOSM Kernel v0.1.0\n");
    writer.write_string("Rust no_std/no_main\n");
    writer.write_byte(b'\n');
}

fn cmd_reboot() {
    unsafe {
        outb(0x64, 0xFE); // Keyboard controller reset
    }
    loop { unsafe { core::arch::asm!("hlt") } }
}

fn cmd_shutdown() {
    // QEMU ACPI Shutdown - use 16-bit port writes
    unsafe {
        // QEMU ISA bridge at 0x604, 0x4004 - need outw
        core::arch::asm!("out dx, ax", in("dx") 0x604u16, in("ax") 0x2000u16, options(nostack, preserves_flags));
        core::arch::asm!("out dx, ax", in("dx") 0x4004u16, in("ax") 0x2000u16, options(nostack, preserves_flags));
        // Bochs/QEMU ACPI PM1a_CNT_BLK
        core::arch::asm!("out dx, ax", in("dx") 0xB004u16, in("ax") 0x2000u16, options(nostack, preserves_flags));
    }
    loop { unsafe { core::arch::asm!("hlt") } }
}

fn cmd_mem(writer: &mut VgaWriter) {
    writer.write_string("\nSpeicherlayout:\n");
    writer.write_string("  Kernel:    0x10000 - 0x1FFFF  (64 KB)\n");
    writer.write_string("  Stack:     0x90000  (512 KB)\n");
    writer.write_string("  VGA Text:  0xB8000  (4 KB)\n");
    writer.write_string("  EBDA:      0x9FC00  (1 KB)\n");
    writer.write_string("  BIOS:      0xF0000 - 0xFFFFF (64 KB)\n");
}

fn cmd_color(writer: &mut VgaWriter) {
    writer.write_string("\nFarben (Background << 4 | Foreground):\n");
    for bg in 0..8 {
        for fg in 0..16 {
            let color = (bg << 4) | fg;
            writer.color = color;
            writer.write_string("██");
        }
        writer.write_byte(b'\n');
    }
    writer.color = VGA_COLOR;
}

// ---------------------------------------------------------
// Dateisystem-Befehle
// ---------------------------------------------------------
fn cmd_ls(writer: &mut VgaWriter, args: &str) {
    let path = args.trim();
    let path = if path.is_empty() { "/" } else { path };
    writer.write_string("\nVerzeichnis: ");
    writer.write_string(path);
    writer.write_byte(b'\n');
    fs_list_dir(path, writer);
}

fn cmd_cat(writer: &mut VgaWriter, args: &str) {
    let path = args.trim();
    if path.is_empty() {
        writer.write_string("\nUsage: cat <datei>\n");
        return;
    }
    writer.write_string("\n--- ");
    writer.write_string(path);
    writer.write_string(" ---\n");
    let mut buffer = [0u8; 1024];
    let read = fs_read_file(path, &mut buffer);
    if read > 0 {
        writer.write_string(core::str::from_utf8(&buffer[..read]).unwrap_or(""));
    } else {
        writer.write_string("(leer oder nicht gefunden)\n");
    }
}

fn cmd_write(writer: &mut VgaWriter, args: &str) {
    // Format: write <datei> <text>
    let args = args.trim();
    if args.is_empty() {
        writer.write_string("\nUsage: write <datei> <text>\n");
        return;
    }
    let mut parts = args.splitn(2, ' ');
    let path = parts.next().unwrap_or("");
    let text = parts.next().unwrap_or("");
    if path.is_empty() {
        writer.write_string("\nUsage: write <datei> <text>\n");
        return;
    }
    let bytes = text.as_bytes();
    let written = fs_write_file(path, bytes);
    writer.write_string("\nGeschrieben: ");
    write_u32(writer, written as u32);
    writer.write_string(" Bytes\n");
}

fn cmd_mkdir(writer: &mut VgaWriter, args: &str) {
    let path = args.trim();
    if path.is_empty() {
        writer.write_string("\nUsage: mkdir <verzeichnis>\n");
        return;
    }
    if fs_create_dir(path) {
        writer.write_string("\nVerzeichnis erstellt: ");
        writer.write_string(path);
        writer.write_byte(b'\n');
    } else {
        writer.write_string("\nFehler: Konnte Verzeichnis nicht erstellen\n");
    }
}

fn cmd_rm(writer: &mut VgaWriter, args: &str) {
    let path = args.trim();
    if path.is_empty() {
        writer.write_string("\nUsage: rm <datei|verzeichnis>\n");
        return;
    }
    if fs_delete(path) {
        writer.write_string("\nGelöscht: ");
        writer.write_string(path);
        writer.write_byte(b'\n');
    } else {
        writer.write_string("\nFehler: Konnte nicht löschen\n");
    }
}

fn cmd_fsck(writer: &mut VgaWriter) {
    writer.write_string("\nDateisystem-Prüfung...\n");
    if fs_mounted() {
        writer.write_string("FS gemountet: OK\n");
    } else {
        writer.write_string("FS nicht gemountet\n");
    }
}

fn cmd_format(writer: &mut VgaWriter) {
    writer.write_string("\nFormatiere Dateisystem... ");
    if fs_format() {
        writer.write_string("OK\n");
    } else {
        writer.write_string("FEHLGESCHLAGEN\n");
    }
}

fn execute_command(writer: &mut VgaWriter, input: &str) {
    let input = input.trim();
    if input.is_empty() {
        return;
    }

    // Manual split ohne Vec
    let mut cmd_end = 0;
    let input_bytes = input.as_bytes();
    while cmd_end < input_bytes.len() && input_bytes[cmd_end] != b' ' {
        cmd_end += 1;
    }
    
    // Manual string comparison
    let cmd_bytes = &input_bytes[..cmd_end];
    let args = if cmd_end < input_bytes.len() { &input[cmd_end + 1..] } else { "" };
    
    // Compare commands manually to avoid memcmp
    let is_help = cmd_bytes == b"help";
    let is_clear = cmd_bytes == b"clear";
    let is_echo = cmd_bytes == b"echo";
    let is_version = cmd_bytes == b"version";
    let is_reboot = cmd_bytes == b"reboot";
    let is_shutdown = cmd_bytes == b"shutdown" || cmd_bytes == b"poweroff" || cmd_bytes == b"halt";
    let is_mem = cmd_bytes == b"mem" || cmd_bytes == b"memory";
    let is_color = cmd_bytes == b"color" || cmd_bytes == b"colours";
    // FS commands
    let is_ls = cmd_bytes == b"ls" || cmd_bytes == b"dir";
    let is_cat = cmd_bytes == b"cat";
    let is_write = cmd_bytes == b"write";
    let is_mkdir = cmd_bytes == b"mkdir";
    let is_rm = cmd_bytes == b"rm" || cmd_bytes == b"del";
    let is_fsck = cmd_bytes == b"fsck";
    let is_format = cmd_bytes == b"format";

    if is_help {
        cmd_help(writer);
    } else if is_clear {
        writer.clear_screen();
    } else if is_echo {
        cmd_echo(writer, args);
    } else if is_version {
        cmd_version(writer);
    } else if is_reboot {
        cmd_reboot();
    } else if is_shutdown {
        cmd_shutdown();
    } else if is_mem {
        cmd_mem(writer);
    } else if is_color {
        cmd_color(writer);
    } else if is_ls {
        cmd_ls(writer, args);
    } else if is_cat {
        cmd_cat(writer, args);
    } else if is_write {
        cmd_write(writer, args);
    } else if is_mkdir {
        cmd_mkdir(writer, args);
    } else if is_rm {
        cmd_rm(writer, args);
    } else if is_fsck {
        cmd_fsck(writer);
    } else if is_format {
        cmd_format(writer);
    } else {
        writer.write_string("\nUnbekannter Befehl: ");
        writer.write_string(core::str::from_utf8(cmd_bytes).unwrap_or(""));
        writer.write_string(" (Tippe 'help' fuer Hilfe)\n");
    }
}

// Serial Port Funktionen
unsafe fn serial_write_byte(byte: u8) {
    // Warten bis Transmitter Holding Register leer ist
    while (inb(COM1 + 5) & 0x20) == 0 {}
    outb(COM1, byte);
}

unsafe fn serial_init() {
    // Disable interrupts
    outb(COM1 + 1, 0x00);
    // Enable DLAB (set baud rate divisor)
    outb(COM1 + 3, 0x80);
    // Set divisor to 3 (lo byte) 38400 baud
    outb(COM1 + 0, 0x03);
    //                  (hi byte)
    outb(COM1 + 1, 0x00);
    // 8 bits, no parity, one stop bit
    outb(COM1 + 3, 0x03);
    // Enable FIFO, clear them, with 14-byte threshold
    outb(COM1 + 2, 0xC7);
    // IRQs enabled, RTS/DSR set
    outb(COM1 + 4, 0x0B);
}

// Port I/O via inline asm
unsafe fn inb(port: u16) -> u8 {
    let mut data: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") data, options(nostack, preserves_flags));
    data
}

unsafe fn outb(port: u16, data: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") data, options(nostack, preserves_flags));
}

// ---------------------------------------------------------
// memcmp / memcpy für no_std
// ---------------------------------------------------------
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return a as i32 - b as i32;
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        *dest.add(i) = *src.add(i);
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let dest_ptr = dest as usize;
    let src_ptr = src as usize;
    if dest_ptr > src_ptr {
        for i in (0..n).rev() {
            *dest.add(i) = *src.add(i);
        }
    } else {
        for i in 0..n {
            *dest.add(i) = *src.add(i);
        }
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    for i in 0..n {
        *s.add(i) = c as u8;
    }
    s
}

// ---------------------------------------------------------
// PS/2 Keyboard Driver
// ---------------------------------------------------------
// Scancode Set 1 (Standard) - Make codes
const SCANCODE_ESC: u8 = 0x01;
const SCANCODE_ENTER: u8 = 0x1C;
const SCANCODE_BACKSPACE: u8 = 0x0E;
const SCANCODE_TAB: u8 = 0x0F;
const SCANCODE_LSHIFT: u8 = 0x2A;
const SCANCODE_RSHIFT: u8 = 0x36;
const SCANCODE_LCTRL: u8 = 0x1D;
const SCANCODE_LALT: u8 = 0x38;
const SCANCODE_CAPSLOCK: u8 = 0x3A;
const SCANCODE_SPACE: u8 = 0x39;

// Scancode to ASCII mapping (ohne Shift)
static SCANCODE_TO_ASCII: [u8; 128] = [
    0, 27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 8, b'\t',
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n', 0, b'a', b's',
    b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`', 0, b'\\', b'z', b'x', b'c', b'v',
    b'b', b'n', b'm', b',', b'.', b'/', 0, 0, 0, b' ', 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// Scancode to ASCII mapping (mit Shift)
static SCANCODE_TO_ASCII_SHIFT: [u8; 128] = [
    0, 27, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', 8, b'\t',
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n', 0, b'A', b'S',
    b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~', 0, b'|', b'Z', b'X', b'C', b'V',
    b'B', b'N', b'M', b'<', b'>', b'?', 0, 0, 0, b' ', 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

struct Keyboard {
    shift_pressed: bool,
    caps_lock: bool,
    ctrl_pressed: bool,
    alt_pressed: bool,
}

static mut KEYBOARD: Keyboard = Keyboard {
    shift_pressed: false,
    caps_lock: false,
    ctrl_pressed: false,
    alt_pressed: false,
};

impl Keyboard {
    fn init() {
        unsafe {
            // Einfache Initialisierung - nur Keyboard aktivieren
            // Warte bis Controller bereit
            for _ in 0..10000 {
                if (inb(KBD_STATUS_PORT) & 0x02) == 0 { break; }
                core::arch::asm!("pause");
            }
            // Disable keyboard
            outb(KBD_CMD_PORT, 0xAD);
            // Flush output buffer
            for _ in 0..1000 {
                if (inb(KBD_STATUS_PORT) & 0x01) == 0 { break; }
                inb(KBD_DATA_PORT);
            }
            // Enable keyboard (simple)
            outb(KBD_CMD_PORT, 0xAE);
            // Reset keyboard
            outb(KBD_DATA_PORT, 0xFF);
            // Wait for ACK (with timeout)
            for _ in 0..100000 {
                if (inb(KBD_STATUS_PORT) & 0x01) != 0 {
                    let resp = inb(KBD_DATA_PORT);
                    if resp == 0xFA { break; }
                }
                core::arch::asm!("pause");
            }
        }
    }

    fn read_scancode() -> Option<u8> {
        unsafe {
            if (inb(KBD_STATUS_PORT) & 0x01) != 0 {
                Some(inb(KBD_DATA_PORT))
            } else {
                None
            }
        }
    }

    fn process_scancode(&mut self, scancode: u8) -> Option<char> {
        let is_break = (scancode & 0x80) != 0;
        let code = scancode & 0x7F;

        // Modifier keys
        match code {
            SCANCODE_LSHIFT | SCANCODE_RSHIFT => {
                self.shift_pressed = !is_break;
                return None;
            }
            SCANCODE_LCTRL => {
                self.ctrl_pressed = !is_break;
                return None;
            }
            SCANCODE_LALT => {
                self.alt_pressed = !is_break;
                return None;
            }
            SCANCODE_CAPSLOCK => {
                if !is_break {
                    self.caps_lock = !self.caps_lock;
                }
                return None;
            }
            _ => {}
        }

        if is_break {
            return None;
        }

        // Normale Tasten
        let ascii = if self.shift_pressed {
            SCANCODE_TO_ASCII_SHIFT[code as usize]
        } else {
            SCANCODE_TO_ASCII[code as usize]
        };

        // Caps Lock invertiert Buchstaben
        if self.caps_lock && ascii >= b'a' && ascii <= b'z' {
            return Some((ascii - b'a' + b'A') as char);
        } else if self.caps_lock && ascii >= b'A' && ascii <= b'Z' {
            return Some((ascii - b'A' + b'a') as char);
        }

        if ascii != 0 {
            Some(ascii as char)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------
// Serial Input (für -nographic mode)
// ---------------------------------------------------------
unsafe fn serial_read_byte() -> Option<u8> {
    // Prüfen ob Daten im Receiver Buffer (COM1 + 5, bit 0)
    if (inb(COM1 + 5) & 0x01) != 0 {
        Some(inb(COM1))
    } else {
        None
    }
}

// ---------------------------------------------------------
// Entry Point (_start) - wird vom Bootloader aufgerufen
// ---------------------------------------------------------
global_asm!(
    r#"
    .section .text._start
    .global _start
    _start:
        /* Stack initialisieren */
        mov esp, 0x90000
        /* Rust main aufrufen */
        call {kernel_main}
        /* Falls main zurückkehrt: Halt */
        cli
        hlt
    "#,
    kernel_main = sym kernel_main
);

// ---------------------------------------------------------
// Kernel Main Funktion
// ---------------------------------------------------------
#[no_mangle]
extern "C" fn kernel_main() -> ! {
    unsafe {
        serial_init();
        
        WRITER.clear_screen();
        WRITER.write_string("========================================\n");
        WRITER.write_string("  OSM Kernel - Rust no_std/no_main\n");
        WRITER.write_string("========================================\n\n");
        
        WRITER.write_string("Kernel erfolgreich gestartet!\n");
        WRITER.write_string("VGA Textmodus: 80x25, Farbe: Weiß auf Schwarz\n\n");
        
        // Keyboard initialisieren
        WRITER.write_string("Initialisiere Keyboard... ");
        Keyboard::init();
        WRITER.write_string("OK\n");
        
        // Dateisystem initialisieren
        WRITER.write_string("Initialisiere Dateisystem... ");
        if fs_init() {
            WRITER.write_string("OK\n");
        } else {
            WRITER.write_string("FEHLGESCHLAGEN\n");
        }
        
        WRITER.write_string("\nTippe 'help' fuer Befehle.\n\n");
        
        // Shell Loop
        loop {
            WRITER.write_prompt();
            // Flush to serial
            unsafe { serial_write_byte(0); }
            
            // Input lesen
            unsafe {
                INPUT_BUFFER.clear();
            }
            
            loop {
                // 1. PS/2 Keyboard lesen
                if let Some(scancode) = Keyboard::read_scancode() {
                    if let Some(c) = KEYBOARD.process_scancode(scancode) {
                        match c {
                            '\n' => {
                                WRITER.write_byte(b'\n');
                                break;
                            }
                            '\x08' => { // Backspace
                                unsafe {
                                    if INPUT_BUFFER.backspace() {
                                        WRITER.backspace();
                                    }
                                }
                            }
                            c if c.is_ascii_graphic() || c == ' ' => {
                                unsafe {
                                    INPUT_BUFFER.push(c);
                                }
                                WRITER.write_byte(c as u8);
                            }
                            _ => {}
                        }
                    }
                }
                
                // 2. Serial Input lesen (für -nographic mode)
                if let Some(byte) = unsafe { serial_read_byte() } {
                    match byte {
                        b'\n' | b'\r' => {
                            WRITER.write_byte(b'\n');
                            break;
                        }
                        8 | 127 => { // Backspace / DEL
                            unsafe {
                                if INPUT_BUFFER.backspace() {
                                    WRITER.backspace();
                                }
                            }
                        }
                        byte if byte.is_ascii_graphic() || byte == b' ' => {
                            unsafe {
                                INPUT_BUFFER.push(byte as char);
                            }
                            WRITER.write_byte(byte);
                        }
                        _ => {}
                    }
                }
                
                // Kurze Pause um CPU nicht 100% zu nutzen
                unsafe { core::arch::asm!("pause") }
            }
            
            // Befehl ausführen
            let cmd = unsafe { core::str::from_utf8_unchecked(&INPUT_BUFFER.buffer[..INPUT_BUFFER.len]) };
            execute_command(&mut WRITER, cmd);
            
            // Farbe zurücksetzen
            WRITER.restore_color();
        }
    }
}