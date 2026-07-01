// src/frontend/input.rs
//
// Remappable key bindings: game buttons + emulator hotkeys. Data-driven (maps)
// so bindings can change at runtime / be loaded from a config file later, rather
// than being hardcoded in a match.

use sdl3::keyboard::Keycode;
use std::collections::HashMap;

/// A joypad target: a bit in the D-pad nibble or the action-button nibble.
#[derive(Clone, Copy)]
pub enum Pad {
    Dpad(u8),
    Btn(u8),
}

/// Emulator hotkeys (not game input). Some are wired up in later steps.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hotkey {
    Quit,
    Debug,
    Pause,
    QuickSave,
    QuickLoad,
    SaveFile,
    LoadRom,
    FastForward,
}

pub struct InputConfig {
    game: HashMap<Keycode, Pad>,
    hotkeys: HashMap<Keycode, Hotkey>,
}

impl Default for InputConfig {
    fn default() -> Self {
        let game = HashMap::from([
            (Keycode::Right, Pad::Dpad(0b0001)),
            (Keycode::Left, Pad::Dpad(0b0010)),
            (Keycode::Up, Pad::Dpad(0b0100)),
            (Keycode::Down, Pad::Dpad(0b1000)),
            (Keycode::Z, Pad::Btn(0b0001)),         // A
            (Keycode::X, Pad::Btn(0b0010)),         // B
            (Keycode::Backspace, Pad::Btn(0b0100)), // Select
            (Keycode::Return, Pad::Btn(0b1000)),    // Start
        ]);
        let hotkeys = HashMap::from([
            (Keycode::Escape, Hotkey::Quit),
            (Keycode::D, Hotkey::Debug),
            (Keycode::P, Hotkey::Pause),
            (Keycode::F5, Hotkey::QuickSave),
            (Keycode::F9, Hotkey::QuickLoad),
            (Keycode::F2, Hotkey::SaveFile),
            (Keycode::O, Hotkey::LoadRom),
            (Keycode::Space, Hotkey::FastForward),
        ]);
        Self { game, hotkeys }
    }
}

impl InputConfig {
    pub fn game(&self, k: Keycode) -> Option<Pad> {
        self.game.get(&k).copied()
    }
    pub fn hotkey(&self, k: Keycode) -> Option<Hotkey> {
        self.hotkeys.get(&k).copied()
    }
}
