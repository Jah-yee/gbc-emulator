# Understanding the CPU Architecture

**8-bit registers** (each holds a number from 0-255)
- `A`: Accumulator (main register for math)
- `F`: Flags (stores results of operations: Zero, Carry, etc)
- `B, C, D, E, H, L`: General Purpose Registers

**These can pair up into 16-bit registers**:
- `BC`, `DE`, `HL`: Can address 64KB of memory (0x0000 to 0xFFFF)

**Special 16-bit registers**:
- `PC` (Program Counter): Points to the next instruction to execute
- `SP` (Stack Pointer): Points to the top of the stack in memory

**The F (Flags) register has specific bits**:
```
Bit 7: Zero Flag (Z) - Set if result was zero
Bit 6: Subtract Flag (N) - Set if the last operation was subtraction
Bit 5: Half Carry (H) - Set if carry from bit 3 to bit 4
Bit 4: Carry Flag (C) - Set if carry/borrow occurred
Bits 3-0: Always zero
```
