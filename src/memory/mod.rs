// src/memory/mod.rs

pub struct Memory {
    // ROM Bank 0 (first 16KB of cartridge)
    rom_bank_0: [u8; 0x4000],

    // ROM BANK N (switchable banks)
    rom_bank_n: [u8; 0x4000],

    // Video RAM
    vram: [u8; 0x2000],

    // External RAM (Cartridge RAM)
    external_ram: [u8; 0x2000],

    // Work RAM
    wram: [u8; 0x2000],

    // Sprite Attribute Table
    oam: [u8; 0xA0],

    // I/O Registers
    io_registers: [u8; 0x80],

    // High RAM
    hram: [u8; 0x7F],

    // Interrupt Enable Register
    ie_register: u8,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            rom_bank_0: [0; 0x4000],
            rom_bank_n: [0; 0x4000],
            vram: [0; 0x2000],
            external_ram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 0xA0],
            io_registers: [0; 0x80],
            hram: [0; 0x7F],
            ie_register: 0
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            // ROM Bank 0
            0x0000..=0x3FFF => self.rom_bank_0[address as usize],

            // ROM Bank N
            0x4000..=0x7FFF => self.rom_bank_n[(address - 0x4000) as usize],

            // VRAM
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize],

            // External RAM
            0xA000..=0xBFFF => self.external_ram[(address - 0xA000) as usize],

            // Work RAM
            0xC000..=0xDFFF => self.wram[(address - 0xC000) as usize],

            // Echo RAM (mirror of Work RAM)
            0xE000..=0xFDFF => self.wram[(address - 0xE000) as usize],

            // OAM
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize],

            // Not usable
            0xFEA0..=0xFEFF => 0xFF,

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
            // ROM is read-only (writes may trigger bank switching)
            0x0000..=0x7FFF => {
                // For now, ignore writes to ROM
                // Later we'll implement Memory Bank Controllers (MBC)
            }

            // VRAM
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize] = value,

            // External RAM
            0xA000..=0xBFFF => self.external_ram[(address - 0xA000) as usize] = value,

            // Work RAM
            0xC000..=0xDFFF => self.wram[(address - 0xC000) as usize] = value,

            // Echo RAM (mirror of Work RAM)
            0xE000..=0xFDFF => self.wram[(address - 0xE000) as usize] = value,

            // OAM
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize] = value,

            // Not usable
            0xFEA0..=0xFEFF => {}

            // I/O Registers
            0xFF00..=0xFF7F => self.io_registers[(address - 0xFF00) as usize] = value,

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

    // Load ROM into memory
    pub fn load_rom(&mut self, rom: &[u8]) {
        let bank_0_size = std::cmp::min(rom.len(), 0x4000);
        self.rom_bank_0[..bank_0_size].copy_from_slice(&rom[..bank_0_size]);

        if rom.len() > 0x4000 {
            let bank_n_size = std::cmp::min(rom.len() - 0x4000, 0x4000);
            self.rom_bank_n[..bank_n_size].copy_from_slice(&rom[0x4000..0x4000 + bank_n_size]);
        }
    }
}