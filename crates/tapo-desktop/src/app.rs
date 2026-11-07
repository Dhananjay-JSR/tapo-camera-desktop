//! egui UI. All network work is delegated to the worker thread; the UI only sends `Cmd`s and
//! renders `Evt`s, so it never blocks.

use crate::decode::YuvFrame;
use crate::store;
use crate::worker::{self, Cmd, Evt, Toggle};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tapo_core::{Alert, Camera, Clip};

#[derive(PartialEq)]
enum Auth {
    Out,
    Mfa,
    In,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Live,
    Playback,
    Settings,
    Log,
}

pub struct App {
    tx: Sender<Cmd>,
    rx: Receiver<Evt>,
    auth: Auth,
    email: String,
    password: String,
    mfa_code: String,
    status: String,
    cams: Vec<Camera>,
    selected: Option<String>,
    live_id: Option<String>,
    presets: HashMap<String, Vec<(String, String)>>,
    tab: Tab,
    days: Vec<String>,
    clips: Vec<Clip>,
    sel_day: Option<String>,
    alerts: Vec<Alert>,
    notif_on: bool,
    last_poll: Instant,
    poll_since: i64,
    talk_on: bool,
    mic: Option<crate::audio_in::Mic>,
    frame_tx: Sender<YuvFrame>,
    frame_rx: Receiver<YuvFrame>,
    video: crate::video_gl::GpuVideo,
    aspect: f32,
    playing: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (evt_tx, rx) = std::sync::mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        let tx = worker::spawn(store::load(), evt_tx, move || ctx.request_repaint());
        let (frame_tx, frame_rx) = std::sync::mpsc::channel();
        App {
            tx,
            rx,
            auth: Auth::Out,
            email: String::new(),
            password: String::new(),
            mfa_code: String::new(),
            status: String::new(),
            cams: Vec::new(),
            selected: None,
            live_id: None,
            presets: HashMap::new(),
            tab: Tab::Live,
            days: Vec::new(),
            clips: Vec::new(),
            sel_day: None,
            alerts: Vec::new(),
            notif_on: false,
            last_poll: Instant::now(),
            poll_since: now_ms(),
            talk_on: false,
            mic: None,
            frame_tx,
            frame_rx,
            video: crate::video_gl::GpuVideo::new(),
            aspect: 9.0 / 16.0,
            playing: false,
        }
    }

    fn drain(&mut self) {
        while let Ok(e) = self.rx.try_recv() {
            match e {
                Evt::LoggedOut => {
                    self.auth = Auth::Out;
                    self.cams.clear();
                    self.selected = None;
                    self.video = crate::video_gl::GpuVideo::new();
                    self.playing = false;
                    self.live_id = None;
                    self.presets.clear();
                    self.status.clear();
                    self.email.clear();
                    self.password.clear();
                }
                Evt::LoggedIn => {
                    self.auth = Auth::In;
                    self.password.clear();
                    let _ = self.tx.send(Cmd::Cameras);
                }
                Evt::MfaRequired(email) => {
                    self.auth = Auth::Mfa;
                    self.status = format!("Verification code sent to {email}");
                }
                Evt::Cameras(c) => {
                    if self.selected.is_none() {
                        self.selected = c.first().map(|x| x.id.clone());
                    }
                    self.cams = c;
                }
                Evt::Presets(id, p) => {
                    self.presets.insert(id, p);
                }
                Evt::Days(id, d) => {
                    if self.selected.as_deref() == Some(&id) {
                        self.days = d;
                    }
                }
                Evt::Clips(id, c) => {
                    if self.selected.as_deref() == Some(&id) {
                        self.clips = c;
                    }
                }
                Evt::Alerts(a) => {
                    if let Some(last) = a.last() {
                        self.poll_since = last.time_ms + 1;
                    }
                    for al in &a {
                        let line = format!("{}: {}", al.device, al.text);
                        self.push_log(&line);
                        self.status = line;
                    }
                    self.alerts.splice(0..0, a);
                    self.alerts.truncate(200);
                }
                Evt::Status(s) | Evt::Error(s) => {
                    self.push_log(&s);
                    self.status = s;
                }
            }
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain();

        // Upload the latest decoded frame (drop older ones to stay live).
        let mut latest = None;
        while let Ok(f) = self.frame_rx.try_recv() {
            latest = Some(f);
        }
        if let Some(f) = latest {
            self.aspect = f.h as f32 / f.w as f32;
            self.video.set_frame(f);
        }
        if self.playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }

        if self.notif_on && self.last_poll.elapsed().as_secs() >= 15 {
            self.last_poll = Instant::now();
            let _ = self.tx.send(Cmd::Alerts(self.poll_since));
            ctx.request_repaint_after(std::time::Duration::from_secs(15));
        }

        if self.auth != Auth::In {
            self.login_ui(ctx);
            return;
        }
        self.main_ui(ctx);
    }
}

impl App {
    fn login_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(140.0);
                if self.auth == Auth::Mfa {
                    ui.label(&self.status);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.mfa_code)
                            .hint_text("Verification code"),
                    );
                    if ui.button("Verify").clicked() && !self.mfa_code.is_empty() {
                        let _ = self.tx.send(Cmd::Mfa(self.mfa_code.clone()));
                    }
                } else {
                    ui.add(egui::TextEdit::singleline(&mut self.email).hint_text("TP-Link email"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.password)
                            .password(true)
                            .hint_text("Password"),
                    );
                    if ui.button("Log in").clicked() && !self.email.is_empty() {
                        self.status = "Signing in…".into();
                        let _ = self.tx.send(Cmd::Login {
                            email: self.email.clone(),
                            password: self.password.clone(),
                        });
                    }
                    if !self.status.is_empty() {
                        ui.colored_label(egui::Color32::LIGHT_RED, &self.status);
                    }
                }
            });
        });
    }

    fn main_ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Live, "Live");
                ui.selectable_value(&mut self.tab, Tab::Playback, "Playback");
                ui.selectable_value(&mut self.tab, Tab::Settings, "Settings");
                ui.selectable_value(&mut self.tab, Tab::Log, "Log");
                ui.separator();
                if ui.checkbox(&mut self.notif_on, "Notifications").changed() && self.notif_on {
                    self.poll_since = now_ms();
                    let _ = self.tx.send(Cmd::Alerts(self.poll_since));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Log out").clicked() {
                        let _ = self.tx.send(Cmd::Logout);
                    }
                });
            });
        });
        egui::SidePanel::left("cams")
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Cameras");
                for c in &self.cams.clone() {
                    let dot = if c.online { "🟢" } else { "⚪" };
                    if ui
                        .selectable_label(
                            self.selected.as_deref() == Some(&c.id),
                            format!("{dot} {}", c.name),
                        )
                        .clicked()
                    {
                        self.selected = Some(c.id.clone());
                        let _ = self.tx.send(Cmd::Presets(c.id.clone()));
                    }
                }
            });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });
        egui::CentralPanel::default().show(ctx, |ui| match self.tab {
            Tab::Live => self.live_ui(ui),
            Tab::Playback => self.playback_ui(ui),
            Tab::Settings => self.settings_ui(ui),
            Tab::Log => self.log_ui(ui),
        });
    }

    fn live_ui(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected.clone() else {
            ui.label("Select a camera.");
            return;
        };
        let name = self
            .cams
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        ui.heading(&name);
        let streaming = self.live_id.as_deref() == Some(id.as_str());
        ui.horizontal(|ui| {
            if !streaming {
                if ui.button("▶ Live view").clicked() {
                    // Only one live stream at a time: stop the previous so they don't overlap.
                    if let Some(prev) = self.live_id.take() {
                        let _ = self.tx.send(Cmd::StopLive(prev));
                    }
                    self.live_id = Some(id.clone());
                    self.video = crate::video_gl::GpuVideo::new();
                    self.playing = true;
                    let _ = self.tx.send(Cmd::StartLive {
                        id: id.clone(),
                        frames: self.frame_tx.clone(),
                    });
                }
            } else if ui.button("⏹ Stop").clicked() {
                self.live_id = None;
                self.playing = false;
                self.video = crate::video_gl::GpuVideo::new();
                let _ = self.tx.send(Cmd::StopLive(id.clone()));
            }
        });
        if self.video.has_frame() {
            let w = ui.available_width().min(960.0);
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(w, w * self.aspect), egui::Sense::hover());
            self.video.paint(ui, rect);
        } else if streaming {
            ui.label("Connecting… (waiting for the first keyframe)");
        }
        ui.add_space(8.0);
        if ui
            .checkbox(&mut self.talk_on, "🎤 Talk (hold the conversation open)")
            .changed()
        {
            if self.talk_on {
                let _ = self.tx.send(Cmd::StartTalk(id.clone()));
                self.mic = crate::audio_in::start(self.tx.clone());
                if self.mic.is_none() {
                    self.status = "No microphone available".into();
                    self.talk_on = false;
                    let _ = self.tx.send(Cmd::StopTalk);
                }
            } else {
                self.mic = None;
                let _ = self.tx.send(Cmd::StopTalk);
            }
        }
        ui.add_space(12.0);

        ui.label("Pan / tilt");
        ui.horizontal(|ui| {
            ui.add_space(30.0);
            if ui.button("⬆").clicked() {
                let _ = self.tx.send(Cmd::Ptz {
                    id: id.clone(),
                    x: 0,
                    y: 20,
                });
            }
        });
        ui.horizontal(|ui| {
            if ui.button("⬅").clicked() {
                let _ = self.tx.send(Cmd::Ptz {
                    id: id.clone(),
                    x: -20,
                    y: 0,
                });
            }
            if ui.button("➡").clicked() {
                let _ = self.tx.send(Cmd::Ptz {
                    id: id.clone(),
                    x: 20,
                    y: 0,
                });
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(30.0);
            if ui.button("⬇").clicked() {
                let _ = self.tx.send(Cmd::Ptz {
                    id: id.clone(),
                    x: 0,
                    y: -20,
                });
            }
        });

        if let Some(presets) = self.presets.get(&id).cloned() {
            ui.add_space(8.0);
            ui.label("Presets");
            ui.horizontal_wrapped(|ui| {
                for (pid, pname) in presets {
                    if ui.button(pname).clicked() {
                        let _ = self.tx.send(Cmd::Preset {
                            id: id.clone(),
                            preset: pid,
                        });
                    }
                }
            });
        }
    }

    fn playback_ui(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected.clone() else {
            ui.label("Select a camera.");
            return;
        };
        if ui.button("Load recording days").clicked() {
            let _ = self.tx.send(Cmd::Days(id.clone()));
        }
        ui.horizontal_wrapped(|ui| {
            for d in &self.days.clone() {
                if ui
                    .selectable_label(self.sel_day.as_deref() == Some(d), d)
                    .clicked()
                {
                    self.sel_day = Some(d.clone());
                    let _ = self.tx.send(Cmd::Clips {
                        id: id.clone(),
                        date: d.clone(),
                    });
                }
            }
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            for c in &self.clips.clone() {
                if ui.button(format!("▶ {}", fmt_time(c.start))).clicked() {
                    self.video = crate::video_gl::GpuVideo::new();
                    self.playing = true;
                    let _ = self.tx.send(Cmd::PlayClip {
                        id: id.clone(),
                        clip: c.clone(),
                        frames: self.frame_tx.clone(),
                    });
                }
            }
        });
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected.clone() else {
            ui.label("Select a camera.");
            return;
        };
        ui.heading("Camera controls");
        ui.horizontal(|ui| {
            if ui.button("LED on").clicked() {
                self.toggle(&id, Toggle::Led, true);
            }
            if ui.button("LED off").clicked() {
                self.toggle(&id, Toggle::Led, false);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Privacy on").clicked() {
                self.toggle(&id, Toggle::Privacy, true);
            }
            if ui.button("Privacy off").clicked() {
                self.toggle(&id, Toggle::Privacy, false);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Arm alarm").clicked() {
                self.toggle(&id, Toggle::Alarm, true);
            }
            if ui.button("Disarm alarm").clicked() {
                self.toggle(&id, Toggle::Alarm, false);
            }
        });
        ui.add_space(16.0);
        ui.heading("Recent alerts");
        egui::ScrollArea::vertical().show(ui, |ui| {
            for a in &self.alerts {
                ui.label(format!(
                    "{} · {} · {}",
                    fmt_time(a.time_ms / 1000),
                    a.device,
                    a.text
                ));
            }
        });
    }

    fn toggle(&self, id: &str, what: Toggle, on: bool) {
        let _ = self.tx.send(Cmd::Toggle {
            id: id.to_string(),
            what,
            on,
        });
    }
}

impl App {
    fn push_log(&self, msg: &str) {
        crate::logbuf::log(msg);
    }

    fn log_ui(&mut self, ui: &mut egui::Ui) {
        let lines = crate::logbuf::snapshot();
        ui.horizontal(|ui| {
            ui.heading("Log");
            if ui.button("Copy all").clicked() {
                ui.output_mut(|o| o.copied_text = lines.join("\n"));
            }
            if ui.button("Open log file").clicked() {
                crate::logbuf::open_file();
            }
            if ui.button("Clear").clicked() {
                crate::logbuf::clear();
            }
        });
        if let Some(p) = crate::logbuf::path() {
            ui.weak(format!("Log file: {}", p.display()));
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &lines {
                    ui.monospace(line);
                }
            });
    }
}

fn fmt_time(epoch_secs: i64) -> String {
    // minimal local-ish HH:MM:SS from epoch seconds without pulling a date crate.
    let s = epoch_secs.rem_euclid(86400);
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}
