use rdev::{listen, simulate, Event, EventType, Key as RKey};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MacroEvent {
    pub time_us: u128,
    pub kind: MacroEventKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MacroEventKind {
    KeyPress(String),
    KeyRelease(String),
}

fn key_to_string(k: &RKey) -> String {
    format!("{:?}", k)
}

fn string_to_key(s: &str) -> Option<RKey> {
    use RKey::*;
    Some(match s {
        "Alt" => Alt,
        "AltGr" => AltGr,
        "Backspace" => Backspace,
        "CapsLock" => CapsLock,
        "ControlLeft" => ControlLeft,
        "ControlRight" => ControlRight,
        "Delete" => Delete,
        "DownArrow" => DownArrow,
        "End" => End,
        "Escape" => Escape,
        "F1" => F1,
        "F2" => F2,
        "F3" => F3,
        "F4" => F4,
        "F5" => F5,
        "F6" => F6,
        "F7" => F7,
        "F8" => F8,
        "F9" => F9,
        "F10" => F10,
        "F11" => F11,
        "F12" => F12,
        "Home" => Home,
        "LeftArrow" => LeftArrow,
        "MetaLeft" => MetaLeft,
        "MetaRight" => MetaRight,
        "PageDown" => PageDown,
        "PageUp" => PageUp,
        "Return" => Return,
        "RightArrow" => RightArrow,
        "ShiftLeft" => ShiftLeft,
        "ShiftRight" => ShiftRight,
        "Space" => Space,
        "Tab" => Tab,
        "UpArrow" => UpArrow,
        "KeyA" => KeyA,
        "KeyB" => KeyB,
        "KeyC" => KeyC,
        "KeyD" => KeyD,
        "KeyE" => KeyE,
        "KeyF" => KeyF,
        "KeyG" => KeyG,
        "KeyH" => KeyH,
        "KeyI" => KeyI,
        "KeyJ" => KeyJ,
        "KeyK" => KeyK,
        "KeyL" => KeyL,
        "KeyM" => KeyM,
        "KeyN" => KeyN,
        "KeyO" => KeyO,
        "KeyP" => KeyP,
        "KeyQ" => KeyQ,
        "KeyR" => KeyR,
        "KeyS" => KeyS,
        "KeyT" => KeyT,
        "KeyU" => KeyU,
        "KeyV" => KeyV,
        "KeyW" => KeyW,
        "KeyX" => KeyX,
        "KeyY" => KeyY,
        "KeyZ" => KeyZ,
        "Num0" => Num0,
        "Num1" => Num1,
        "Num2" => Num2,
        "Num3" => Num3,
        "Num4" => Num4,
        "Num5" => Num5,
        "Num6" => Num6,
        "Num7" => Num7,
        "Num8" => Num8,
        "Num9" => Num9,
        _ => return None,
    })
}

pub struct MacroRecorder {
    pub events: Arc<Mutex<Vec<MacroEvent>>>,
    pub recording: Arc<AtomicBool>,
    pub stop_key: String,
}

impl MacroRecorder {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            recording: Arc::new(AtomicBool::new(false)),
            stop_key: "F8".to_string(),
        }
    }
    pub fn start(&self) {
        if self.recording.load(Ordering::SeqCst) {
            return;
        }
        self.recording.store(true, Ordering::SeqCst);
        self.events.lock().unwrap().clear();

        let events = self.events.clone();
        let recording = self.recording.clone();
        let stop_key = self.stop_key.clone();

        thread::spawn(move || {
            let start = Instant::now();
            let recording_inner = recording.clone();

            let _ = listen(move |event: Event| {
                if !recording_inner.load(Ordering::SeqCst) {
                    return;
                }

                let t = start.elapsed().as_micros();
                match event.event_type {
                    EventType::KeyPress(k) => {
                        let name = key_to_string(&k);
                        if name == stop_key {
                            recording_inner.store(false, Ordering::SeqCst);
                            return;
                        }
                        events.lock().unwrap().push(MacroEvent {
                            time_us: t,
                            kind: MacroEventKind::KeyPress(name),
                        });
                    }
                    EventType::KeyRelease(k) => {
                        let name = key_to_string(&k);
                        if name == stop_key {
                            return;
                        }
                        events.lock().unwrap().push(MacroEvent {
                            time_us: t,
                            kind: MacroEventKind::KeyRelease(name),
                        });
                    }
                    _ => {}
                }
            });
        });
    }

    pub fn stop(&self) {
        self.recording.store(false, Ordering::SeqCst);
    }

    pub fn play(&self, speed: f32) {
        let events = self.events.lock().unwrap().clone();
        if events.is_empty() {
            return;
        }
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            let mut last_t = 0u128;
            for ev in events {
                let dt = ev.time_us.saturating_sub(last_t);
                last_t = ev.time_us;
                let scaled = (dt as f32 / speed.max(0.01)) as u64;
                if scaled > 0 {
                    thread::sleep(Duration::from_micros(scaled));
                }
                match ev.kind {
                    MacroEventKind::KeyPress(name) => {
                        if let Some(k) = string_to_key(&name) {
                            let _ = simulate(&EventType::KeyPress(k));
                        }
                    }
                    MacroEventKind::KeyRelease(name) => {
                        if let Some(k) = string_to_key(&name) {
                            let _ = simulate(&EventType::KeyRelease(k));
                        }
                    }
                }
            }
        });
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let events = self.events.lock().unwrap().clone();
        let json = serde_json::to_string_pretty(&events)?;
        std::fs::write(path, json)
    }

    pub fn load(&self, path: &std::path::Path) -> std::io::Result<()> {
        let data = std::fs::read_to_string(path)?;
        let evs: Vec<MacroEvent> = serde_json::from_str(&data)?;
        *self.events.lock().unwrap() = evs;
        Ok(())
    }
}
