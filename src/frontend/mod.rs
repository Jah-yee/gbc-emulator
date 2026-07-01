// src/frontend/mod.rs
//
// SDL3 windowed frontend, structured as a small state machine over the emulator
// core. Right now there's only a Playing state; pause/menu arrive in later steps.
// SDL handles live as locals in `run()` (not in `App`) so the streaming texture
// can borrow the texture-creator without a self-referential struct.

use crate::cpu::Cpu;
use sdl3::audio::{AudioFormat, AudioSpec};
use sdl3::{event::Event, keyboard::Keycode, pixels::PixelFormat};

const SCALE: u32 = 4;

/// What the frontend is currently doing.
enum AppState {
    Playing,
}

/// Frontend + emulator state (no SDL handles - those live in `run()`).
struct App {
    cpu: Cpu,
    state: AppState,
    #[allow(dead_code)] // used by load-ROM / save-state in later steps
    rom_path: String,
    dpad: u8,
    buttons: u8,
    should_quit: bool,
}

impl App {
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => self.should_quit = true,
            // Press D to dump CPU + hardware state to the terminal.
            Event::KeyDown {
                keycode: Some(Keycode::D),
                ..
            } => eprintln!("{}", self.cpu.debug_state()),
            Event::KeyDown {
                keycode: Some(k), ..
            } => set_key(k, true, &mut self.dpad, &mut self.buttons),
            Event::KeyUp {
                keycode: Some(k), ..
            } => set_key(k, false, &mut self.dpad, &mut self.buttons),
            _ => {}
        }
    }
}

/// Run the windowed emulator until the user quits; returns the Cpu so the caller
/// can persist the battery save.
pub fn run(cpu: Cpu, rom_path: String) -> Cpu {
    let sdl = sdl3::init().unwrap();
    let video = sdl.video().unwrap();
    let window = video
        .window(&format!("gbc - {rom_path}"), 160 * SCALE, 144 * SCALE)
        .position_centered()
        .build()
        .unwrap();

    // No vsync: we pace to the audio stream (below) so speed is correct on any
    // monitor. SDL3 into_canvas returns the Canvas directly (no builder).
    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormat::RGB24, 160, 144)
        .unwrap();

    let mut event_pump = sdl.event_pump().unwrap();

    // Audio: default playback device + an f32 stereo stream we push samples into.
    let audio = sdl.audio().unwrap();
    let spec = AudioSpec {
        freq: Some(44_100),
        channels: Some(2),
        format: Some(AudioFormat::f32_sys()),
    };
    let device = audio.open_playback_device(&spec).unwrap();
    let audio_stream = device.open_device_stream(Some(&spec)).unwrap();
    audio_stream.resume().unwrap();

    let mut app = App {
        cpu,
        state: AppState::Playing,
        rom_path,
        dpad: 0,
        buttons: 0,
        should_quit: false,
    };

    'running: loop {
        for event in event_pump.poll_iter() {
            app.handle_event(event);
        }
        if app.should_quit {
            break 'running;
        }

        match app.state {
            AppState::Playing => {
                app.cpu.memory.set_joypad(app.dpad, app.buttons);

                // Pace to audio playback: if the stream still has plenty buffered,
                // wait and loop back (events keep being polled -> stays responsive).
                if audio_stream.queued_bytes().unwrap_or(0) > 16_384 {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }

                run_one_frame(&mut app.cpu);

                let samples: Vec<f32> = app.cpu.memory.apu.output.drain(..).collect();
                let _ = audio_stream.put_data_f32(&samples);

                blit(&app.cpu, &mut texture);
                canvas.clear();
                canvas.copy(&texture, None, None).unwrap();
                canvas.present();
            }
        }
    }

    app.cpu
}

/// Copy the emulator's RGB framebuffer into the streaming texture.
fn blit(cpu: &Cpu, texture: &mut sdl3::render::Texture) {
    texture
        .with_lock(None, |buf: &mut [u8], pitch: usize| {
            for y in 0..144 {
                for x in 0..160 {
                    let (r, g, b) = cpu.framebuffer[y * 160 + x];
                    let off = y * pitch + x * 3;
                    buf[off] = r;
                    buf[off + 1] = g;
                    buf[off + 2] = b;
                }
            }
        })
        .unwrap();
}

/// Run exactly one PPU frame's worth of CPU cycles (double-speed runs 2x).
fn run_one_frame(cpu: &mut Cpu) {
    let per_frame = 70224 * if cpu.memory.double_speed { 2 } else { 1 };
    let target = cpu.cycles + per_frame;
    while cpu.cycles < target {
        cpu.step();
    }
}

/// Map a key to a joypad button and set/clear its bit. Arrows = D-pad;
/// Z=A, X=B, Backspace=Select, Return=Start.
fn set_key(k: Keycode, pressed: bool, dpad: &mut u8, buttons: &mut u8) {
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
