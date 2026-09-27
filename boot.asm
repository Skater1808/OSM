; x86 Bootloader (NASM) - boot.asm
; Lädt den Kernel von der Festplatte und springt in den 32-bit Protected Mode

bits 16
org 0x7C00

; ---------------------------------------------------------
; BIOS Parameter Block (BPB) - für FAT12 Formatierung
; ---------------------------------------------------------
jmp short start
nop

OEMLabel           db "OSMKRN  "   ; 8 Bytes
BytesPerSector     dw 512
SectorsPerCluster  db 1
ReservedSectors    dw 1
NumberOfFATs       db 2
RootDirEntries     dw 224
TotalSectors16     dw 2880
MediaDescriptor    db 0xF0
SectorsPerFAT      dw 9
SectorsPerTrack    dw 18
NumHeads           dw 2
HiddenSectors      dd 0
TotalSectors32     dd 0
DriveNumber        db 0
Reserved           db 0
Signature          db 0x29
VolumeID           dd 0x12345678
VolumeLabel        db "OSM KERNEL "
FileSystem         db "FAT12   "

; ---------------------------------------------------------
; Start des Bootloaders
; ---------------------------------------------------------
start:
    ; Segmentregister initialisieren
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00
    sti

    ; Laufwerksnummer speichern (DL enthält Boot-Laufwerk vom BIOS)
    mov [boot_drive], dl

    ; Meldung ausgeben
    mov si, msg_loading
    call print_string_16

    ; ---------------------------------------------------------
    ; Kernel von Diskette/Festplatte laden (LBA Sektoren 1+)
    ; Kernel wird nach 0x10000 (1MB) geladen
    ; ---------------------------------------------------------
    mov ax, 0x1000
    mov es, ax
    xor bx, bx

    mov ah, 0x02        ; BIOS Read Sector(s)
    mov al, 32          ; Anzahl Sektoren zu lesen (16KB Kernel)
    mov ch, 0           ; Zylinder 0
    mov cl, 2           ; Sektor 2 (Sektor 1 = Bootloader)
    mov dh, 0           ; Kopf 0
    mov dl, [boot_drive]
    int 0x13
    jc disk_error

    ; ---------------------------------------------------------
    ; In Protected Mode wechseln
    ; ---------------------------------------------------------
    cli
    lgdt [gdt_descriptor]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    jmp CODE_SEG:init_pm

; ---------------------------------------------------------
; 16-bit Funktionen
; ---------------------------------------------------------
print_string_16:
    pusha
    mov ah, 0x0E
.loop:
    lodsb
    test al, al
    jz .done
    int 0x10
    jmp .loop
.done:
    popa
    ret

disk_error:
    mov si, msg_disk_error
    call print_string_16
    jmp $

; ---------------------------------------------------------
; Daten
; ---------------------------------------------------------
boot_drive db 0
msg_loading db "Lade Kernel...", 0x0D, 0x0A, 0
msg_disk_error db "Fehler beim Lesen der Disk!", 0x0D, 0x0A, 0

; ---------------------------------------------------------
; GDT (Global Descriptor Table)
; ---------------------------------------------------------
gdt_start:
gdt_null:
    dq 0x0000000000000000

gdt_code:
    dw 0xFFFF       ; Limit 0-15
    dw 0x0000       ; Base 0-15
    db 0x00         ; Base 16-23
    db 10011010b    ; Present, Ring0, Code, Execute/Read
    db 11001111b    ; Granularity 4KB, 32-bit, Limit 16-19
    db 0x00         ; Base 24-31

gdt_data:
    dw 0xFFFF
    dw 0x0000
    db 0x00
    db 10010010b    ; Present, Ring0, Data, Read/Write
    db 11001111b
    db 0x00

gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

CODE_SEG equ gdt_code - gdt_start
DATA_SEG equ gdt_data - gdt_start

; ---------------------------------------------------------
; 32-bit Protected Mode Code
; ---------------------------------------------------------
bits 32
init_pm:
    ; Segmentregister setzen
    mov ax, DATA_SEG
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000

    ; Debug: Zeichen an Video-RAM schreiben (zeigt dass PM läuft)
    mov byte [0xB8000], 'P'
    mov byte [0xB8002], 'M'
    mov byte [0xB8004], ' '

    ; Kernel aufrufen (ist bei 0x10000 geladen)
    call 0x10000

    ; Falls Kernel zurückkehrt: Halten
    cli
    hlt

; ---------------------------------------------------------
; Boot-Signatur
; ---------------------------------------------------------
times 510 - ($ - $$) db 0
dw 0xAA55