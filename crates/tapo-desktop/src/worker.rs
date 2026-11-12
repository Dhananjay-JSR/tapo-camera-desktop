//! Background worker: owns the `Cloud`, runs every network call off the UI thread, and streams
//! live/playback/talk on their own threads. UI ↔ worker communicate over channels.

use crate::audio_out::Speaker;
use crate::decode::YuvFrame;
use crate::{store, stream};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread;
use tapo_core::{Camera, Cloud, Error, Session, TalkSession};

pub enum Cmd {
    Login {
        email: String,
        password: String,
    },
    Mfa(String),
    Cameras,
    Ptz {
        id: String,
        x: i32,
        y: i32,
    },
    Preset {
        id: String,
        preset: String,
    },
    Cruise {
        id: String,
        horizontal: bool,
    },
    CruiseStop(String),
    Presets(String),
    Toggle {
        id: String,
        what: Toggle,
        on: bool,
    },
    StartLive {
        id: String,
        frames: Sender<YuvFrame>,
    },
    StopLive(String),
    Days(String),
    Clips {
        id: String,
        date: String,
    },
    PlayClip {
        id: String,
        clip: tapo_core::Clip,
        frames: Sender<YuvFrame>,
    },
    Alerts(i64),
    StartTalk(String),
    StopTalk,
    Mic(Vec<u8>),
    Logout,
}

#[derive(Clone, Copy)]
pub enum Toggle {
    Led,
    Privacy,
    Alarm,
}

pub enum Evt {
    LoggedIn,
    LoggedOut,
    MfaRequired(String),
    Cameras(Vec<Camera>),
    Presets(String, Vec<(String, String)>),
    Days(String, Vec<String>),
    Clips(String, Vec<tapo_core::Clip>),
    Alerts(Vec<tapo_core::Alert>),
    Status(String),
    Error(String),
}

pub fn spawn(
    session: Session,
    evt: Sender<Evt>,
    repaint: impl Fn() + Send + 'static,
) -> Sender<Cmd> {
    let (tx, rx): (Sender<Cmd>, Receiver<Cmd>) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut cloud = Cloud::new(session).expect("tls client");
        let mut lives: HashMap<String, Arc<AtomicBool>> = HashMap::new();
        let mut talk: Option<TalkSession> = None;
        let send = |e: Evt| {
            let _ = evt.send(e);
            repaint();
        };
        if cloud.session.logged_in() {
            send(Evt::LoggedIn);
        }
        while let Ok(cmd) = rx.recv() {
            match cmd {
                Cmd::Login { email, password } => match cloud.login(&email, &password) {
                    Ok(()) => {
                        store::save(&cloud.session);
                        send(Evt::LoggedIn);
                    }
                    Err(Error::MfaRequired { email, .. }) => send(Evt::MfaRequired(email)),
                    Err(e) => send(Evt::Error(e.to_string())),
                },
                Cmd::Mfa(code) => match cloud.complete_mfa(&code) {
                    Ok(()) => {
                        store::save(&cloud.session);
                        send(Evt::LoggedIn);
                    }
                    Err(e) => send(Evt::Error(e.to_string())),
                },
                Cmd::Cameras => match cloud.cameras() {
                    Ok(c) => send(Evt::Cameras(c)),
                    Err(e) => send(Evt::Error(e.to_string())),
                },
                Cmd::Presets(id) => match cloud.presets(&id) {
                    Ok(p) => send(Evt::Presets(id, p)),
                    Err(e) => send(Evt::Error(e.to_string())),
                },
                Cmd::Days(id) => {
                    let c = cloud.clone();
                    let evt = evt.clone();
                    thread::spawn(move || {
                        let today = 20260101; // shortcut: wide range; narrow once a date picker exists.
                        match c.recording_days(
                            &id,
                            "20200101",
                            &format!("{}", today + 991231 - 10101),
                        ) {
                            Ok(d) => {
                                let _ = evt.send(Evt::Days(id, d));
                            }
                            Err(e) => {
                                let _ = evt.send(Evt::Error(e.to_string()));
                            }
                        }
                    });
                }
                Cmd::Clips { id, date } => {
                    let c = cloud.clone();
                    let evt = evt.clone();
                    thread::spawn(move || match c.clips(&id, &date) {
                        Ok(cl) => {
                            let _ = evt.send(Evt::Clips(id, cl));
                        }
                        Err(e) => {
                            let _ = evt.send(Evt::Error(e.to_string()));
                        }
                    });
                }
                Cmd::Alerts(since) => {
                    let c = cloud.clone();
                    let evt = evt.clone();
                    thread::spawn(move || match c.alerts(since, 0) {
                        Ok(a) => {
                            let _ = evt.send(Evt::Alerts(a));
                        }
                        Err(e) => {
                            let _ = evt.send(Evt::Error(e.to_string()));
                        }
                    });
                }
                Cmd::Ptz { id, x, y } => quick(&cloud, &evt, move |c| c.move_motor(&id, x, y)),
                Cmd::Preset { id, preset } => {
                    quick(&cloud, &evt, move |c| c.goto_preset(&id, &preset))
                }
                Cmd::Cruise { id, horizontal } => {
                    quick(&cloud, &evt, move |c| c.cruise(&id, horizontal))
                }
                Cmd::CruiseStop(id) => quick(&cloud, &evt, move |c| c.cruise_stop(&id)),
                Cmd::Toggle { id, what, on } => quick(&cloud, &evt, move |c| match what {
                    Toggle::Led => c.set_led(&id, on),
                    Toggle::Privacy => c.set_privacy(&id, on),
                    Toggle::Alarm => c.set_alarm(&id, on),
                }),
                Cmd::StartLive { id, frames } => {
                    if lives.contains_key(&id) {
                        continue;
                    }
                    let stop = Arc::new(AtomicBool::new(false));
                    lives.insert(id.clone(), stop.clone());
                    let c = cloud.clone();
                    let evt = evt.clone();
                    thread::spawn(move || {
                        let rate = c.mic_rate(&id).unwrap_or(16000);
                        match c.open_stream(&id, json!({"audio_config": {"encode_type": "G711ulaw", "sample_rate": "16"}})) {
                            Ok(parts) => {
                                if let Err(e) = stream::run(parts, frames, stop, Speaker::open(), rate) {
                                    let _ = evt.send(Evt::Error(e));
                                }
                            }
                            Err(e) => { let _ = evt.send(Evt::Error(e.to_string())); }
                        }
                    });
                }
                Cmd::StopLive(id) => {
                    if let Some(stop) = lives.remove(&id) {
                        stop.store(true, Ordering::Relaxed);
                    }
                }
                Cmd::PlayClip { id, clip, frames } => {
                    let c = cloud.clone();
                    let evt = evt.clone();
                    thread::spawn(move || {
                        let rate = c.mic_rate(&id).unwrap_or(16000);
                        match c.open_playback(&id, &clip) {
                            Ok(parts) => {
                                if let Err(e) = stream::run(
                                    parts,
                                    frames,
                                    Arc::new(AtomicBool::new(false)),
                                    Speaker::open(),
                                    rate,
                                ) {
                                    let _ = evt.send(Evt::Error(e));
                                }
                            }
                            Err(e) => {
                                let _ = evt.send(Evt::Error(e.to_string()));
                            }
                        }
                    });
                }
                Cmd::StartTalk(id) => match cloud.start_talk(&id) {
                    Ok(t) => {
                        talk = Some(t);
                        send(Evt::Status("Talk started".into()));
                    }
                    Err(e) => send(Evt::Error(e.to_string())),
                },
                Cmd::Mic(pcm) => {
                    if let Some(t) = &mut talk {
                        if t.send(&pcm).is_err() {
                            talk = None;
                        }
                    }
                }
                Cmd::StopTalk => talk = None,
                Cmd::Logout => {
                    for (_, stop) in lives.drain() {
                        stop.store(true, Ordering::Relaxed);
                    }
                    talk = None;
                    cloud.session.token = None;
                    cloud.session.wap_v2 = None;
                    cloud.session.cipc = None;
                    cloud.session.app_server = None;
                    store::clear();
                    send(Evt::LoggedOut);
                }
            }
        }
    });
    tx
}

/// Run a quick camera command on a worker clone so the UI never blocks.
fn quick(
    cloud: &Cloud,
    evt: &Sender<Evt>,
    f: impl FnOnce(&Cloud) -> tapo_core::Result<()> + Send + 'static,
) {
    let c = cloud.clone();
    let evt = evt.clone();
    thread::spawn(move || {
        if let Err(e) = f(&c) {
            let _ = evt.send(Evt::Error(e.to_string()));
        }
    });
}
