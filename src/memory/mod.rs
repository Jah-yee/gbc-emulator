// src/memory/mod.rs

use crate::apu::Apu;

#[derive(Clone)]
pub struct Memory {
    // The entire cartridge ROM (all banks, however big).
    rom: Vec<u8>,

    // Which 16KB ROM bank is currently mapped at 0x4000-0x7FFF. Boots at 1
    // (bank 0 is always fixed at 0x0000-0x3FFF). The MBC changes this.
    rom_bank: usize,

    // Video RAM (CGB has 2 banks; DMG uses only bank 0)
    vram: [u8; 0x4000],

    // External RAM (Cartridge RAM)
    external_ram: [u8; 0x8000], // up to 4 x 8KB cartridge RAM banks

    // Work RAM (CGB has 8 x 4KB banks; DMG uses the first two contiguously)
    wram: [u8; 0x8000],

    // Sprite Attribute Table
    oam: [u8; 0xA0],

    // I/O Registers
    io_registers: [u8; 0x80],

    // High RAM
    hram: [u8; 0x7F],

    // Interrupt Enable Register
    ie_register: u8,

    // Captured serial output (test ROMs print Pass/Fail here via the link port).
    pub serial: String,

    // Gameboy Doctor trace mode (GBC_TRACE env var): stub LY reads to 0x90 and
    // suppress live serial printing so the trace log stays clean.
    pub trace: bool,

    // Joypad (0xFF00). `select` holds the group bits 4-5 the game wrote (0=selected).
    // `dpad`/`buttons` are low-nibble press masks (1=pressed) set by the frontend.
    joypad_select: u8,
    dpad: u8,
    buttons: u8,

    // Cartridge / MBC state.
    cart_type: u8,     // header byte 0x0147 (0x00 = ROM only, 0x01-0x03 = MBC1, 0x0F-0x13 = MBC3)
    ram_enabled: bool, // cartridge RAM gate (set via 0x0000-0x1FFF writes)
    ram_bank: usize,   // which 8KB cartridge-RAM bank is mapped at 0xA000
    // MBC1 register state (bank1 = low 5 ROM bits, bank2 = 2 high ROM / RAM bits).
    mbc1_bank1: u8,
    mbc1_bank2: u8,
    mbc1_mode: bool, // false = ROM banking (bank2 = high ROM bits), true = RAM banking

    // CGB (Game Boy Color) state.
    cgb_mode: bool,        // cartridge supports CGB (header 0x0143 bit 7)
    vram_bank: usize,      // 0xFF4F VBK: which 8KB VRAM bank is mapped at 0x8000
    bg_palette: [u8; 64],  // CGB BG palette RAM: 8 palettes x 4 colors x 2 bytes (RGB555)
    obj_palette: [u8; 64], // CGB sprite palette RAM
    bcps: u8,              // 0xFF68: BG palette index (bits 0-5) + auto-increment (bit 7)
    ocps: u8,              // 0xFF6A: OBJ palette index + auto-increment
    svbk: usize,           // 0xFF70: which 4KB WRAM bank is mapped at 0xD000 (1-7)
    hdma_src: u16,         // 0xFF51/52: VRAM DMA source
    hdma_dst: u16,         // 0xFF53/54: VRAM DMA destination (within VRAM)
    pub double_speed: bool, // CGB double-speed mode (KEY1 bit 7)
    key1_prepare: bool,    // KEY1 bit 0: a speed switch is armed for the next STOP

    // Audio.
    pub apu: Apu,
}

impl Memory {
    pub fn new() -> Self {
        let mut memory = Self {
            rom: Vec::new(),
            rom_bank: 1,
            vram: [0; 0x4000],
            external_ram: [0; 0x8000],
            wram: [0; 0x8000],
            oam: [0; 0xA0],
            io_registers: [0; 0x80],
            hram: [0; 0x7F],
            ie_register: 0,
            serial: String::new(),
            trace: std::env::var("GBC_TRACE").is_ok(),
            joypad_select: 0x30, // nothing selected
            dpad: 0,
            buttons: 0,
            cart_type: 0,
            ram_enabled: false,
            ram_bank: 0,
            mbc1_bank1: 1,
            mbc1_bank2: 0,
            mbc1_mode: false,
            cgb_mode: false,
            vram_bank: 0,
            bg_palette: [0; 64],
            obj_palette: [0; 64],
            bcps: 0,
            ocps: 0,
            svbk: 1,
            hdma_src: 0,
            hdma_dst: 0,
            double_speed: false,
            key1_prepare: false,
            apu: Apu::new(),
        };
        // Post-boot register defaults (values the boot ROM leaves behind). Games
        // like Tetris rely on these instead of setting them, so without them the
        // palette is 0 and everything renders as the lightest shade (blank).
        memory.io_registers[0x40] = 0x91; // LCDC: LCD on, BG on, 0x8000 tile data
        memory.io_registers[0x47] = 0xFC; // BGP: background palette
        memory.io_registers[0x48] = 0xFF; // OBP0: sprite palette 0
        memory.io_registers[0x49] = 0xFF; // OBP1: sprite palette 1
        memory
    }

    // Map a WRAM or echo address to a flat index, honoring the CGB WRAM bank.
    // 0xC000-0xCFFF = bank 0 (fixed); 0xD000-0xDFFF = bank svbk (1-7). Echo
    // (0xE000-0xFDFF) mirrors 0xC000-0xDDFF. In DMG mode svbk stays 1, so the
    // first 8KB is contiguous exactly as before.
    fn wram_index(&self, address: u16) -> usize {
        let a = if address >= 0xE000 { address - 0x2000 } else { address };
        if a < 0xD000 {
            (a - 0xC000) as usize
        } else {
            self.svbk * 0x1000 + (a - 0xD000) as usize
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        // Gameboy Doctor: its reference logs were generated with LY hardcoded to
        // 0x90, so stub it here while tracing to avoid spurious PPU-timing diffs.
        if self.trace && address == 0xFF44 {
            return 0x90;
        }
        match address {
            // ROM Bank 0 (fixed: the first 16KB of the cartridge)
            0x0000..=0x3FFF => self.rom.get(address as usize).copied().unwrap_or(0xFF),

            // ROM Bank N (switchable): index into the selected 16KB bank.
            0x4000..=0x7FFF => {
                let offset = self.rom_bank * 0x4000 + (address as usize - 0x4000);
                self.rom.get(offset).copied().unwrap_or(0xFF)
            }

            // VRAM
            0x8000..=0x9FFF => self.vram[self.vram_bank * 0x2000 + (address - 0x8000) as usize],

            // External (cartridge) RAM - only accessible while enabled.
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    0xFF
                } else if matches!(self.cart_type, 0x05 | 0x06) {
                    // MBC2: built-in 512 x 4-bit RAM (echoed); upper nibble reads 1.
                    0xF0 | (self.external_ram[address as usize & 0x1FF] & 0x0F)
                } else {
                    let off = self.ram_bank * 0x2000 + (address - 0xA000) as usize;
                    self.external_ram.get(off).copied().unwrap_or(0xFF)
                }
            }

            // Work RAM (and its echo) with CGB bank switching.
            0xC000..=0xDFFF => self.wram[self.wram_index(address)],
            0xE000..=0xFDFF => self.wram[self.wram_index(address)],

            // OAM
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize],

            // Not usable
            0xFEA0..=0xFEFF => 0xFF,

            // Joypad: bits 7-6 read 1, bits 5-4 = selected group, bits 3-0 = button
            // states for the selected group(s), active-low (0 = pressed).
            0xFF00 => {
                let mut low = 0x0F;
                if self.joypad_select & 0x10 == 0 {
                    low &= !self.dpad & 0x0F; // directions selected
                }
                if self.joypad_select & 0x20 == 0 {
                    low &= !self.buttons & 0x0F; // action buttons selected
                }
                0xC0 | self.joypad_select | low
            }

            // Audio registers delegate to the APU.
            0xFF10..=0xFF3F => self.apu.read_reg(address),

            // CGB KEY1: bit 7 = current speed, bit 0 = armed switch.
            0xFF4D => {
                let mut v = 0x7E; // unused bits read 1
                if self.double_speed {
                    v |= 0x80;
                }
                if self.key1_prepare {
                    v |= 0x01;
                }
                v
            }

            // CGB: VRAM bank register (unused bits read as 1).
            0xFF4F => 0xFE | self.vram_bank as u8,

            // CGB: WRAM bank register (unused bits read as 1).
            0xFF70 => 0xF8 | self.svbk as u8,

            // CGB VRAM DMA status: we complete transfers instantly, so always done.
            0xFF55 => 0xFF,

            // CGB palette registers: index registers read back directly, data
            // registers read the palette-RAM byte at the current index.
            0xFF68 => self.bcps,
            0xFF69 => self.bg_palette[(self.bcps & 0x3F) as usize],
            0xFF6A => self.ocps,
            0xFF6B => self.obj_palette[(self.ocps & 0x3F) as usize],

            // I/O Registers
            0xFF00..=0xFF7F => self.io_registers[(address - 0xFF00) as usize],

            // High RAM
            0xFF80..=0xFFFE => self.hram[(address - 0xFF80) as usize],

            // Interrupt Enable
            0xFFFF => self.ie_register,
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            // Writes to ROM space aren't stored - they're MBC control commands.
            0x0000..=0x7FFF => match self.cart_type {
                // ROM only: no MBC, ignore.
                0x00 => {}

                // MBC1.
                0x01..=0x03 => match address {
                    // RAM enable: low nibble == 0xA enables cartridge RAM.
                    0x0000..=0x1FFF => self.ram_enabled = value & 0x0F == 0x0A,
                    // ROM bank low 5 bits; a request for 0 maps to 1.
                    0x2000..=0x3FFF => {
                        let n = value & 0x1F;
                        self.mbc1_bank1 = if n == 0 { 1 } else { n };
                        self.mbc1_update();
                    }
                    // 2-bit register: high ROM bits (ROM mode) or RAM bank (RAM mode).
                    0x4000..=0x5FFF => {
                        self.mbc1_bank2 = value & 0x03;
                        self.mbc1_update();
                    }
                    // Banking mode select.
                    _ => {
                        self.mbc1_mode = value & 0x01 != 0;
                        self.mbc1_update();
                    }
                },

                // MBC2: registers are all in 0x0000-0x3FFF; address bit 8 selects
                // RAM-enable (bit clear) vs ROM bank (bit set, low 4 bits, 0->1).
                0x05..=0x06 => {
                    if address <= 0x3FFF {
                        if address & 0x0100 == 0 {
                            self.ram_enabled = value & 0x0F == 0x0A;
                        } else {
                            let n = (value & 0x0F) as usize;
                            self.rom_bank = if n == 0 { 1 } else { n };
                        }
                    }
                }

                // MBC3.
                0x0F..=0x13 => match address {
                    // RAM (and RTC) enable.
                    0x0000..=0x1FFF => self.ram_enabled = value & 0x0F == 0x0A,
                    // ROM bank: full 7 bits; bank 0 maps to 1.
                    0x2000..=0x3FFF => {
                        let n = (value & 0x7F) as usize;
                        self.rom_bank = if n == 0 { 1 } else { n };
                    }
                    // 0x4000-0x5FFF: 0x00-0x03 selects a RAM bank; 0x08-0x0C selects
                    // an RTC register (clock not implemented - we just ignore those).
                    0x4000..=0x5FFF => {
                        if value <= 0x03 {
                            self.ram_bank = value as usize;
                        }
                    }
                    // 0x6000-0x7FFF: latch clock data (RTC stub).
                    _ => {}
                },

                // MBC5.
                0x19..=0x1E => match address {
                    0x0000..=0x1FFF => self.ram_enabled = value & 0x0F == 0x0A,
                    // ROM bank low 8 bits. No bank-0 quirk - bank 0 is valid here.
                    0x2000..=0x2FFF => self.rom_bank = (self.rom_bank & 0x100) | value as usize,
                    // ROM bank bit 8 (the 9th bit, for ROMs > 1MB).
                    0x3000..=0x3FFF => {
                        self.rom_bank = (self.rom_bank & 0xFF) | ((value as usize & 1) << 8)
                    }
                    // RAM bank (low 4 bits).
                    0x4000..=0x5FFF => self.ram_bank = (value & 0x0F) as usize,
                    _ => {}
                },

                _ => {} // other MBCs not supported yet
            },

            // VRAM
            0x8000..=0x9FFF => {
                self.vram[self.vram_bank * 0x2000 + (address - 0x8000) as usize] = value
            }

            // External RAM
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    if matches!(self.cart_type, 0x05 | 0x06) {
                        // MBC2: only the low nibble is stored.
                        self.external_ram[address as usize & 0x1FF] = value & 0x0F;
                    } else {
                        let off = self.ram_bank * 0x2000 + (address - 0xA000) as usize;
                        if let Some(slot) = self.external_ram.get_mut(off) {
                            *slot = value;
                        }
                    }
                }
            }

            // Work RAM (and its echo) with CGB bank switching.
            0xC000..=0xDFFF => self.wram[self.wram_index(address)] = value,
            0xE000..=0xFDFF => self.wram[self.wram_index(address)] = value,

            // OAM
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize] = value,

            // Not usable
            0xFEA0..=0xFEFF => {}

            // Joypad: the game only writes the group-select bits 4-5.
            0xFF00 => self.joypad_select = value & 0x30,

            // OAM DMA: writing the source high-byte here copies 160 bytes
            // (source = value << 8) into OAM at 0xFE00. Games do this each frame.
            // Done instantly here (real hardware takes ~160 cycles).
            0xFF46 => {
                let src = (value as u16) << 8;
                for i in 0..0xA0u16 {
                    let byte = self.read_byte(src + i);
                    self.write_byte(0xFE00 + i, byte);
                }
                self.io_registers[0x46] = value; // keep the register readable
            }

            // Audio registers delegate to the APU.
            0xFF10..=0xFF3F => self.apu.write_reg(address, value),

            // CGB KEY1: the game arms a speed switch via bit 0 (STOP performs it).
            0xFF4D => self.key1_prepare = value & 0x01 != 0,

            // CGB: select the VRAM bank (bit 0).
            0xFF4F => self.vram_bank = value as usize & 1,

            // CGB: select the WRAM bank at 0xD000 (bits 0-2; 0 acts as 1).
            0xFF70 => {
                let b = (value & 0x07) as usize;
                self.svbk = if b == 0 { 1 } else { b };
            }

            // CGB VRAM DMA source/destination latches.
            0xFF51 => self.hdma_src = (self.hdma_src & 0x00FF) | ((value as u16) << 8),
            0xFF52 => self.hdma_src = (self.hdma_src & 0xFF00) | (value & 0xF0) as u16,
            0xFF53 => self.hdma_dst = (self.hdma_dst & 0x00FF) | (((value & 0x1F) as u16) << 8),
            0xFF54 => self.hdma_dst = (self.hdma_dst & 0xFF00) | (value & 0xF0) as u16,
            // HDMA5 triggers the transfer. Bit 7 selects HBlank vs general mode;
            // we do both instantly (HBlank timing not modeled - a simplification,
            // but the data still lands, which is what most games need).
            0xFF55 => {
                let src = self.hdma_src & 0xFFF0;
                let dst = 0x8000 | (self.hdma_dst & 0x1FF0);
                let length = ((value & 0x7F) as usize + 1) * 0x10;
                for i in 0..length as u16 {
                    let b = self.read_byte(src + i);
                    self.write_byte(dst + i, b);
                }
            }

            // CGB BG palette: 0xFF68 sets the index (+auto-increment bit), 0xFF69
            // writes the color byte at that index (and bumps the index if enabled).
            0xFF68 => self.bcps = value,
            0xFF69 => {
                self.bg_palette[(self.bcps & 0x3F) as usize] = value;
                if self.bcps & 0x80 != 0 {
                    self.bcps = (self.bcps & 0x80) | (self.bcps.wrapping_add(1) & 0x3F);
                }
            }
            // CGB OBJ (sprite) palette: same scheme via 0xFF6A/0xFF6B.
            0xFF6A => self.ocps = value,
            0xFF6B => {
                self.obj_palette[(self.ocps & 0x3F) as usize] = value;
                if self.ocps & 0x80 != 0 {
                    self.ocps = (self.ocps & 0x80) | (self.ocps.wrapping_add(1) & 0x3F);
                }
            }

            // I/O Registers
            0xFF00..=0xFF7F => {
                self.io_registers[(address - 0xFF00) as usize] = value;

                // Serial: writing SC (0xFF02) with bit 7 set "sends" the byte sitting
                // in SB (0xFF01). Capture it so test ROMs (e.g. Blargg's) can report
                // Pass/Fail through the link port. Print live and keep a buffer.
                if address == 0xFF02 && value & 0x80 != 0 {
                    let byte = self.io_registers[0x01]; // SB = 0xFF01
                    self.serial.push(byte as char);
                    if !self.trace {
                        print!("{}", byte as char);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    }
                }
            }

            // High RAM
            0xFF80..=0xFFFE => self.hram[(address - 0xFF80) as usize] = value,

            // Interrupt Enable
            0xFFFF => self.ie_register = value,
        }
    }

    // Helper for reading 16-bit values (little-endian)
    pub fn read_word(&self, address: u16) -> u16 {
        let low = self.read_byte(address) as u16;
        let high = self.read_byte(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    // Helper for writing 16-bit values
    pub fn write_word(&mut self, address: u16, value: u16) {
        self.write_byte(address, (value & 0xFF) as u8);
        self.write_byte(address.wrapping_add(1), (value >> 8) as u8);
    }

    // Load the whole cartridge ROM (all banks) into memory.
    pub fn load_rom(&mut self, rom: &[u8]) {
        self.rom = rom.to_vec();
        // Cartridge type lives in the header at 0x0147.
        self.cart_type = self.rom.get(0x0147).copied().unwrap_or(0);
        // CGB support flag: header 0x0143 bit 7 (0x80 = CGB-enhanced, 0xC0 = CGB-only).
        self.cgb_mode = self.rom.get(0x0143).copied().unwrap_or(0) & 0x80 != 0;
        self.rom_bank = 1;
        self.ram_bank = 0;
        self.ram_enabled = false;
    }

    /// Recompute the effective MBC1 ROM/RAM banks from its register state.
    /// The switchable ROM bank is always (bank2 << 5) | bank1; the 2-bit bank2
    /// doubles as the RAM bank only in RAM-banking mode.
    fn mbc1_update(&mut self) {
        self.rom_bank = ((self.mbc1_bank2 << 5) | self.mbc1_bank1) as usize;
        self.ram_bank = if self.mbc1_mode {
            self.mbc1_bank2 as usize
        } else {
            0
        };
    }

    /// True if the cartridge declares Game Boy Color support (header 0x0143).
    pub fn is_cgb(&self) -> bool {
        self.cgb_mode
    }

    /// Perform an armed CGB speed switch (called by STOP). Returns whether one
    /// happened - if not, STOP was a real "stop the CPU".
    pub fn try_speed_switch(&mut self) -> bool {
        if self.key1_prepare {
            self.double_speed = !self.double_speed;
            self.key1_prepare = false;
            true
        } else {
            false
        }
    }

    /// Read a byte from a specific VRAM bank (the PPU needs bank 0 for tiles/maps
    /// and bank 1 for CGB tile attributes, regardless of the current VBK setting).
    pub fn vram_read(&self, bank: usize, addr: u16) -> u8 {
        self.vram[bank * 0x2000 + (addr - 0x8000) as usize]
    }

    /// A CGB background color: palette 0-7, color id 0-3 -> RGB888.
    pub fn cgb_bg_color(&self, palette: usize, color: usize) -> (u8, u8, u8) {
        let i = palette * 8 + color * 2; // 8 bytes per palette, 2 per color
        let rgb555 = self.bg_palette[i] as u16 | ((self.bg_palette[i + 1] as u16) << 8);
        rgb555_to_rgb888(rgb555)
    }

    /// A CGB sprite color: palette 0-7, color id 0-3 -> RGB888.
    pub fn cgb_obj_color(&self, palette: usize, color: usize) -> (u8, u8, u8) {
        let i = palette * 8 + color * 2;
        let rgb555 = self.obj_palette[i] as u16 | ((self.obj_palette[i + 1] as u16) << 8);
        rgb555_to_rgb888(rgb555)
    }

    /// True if the cartridge has battery-backed RAM (its save persists).
    pub fn has_battery(&self) -> bool {
        matches!(
            self.cart_type,
            0x03 | 0x06 | 0x09 | 0x0D | 0x0F | 0x10 | 0x13 | 0x1B | 0x1E
        )
    }

    /// Current contents of cartridge RAM (for writing a .sav file).
    pub fn ram_snapshot(&self) -> &[u8] {
        &self.external_ram
    }

    /// Load saved cartridge RAM (from a .sav file) back into memory.
    pub fn load_ram(&mut self, data: &[u8]) {
        let n = data.len().min(self.external_ram.len());
        self.external_ram[..n].copy_from_slice(&data[..n]);
    }

    /// Set joypad press state from the frontend. Each is a low-nibble mask
    /// (1 = pressed): dpad = Right/Left/Up/Down (bits 0-3),
    /// buttons = A/B/Select/Start (bits 0-3).
    pub fn set_joypad(&mut self, dpad: u8, buttons: u8) {
        self.dpad = dpad & 0x0F;
        self.buttons = buttons & 0x0F;
    }
}

/// Convert a 15-bit CGB color (RGB555, 5 bits each) to 8-bit-per-channel RGB.
/// Scale 5->8 bits by `(v << 3) | (v >> 2)` so 0x1F maps to 0xFF.
fn rgb555_to_rgb888(c: u16) -> (u8, u8, u8) {
    let r = (c & 0x1F) as u8;
    let g = ((c >> 5) & 0x1F) as u8;
    let b = ((c >> 10) & 0x1F) as u8;
    ((r << 3) | (r >> 2), (g << 3) | (g >> 2), (b << 3) | (b >> 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_read_write() {
        let mut memory = Memory::new();

        // Test WRAM (Work RAM)
        memory.write_byte(0xC000, 0x42);
        assert_eq!(memory.read_byte(0xC000), 0x42);

        memory.write_byte(0xDFFF, 0xFF);
        assert_eq!(memory.read_byte(0xDFFF), 0xFF);

        // Test HRAM (High RAM)
        memory.write_byte(0xFF80, 0x12);
        assert_eq!(memory.read_byte(0xFF80), 0x12);

        memory.write_byte(0xFFFE, 0x34);
        assert_eq!(memory.read_byte(0xFFFE), 0x34);

        // Test Interrupt Enable Register
        memory.write_byte(0xFFFF, 0b0001_1111);
        assert_eq!(memory.read_byte(0xFFFF), 0b0001_1111);
    }

    #[test]
    fn test_echo_ram_mirrors_wram() {
        let mut memory = Memory::new();

        // Write to WRAM
        memory.write_byte(0xC000, 0xAB);

        // Read from Echo RAM (should mirror)
        assert_eq!(memory.read_byte(0xE000), 0xAB);

        // Write to Echo RAM
        memory.write_byte(0xE100, 0xCD);

        // Read from WRAM (should be mirrored)
        assert_eq!(memory.read_byte(0xC100), 0xCD)
    }

    #[test]
    fn test_unusable_memory_returns_ff() {
        let memory = Memory::new();

        // The range 0xFEA0-0xFEFF is not usable and should return 0xFF
        for addr in 0xFEA0..=0xFEFF {
            assert_eq!(memory.read_byte(addr), 0xFF);
        }
    }

    #[test]
    fn test_read_write_word() {
        let mut memory = Memory::new();

        // Write a 16-bit word
        memory.write_word(0xC000, 0x1234);

        // Check individual bytes (little-endian)
        assert_eq!(memory.read_byte(0xC000), 0x34); // Low byte
        assert_eq!(memory.read_byte(0xC001), 0x12); // High byte

        // Read back as word
        assert_eq!(memory.read_word(0xC000), 0x1234);
    }

    #[test]
    fn test_rom_loading() {
        let mut memory = Memory::new();

        let rom = vec![
            0x00, 0x01, 0x02,
            0x03, // First few bytes
                 // ...imagine it continues
        ];

        memory.load_rom(&rom);

        // Verify the ROM was loaded
        assert_eq!(memory.read_byte(0x0000), 0x00);
        assert_eq!(memory.read_byte(0x0001), 0x01);
        assert_eq!(memory.read_byte(0x0002), 0x02);
        assert_eq!(memory.read_byte(0x0003), 0x03);
    }

    #[test]
    fn test_rom_is_read_only() {
        let mut memory = Memory::new();

        let rom = vec![0xAA; 0x8000]; // Fill ROM with 0xAA
        memory.load_rom(&rom);

        // Try to write to ROM
        memory.write_byte(0x0000, 0x55);

        // ROM should still contain original value
        assert_eq!(memory.read_byte(0x0000), 0xAA);
    }

    #[test]
    fn test_joypad_nothing_pressed_reads_high() {
        let mut memory = Memory::new();
        memory.write_byte(0xFF00, 0x20); // select directions (bit4=0)
        // Nothing pressed -> low nibble all 1s.
        assert_eq!(memory.read_byte(0xFF00) & 0x0F, 0x0F);
    }

    #[test]
    fn test_joypad_press_only_shows_in_selected_group() {
        let mut memory = Memory::new();
        memory.set_joypad(0b0001, 0b0000); // Right pressed (dpad bit 0)

        // Select directions: Right reads as 0 (pressed, active-low).
        memory.write_byte(0xFF00, 0x20); // bit4=0 dir selected, bit5=1 actions not
        assert_eq!(memory.read_byte(0xFF00) & 0x01, 0x00);

        // Select action buttons instead: the direction press must NOT appear.
        memory.write_byte(0xFF00, 0x10); // bit5=0 actions selected, bit4=1 dir not
        assert_eq!(memory.read_byte(0xFF00) & 0x0F, 0x0F);
    }

    #[test]
    fn test_mbc1_rom_bank_switch() {
        let mut memory = Memory::new();
        // 3-bank ROM (48KB); mark each bank's first byte so we can tell them apart.
        let mut rom = vec![0u8; 0x4000 * 3];
        rom[0x0147] = 0x01; // cartridge type = MBC1
        rom[0x4000] = 0xAA; // bank 1, first byte
        rom[0x8000] = 0xBB; // bank 2, first byte
        memory.load_rom(&rom);

        // Boots with bank 1 mapped at 0x4000.
        assert_eq!(memory.read_byte(0x4000), 0xAA);

        // Switch to bank 2.
        memory.write_byte(0x2000, 0x02);
        assert_eq!(memory.read_byte(0x4000), 0xBB);

        // Requesting bank 0 maps to bank 1 (the MBC1 quirk).
        memory.write_byte(0x2000, 0x00);
        assert_eq!(memory.read_byte(0x4000), 0xAA);
    }

    #[test]
    fn test_mbc3_rom_and_ram_banks() {
        let mut memory = Memory::new();
        // ROM big enough for bank 0x40 (65 banks); mark bank 0x40's first byte.
        let mut rom = vec![0u8; 0x4000 * 0x41];
        rom[0x0147] = 0x13; // MBC3 + RAM + battery
        rom[0x4000 * 0x40] = 0xCD; // bank 0x40, first byte
        memory.load_rom(&rom);

        // 7-bit ROM bank: 0x40 is reachable (MBC1's 5 bits couldn't).
        memory.write_byte(0x2000, 0x40);
        assert_eq!(memory.read_byte(0x4000), 0xCD);

        // RAM bank switching: write distinct bytes to banks 0 and 1.
        memory.write_byte(0x0000, 0x0A); // enable RAM
        memory.write_byte(0x4000, 0x00); // RAM bank 0
        memory.write_byte(0xA000, 0x11);
        memory.write_byte(0x4000, 0x01); // RAM bank 1
        memory.write_byte(0xA000, 0x22);

        memory.write_byte(0x4000, 0x00);
        assert_eq!(memory.read_byte(0xA000), 0x11);
        memory.write_byte(0x4000, 0x01);
        assert_eq!(memory.read_byte(0xA000), 0x22);
    }

    #[test]
    fn test_mbc1_high_rom_banks() {
        let mut memory = Memory::new();
        let mut rom = vec![0u8; 0x4000 * 0x22]; // 34 banks so bank 0x21 exists
        rom[0x0147] = 0x01; // MBC1
        rom[0x4000 * 0x21] = 0x99; // bank 0x21
        memory.load_rom(&rom);

        // bank1 = 1 (low 5 bits), bank2 = 1 (high 2 bits) -> bank (1<<5)|1 = 0x21.
        memory.write_byte(0x2000, 0x01);
        memory.write_byte(0x4000, 0x01);
        assert_eq!(memory.read_byte(0x4000), 0x99);
    }

    #[test]
    fn test_mbc2_bank_and_ram() {
        let mut memory = Memory::new();
        let mut rom = vec![0u8; 0x4000 * 3];
        rom[0x0147] = 0x06; // MBC2 + battery
        rom[0x4000] = 0xAA; // bank 1
        rom[0x8000] = 0xBB; // bank 2
        memory.load_rom(&rom);

        // ROM bank: address bit 8 SET selects the bank (low 4 bits).
        memory.write_byte(0x2100, 0x02);
        assert_eq!(memory.read_byte(0x4000), 0xBB);

        // RAM enable: address bit 8 CLEAR.
        memory.write_byte(0x0000, 0x0A);
        // 4-bit RAM: only the low nibble stores; high nibble reads as 1.
        memory.write_byte(0xA000, 0xAB);
        assert_eq!(memory.read_byte(0xA000), 0xFB); // 0xF0 | 0x0B
        assert_eq!(memory.read_byte(0xA200), 0xFB); // echoes every 0x200
    }

    #[test]
    fn test_mbc5_rom_bank_no_zero_quirk() {
        let mut memory = Memory::new();
        let mut rom = vec![0u8; 0x4000 * 3];
        rom[0x0147] = 0x1B; // MBC5 + RAM + battery (Pokemon Yellow's type)
        rom[0x0000] = 0x10; // bank 0, first byte
        rom[0x4000] = 0xAA; // bank 1
        rom[0x8000] = 0xBB; // bank 2
        memory.load_rom(&rom);

        memory.write_byte(0x2000, 0x02); // select bank 2 (low byte)
        assert_eq!(memory.read_byte(0x4000), 0xBB);

        // MBC5 has NO bank-0->1 quirk: bank 0 maps to bank 0.
        memory.write_byte(0x2000, 0x00);
        assert_eq!(memory.read_byte(0x4000), 0x10);
    }

    #[test]
    fn test_cartridge_ram_enable_gate() {
        let mut memory = Memory::new();
        let mut rom = vec![0u8; 0x8000];
        rom[0x0147] = 0x03; // MBC1 + RAM + battery
        memory.load_rom(&rom);

        // Disabled by default: writes dropped, reads return 0xFF.
        memory.write_byte(0xA000, 0x42);
        assert_eq!(memory.read_byte(0xA000), 0xFF);

        // Enable, then RAM works.
        memory.write_byte(0x0000, 0x0A);
        memory.write_byte(0xA000, 0x42);
        assert_eq!(memory.read_byte(0xA000), 0x42);

        // Disable again: data is retained but gated off (reads 0xFF).
        memory.write_byte(0x0000, 0x00);
        assert_eq!(memory.read_byte(0xA000), 0xFF);
    }

    #[test]
    fn test_key1_speed_switch() {
        let mut memory = Memory::new();
        assert!(!memory.double_speed);
        memory.write_byte(0xFF4D, 0x01); // arm a switch
        assert!(memory.try_speed_switch());
        assert!(memory.double_speed);
        assert_eq!(memory.read_byte(0xFF4D) & 0x80, 0x80); // reports double speed
        assert!(!memory.try_speed_switch()); // nothing armed -> no switch
        assert!(memory.double_speed);
    }

    #[test]
    fn test_hdma_copies_to_vram() {
        let mut memory = Memory::new();
        // Stage source bytes in WRAM at 0xC000.
        memory.write_byte(0xC000, 0xAB);
        memory.write_byte(0xC00F, 0xCD);
        // Source = 0xC000, dest = 0x8000, length = 1 block (0x10 bytes).
        memory.write_byte(0xFF51, 0xC0);
        memory.write_byte(0xFF52, 0x00);
        memory.write_byte(0xFF53, 0x00);
        memory.write_byte(0xFF54, 0x00);
        memory.write_byte(0xFF55, 0x00); // trigger, length = (0+1)*0x10

        assert_eq!(memory.read_byte(0x8000), 0xAB);
        assert_eq!(memory.read_byte(0x800F), 0xCD);
        assert_eq!(memory.read_byte(0xFF55), 0xFF); // reports done
    }

    #[test]
    fn test_wram_banking() {
        let mut memory = Memory::new();
        // Bank 0 (0xC000) is fixed; 0xD000 is the switchable bank.
        memory.write_byte(0xFF70, 1); // WRAM bank 1
        memory.write_byte(0xD000, 0x11);
        memory.write_byte(0xFF70, 2); // WRAM bank 2
        memory.write_byte(0xD000, 0x22);

        assert_eq!(memory.read_byte(0xD000), 0x22); // bank 2
        memory.write_byte(0xFF70, 1);
        assert_eq!(memory.read_byte(0xD000), 0x11); // bank 1 preserved
    }

    #[test]
    fn test_vram_banking() {
        let mut memory = Memory::new();
        memory.write_byte(0xFF4F, 0); // select VRAM bank 0
        memory.write_byte(0x8000, 0x11);
        memory.write_byte(0xFF4F, 1); // select VRAM bank 1
        memory.write_byte(0x8000, 0x22);

        assert_eq!(memory.read_byte(0x8000), 0x22); // bank 1's value
        memory.write_byte(0xFF4F, 0);
        assert_eq!(memory.read_byte(0x8000), 0x11); // bank 0 preserved independently
    }

    #[test]
    fn test_cgb_bg_palette_autoincrement() {
        let mut memory = Memory::new();
        memory.write_byte(0xFF68, 0x80); // index 0, auto-increment enabled
        memory.write_byte(0xFF69, 0xAA); // -> palette byte 0, index advances to 1
        memory.write_byte(0xFF69, 0xBB); // -> palette byte 1, index advances to 2

        memory.write_byte(0xFF68, 0x00); // point at index 0 (no auto-increment)
        assert_eq!(memory.read_byte(0xFF69), 0xAA);
        memory.write_byte(0xFF68, 0x01);
        assert_eq!(memory.read_byte(0xFF69), 0xBB);
    }

    #[test]
    fn test_oam_dma_copies_to_oam() {
        let mut memory = Memory::new();
        // Stage a sprite table in WRAM at 0xC000.
        memory.write_byte(0xC000, 0x42);
        memory.write_byte(0xC09F, 0x99); // last of the 160 bytes

        memory.write_byte(0xFF46, 0xC0); // trigger DMA from 0xC000

        assert_eq!(memory.read_byte(0xFE00), 0x42); // first OAM byte
        assert_eq!(memory.read_byte(0xFE9F), 0x99); // last OAM byte
    }
}

