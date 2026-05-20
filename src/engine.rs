use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum Command {
    Line(String),
    StartDelay(u64),
    SetDelay(u64),
    SetWriteDelay(u64),
    Key(String, KeyMode),
    Type(String, u64),
    Pause(u64),
    Comment,
}

#[derive(Clone, Debug)]
pub enum KeyMode {
    Click,
    Press,
    Release,
}

pub struct EngineConfig {
    pub start_delay_ms: u64,
    pub line_delay_ms: u64,
    pub write_delay_ms: u64,
    pub repeat: bool,
    pub press_enter: bool,
}

pub fn parse_script(text: &str) -> Vec<Command> {
    text.lines().map(parse_line).collect()
}

fn parse_line(line: &str) -> Command {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return Command::Line(line.to_string());
    }

    // script directives
    let body = trimmed.trim_start_matches('#').trim();
    let mut parts = body.split_whitespace();
    let cmd = match parts.next() {
        Some(c) => c,
        None => return Command::Comment,
    };

    match cmd.to_ascii_uppercase().as_str() {
        "MANUAL_SLEEP" => {
            let ms = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            Command::StartDelay(ms)
        }
        "MANUAL_DELAY" => {
            let ms = parts.next().and_then(|s| s.parse().ok()).unwrap_or(100);
            Command::SetDelay(ms)
        }
        "MANUAL_WRITE_DELAY" => {
            let ms = parts.next().and_then(|s| s.parse().ok()).unwrap_or(20);
            Command::SetWriteDelay(ms)
        }
        "KEY" => {
            let key = parts.next().unwrap_or("").to_string();
            let mode = match parts.next() {
                Some("press") => KeyMode::Press,
                Some("release") => KeyMode::Release,
                _ => KeyMode::Click,
            };
            Command::Key(key, mode)
        }
        "TYPE" => {
            // #type "text" delay
            if let Some(start) = body.find('"') {
                if let Some(end) = body[start + 1..].find('"') {
                    let text = body[start + 1..start + 1 + end].to_string();
                    let rest = &body[start + 1 + end + 1..];
                    let delay = rest
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(10);
                    return Command::Type(text, delay);
                }
            }
            Command::Comment
        }
        "PAUSE" => {
            let ms = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            Command::Pause(ms)
        }
        _ => Command::Comment,
    }
}

fn parse_key_name(name: &str) -> Option<Key> {
    let n = name.to_ascii_lowercase();
    match n.as_str() {
        "enter" | "return" => Some(Key::Return),
        "tab" => Some(Key::Tab),
        "space" => Some(Key::Space),
        "esc" | "escape" => Some(Key::Escape),
        "backspace" => Some(Key::Backspace),
        "delete" | "del" => Some(Key::Delete),
        "up" => Some(Key::UpArrow),
        "down" => Some(Key::DownArrow),
        "left" => Some(Key::LeftArrow),
        "right" => Some(Key::RightArrow),
        "shift" => Some(Key::Shift),
        "ctrl" | "control" => Some(Key::Control),
        "alt" => Some(Key::Alt),
        "home" => Some(Key::Home),
        "end" => Some(Key::End),
        "pageup" => Some(Key::PageUp),
        "pagedown" => Some(Key::PageDown),
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        _ => {
            // single smbl
            let mut chars = n.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                Some(Key::Unicode(c))
            } else {
                None
            }
        }
    }
}

fn write_with_delay(enigo: &mut Enigo, text: &str, per_char_ms: u64, running: &Arc<AtomicBool>) {
    for ch in text.chars() {
        if !running.load(Ordering::SeqCst) {
            return;
        }
        let s = ch.to_string();
        let _ = enigo.text(&s);
        if per_char_ms > 0 {
            thread::sleep(Duration::from_millis(per_char_ms));
        }
    }
}
// threaded
pub fn run(
    commands: Vec<Command>,
    mut cfg: EngineConfig,
    running: Arc<AtomicBool>,
    progress: Arc<std::sync::Mutex<(usize, usize)>>,
) {
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(_) => {
            running.store(false, Ordering::SeqCst);
            return;
        }
    };

    let total_start = cfg.start_delay_ms;
    let step = 50u64;
    let mut waited = 0u64;
    while waited < total_start && running.load(Ordering::SeqCst) {
        let s = step.min(total_start - waited);
        thread::sleep(Duration::from_millis(s));
        waited += s;
    }

    let total = commands.len();

    loop {
        for (i, cmd) in commands.iter().enumerate() {
            if !running.load(Ordering::SeqCst) {
                return;
            }

            if let Ok(mut p) = progress.lock() {
                *p = (i + 1, total);
            }

            match cmd {
                Command::Line(s) if !s.is_empty() => {
                    if cfg.press_enter {
                        let _ = enigo.key(Key::Return, Direction::Click);
                    }
                    write_with_delay(&mut enigo, s, cfg.write_delay_ms, &running);
                    if cfg.press_enter {
                        let _ = enigo.key(Key::Return, Direction::Click);
                    }
                    thread::sleep(Duration::from_millis(cfg.line_delay_ms));
                }
                Command::Line(_) => {
                    thread::sleep(Duration::from_millis(cfg.line_delay_ms));
                }
                Command::SetDelay(ms) => cfg.line_delay_ms = *ms,
                Command::SetWriteDelay(ms) => cfg.write_delay_ms = *ms,
                Command::Key(name, mode) => {
                    if let Some(k) = parse_key_name(name) {
                        let dir = match mode {
                            KeyMode::Click => Direction::Click,
                            KeyMode::Press => Direction::Press,
                            KeyMode::Release => Direction::Release,
                        };
                        let _ = enigo.key(k, dir);
                    }
                }
                Command::Type(text, delay) => {
                    write_with_delay(&mut enigo, text, *delay, &running);
                }
                Command::Pause(ms) => {
                    let mut w = 0u64;
                    while w < *ms && running.load(Ordering::SeqCst) {
                        let s = step.min(*ms - w);
                        thread::sleep(Duration::from_millis(s));
                        w += s;
                    }
                }
                Command::StartDelay(_) | Command::Comment => {}
            }
        }

        if !cfg.repeat {
            break;
        }
    }

    running.store(false, Ordering::SeqCst);
}
