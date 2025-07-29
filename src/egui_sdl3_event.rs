/// Translates `sdl3` events to `egui` events.

type SEv = sdl3::event::Event;
type EEv = egui::Event;
type SKey = sdl3::keyboard::Keycode;
type SScan = sdl3::keyboard::Scancode;
type EKey = egui::Key;

pub fn sdl3_to_egui_event(event: SEv, modifiers: &egui::Modifiers) -> Option<EEv> {
    fn mouse_button(
        mouse_btn: sdl3::mouse::MouseButton,
        x: f32,
        y: f32,
        pressed: bool,
        modifiers: &egui::Modifiers,
    ) -> Option<egui::Event> {
        let button = (|mouse_btn| {
            let button = match mouse_btn {
                sdl3::mouse::MouseButton::Left => egui::PointerButton::Primary,
                sdl3::mouse::MouseButton::Middle => egui::PointerButton::Middle,
                sdl3::mouse::MouseButton::Right => egui::PointerButton::Secondary,
                sdl3::mouse::MouseButton::X1 => egui::PointerButton::Extra1,
                sdl3::mouse::MouseButton::X2 => egui::PointerButton::Extra2,
                _ => None?,
            };
            Some(button)
        })(mouse_btn);

        button.map(|button| EEv::PointerButton {
            pos: egui::Pos2::new(x, y),
            button,
            pressed,
            modifiers: *modifiers,
        })
    }

    fn key(
        keycode: Option<SKey>,
        scancode: Option<SScan>,
        keymod: sdl3::keyboard::Mod,
        repeat: bool,
        pressed: bool,
    ) -> Option<egui::Event> {
        let physical_key = scancode.and_then(|k| sdl3_to_egui_scancode(k));

        let key = keycode
            .and_then(|k| sdl3_to_egui_keycode(k))
            .or(physical_key);

        key.map(|key| EEv::Key {
            key,
            physical_key,
            pressed,
            repeat,
            modifiers: sdl3_to_egui_modifiers(keymod),
        })
    }

    match event {
        SEv::KeyDown {
            timestamp: _,
            window_id: _,
            keycode,
            scancode,
            keymod,
            repeat,
            which: _,
            raw: _,
        } => key(keycode, scancode, keymod, repeat, true),
        SEv::KeyUp {
            timestamp: _,
            window_id: _,
            keycode,
            scancode,
            keymod,
            repeat,
            which: _,
            raw: _,
        } => key(keycode, scancode, keymod, repeat, false),
        SEv::MouseMotion {
            timestamp: _,
            window_id: _,
            which: _,
            mousestate: _,
            x,
            y,
            xrel: _,
            yrel: _,
        } => Some(EEv::PointerMoved(egui::Pos2::new(x, y))),
        SEv::TextInput {
            timestamp: _,
            window_id: _,
            text,
        } => Some(EEv::Text(text)),
        SEv::MouseWheel {
            timestamp: _,
            window_id: _,
            which: _,
            x,
            y,
            direction: _,
            mouse_x: _,
            mouse_y: _,
        } => Some(EEv::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::Vec2::new(x, y),
            modifiers: *modifiers,
        }),
        SEv::MouseButtonDown {
            timestamp: _,
            window_id: _,
            which: _,
            mouse_btn,
            clicks: _,
            x,
            y,
        } => mouse_button(mouse_btn, x, y, true, modifiers),
        SEv::MouseButtonUp {
            timestamp: _,
            window_id: _,
            which: _,
            mouse_btn,
            clicks: _,
            x,
            y,
        } => mouse_button(mouse_btn, x, y, false, modifiers),
        SEv::Window {
            timestamp: _,
            window_id: _,
            win_event,
        } => (|| {
            use sdl3::event::WindowEvent as SWEv;
            let event = match win_event {
                SWEv::MouseLeave => EEv::PointerGone,
                SWEv::FocusGained => EEv::WindowFocused(true),
                SWEv::FocusLost => EEv::WindowFocused(false),
                _ => None?,
            };
            Some(event)
        })(),
        _ => None,
    }
}

fn sdl3_to_egui_keycode(keycode: SKey) -> Option<EKey> {
    let key = match keycode {
        SKey::Return => EKey::Enter,
        SKey::Escape => EKey::Escape,
        SKey::Backspace => EKey::Backspace,
        SKey::Tab => EKey::Tab,
        SKey::Space => EKey::Space,
        SKey::Exclaim => EKey::Exclamationmark,
        SKey::Apostrophe => EKey::Quote,
        SKey::Plus => EKey::Plus,
        SKey::Comma => EKey::Comma,
        SKey::Minus => EKey::Minus,
        SKey::Period => EKey::Period,
        SKey::Slash => EKey::Slash,
        SKey::_0 => EKey::Num0,
        SKey::_1 => EKey::Num1,
        SKey::_2 => EKey::Num2,
        SKey::_3 => EKey::Num3,
        SKey::_4 => EKey::Num4,
        SKey::_5 => EKey::Num5,
        SKey::_6 => EKey::Num6,
        SKey::_7 => EKey::Num7,
        SKey::_8 => EKey::Num8,
        SKey::_9 => EKey::Num9,
        SKey::Colon => EKey::Colon,
        SKey::Semicolon => EKey::Semicolon,
        SKey::Equals => EKey::Equals,
        SKey::Question => EKey::Questionmark,
        SKey::LeftBracket => EKey::OpenBracket,
        SKey::Backslash => EKey::Backslash,
        SKey::RightBracket => EKey::CloseBracket,
        SKey::Grave => EKey::Backtick,
        SKey::A => EKey::A,
        SKey::B => EKey::B,
        SKey::C => EKey::C,
        SKey::D => EKey::D,
        SKey::E => EKey::E,
        SKey::F => EKey::F,
        SKey::G => EKey::G,
        SKey::H => EKey::H,
        SKey::I => EKey::I,
        SKey::J => EKey::J,
        SKey::K => EKey::K,
        SKey::L => EKey::L,
        SKey::M => EKey::M,
        SKey::N => EKey::N,
        SKey::O => EKey::O,
        SKey::P => EKey::P,
        SKey::Q => EKey::Q,
        SKey::R => EKey::R,
        SKey::S => EKey::S,
        SKey::T => EKey::T,
        SKey::U => EKey::U,
        SKey::V => EKey::V,
        SKey::W => EKey::W,
        SKey::X => EKey::X,
        SKey::Y => EKey::Y,
        SKey::Z => EKey::Z,
        SKey::LeftBrace => EKey::OpenCurlyBracket,
        SKey::Pipe => EKey::Pipe,
        SKey::RightBrace => EKey::CloseCurlyBracket,
        SKey::Delete => EKey::Delete,
        SKey::F1 => EKey::F1,
        SKey::F2 => EKey::F2,
        SKey::F3 => EKey::F3,
        SKey::F4 => EKey::F4,
        SKey::F5 => EKey::F5,
        SKey::F6 => EKey::F6,
        SKey::F7 => EKey::F7,
        SKey::F8 => EKey::F8,
        SKey::F9 => EKey::F9,
        SKey::F10 => EKey::F10,
        SKey::F11 => EKey::F11,
        SKey::F12 => EKey::F12,
        SKey::Insert => EKey::Insert,
        SKey::Home => EKey::Home,
        SKey::PageUp => EKey::PageUp,
        SKey::End => EKey::End,
        SKey::PageDown => EKey::PageDown,
        SKey::Right => EKey::ArrowRight,
        SKey::Left => EKey::ArrowLeft,
        SKey::Down => EKey::ArrowDown,
        SKey::Up => EKey::ArrowUp,
        SKey::F13 => EKey::F13,
        SKey::F14 => EKey::F14,
        SKey::F15 => EKey::F15,
        SKey::F16 => EKey::F16,
        SKey::F17 => EKey::F17,
        SKey::F18 => EKey::F18,
        SKey::F19 => EKey::F19,
        SKey::F20 => EKey::F20,
        SKey::F21 => EKey::F21,
        SKey::F22 => EKey::F22,
        SKey::F23 => EKey::F23,
        SKey::F24 => EKey::F24,
        SKey::Cut => EKey::Cut,
        SKey::Copy => EKey::Copy,
        SKey::Paste => EKey::Paste,
        _ => None?,
    };

    Some(key)
}

pub fn sdl3_to_egui_modifiers(modifiers: sdl3::keyboard::Mod) -> egui::Modifiers {
    let mut ret = egui::Modifiers::NONE;

    if modifiers.intersects(sdl3::keyboard::Mod::LCTRLMOD | sdl3::keyboard::Mod::RCTRLMOD) {
        ret |= egui::Modifiers::COMMAND
    }

    if modifiers.intersects(sdl3::keyboard::Mod::LALTMOD | sdl3::keyboard::Mod::RALTMOD) {
        ret |= egui::Modifiers::ALT
    }

    if modifiers.intersects(sdl3::keyboard::Mod::LSHIFTMOD | sdl3::keyboard::Mod::RSHIFTMOD) {
        ret |= egui::Modifiers::SHIFT;
    }

    ret
}

fn sdl3_to_egui_scancode(scancode: SScan) -> Option<EKey> {
    let key = match scancode {
        SScan::A => EKey::A,
        SScan::B => EKey::B,
        SScan::C => EKey::C,
        SScan::D => EKey::D,
        SScan::E => EKey::E,
        SScan::F => EKey::F,
        SScan::G => EKey::G,
        SScan::H => EKey::H,
        SScan::I => EKey::I,
        SScan::J => EKey::J,
        SScan::K => EKey::K,
        SScan::L => EKey::L,
        SScan::M => EKey::M,
        SScan::N => EKey::N,
        SScan::O => EKey::O,
        SScan::P => EKey::P,
        SScan::Q => EKey::Q,
        SScan::R => EKey::R,
        SScan::S => EKey::S,
        SScan::T => EKey::T,
        SScan::U => EKey::U,
        SScan::V => EKey::V,
        SScan::W => EKey::W,
        SScan::X => EKey::X,
        SScan::Y => EKey::Y,
        SScan::Z => EKey::Z,
        SScan::_1 => EKey::Num0,
        SScan::_2 => EKey::Num1,
        SScan::_3 => EKey::Num2,
        SScan::_4 => EKey::Num3,
        SScan::_5 => EKey::Num4,
        SScan::_6 => EKey::Num5,
        SScan::_7 => EKey::Num6,
        SScan::_8 => EKey::Num7,
        SScan::_9 => EKey::Num8,
        SScan::_0 => EKey::Num9,
        SScan::Return => EKey::Enter,
        SScan::Escape => EKey::Escape,
        SScan::Backspace => EKey::Backspace,
        SScan::Tab => EKey::Tab,
        SScan::Space => EKey::Space,
        SScan::Minus => EKey::Minus,
        SScan::Equals => EKey::Equals,
        SScan::LeftBracket => EKey::OpenBracket,
        SScan::RightBracket => EKey::CloseBracket,
        SScan::Backslash => EKey::Backslash,
        SScan::Semicolon => EKey::Semicolon,
        SScan::Apostrophe => EKey::Quote,
        SScan::Grave => EKey::Backtick,
        SScan::Comma => EKey::Comma,
        SScan::Period => EKey::Period,
        SScan::Slash => EKey::Slash,
        SScan::F1 => EKey::F1,
        SScan::F2 => EKey::F2,
        SScan::F3 => EKey::F3,
        SScan::F4 => EKey::F4,
        SScan::F5 => EKey::F5,
        SScan::F6 => EKey::F6,
        SScan::F7 => EKey::F7,
        SScan::F8 => EKey::F8,
        SScan::F9 => EKey::F9,
        SScan::F10 => EKey::F10,
        SScan::F11 => EKey::F11,
        SScan::F12 => EKey::F12,
        SScan::Insert => EKey::Insert,
        SScan::Home => EKey::Home,
        SScan::PageUp => EKey::PageUp,
        SScan::Delete => EKey::Delete,
        SScan::End => EKey::End,
        SScan::PageDown => EKey::PageDown,
        SScan::Right => EKey::ArrowRight,
        SScan::Left => EKey::ArrowLeft,
        SScan::Down => EKey::ArrowDown,
        SScan::Up => EKey::ArrowUp,
        SScan::F13 => EKey::F13,
        SScan::F14 => EKey::F14,
        SScan::F15 => EKey::F15,
        SScan::F16 => EKey::F16,
        SScan::F17 => EKey::F17,
        SScan::F18 => EKey::F18,
        SScan::F19 => EKey::F19,
        SScan::F20 => EKey::F20,
        SScan::F21 => EKey::F21,
        SScan::F22 => EKey::F22,
        SScan::F23 => EKey::F23,
        SScan::F24 => EKey::F24,
        SScan::Cut => EKey::Cut,
        SScan::Copy => EKey::Copy,
        SScan::Paste => EKey::Paste,
        _ => None?,
    };

    Some(key)
}
