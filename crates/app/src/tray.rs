//! StatusNotifierItem (`ksni`) system tray menu for waybar.

use std::sync::Arc;

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
    ToggleSelfCare,
    ToggleAutoWork,
    FeedSnack,
    GiveWater,
    FeedItem(String),
    StartWork(String),
    CancelWork,
    PromptAiChat,
    ReloadConfig,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrayFoodItem {
    pub name: String,
    pub price: f64,
}

#[derive(Debug, Clone)]
pub struct TraySnapshot {
    pub title: String,
    pub pet_level: u32,
    pub pet_money: f64,
    pub visible: bool,
    pub click_through: bool,
    pub overlay: bool,
    pub sleeping: bool,
    pub self_care: bool,
    pub auto_work: bool,
    pub active_work: Option<(String, u32)>,
    pub available_works: Vec<(String, u32)>,
    pub meals: Arc<Vec<TrayFoodItem>>,
    pub drinks: Arc<Vec<TrayFoodItem>>,
    pub snacks: Arc<Vec<TrayFoodItem>>,
    pub drugs: Arc<Vec<TrayFoodItem>>,
    pub gifts: Arc<Vec<TrayFoodItem>>,
    pub ai_enabled: bool,
}

impl Default for TraySnapshot {
    fn default() -> Self {
        Self {
            title: "VPetRS".to_owned(),
            pet_level: 1,
            pet_money: 0.0,
            visible: true,
            click_through: false,
            overlay: false,
            sleeping: false,
            self_care: true,
            auto_work: false,
            active_work: None,
            available_works: Vec::new(),
            meals: Arc::new(Vec::new()),
            drinks: Arc::new(Vec::new()),
            snacks: Arc::new(Vec::new()),
            drugs: Arc::new(Vec::new()),
            gifts: Arc::new(Vec::new()),
            ai_enabled: true,
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
        use ksni::menu::{CheckmarkItem, MenuItem, StandardItem, SubMenu};

        let mut items = vec![
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
            CheckmarkItem {
                label: "Self-Care (Auto-Feed)".to_owned(),
                checked: self.snap.self_care,
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::ToggleSelfCare);
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Auto-Work".to_owned(),
                checked: self.snap.auto_work,
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::ToggleAutoWork);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quick Snack".to_owned(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::FeedSnack);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quick Water".to_owned(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::GiveWater);
                }),
                ..Default::default()
            }
            .into(),
        ];

        // Food Store section
        let build_food_submenu =
            |cat_name: &str, foods: &[TrayFoodItem], money: f64| -> ksni::MenuItem<Self> {
                let sub_items: Vec<ksni::MenuItem<Self>> = foods
                    .iter()
                    .map(|f| {
                        let owned_name = f.name.clone();
                        let affordable = money >= f.price;
                        let label = format!("{} (${:.2})", f.name, f.price);
                        StandardItem {
                            label,
                            enabled: affordable,
                            activate: Box::new(move |this: &mut Self| {
                                let _ = this.tx.send(TrayCommand::FeedItem(owned_name.clone()));
                            }),
                            ..Default::default()
                        }
                        .into()
                    })
                    .collect();

                SubMenu {
                    label: format!("{cat_name} ({})", foods.len()),
                    submenu: sub_items,
                    ..Default::default()
                }
                .into()
            };

        let mut store_submenus = Vec::new();
        if !self.snap.meals.is_empty() {
            store_submenus.push(build_food_submenu(
                "Meals",
                &self.snap.meals,
                self.snap.pet_money,
            ));
        }
        if !self.snap.drinks.is_empty() {
            store_submenus.push(build_food_submenu(
                "Drinks",
                &self.snap.drinks,
                self.snap.pet_money,
            ));
        }
        if !self.snap.snacks.is_empty() {
            store_submenus.push(build_food_submenu(
                "Snacks",
                &self.snap.snacks,
                self.snap.pet_money,
            ));
        }
        if !self.snap.drugs.is_empty() {
            store_submenus.push(build_food_submenu(
                "Medicine",
                &self.snap.drugs,
                self.snap.pet_money,
            ));
        }
        if !self.snap.gifts.is_empty() {
            store_submenus.push(build_food_submenu(
                "Gifts & Fun",
                &self.snap.gifts,
                self.snap.pet_money,
            ));
        }

        if !store_submenus.is_empty() {
            items.push(
                SubMenu {
                    label: format!("Food Store (${:.2})", self.snap.pet_money),
                    submenu: store_submenus,
                    ..Default::default()
                }
                .into(),
            );
        }

        // Work / Study / Play section
        if let Some((work_name, rem_mins)) = &self.snap.active_work {
            items.push(MenuItem::Separator);
            items.push(
                StandardItem {
                    label: format!("Working: {work_name} ({rem_mins}m left)"),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
            items.push(
                StandardItem {
                    label: "Cancel Work".to_owned(),
                    activate: Box::new(|this: &mut Self| {
                        let _ = this.tx.send(TrayCommand::CancelWork);
                    }),
                    ..Default::default()
                }
                .into(),
            );
        } else if !self.snap.available_works.is_empty() {
            let cur_lv = self.snap.pet_level;
            let sub_items: Vec<ksni::MenuItem<Self>> = self
                .snap
                .available_works
                .iter()
                .map(|(name, req_lv)| {
                    let owned_name = name.clone();
                    let unlocked = cur_lv >= *req_lv;
                    let label = if unlocked {
                        name.clone()
                    } else {
                        format!("{name} (Requires Lv.{req_lv})")
                    };
                    StandardItem {
                        label,
                        enabled: unlocked,
                        activate: Box::new(move |this: &mut Self| {
                            let _ = this.tx.send(TrayCommand::StartWork(owned_name.clone()));
                        }),
                        ..Default::default()
                    }
                    .into()
                })
                .collect();

            items.push(
                SubMenu {
                    label: "Work / Study / Play".to_owned(),
                    submenu: sub_items,
                    ..Default::default()
                }
                .into(),
            );
        }

        if self.snap.ai_enabled {
            items.push(
                StandardItem {
                    label: "Chat with Pet...".to_owned(),
                    activate: Box::new(|this: &mut Self| {
                        let _ = this.tx.send(TrayCommand::PromptAiChat);
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Reload Config".to_owned(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::ReloadConfig);
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "Quit".to_owned(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }
            .into(),
        );

        items
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
