//! StatusNotifierItem (`ksni`) system tray menu for waybar.

use calloop::channel::Sender;
use ksni::blocking::{Handle, TrayMethods};
use tracing::warn;
use vpet_render::procedural_mascot_pixmap;

#[derive(Debug, Clone)]
pub enum TrayCommand {
    SetVisible(bool),
    SetClickThrough(bool),
    SetOverlay(bool),
    ToggleSleep,
    FeedSnack,
    ReloadConfig,
    Quit,
}

#[derive(Debug, Clone)]
pub struct TraySnapshot {
    pub title: String,
    pub visible: bool,
    pub click_through: bool,
    pub overlay: bool,
    pub sleeping: bool,
}

impl Default for TraySnapshot {
    fn default() -> Self {
        Self {
            title: "VPetRust".to_owned(),
            visible: true,
            click_through: false,
            overlay: false,
            sleeping: false,
        }
    }
}

struct VPetTray {
    snap: TraySnapshot,
    tx: Sender<TrayCommand>,
    icon_argb: Vec<u8>,
}

impl ksni::Tray for VPetTray {
    fn id(&self) -> String {
        "vpet-rs".to_owned()
    }

    fn title(&self) -> String {
        self.snap.title.clone()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: 32,
            height: 32,
            data: self.icon_argb.clone(),
        }]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let next = !self.snap.visible;
        let _ = self.tx.send(TrayCommand::SetVisible(next));
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};

        vec![
            StandardItem {
                label: self.snap.title.clone(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Show Pet".to_owned(),
                checked: self.snap.visible,
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::SetVisible(!this.snap.visible));
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Click-Through".to_owned(),
                checked: self.snap.click_through,
                activate: Box::new(|this: &mut Self| {
                    let _ = this
                        .tx
                        .send(TrayCommand::SetClickThrough(!this.snap.click_through));
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Overlay Above Fullscreen".to_owned(),
                checked: self.snap.overlay,
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::SetOverlay(!this.snap.overlay));
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Sleeping".to_owned(),
                checked: self.snap.sleeping,
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::ToggleSleep);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Feed Snack".to_owned(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::FeedSnack);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Reload Config".to_owned(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::ReloadConfig);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_owned(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub struct TrayController {
    handle: Option<Handle<VPetTray>>,
}

impl TrayController {
    #[must_use]
    pub fn spawn(initial: TraySnapshot, tx: Sender<TrayCommand>) -> Self {
        let icon_argb = build_32x32_argb_icon();
        let tray = VPetTray {
            snap: initial,
            tx,
            icon_argb,
        };
        let handle = match tray.assume_sni_available(true).spawn() {
            Ok(h) => Some(h),
            Err(err) => {
                warn!(%err, "StatusNotifierItem tray unavailable; continuing without tray");
                None
            }
        };
        Self { handle }
    }

    pub fn sync(&self, next: TraySnapshot) {
        if let Some(h) = &self.handle {
            let _ = h.update(move |tray| {
                tray.snap = next;
            });
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = h.shutdown();
        }
    }
}

fn build_32x32_argb_icon() -> Vec<u8> {
    let Ok(pm) = procedural_mascot_pixmap(32) else {
        return vec![0_u8; 32 * 32 * 4];
    };
    let mut out = Vec::with_capacity(32 * 32 * 4);
    let (pixels, _) = pm.data().as_chunks::<4>();
    for &[r, g, b, a] in pixels {
        // Network byte order ARGB32 expected by StatusNotifierItem.
        out.extend_from_slice(&[a, r, g, b]);
    }
    out
}
