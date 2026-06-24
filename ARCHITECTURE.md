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

## Interrupts

Three things gate whether an interrupt fires:
- **IME** (a single bool, `interrupts_enabled`) - the master switch. Set by
  `EI`/`RETI`, cleared by `DI` and automatically on dispatch.
- **IE** (`0xFFFF`) - which interrupts the program has *enabled*.
- **IF** (`0xFF0F`) - which interrupts hardware has *requested*.

An interrupt fires when IME is on AND a bit is set in both IE and IF.
Dispatch (in `handle_interrupts`, run before each fetch) acts like a
hardware `CALL`: acknowledge (clear the IF bit), clear IME, push PC, and
jump to the interrupt's vector. `RETI` is the matching return.
A pending interrupt also wakes the CPU from `HALT`, even when IME is off.

See MEMORY_MAP.md for the 5 interrupts, their bits, and vectors.

## Timer

Four registers (DIV/TIMA/TMA/TAC, see MEMORY_MAP.md) driven by elapsed CPU
cycles each step:
- **DIV** is the high byte of a free-running 16-bit counter (ticks every 256 cycles).
- **TIMA** counts up at the rate TAC selects (when TAC bit 2 is set). On
  overflow past `0xFF` it reloads from **TMA** and requests the Timer
  interrupt (IF bit 2) - which then dispatches through the interrupt system above.

## Implementation status

- **CPU instruction set**: complete (full 8/16-bit ops, control flow, CB-prefix table).
- **Interrupts + timer**: complete.
- **PPU (graphics)**: not started - next subsystem.
- **MBC (cartridge banking)**: not started (ROM-region writes are currently a no-op stub).
