// src/frontend/mod.rs
//
// SDL3 windowed frontend, structured as a small state machine over the emulator
// core. Right now there's only a Playing state; pause/menu arrive in later steps.
// SDL handles live as locals in `run()` (not in `App`) so the streaming texture
// can borrow the texture-creator without a self-referential struct.

mod input;
mod overlay;

use crate::cpu::Cpu;
use input::{Hotkey, InputConfig, Pad};
use sdl3::audio::{AudioFormat, AudioSpec};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::rect::FRect;
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
    /// Fast-forward multiplier (frames run per loop while held). Adjustable.
    speed_mult: u32,
    /// Debug: show the real-time overlay.
    show_debug: bool,
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
                    _ => AppState::Playing,
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
            Hotkey::SpeedDown => self.speed_mult = (self.speed_mult - 1).max(2),
            Hotkey::SpeedUp => self.speed_mult = (self.speed_mult + 1).min(8),
            Hotkey::Mute(ch) => {
                self.cpu.memory.apu.toggle_mute(ch as usize);
                eprintln!(
                    "ch{} {}",
                    ch + 1,
                    if self.cpu.memory.apu.is_muted(ch as usize) {
                        "muted"
                    } else {
                        "unmuted"
                    }
                );
            }
            Hotkey::ToggleOverlay => self.show_debug = !self.show_debug,
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
        speed_mult: 4,
        show_debug: false,
    };

    // Measure TRUE speed: emulated frames vs wall-clock (GB is 59.7275 fps = 100%),
    // shown in the title bar and refreshed ~twice a second.
    let mut frames_since: u32 = 0;
    let mut last_measure = std::time::Instant::now();
    let mut measured_pct: u32 = 100; // last measured speed, for the overlay
    let mut last_debug = false; // track overlay toggle to resize the window

    'running: loop {
        for event in event_pump.poll_iter() {
            app.handle_event(event);
        }
        if app.should_quit {
            break 'running;
        }

        if last_measure.elapsed().as_millis() >= 500 {
            let secs = last_measure.elapsed().as_secs_f32();
            let pct = (frames_since as f32 / secs / 59.7275 * 100.0).round() as u32;
            measured_pct = pct;
            let _ = canvas.window_mut().set_title(&format!(
                "gbc - {} - {}% (FF {}x)",
                app.rom_path, pct, app.speed_mult
            ));
            frames_since = 0;
            last_measure = std::time::Instant::now();
        }

        // Widen the window to dock the debug panel; shrink back when it's hidden.
        if app.show_debug != last_debug {
            last_debug = app.show_debug;
            let extra = if app.show_debug {
                overlay::PANEL_W as u32
            } else {
                0
            };
            let _ = canvas
                .window_mut()
                .set_size(160 * SCALE + extra, 144 * SCALE);
        }

        match app.state {
            AppState::Playing => {
                app.cpu.memory.set_joypad(app.dpad, app.buttons);

                // Pace to audio playback (keeps speed correct on any monitor and
                // keeps input responsive - events are polled at the top of the loop).
                if audio_stream.queued_bytes().unwrap_or(0) > 16_384 {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }

                // Fast-forward runs N frames per paced step; audio is decimated by
                // N so it plays sped up at the correct rate (no muting, no backlog).
                let n = if app.fast_forward {
                    app.speed_mult as usize
                } else {
                    1
                };
                for _ in 0..n {
                    run_one_frame(&mut app.cpu);
                }
                frames_since += n as u32; // for the true-speed measurement
                let out = &mut app.cpu.memory.apu.output;
                let samples: Vec<f32> = if n == 1 {
                    out.drain(..).collect()
                } else {
                    let d: Vec<f32> = out
                        .chunks_exact(2)
                        .step_by(n)
                        .flat_map(|c| [c[0], c[1]])
                        .collect();
                    out.clear();
                    d
                };
                let _ = audio_stream.put_data_f32(&samples);

                blit(&app.cpu, &mut texture);
                canvas.set_draw_color(Color::RGB(0, 0, 0));
                canvas.clear();
                canvas.copy(&texture, None, game_rect(app.show_debug)).unwrap();
                if app.show_debug {
                    let q = audio_stream.queued_bytes().unwrap_or(0);
                    overlay::draw(&mut canvas, &app.cpu, q, measured_pct);
                }
                canvas.present();
            }
            AppState::Paused => {
                // Don't step the CPU. Re-present the last frame with a dim overlay.
                // Events are still polled at the top of the loop, so P (unpause) and
                // window-close stay responsive; sleep keeps this from busy-spinning.
                canvas.set_draw_color(Color::RGB(0, 0, 0));
                canvas.clear();
                canvas.copy(&texture, None, game_rect(app.show_debug)).unwrap();
                // Blend only for the dim overlay, then restore opaque drawing so the
                // per-frame game copy stays a cheap straight blit (avoids lag on a
                // software renderer).
                canvas.set_blend_mode(BlendMode::Blend);
                canvas.set_draw_color(Color::RGBA(0, 0, 0, 128));
                let _ = canvas.fill_rect(None);
                canvas.set_blend_mode(BlendMode::None);
                if app.show_debug {
                    let q = audio_stream.queued_bytes().unwrap_or(0);
                    overlay::draw(&mut canvas, &app.cpu, q, measured_pct);
                }
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

/// Destination rect for the game frame: a left sub-rect when the debug panel is
/// docked, or the whole window (None) otherwise.
fn game_rect(show_debug: bool) -> Option<FRect> {
    if show_debug {
        Some(FRect::new(0.0, 0.0, (160 * SCALE) as f32, (144 * SCALE) as f32))
    } else {
        None
    }
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

