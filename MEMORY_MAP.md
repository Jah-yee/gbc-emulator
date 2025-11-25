# THE MEMORY MAP
```
0x0000-0x00FF: Boot ROM (disabled after boot)
0x0100-0x3FFF: ROM Bank 0 (first 16KB of cartridge)
0x4000-0x7FFF: ROM Bank N (switchable, for games > 32KB)
0x8000-0x9FFF: Video RAM (VRAM) - holds tile graphics
0xA000-0xBFFF: External RAM (on cartridge, for save data)
0xC000-0xDFFF: Work RAM (WRAM) - 8KB for game use
0xE000-0xFDFF: Echo RAM (mirror of C000-DDFF, not used)
0xFE00-0xFE9F: Sprite Attributes Table (OAM) - sprite data
0xFEA0-0xFEFF: Not usable
0xFF00-0xFF7F: I/O Registers (controls hardware)
0xFF80-0xFFFE: High RAM (HRAM) - first 127 bytes
0xFFFF: Interrupt Enable Register
```