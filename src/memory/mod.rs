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
}

impl Memory {
    pub fn new() -> Self {
        let mut memory = Self {
            rom_bank_0: [0; 0x4000],
            rom_bank_n: [0; 0x4000],
            vram: [0; 0x2000],
            external_ram: [0; 0x2000],
            wram: [0; 0x2000],
            oam: [0; 0xA0],
            io_registers: [0; 0x80],
            hram: [0; 0x7F],
            ie_register: 0,
            serial: String::new(),
            trace: std::env::var("GBC_TRACE").is_ok(),
            joypad_select: 0x30, // nothing selected
            dpad: 0,
            buttons: 0,
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

    pub fn read_byte(&self, address: u16) -> u8 {
        // Gameboy Doctor: its reference logs were generated with LY hardcoded to
        // 0x90, so stub it here while tracing to avoid spurious PPU-timing diffs.
        if self.trace && address == 0xFF44 {
            return 0x90;
        }
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

            // Joypad: the game only writes the group-select bits 4-5.
            0xFF00 => self.joypad_select = value & 0x30,

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

    // Load ROM into memory
    pub fn load_rom(&mut self, rom: &[u8]) {
        let bank_0_size = std::cmp::min(rom.len(), 0x4000);
        self.rom_bank_0[..bank_0_size].copy_from_slice(&rom[..bank_0_size]);

        if rom.len() > 0x4000 {
            let bank_n_size = std::cmp::min(rom.len() - 0x4000, 0x4000);
            self.rom_bank_n[..bank_n_size].copy_from_slice(&rom[0x4000..0x4000 + bank_n_size]);
        }
    }

    /// Set joypad press state from the frontend. Each is a low-nibble mask
    /// (1 = pressed): dpad = Right/Left/Up/Down (bits 0-3),
    /// buttons = A/B/Select/Start (bits 0-3).
    pub fn set_joypad(&mut self, dpad: u8, buttons: u8) {
        self.dpad = dpad & 0x0F;
        self.buttons = buttons & 0x0F;
    }
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
}

