// src/main.rs

mod apu;
mod cpu;
mod memory;

#[cfg(test)]
mod blargg_tests;

#[cfg(feature = "gui")]
mod frontend;

use cpu::Cpu;

fn main() {
    let mut cpu = Cpu::new();
    let path = std::env::args().nth(1).expect("usage: gbc <rom.gb>");
    let rom = std::fs::read(&path).expect("failed to read ROM");
    cpu.memory.load_rom(&rom);

    // Tell CGB cartridges they're on a Game Boy Color (A=0x11 at boot), so they
    // enable color. DMG carts keep the DMG post-boot A=0x01.
    if cpu.memory.is_cgb() {
        cpu.registers.a = 0x11;
    }

    // Battery save: load the .sav next to the ROM, if one exists.
    let save_path = format!("{path}.sav");
    if let Ok(data) = std::fs::read(&save_path) {
        cpu.memory.load_ram(&data);
        eprintln!("loaded save: {save_path}");
    }

    #[cfg(feature = "gui")]
    {
        cpu = frontend::run(cpu, path.clone());
    }

    #[cfg(not(feature = "gui"))]
    {
        // Headless: run until the ROM reports over serial (test ROMs), or a cap.
        // With GBC_TRACE=1 this emits a Gameboy Doctor log on stdout; the serial
        // summary goes to stderr so stdout stays a clean trace.
        let mut last = 0;
        while cpu.cycles < 250_000_000 {
            cpu.step();
            let len = cpu.memory.serial.len();
            if len != last {
                last = len;
                if cpu.memory.serial.contains("Passed") || cpu.memory.serial.contains("Failed") {
                    break;
                }
            }
        }
        eprintln!("[serial] {}", cpu.memory.serial);
        if !cpu.trace {
            print_frame_ascii(&cpu);
        }
    }

    // Battery save: write cartridge RAM back out on exit (battery carts only).
    if cpu.memory.has_battery() {
        if std::fs::write(&save_path, cpu.memory.ram_snapshot()).is_ok() {
            eprintln!("saved: {save_path}");
        }
    }
}

/// Zero-dependency headless renderer: dump one frame to the terminal with
/// truecolor half-blocks. Used when the `gui` feature is off.
#[cfg(not(feature = "gui"))]
fn print_frame_ascii(cpu: &Cpu) {
    for y in (0..144).step_by(2) {
        let mut line = String::new();
        for x in (0..160).step_by(2) {
            let (tr, tg, tb) = cpu.framebuffer[y * 160 + x];
            let (br, bg, bb) = cpu.framebuffer[(y + 1) * 160 + x];
            line.push_str(&format!(
                "\x1b[38;2;{tr};{tg};{tb}m\x1b[48;2;{br};{bg};{bb}m\u{2580}",
            ));
        }
        line.push_str("\x1b[0m"); // reset so the last color doesn't bleed into the shell
        println!("{line}");
    }
}
