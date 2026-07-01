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
    LoadFile,
    LoadRom,
    FastForward,
    SpeedDown,
    SpeedUp,
    /// Debug: toggle mute of APU channel 0-3.
    Mute(u8),
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
        // Avoid the function-row keys: macOS hijacks F1-F12 for system functions
        // and SDL often never sees them. Use the number row + letters instead.
        let hotkeys = HashMap::from([
            (Keycode::Escape, Hotkey::Quit),
            (Keycode::D, Hotkey::Debug),
            (Keycode::P, Hotkey::Pause),
            (Keycode::_1, Hotkey::QuickSave),
            (Keycode::_2, Hotkey::QuickLoad),
            (Keycode::_3, Hotkey::SaveFile),
            (Keycode::_4, Hotkey::LoadFile),
            (Keycode::O, Hotkey::LoadRom),
            (Keycode::Space, Hotkey::FastForward),
            (Keycode::Minus, Hotkey::SpeedDown),
            (Keycode::Equals, Hotkey::SpeedUp),
            // Debug: mute APU channels 1-4.
            (Keycode::_5, Hotkey::Mute(0)),
            (Keycode::_6, Hotkey::Mute(1)),
            (Keycode::_7, Hotkey::Mute(2)),
            (Keycode::_8, Hotkey::Mute(3)),
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
