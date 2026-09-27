// SimpleFS - Einfaches RAM-basiertes Dateisystem für OSM Kernel
// In-Memory, keine Hardware-Abhängigkeiten

use core::fmt::Write;
use crate::VgaWriter;

// ---------------------------------------------------------
// Konstanten
// ---------------------------------------------------------
pub const BLOCK_SIZE: usize = 512;
pub const FS_MAGIC: u32 = 0x5346534D; // "SFSM"
pub const FS_VERSION: u16 = 1;
pub const MAX_FILENAME: usize = 32;
pub const MAX_INODES: usize = 64;
pub const MAX_BLOCKS: usize = 128; // 64KB RAM-Disk (passt in Datenbereich)

// Datei-Typen
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum FileType {
    Free = 0,
    File = 1,
    Directory = 2,
}

// ---------------------------------------------------------
// Superblock
// ---------------------------------------------------------
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Superblock {
    pub magic: u32,
    pub version: u16,
    pub block_size: u16,
    pub total_blocks: u32,
    pub inode_count: u16,
    pub root_inode: u16,
    pub reserved: [u8; 488],
}

impl Superblock {
    pub const fn new() -> Self {
        Superblock {
            magic: FS_MAGIC,
            version: FS_VERSION,
            block_size: BLOCK_SIZE as u16,
            total_blocks: 128, // MAX_BLOCKS
            inode_count: MAX_INODES as u16,
            root_inode: 0,
            reserved: [0; 488],
        }
    }
    
    pub fn is_valid(&self) -> bool {
        self.magic == FS_MAGIC && self.version == FS_VERSION
    }
}

// ---------------------------------------------------------
// Inode
// ---------------------------------------------------------
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Inode {
    pub file_type: u8,
    pub flags: u8,
    pub size: u32,
    pub start_block: u32,
    pub block_count: u16,
    pub created_time: u32,
    pub modified_time: u32,
    pub name: [u8; MAX_FILENAME],
    pub reserved: [u8; 10],
}

impl Inode {
    pub const fn new() -> Self {
        Inode {
            file_type: FileType::Free as u8,
            flags: 0,
            size: 0,
            start_block: 0,
            block_count: 0,
            created_time: 0,
            modified_time: 0,
            name: [0; MAX_FILENAME],
            reserved: [0; 10],
        }
    }
    
    pub fn is_free(&self) -> bool {
        self.file_type == FileType::Free as u8
    }
    
    pub fn set_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = core::cmp::min(bytes.len(), MAX_FILENAME - 1);
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name[len] = 0;
    }
}

// ---------------------------------------------------------
// Block Bitmap
// ---------------------------------------------------------
pub struct BlockBitmap {
    data: [u8; (128 + 7) / 8], // MAX_BLOCKS = 128
}

impl BlockBitmap {
    pub const fn new() -> Self {
        BlockBitmap {
            data: [0; (MAX_BLOCKS + 7) / 8],
        }
    }
    
    pub fn alloc_block(&mut self) -> Option<u32> {
        for (i, byte) in self.data.iter_mut().enumerate() {
            if *byte != 0xFF {
                for bit in 0..8 {
                    let block = (i * 8 + bit) as u32;
                    if block >= MAX_BLOCKS as u32 {
                        return None;
                    }
                    if (*byte & (1 << bit)) == 0 {
                        *byte |= 1 << bit;
                        return Some(block);
                    }
                }
            }
        }
        None
    }
    
    pub fn free_block(&mut self, block: u32) {
        if block < MAX_BLOCKS as u32 {
            let byte_idx = (block / 8) as usize;
            let bit = (block % 8) as u8;
            self.data[byte_idx] &= !(1 << bit);
        }
    }
}

// ---------------------------------------------------------
// Dateisystem-Handle
// ---------------------------------------------------------
pub struct FileHandle {
    pub inode_index: u16,
    pub offset: u32,
}

// ---------------------------------------------------------
// Globale FS-Instanz
// ---------------------------------------------------------
pub static mut FS: Option<SimpleFS> = None;

pub struct SimpleFS {
    superblock: Superblock,
    bitmap: BlockBitmap,
    inodes: [Inode; MAX_INODES],
    blocks: [[u8; BLOCK_SIZE]; 128], // MAX_BLOCKS = 128
    mounted: bool,
}

impl SimpleFS {
    pub const fn new() -> Self {
        SimpleFS {
            superblock: Superblock::new(),
            bitmap: BlockBitmap::new(),
            inodes: [Inode::new(); MAX_INODES],
            blocks: [[0u8; BLOCK_SIZE]; 128],
            mounted: false,
        }
    }
    
    // ---------------------------------------------------------
    // Mount / Format
    // ---------------------------------------------------------
    pub fn mount(&mut self) -> bool {
        // Im RAM-FS: immer formatieren beim ersten Mount
        self.format()
    }
    
    pub fn format(&mut self) -> bool {
        self.superblock = Superblock::new();
        self.bitmap = BlockBitmap::new();
        self.inodes = [Inode::new(); MAX_INODES];
        self.blocks = [[0u8; BLOCK_SIZE]; 128];
        
        // Root Directory erstellen (Inode 0)
        self.inodes[0] = Inode::new();
        self.inodes[0].file_type = FileType::Directory as u8;
        self.inodes[0].set_name("/");
        self.inodes[0].start_block = self.bitmap.alloc_block().unwrap_or(0);
        self.inodes[0].block_count = 1;
        self.inodes[0].size = BLOCK_SIZE as u32;
        self.superblock.root_inode = 0;
        
        self.mounted = true;
        true
    }
    
    // ---------------------------------------------------------
    // Hilfsfunktionen
    // ---------------------------------------------------------
    fn find_inode(&self, name: &str, parent_inode: u16) -> Option<u16> {
        if parent_inode >= MAX_INODES as u16 {
            return None;
        }
        let parent = &self.inodes[parent_inode as usize];
        if parent.file_type != FileType::Directory as u8 {
            return None;
        }
        
        let block_idx = parent.start_block as usize;
        if block_idx >= MAX_BLOCKS { return None; }
        
        // Einfaches Directory-Format: 32 Byte pro Eintrag
        unsafe {
            for i in 0..(BLOCK_SIZE / 32) {
                let entry_ptr = self.blocks[block_idx].as_ptr().add(i * 32);
                let entry_name_ptr = entry_ptr as *const u8;
                let entry_inode_ptr = entry_ptr.add(MAX_FILENAME) as *const u16;
                
                let entry_inode = *entry_inode_ptr;
                if entry_inode == 0 { continue; }
                
                let mut name_buf = [0u8; MAX_FILENAME];
                for j in 0..MAX_FILENAME {
                    let c = *entry_name_ptr.add(j);
                    name_buf[j] = c;
                    if c == 0 { break; }
                }
                let entry_name = core::str::from_utf8_unchecked(&name_buf).trim_end_matches('\0');
                if entry_name == name {
                    return Some(entry_inode);
                }
            }
        }
        None
    }
    
    pub fn list_dir(&self, path: &str, writer: &mut VgaWriter) {
        let inode_idx = self.resolve_path(path);
        if let Some(idx) = inode_idx {
            let inode = &self.inodes[idx as usize];
            if inode.file_type != FileType::Directory as u8 {
                writer.write_string("Kein Verzeichnis\n");
                return;
            }
            
            let block_idx = inode.start_block as usize;
            if block_idx >= MAX_BLOCKS { return; }
            
            unsafe {
                for i in 0..(BLOCK_SIZE / 32) {
                    let entry_ptr = self.blocks[block_idx].as_ptr().add(i * 32);
                    let entry_name_ptr = entry_ptr as *const u8;
                    let entry_inode_ptr = entry_ptr.add(MAX_FILENAME) as *const u16;
                    
                    let entry_inode = *entry_inode_ptr;
                    if entry_inode == 0 { continue; }
                    
                    let mut name_buf = [0u8; MAX_FILENAME];
                    for j in 0..MAX_FILENAME {
                        let c = *entry_name_ptr.add(j);
                        name_buf[j] = c;
                        if c == 0 { break; }
                    }
                    let entry_name = core::str::from_utf8_unchecked(&name_buf).trim_end_matches('\0');
                    
                    let entry_inode_ref = &self.inodes[entry_inode as usize];
                    let type_char = if entry_inode_ref.file_type == FileType::Directory as u8 { 'd' } else { 'f' };
                    
                    let entry_size = entry_inode_ref.size;
                    write!(writer, "{} {:<12} {} bytes\n", type_char, entry_name, entry_size).ok();
                }
            }
        } else {
            writer.write_string("Verzeichnis nicht gefunden\n");
        }
    }
    
    fn resolve_path(&self, path: &str) -> Option<u16> {
        if path == "/" {
            return Some(self.superblock.root_inode);
        }
        
        let mut current = self.superblock.root_inode;
        let mut path_parts: [&str; 8] = [""; 8];
        let mut part_count = 0;
        
        for part in path.split('/') {
            if !part.is_empty() && part_count < 8 {
                path_parts[part_count] = part;
                part_count += 1;
            }
        }
        
        for i in 0..part_count {
            let part = path_parts[i];
            if let Some(next) = self.find_inode(part, current) {
                current = next;
            } else {
                return None;
            }
        }
        Some(current)
    }
    
    pub fn create_file(&mut self, path: &str, is_dir: bool) -> bool {
        // Eltern-Verzeichnis finden
        let (parent_path, name) = split_path(path);
        let parent_idx = match self.resolve_path(parent_path) {
            Some(idx) => idx,
            None => return false,
        };
        
        // Prüfen ob schon existiert
        if self.find_inode(name, parent_idx).is_some() {
            return false;
        }
        
        // Freien Inode finden
        let free_inode = match self.inodes.iter().position(|i| i.is_free()) {
            Some(idx) => idx as u16,
            None => return false,
        };
        
        // Datenblock allokieren
        let block = match self.bitmap.alloc_block() {
            Some(b) => b,
            None => return false,
        };
        
        // Inode initialisieren
        self.inodes[free_inode as usize] = Inode::new();
        self.inodes[free_inode as usize].file_type = if is_dir { FileType::Directory as u8 } else { FileType::File as u8 };
        self.inodes[free_inode as usize].set_name(name);
        self.inodes[free_inode as usize].start_block = block;
        self.inodes[free_inode as usize].block_count = 1;
        self.inodes[free_inode as usize].size = if is_dir { BLOCK_SIZE as u32 } else { 0 };
        
        // Directory-Eintrag im Parent hinzufügen
        if !self.add_dir_entry(parent_idx, name, free_inode) {
            self.bitmap.free_block(block);
            return false;
        }
        
        // Block leeren (für Directory)
        if is_dir {
            self.blocks[block as usize] = [0u8; BLOCK_SIZE];
        }
        
        true
    }
    
    fn add_dir_entry(&mut self, parent_inode: u16, name: &str, child_inode: u16) -> bool {
        let parent = &self.inodes[parent_inode as usize];
        let block_idx = parent.start_block as usize;
        if block_idx >= MAX_BLOCKS { return false; }
        
        for i in 0..(BLOCK_SIZE / 32) {
            unsafe {
                let entry_ptr = self.blocks[block_idx].as_mut_ptr().add(i * 32);
                let entry_inode_ptr = entry_ptr.add(MAX_FILENAME) as *mut u16;
                
                if *entry_inode_ptr == 0 {
                    // Namen kopieren
                    let name_bytes = name.as_bytes();
                    let len = core::cmp::min(name_bytes.len(), MAX_FILENAME - 1);
                    for j in 0..len {
                        *entry_ptr.add(j) = name_bytes[j];
                    }
                    *entry_ptr.add(len) = 0;
                    // Inode-Index
                    *entry_inode_ptr = child_inode;
                    return true;
                }
            }
        }
        false
    }
    
    pub fn open_file(&self, path: &str) -> Option<FileHandle> {
        let inode_idx = self.resolve_path(path)?;
        let inode = self.inodes[inode_idx as usize];
        if inode.file_type != FileType::File as u8 {
            return None;
        }
        Some(FileHandle {
            inode_index: inode_idx,
            offset: 0,
        })
    }
    
    pub fn read_file(&self, handle: &mut FileHandle, buffer: &mut [u8]) -> usize {
        let inode = &self.inodes[handle.inode_index as usize];
        if handle.offset >= inode.size {
            return 0;
        }
        
        let to_read = core::cmp::min(buffer.len(), (inode.size - handle.offset) as usize);
        let block_idx = inode.start_block as usize;
        if block_idx >= MAX_BLOCKS { return 0; }
        
        let block_offset = handle.offset as usize;
        let src = &self.blocks[block_idx][block_offset..block_offset + to_read];
        buffer[..to_read].copy_from_slice(src);
        
        handle.offset += to_read as u32;
        to_read
    }
    
    pub fn write_file(&mut self, handle: &mut FileHandle, buffer: &[u8]) -> usize {
        let inode_idx = handle.inode_index as usize;
        let inode = &mut self.inodes[inode_idx];
        
        let mut written = 0;
        let mut remaining = buffer.len();
        let mut src_offset = 0;
        
        while remaining > 0 {
            let block_idx = inode.start_block as usize;
            if block_idx >= MAX_BLOCKS { break; }
            
            let block_offset = handle.offset as usize;
            let to_write = core::cmp::min(remaining, BLOCK_SIZE - block_offset);
            
            self.blocks[block_idx][block_offset..block_offset + to_write]
                .copy_from_slice(&buffer[src_offset..src_offset + to_write]);
            
            written += to_write;
            remaining -= to_write;
            src_offset += to_write;
            handle.offset += to_write as u32;
        }
        
        // Inode-Größe aktualisieren
        if handle.offset > inode.size {
            inode.size = handle.offset;
        }
        
        written
    }
    
    pub fn delete_file(&mut self, path: &str) -> bool {
        let (parent_path, name) = split_path(path);
        let parent_idx = match self.resolve_path(parent_path) {
            Some(idx) => idx,
            None => return false,
        };
        
        // Inode finden
        let inode_idx = match self.find_inode(name, parent_idx) {
            Some(idx) => idx,
            None => return false,
        };
        let inode = &mut self.inodes[inode_idx as usize];
        
        // Blöcke freigeben
        for i in 0..inode.block_count {
            self.bitmap.free_block(inode.start_block + i as u32);
        }
        
        // Inode freigeben
        inode.file_type = FileType::Free as u8;
        inode.size = 0;
        inode.start_block = 0;
        inode.block_count = 0;
        
        // Directory-Eintrag entfernen
        if !self.remove_dir_entry(parent_idx, name) {
            return false;
        }
        
        true
    }
    
    fn remove_dir_entry(&mut self, parent_inode: u16, name: &str) -> bool {
        let parent = &self.inodes[parent_inode as usize];
        let block_idx = parent.start_block as usize;
        if block_idx >= MAX_BLOCKS { return false; }
        
        unsafe {
            for i in 0..(BLOCK_SIZE / 32) {
                let entry_ptr = self.blocks[block_idx].as_mut_ptr().add(i * 32);
                let entry_name_ptr = entry_ptr as *const u8;
                let entry_inode_ptr = entry_ptr.add(MAX_FILENAME) as *mut u16;
                
                let entry_inode = *entry_inode_ptr;
                if entry_inode == 0 { continue; }
                
                let mut name_buf = [0u8; MAX_FILENAME];
                for j in 0..MAX_FILENAME {
                    let c = *entry_name_ptr.add(j);
                    name_buf[j] = c;
                    if c == 0 { break; }
                }
                let entry_name = core::str::from_utf8_unchecked(&name_buf).trim_end_matches('\0');
                
                if entry_name == name {
                    *entry_inode_ptr = 0;
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------
// Hilfsfunktionen
// ---------------------------------------------------------
fn split_path(path: &str) -> (&str, &str) {
    let path = path.trim_end_matches('/');
    if let Some(pos) = path.rfind('/') {
        if pos == 0 {
            ("/", &path[1..])
        } else {
            (&path[..pos], &path[pos+1..])
        }
    } else {
        ("/", path)
    }
}

// ---------------------------------------------------------
// Public API
// ---------------------------------------------------------
pub fn fs_init() -> bool {
    unsafe {
        FS = Some(SimpleFS::new());
        if let Some(fs) = FS.as_mut() {
            fs.mount()
        } else { false }
    }
}

pub fn fs_mounted() -> bool {
    unsafe { FS.as_ref().map(|fs| fs.mounted).unwrap_or(false) }
}

pub fn fs_list_dir(path: &str, writer: &mut VgaWriter) {
    unsafe {
        if let Some(fs) = FS.as_ref() {
            fs.list_dir(path, writer);
        }
    }
}

pub fn fs_create_dir(path: &str) -> bool {
    unsafe {
        if let Some(fs) = FS.as_mut() {
            fs.create_file(path, true)
        } else { false }
    }
}

pub fn fs_read_file(path: &str, buffer: &mut [u8]) -> usize {
    unsafe {
        if let Some(fs) = FS.as_ref() {
            if let Some(mut handle) = fs.open_file(path) {
                fs.read_file(&mut handle, buffer)
            } else { 0 }
        } else { 0 }
    }
}

pub fn fs_write_file(path: &str, buffer: &[u8]) -> usize {
    unsafe {
        if let Some(fs) = FS.as_mut() {
            // Datei erstellen falls nicht existiert
            if fs.open_file(path).is_none() {
                let _ = fs.create_file(path, false);
            }
            if let Some(mut handle) = fs.open_file(path) {
                fs.write_file(&mut handle, buffer)
            } else { 0 }
        } else { 0 }
    }
}

pub fn fs_delete(path: &str) -> bool {
    unsafe {
        if let Some(fs) = FS.as_mut() {
            fs.delete_file(path)
        } else { false }
    }
}

pub fn fs_format() -> bool {
    unsafe {
        FS = Some(SimpleFS::new());
        if let Some(fs) = FS.as_mut() {
            fs.format()
        } else { false }
    }
}