//! Background audio playback detector for music dancing reactions.

use std::{
    process::Command,
    sync::mpsc::{Receiver, channel},
    thread,
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicStatus {
    Playing,
    Stopped,
}

pub struct AudioMonitor {
    rx: Receiver<MusicStatus>,
}

impl AudioMonitor {
    #[must_use]
    pub fn spawn() -> Self {
        let (tx, rx) = channel();
        thread::Builder::new()
            .name("vpet-audio-monitor".to_owned())
            .spawn(move || {
                let mut prev = MusicStatus::Stopped;
                loop {
                    thread::sleep(Duration::from_millis(1500));
                    let current = if check_mpris_playing() {
                        MusicStatus::Playing
                    } else {
                        MusicStatus::Stopped
                    };
                    if current != prev {
                        let _ = tx.send(current);
                        prev = current;
                    }
                }
            })
            .expect("spawn audio monitor thread");
        Self { rx }
    }

    #[must_use]
    pub fn poll_status(&self) -> Option<MusicStatus> {
        let mut latest = None;
        while let Ok(st) = self.rx.try_recv() {
            latest = Some(st);
        }
        latest
    }
}

fn check_mpris_playing() -> bool {
    if let Ok(out) = Command::new("playerctl").args(["status", "-a"]).output()
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        return s.lines().any(|l| l.trim().eq_ignore_ascii_case("playing"));
    }
    false
}
