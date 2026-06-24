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
0xFFFF: Interrupt Enable Register (IE)
```

## Key I/O Registers (0xFF00-0xFF7F)
```
0xFF04: DIV  - Divider (high byte of free-running counter; write resets*)
0xFF05: TIMA - Timer counter (increments at TAC rate)
0xFF06: TMA  - Timer modulo (TIMA reloads to this on overflow)
0xFF07: TAC  - Timer control (bit2 = enable, bits1-0 = clock select)
0xFF0F: IF   - Interrupt Flag (requested interrupts, low 5 bits)
```
*DIV write-reset not yet implemented (documented simplification).

## Interrupts (bit = IE/IF position, vector = jump target)
```
bit 0: VBlank    -> 0x0040
bit 1: LCD STAT  -> 0x0048
bit 2: Timer     -> 0x0050
bit 3: Serial    -> 0x0058
bit 4: Joypad    -> 0x0060
```
Fires when IME on AND (IE & IF) bit set. Dispatch: clear IF bit,
clear IME, push PC, jump to vector. RETI returns and re-enables IME.

## PPU registers (NOT YET IMPLEMENTED - next subsystem)
```
0xFF40: LCDC - LCD control      0xFF44: LY   - current scanline
0xFF41: STAT - LCD status/mode  0xFF45: LYC  - LY compare
0xFF42: SCY  - scroll Y         0xFF47: BGP  - BG palette
0xFF43: SCX  - scroll X
```