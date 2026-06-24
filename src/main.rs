// src/main.rs

mod cpu;
mod memory;

use cpu::Cpu;

fn main() {
    let mut cpu = Cpu::new();

    // Paint tile #1: every row uses the 0x3C/0x7E "curve" pattern.
    for row in 0..8u16 {
        cpu.memory.write_byte(0x8010 + row * 2, 0x3C);
        cpu.memory.write_byte(0x8011 + row * 2, 0x7E);
    }

    // Fill the 32x32 background map with tile #1
    for i in 0..(32 * 32u16) {
        cpu.memory.write_byte(0x9800 + i, 0x01);
    }
    cpu.memory.write_byte(0xFF47, 0xE4); // identity palette (LCDC already 0x91)

    // A HALT at 0x0100 so the CPU idles; the PPU keeps running on elapsed cycles.
    let mut rom = vec![0x00; 0x0101];
    rom[0x0100] = 0x76; // HALT
    cpu.memory.load_rom(&rom);

    // Run ~one frame (70224 cycles; halted steps are 4 cycles each).
    for _ in 0..20000 {
        cpu.step();
    }

    print_frame(&cpu);
}

fn print_frame(cpu: &Cpu) {
    const SHADES: [char; 4] = [' ', '.', '+', '#']; // 0 =lightest .. 3 =darkest

    for y in (0..144).step_by(2) {
        let mut line = String::new();
        for x in (0..160).step_by(2) {
            let shade = cpu.framebuffer[y * 160 + x];
            line.push(SHADES[shade as usize]);
        }
        println!("{line}");
    }
}

