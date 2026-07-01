// src/frontend/overlay.rs
//
// Dep-free real-time debug overlay, drawn on the SDL3 2D canvas (no OpenGL, no
// GUI crate). First panel: per-channel APU level meters (dimmed when muted) and
// the audio buffer depth - the two things we need to chase the audio "echo".

use crate::cpu::Cpu;
use sdl3::pixels::Color;
use sdl3::rect::Rect;
use sdl3::render::Canvas;
use sdl3::video::Window;

// Window is 160x144 scaled by 4 (see SCALE in mod.rs).
const WIN_W: u32 = 160 * 4;

/// Draw the overlay on top of the current frame. `queued_bytes` is the audio
/// stream's current backlog (a bar creeping toward full = latency stacking).
pub fn draw(canvas: &mut Canvas<Window>, cpu: &Cpu, queued_bytes: i32) {
    // --- audio buffer depth bar along the very top ---
    // Full width ~= 64 KB queued; our pace target is ~16 KB, so a healthy buffer
    // sits around a quarter. Steady growth toward full points at the echo.
    let buf_frac = (queued_bytes as f32 / 65_536.0).clamp(0.0, 1.0);
    canvas.set_draw_color(Color::RGB(20, 20, 20));
    let _ = canvas.fill_rect(Rect::new(0, 0, WIN_W, 6));
    canvas.set_draw_color(Color::RGB(220, 200, 40));
    let _ = canvas.fill_rect(Rect::new(0, 0, (WIN_W as f32 * buf_frac) as u32, 6));

    // --- per-channel level meters (top-left) ---
    let levels = cpu.memory.apu.channel_levels();
    let bar_w: u32 = 14;
    let gap: i32 = 4;
    let max_h: i32 = 56;
    let base_y: i32 = 10;
    for (i, &lvl) in levels.iter().enumerate() {
        let x = gap + i as i32 * (bar_w as i32 + gap);
        canvas.set_draw_color(Color::RGB(30, 30, 30)); // track
        let _ = canvas.fill_rect(Rect::new(x, base_y, bar_w, max_h as u32));

        let h = (lvl.clamp(0.0, 1.0) * max_h as f32) as i32;
        let color = if cpu.memory.apu.is_muted(i) {
            Color::RGB(120, 40, 40) // muted -> dim red
        } else {
            Color::RGB(60, 220, 90) // active -> green
        };
        canvas.set_draw_color(color);
        let _ = canvas.fill_rect(Rect::new(x, base_y + (max_h - h), bar_w, h as u32));
    }
}
