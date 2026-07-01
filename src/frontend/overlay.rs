// src/frontend/overlay.rs
//
// Dep-free real-time debug overlay drawn on the SDL3 2D canvas (no OpenGL, no
// GUI crate). Shows: audio buffer depth, per-channel APU level meters, and a
// text panel (speed, CPU registers, PPU LY/LCDC) rendered with an embedded 3x5
// bitmap font. Coordinates are in window pixels (canvas has no logical size).

use crate::cpu::Cpu;
use sdl3::pixels::Color;
use sdl3::rect::Rect;
use sdl3::render::Canvas;
use sdl3::video::Window;

// Window is 160x144 scaled by 4 (see SCALE in mod.rs).
const WIN_W: u32 = 160 * 4;

/// Draw the overlay on top of the current frame. `queued_bytes` is the audio
/// backlog; `pct` is the measured emulation speed.
pub fn draw(canvas: &mut Canvas<Window>, cpu: &Cpu, queued_bytes: i32, pct: u32) {
    // --- audio buffer depth bar along the very top ---
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

    // --- text panel: speed + CPU/PPU state ---
    let r = &cpu.registers;
    let lines = [
        format!("SPD {}%", pct),
        format!("PC {:04X} SP {:04X}", cpu.pc, cpu.sp),
        format!("AF {:02X}{:02X} BC {:02X}{:02X}", r.a, r.f, r.b, r.c),
        format!("DE {:02X}{:02X} HL {:02X}{:02X}", r.d, r.e, r.h, r.l),
        format!(
            "LY {:02X} LCDC {:02X}",
            cpu.memory.read_byte(0xFF44),
            cpu.memory.read_byte(0xFF40)
        ),
    ];

    let scale = 3i32;
    let line_h = 6 * scale; // 5 rows + 1 gap
    let x0 = 4i32;
    let y0 = base_y + max_h + 6;

    // Opaque backdrop so text stays legible over any frame.
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    let _ = canvas.fill_rect(Rect::new(
        2,
        y0 - 2,
        210,
        (lines.len() as i32 * line_h + 4) as u32,
    ));

    let white = Color::RGB(235, 235, 235);
    for (i, line) in lines.iter().enumerate() {
        draw_text(canvas, x0, y0 + i as i32 * line_h, scale, line, white);
    }
}

/// Render an uppercase/hex string with the embedded 3x5 font at `scale`x.
fn draw_text(canvas: &mut Canvas<Window>, x: i32, y: i32, scale: i32, text: &str, color: Color) {
    canvas.set_draw_color(color);
    let mut cx = x;
    for ch in text.chars() {
        let glyph = font3x5(ch);
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..3i32 {
                if bits & (1 << (2 - col)) != 0 {
                    let _ = canvas.fill_rect(Rect::new(
                        cx + col * scale,
                        y + row as i32 * scale,
                        scale as u32,
                        scale as u32,
                    ));
                }
            }
        }
        cx += 4 * scale; // 3 columns + 1 gap
    }
}

/// A minimal 3x5 bitmap font: uppercase, digits, and a few symbols. Each row's
/// low 3 bits are the columns (bit 2 = leftmost). Unknown chars render blank.
fn font3x5(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b111, 0b111, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b110, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        _ => [0, 0, 0, 0, 0], // space + anything unmapped
    }
}
