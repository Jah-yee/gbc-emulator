// src/main.rs

mod cpu;
mod memory;

use cpu::Cpu;

fn main() {
    let mut cpu = Cpu::new();

    // Load a simple test program
    // This program: LD A, 0x05; LD B, 0x03; ADD A, B; HALT
    let program = vec![
        0x3E, 0x05, // LD A, 5
        0x06, 0x03, // LD B, c
        0x80, // ADD A, B (A should not be 8)
        0x76, // HALT
    ];

    // Write program starting at 0x0100 where PC starts
    // pad the ROM with NOPs up to 0x0100
    // for (i, &byte) in program.iter().enumerate() {
    //     cpu.memory.write_byte(0x0100 + i as u16, byte);
    // }


    // Build a ROM:
    // 0x0000..=0x00FF = NOP (0x00)
    // 0x0100..        = program
    let mut rom = vec![0x00; 0x0100 + program.len()];
    rom[0x0100..0x0100 + program.len()].copy_from_slice(&program);

    // IMPORTANT: load the padded ROM, not just `program`
    cpu.memory.load_rom(&rom);


    // Run the CPU for a few steps
    for _ in 0..10 {
        if cpu.halted {
            break;
        }
        cpu.step();
        println!("PC: 0x{:04X}, A: 0x{:02X}, B: 0x{:02X}, Cycles: {}",
                cpu.pc, cpu.registers.a, cpu.registers.b, cpu.cycles);
    }

    println!("\nFinal State:");
    println!("A: 0x{:02X} (should be 0x08)", cpu.registers.a);
    println!("B: 0x{:02X} (should be 0x03)", cpu.registers.b);
}