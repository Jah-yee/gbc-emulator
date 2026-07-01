// src/frontend/mod.rs
//
// SDL3 windowed frontend, structured as a small state machine over the emulator
// core. Right now there's only a Playing state; pause/menu arrive in later steps.
// SDL handles live as locals in `run()` (not in `App`) so the streaming texture
// can borrow the texture-creator without a self-referential struct.

mod input;

use crate::cpu::Cpu;
use input::{Hotkey, InputConfig, Pad};
use sdl3::audio::{AudioFormat, AudioSpec};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::BlendMode;
use sdl3::event::Event;

const SCALE: u32 = 4;

/// What the frontend is currently doing.
enum AppState {
    Playing,
    Paused,
    /// Transient: prompt for and switch to a new ROM, then back to Playing.
    LoadRom,
}

/// Frontend + emulator state (no SDL handles - those live in `run()`).
struct App {
    cpu: Cpu,
    state: AppState,
    rom_path: String,
    input: InputConfig,
    dpad: u8,
    buttons: u8,
    should_quit: bool,
    /// In-memory quick save slot (F5 saves, F9 restores).
    quick_slot: Option<Box<Cpu>>,
    /// True while the fast-forward key is held.
    fast_forward: bool,
}

impl App {
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Quit { .. } => self.should_quit = true,
            Event::KeyDown {
                keycode: Some(k),
                repeat,
                ..
            } => {
                if let Some(pad) = self.input.game(k) {
                    self.set_pad(pad, true);
                } else if let Some(hk) = self.input.hotkey(k) {
                    self.handle_hotkey(hk, repeat);
                }
            }
            Event::KeyUp {
                keycode: Some(k), ..
            } => {
                if let Some(pad) = self.input.game(k) {
                    self.set_pad(pad, false);
                } else if let Some(Hotkey::FastForward) = self.input.hotkey(k) {
                    self.fast_forward = false;
                }
            }
            _ => {}
        }
    }

    fn set_pad(&mut self, pad: Pad, pressed: bool) {
        let (target, mask) = match pad {
            Pad::Dpad(m) => (&mut self.dpad, m),
            Pad::Btn(m) => (&mut self.buttons, m),
        };
        if pressed {
            *target |= mask;
        } else {
            *target &= !mask;
        }
    }

    fn handle_hotkey(&mut self, hk: Hotkey, repeat: bool) {
        if repeat {
            return; // ignore key auto-repeat for edge-triggered hotkeys
        }
        match hk {
            Hotkey::Quit => self.should_quit = true,
            Hotkey::Debug => eprintln!("{}", self.cpu.debug_state()),
            Hotkey::Pause => {
                self.state = match self.state {
                    AppState::Playing => AppState::Paused,
                    AppState::Paused => AppState::Playing,
                };
            }
            Hotkey::QuickSave => {
                self.quick_slot = Some(Box::new(self.cpu.clone()));
                eprintln!("quick save");
            }
            Hotkey::QuickLoad => {
                if let Some(slot) = &self.quick_slot {
                    self.cpu = (**slot).clone();
                    eprintln!("quick load");
                }
            }
            Hotkey::FastForward => self.fast_forward = true,
            Hotkey::LoadRom => self.state = AppState::LoadRom,
            Hotkey::SaveFile => {
                let path = format!("{}.state", self.rom_path);
                match std::fs::write(&path, crate::savestate::to_bytes(&self.cpu)) {
                    Ok(()) => eprintln!("saved state: {path}"),
                    Err(e) => eprintln!("save state failed: {e}"),
                }
            }
            Hotkey::LoadFile => {
                let path = format!("{}.state", self.rom_path);
                match std::fs::read(&path) {
                    Ok(bytes) => match crate::savestate::from_bytes(&bytes) {
                        Some(cpu) => {
                            self.cpu = cpu;
                            eprintln!("loaded state: {path}");
                        }
                        None => eprintln!("state incompatible or corrupt: {path}"),
                    },
                    Err(e) => eprintln!("load state failed: {e}"),
                }
            }
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
        input: InputConfig::default(),
        dpad: 0,
        buttons: 0,
        should_quit: false,
        quick_slot: None,
        fast_forward: false,
    };

    // Enable alpha blending so the pause overlay can dim the frame.
    canvas.set_blend_mode(BlendMode::Blend);

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

                if app.fast_forward {
                    // Run several frames unthrottled, discarding their audio so the
                    // stream doesn't back up, and present only the last frame.
                    const SPEED: u32 = 4;
                    for _ in 0..SPEED {
                        run_one_frame(&mut app.cpu);
                        app.cpu.memory.apu.output.clear();
                    }
                } else {
                    // Pace to audio playback: if the stream still has plenty
                    // buffered, wait and loop back (events keep polling -> stays
                    // responsive).
                    if audio_stream.queued_bytes().unwrap_or(0) > 16_384 {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        continue;
                    }
                    run_one_frame(&mut app.cpu);
                    let samples: Vec<f32> = app.cpu.memory.apu.output.drain(..).collect();
                    let _ = audio_stream.put_data_f32(&samples);
                }

                blit(&app.cpu, &mut texture);
                canvas.set_draw_color(Color::RGB(0, 0, 0));
                canvas.clear();
                canvas.copy(&texture, None, None).unwrap();
                canvas.present();
            }
            AppState::Paused => {
                // Don't step the CPU. Re-present the last frame with a dim overlay.
                // Events are still polled at the top of the loop, so P (unpause) and
                // window-close stay responsive; sleep keeps this from busy-spinning.
                canvas.set_draw_color(Color::RGB(0, 0, 0));
                canvas.clear();
                canvas.copy(&texture, None, None).unwrap();
                canvas.set_draw_color(Color::RGBA(0, 0, 0, 128));
                let _ = canvas.fill_rect(None);
                canvas.present();
                std::thread::sleep(std::time::Duration::from_millis(16));
            }
            AppState::LoadRom => {
                // Persist the current cart's save, then swap in a new ROM. (v1
                // prompts on the terminal; the window is briefly unresponsive.)
                persist_sav(&app.cpu, &app.rom_path);
                if let Some((new_cpu, new_path)) = prompt_and_load() {
                    app.cpu = new_cpu;
                    app.rom_path = new_path;
                    app.dpad = 0;
                    app.buttons = 0;
                    app.quick_slot = None;
                    let _ = audio_stream.clear();
                }
                app.state = AppState::Playing;
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

/// Write the cartridge RAM back to `<rom>.sav` (battery carts only).
fn persist_sav(cpu: &Cpu, rom_path: &str) {
    if cpu.memory.has_battery() {
        let save_path = format!("{rom_path}.sav");
        if std::fs::write(&save_path, cpu.memory.ram_snapshot()).is_ok() {
            eprintln!("saved: {save_path}");
        }
    }
}

/// Prompt on the terminal for a ROM path and build a fresh Cpu for it (applying
/// the CGB boot flag and loading its battery save). Returns None on empty input
/// or a read error.
fn prompt_and_load() -> Option<(Cpu, String)> {
    use std::io::Write;
    eprint!("load ROM path: ");
    let _ = std::io::stderr().flush();

    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok()?;
    let path = line.trim();
    if path.is_empty() {
        return None;
    }

    let rom = match std::fs::read(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("load failed: {e}");
            return None;
        }
    };

    let mut cpu = Cpu::new();
    cpu.memory.load_rom(&rom);
    if cpu.memory.is_cgb() {
        cpu.registers.a = 0x11;
    }
    let save_path = format!("{path}.sav");
    if let Ok(data) = std::fs::read(&save_path) {
        cpu.memory.load_ram(&data);
        eprintln!("loaded save: {save_path}");
    }

    Some((cpu, path.to_string()))
}

