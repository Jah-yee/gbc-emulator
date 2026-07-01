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

// Layout (window pixels). Game fills the left GAME_W x GAME_H; the debug panel
// docks to its right. See mod.rs (SCALE=4) and the window-resize logic.
pub const GAME_W: i32 = 160 * 4;
pub const GAME_H: i32 = 144 * 4;
pub const PANEL_W: i32 = 360;

/// Draw the docked debug panel to the right of the game. `queued_bytes` is the
/// audio backlog; `pct` is the measured emulation speed.
pub fn draw(canvas: &mut Canvas<Window>, cpu: &Cpu, queued_bytes: i32, pct: u32) {
    let px = GAME_W; // panel left edge
    canvas.set_draw_color(Color::RGB(16, 16, 20));
    let _ = canvas.fill_rect(Rect::new(px, 0, PANEL_W as u32, GAME_H as u32));

    let white = Color::RGB(235, 235, 235);
    let dim = Color::RGB(150, 150, 165);
    let x0 = px + 8;
    let s = 3i32;
    let lh = 6 * s;

    // --- CPU / PPU state ---
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
    let mut y = 8i32;
    for line in &lines {
        draw_text(canvas, x0, y, s, line, white);
        y += lh;
    }

    // --- audio buffer depth ---
    y += 12;
    draw_text(canvas, x0, y, 2, "AUDIO BUF", dim);
    y += 6 * 2 + 4;
    let bar_full = PANEL_W - 16;
    let frac = (queued_bytes as f32 / 65_536.0).clamp(0.0, 1.0);
    canvas.set_draw_color(Color::RGB(40, 40, 40));
    let _ = canvas.fill_rect(Rect::new(x0, y, bar_full as u32, 10));
    canvas.set_draw_color(Color::RGB(220, 200, 40));
    let _ = canvas.fill_rect(Rect::new(x0, y, (bar_full as f32 * frac) as u32, 10));

    // --- per-channel APU level meters ---
    y += 22;
    draw_text(canvas, x0, y, 2, "CH LEVELS", dim);
    y += 6 * 2 + 6;
    let levels = cpu.memory.apu.channel_levels();
    let bw = 30i32;
    let gap = 18i32;
    let mh = 80i32;
    for (i, &lvl) in levels.iter().enumerate() {
        let bx = x0 + i as i32 * (bw + gap);
        canvas.set_draw_color(Color::RGB(40, 40, 40));
        let _ = canvas.fill_rect(Rect::new(bx, y, bw as u32, mh as u32));

        let h = (lvl.clamp(0.0, 1.0) * mh as f32) as i32;
        let col = if cpu.memory.apu.is_muted(i) {
            Color::RGB(120, 40, 40)
        } else {
            Color::RGB(60, 220, 90)
        };
        canvas.set_draw_color(col);
        let _ = canvas.fill_rect(Rect::new(bx, y + (mh - h), bw as u32, h as u32));

        draw_text(canvas, bx + bw / 2 - 3, y + mh + 4, 2, &format!("{}", i + 1), white);
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
