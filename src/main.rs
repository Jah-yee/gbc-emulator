// src/main.rs

mod cpu;
mod memory;

#[cfg(test)]
mod blargg_tests;

use cpu::Cpu;

/// Paint a teset pattern into VRAM, then load a HALT so the CPU idles
/// while the PPU keeps rendering on elapsed cycles.
fn setup_demo(cpu: &mut Cpu) {
    for row in 0..8u16 {
        cpu.memory.write_byte(0x8010 + row * 2, 0x3C);
        cpu.memory.write_byte(0x8011 + row * 2, 0x7E);
    }

    for i in 0..(32 * 32u16) {
        cpu.memory.write_byte(0x9800 + i, 0x01);
    }

    cpu.memory.write_byte(0xFF47, 0xE4); // BGP identity: 11_10_01_00

    let mut rom = vec![0x00; 0x0101];
    rom[0x0100] = 0x76; // HALT
    cpu.memory.load_rom(&rom);
}

/// Run roughly one frame *~70224 cycles; halted steps are 4 cycles each
fn run_one_frame(cpu: &mut Cpu) {
    for _ in 0..17556 {
        cpu.step();
    }
}

/// Map a key to a joypad button and set/clear its bit. Arrows = D-pad;
/// Z=A, X=B, Backspace=Select, Return=Start.
#[cfg(feature = "gui")]
fn set_key(k: sdl2::keyboard::Keycode, pressed: bool, dpad: &mut u8, buttons: &mut u8) {
    use sdl2::keyboard::Keycode;
    let (mask, target): (u8, &mut u8) = match k {
        Keycode::Right => (0b0001, dpad),
        Keycode::Left => (0b0010, dpad),
        Keycode::Up => (0b0100, dpad),
        Keycode::Down => (0b1000, dpad),
        Keycode::Z => (0b0001, buttons),         // A
        Keycode::X => (0b0010, buttons),         // B
        Keycode::Backspace => (0b0100, buttons), // Select
        Keycode::Return => (0b1000, buttons),    // Start
        _ => return,
    };
    if pressed {
        *target |= mask;
    } else {
        *target &= !mask;
    }
}

///
#[cfg(feature = "gui")]
fn run_window(mut cpu: Cpu) -> Cpu {
    use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum};

    const SCALE: u32 = 4;

    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();

    let window = video
        .window("Gameboy Color", 160 * SCALE, 144 * SCALE)
        .position_centered()
        .build()
        .unwrap();

    // present_vsync paces the loop to the monitor's refresh (~60fps)
    let mut canvas = window.into_canvas().present_vsync().build().unwrap();

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 160, 144)
        .unwrap();

    let mut event_pump = sdl.event_pump().unwrap();

    // Joypad press masks (low nibble each, 1 = pressed), updated on key events.
    let mut dpad = 0u8;
    let mut buttons = 0u8;

    'running: loop {
        // drain pending events; quit on window-close or Escape
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                // Press D to dump CPU + hardware state to the terminal.
                Event::KeyDown {
                    keycode: Some(Keycode::D),
                    ..
                } => eprintln!("{}", cpu.debug_state()),
                Event::KeyDown {
                    keycode: Some(k), ..
                } => set_key(k, true, &mut dpad, &mut buttons),
                Event::KeyUp {
                    keycode: Some(k), ..
                } => set_key(k, false, &mut dpad, &mut buttons),
                _ => {}
            }
        }
        cpu.memory.set_joypad(dpad, buttons);

        // advance one frame
        run_one_frame(&mut cpu);

        // copy framebuffer -> texture (part 3 fills this in)
        texture
            .with_lock(None, |buf: &mut [u8], pitch: usize| {
                for y in 0..144 {
                    for x in 0..160 {
                        let (r, g, b) = cpu.framebuffer[y * 160 + x];
                        let offset = y * pitch + x * 3;
                        buf[offset] = r;
                        buf[offset + 1] = g;
                        buf[offset + 2] = b;
                    }
                }
            })
            .unwrap();

        // draw the texture to the window, scaled to fill
        canvas.clear();
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    }

    cpu // hand the CPU back so main can persist the save
}

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
        cpu = run_window(cpu);
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


