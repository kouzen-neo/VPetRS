//! Full `calloop` application runtime orchestrating Wayland layer-shell, animation player,
//! simulation ticks, hot-reloaded TOML config, Unix socket IPC, StatusNotifier tray, and saves.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use calloop::{
    EventLoop, Interest, Mode, PostAction,
    channel::{self, Event as ChanEvent},
    generic::Generic,
    signals::{Signal, Signals},
    timer::{TimeoutAction, Timer},
};
use tracing::{info, warn};
use vpet_content::{AnimatType, AnimationIndex, ContentDatabase, FoodItem, GraphType, PetConfig};
use vpet_core::{FoodEffect, World};
use vpet_mods::ModLoader;
use vpet_render::{
    AsyncClipLoader, BubbleRenderer, BubbleStyle, ContextMenuRenderer, FrameCache, MenuEntry,
    MenuHit, MenuStyle, Pixmap, RenderedMenu, decode_animation_source, procedural_mascot_pixmap,
};
use vpet_wayland::{BTN_LEFT, BTN_RIGHT, LayerMode, SurfaceConfig, SurfaceEvent, WaylandShell};

use std::sync::Arc;

use crate::{
    anim::{AdvanceViewport, InteractionController, classify_hit_zone, clip_cache_key},
    config::{AppConfig, ConfigWatcher},
    ipc::{IpcRequest, IpcResponse, IpcServer, StatusData},
    paths::{AppPaths, InstanceLock},
    persistence::SaveStore,
    tray::{TrayCommand, TrayController, TrayFoodItem, TrayInventoryItem, TraySnapshot},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopMenuPage {
    Main,
    FoodStore,
    Works,
    Backpack,
}

struct DesktopMenuState {
    _page: DesktopMenuPage,
    anchor: (i32, i32),
    rendered: RenderedMenu,
}

#[derive(Debug, Default)]
pub struct RunOverrides {
    pub layer: Option<LayerMode>,
    pub click_through: Option<bool>,
    pub output: Option<String>,
    pub exit_after: Option<Duration>,
    pub disable_tray: bool,
}

struct RuntimeState {
    paths: AppPaths,
    config: AppConfig,
    watcher: ConfigWatcher,
    _lock: InstanceLock,
    save_store: SaveStore,

    content: ContentDatabase,
    pet_cfg: PetConfig,
    anim_index: AnimationIndex,
    world: World,

    cache: FrameCache,
    loader: AsyncClipLoader,
    bubble_renderer: Option<BubbleRenderer>,
    theme_fonts: Vec<std::path::PathBuf>,
    bubble_style: BubbleStyle,
    fallback_sprite: Pixmap,

    menu_renderer: Option<ContextMenuRenderer>,
    menu_style: MenuStyle,
    desktop_menu: Option<DesktopMenuState>,

    anim: InteractionController,
    shell: WaylandShell,
    tray: Option<TrayController>,
    tray_meals: Arc<Vec<TrayFoodItem>>,
    tray_drinks: Arc<Vec<TrayFoodItem>>,
    tray_snacks: Arc<Vec<TrayFoodItem>>,
    tray_drugs: Arc<Vec<TrayFoodItem>>,
    tray_gifts: Arc<Vec<TrayFoodItem>>,
    ai_worker: crate::ai::AiWorker,
    audio_monitor: crate::music::AudioMonitor,

    last_clip_key: String,
    last_frame_idx: usize,
    last_bubble: Option<String>,
    last_autosave: Instant,
    started_at: Instant,
    exit_after: Option<Duration>,
    shutdown_deadline: Option<Instant>,
    running: bool,
}

impl RuntimeState {
    fn start_shutdown(&mut self) {
        if self.shutdown_deadline.is_none() {
            self.anim
                .trigger_shutdown(&self.anim_index, &mut self.world);
            self.shutdown_deadline = Some(Instant::now() + Duration::from_millis(3000));
            self.refresh_visuals();
            self.shell.present();
        }
    }
    fn sync_tray(&self) {
        if let Some(tray) = &self.tray {
            let active_work = self
                .world
                .active_work
                .as_ref()
                .map(|w| (w.name.clone(), (w.remaining_secs / 60.0).ceil() as u32));
            let available_works = self
                .pet_cfg
                .works
                .iter()
                .map(|w| (w.name.clone(), w.level_limit))
                .collect();
            let backpack = Arc::new(
                self.world
                    .inventory
                    .to_items()
                    .into_iter()
                    .map(|i| TrayInventoryItem {
                        name: i.id,
                        count: i.count,
                    })
                    .collect(),
            );
            tray.sync(TraySnapshot {
                title: format!(
                    "{} — Lv.{} ({})",
                    self.world.pet.name,
                    self.world.pet.level,
                    self.world.pet.mode.as_str()
                ),
                pet_level: self.world.pet.level,
                pet_money: self.world.pet.money,
                visible: self.shell.visible(),
                click_through: self.shell.click_through(),
                overlay: self.shell.layer_mode() == LayerMode::Overlay,
                sleeping: self.world.pet.sleeping,
                self_care: self.config.simulation.self_care,
                auto_work: self.config.simulation.auto_work,
                active_work,
                available_works,
                backpack,
                meals: self.tray_meals.clone(),
                drinks: self.tray_drinks.clone(),
                snacks: self.tray_snacks.clone(),
                drugs: self.tray_drugs.clone(),
                gifts: self.tray_gifts.clone(),
                ai_enabled: self.config.ai.enabled,
            });
        }
    }

    fn apply_reloaded_config(&mut self, next: AppConfig) {
        let sprite_px = next.display.sprite_px();
        self.cache
            .set_max_bytes(next.app.cache_mb.max(16) * 1024 * 1024);
        self.anim.set_target_px(sprite_px);
        self.anim.set_auto_walk(next.display.auto_walk);
        self.shell.set_layer_mode(next.display.layer_mode());
        self.shell.set_click_through(next.display.click_through);
        if self.config.display.output != next.display.output {
            let _ = self.shell.select_output(next.display.preferred_output());
        }
        if self.config.ai != next.ai {
            self.ai_worker = crate::ai::AiWorker::spawn(next.ai.clone());
        }
        self.config = next;
        self.shell.present();
        self.sync_tray();
        info!("reloaded configuration live");
    }

    fn feed_by_name(&mut self, query: &str) -> std::result::Result<String, String> {
        let now_unix = unix_now();
        let (effect, icon_path, graph) = if query.eq_ignore_ascii_case("water") {
            let water_icon = self
                .content
                .foods
                .get("Mineral Water")
                .or_else(|| self.content.foods.get("1L Pure Water"))
                .and_then(|f| f.image_path.clone());

            let found = self
                .content
                .foods
                .all()
                .into_iter()
                .filter(|f| f.graph == "drink" && f.name.to_lowercase().contains("water"))
                .find(|f| f.price <= self.world.pet.money)
                .or_else(|| {
                    self.content
                        .foods
                        .all()
                        .into_iter()
                        .filter(|f| f.graph == "drink")
                        .find(|f| f.price <= self.world.pet.money)
                });

            match found {
                Some(f) => (
                    food_item_to_effect(f),
                    f.image_path.clone().or_else(|| water_icon.clone()),
                    "drink".to_owned(),
                ),
                None => (
                    FoodEffect {
                        name: "Mineral Water".to_owned(),
                        price: 0.0,
                        exp: 2.0,
                        strength: 5.0,
                        strength_food: 0.0,
                        strength_drink: 40.0,
                        feeling: 5.0,
                        health: 1.0,
                        likability: 0.5,
                        buff_id: None,
                        buff_time_mins: 0,
                    },
                    water_icon,
                    "drink".to_owned(),
                ),
            }
        } else if query.eq_ignore_ascii_case("drink") {
            let water_icon = self
                .content
                .foods
                .get("Mineral Water")
                .and_then(|f| f.image_path.clone());
            let found = self
                .content
                .foods
                .all()
                .into_iter()
                .filter(|f| f.graph == "drink")
                .find(|f| f.price <= self.world.pet.money);
            match found {
                Some(f) => (
                    food_item_to_effect(f),
                    f.image_path.clone(),
                    "drink".to_owned(),
                ),
                None => (
                    FoodEffect {
                        name: "Mineral Water".to_owned(),
                        price: 0.0,
                        exp: 2.0,
                        strength: 5.0,
                        strength_food: 0.0,
                        strength_drink: 40.0,
                        feeling: 5.0,
                        health: 1.0,
                        likability: 0.5,
                        buff_id: None,
                        buff_time_mins: 0,
                    },
                    water_icon,
                    "drink".to_owned(),
                ),
            }
        } else if query.eq_ignore_ascii_case("snack")
            || query.eq_ignore_ascii_case("food")
            || query.eq_ignore_ascii_case("meal")
            || query.eq_ignore_ascii_case("apple")
        {
            let default_snack_icon = self
                .content
                .foods
                .get("Sandwich")
                .or_else(|| self.content.foods.get("Bread"))
                .or_else(|| self.content.foods.get("Gummy"))
                .or_else(|| self.content.foods.get("Pork Jerky"))
                .and_then(|f| f.image_path.clone());

            let found = if query.eq_ignore_ascii_case("apple") {
                self.content.foods.get("Apple")
            } else {
                self.content
                    .foods
                    .all()
                    .into_iter()
                    .filter(|f| {
                        f.graph == "eat"
                            && f.category.eq_ignore_ascii_case("snack")
                            && f.image_path.is_some()
                    })
                    .find(|f| f.price <= self.world.pet.money)
                    .or_else(|| {
                        self.content
                            .foods
                            .all()
                            .into_iter()
                            .filter(|f| f.graph == "eat" && f.image_path.is_some())
                            .find(|f| f.price <= self.world.pet.money)
                    })
            };

            match found {
                Some(f) => (
                    food_item_to_effect(f),
                    f.image_path.clone().or_else(|| default_snack_icon.clone()),
                    "eat".to_owned(),
                ),
                None => (default_snack_effect(), default_snack_icon, "eat".to_owned()),
            }
        } else if let Some(item) = self.content.foods.get(query) {
            let fallback_icon = if item.graph == "drink" {
                self.content
                    .foods
                    .get("Mineral Water")
                    .and_then(|f| f.image_path.clone())
            } else {
                self.content
                    .foods
                    .get("Sandwich")
                    .or_else(|| self.content.foods.get("Bread"))
                    .and_then(|f| f.image_path.clone())
            };
            (
                food_item_to_effect(item),
                item.image_path.clone().or(fallback_icon),
                item.graph.clone(),
            )
        } else {
            return Err(format!("unknown food item `{query}`"));
        };

        let label = effect.name.clone();
        let gained_levels = self
            .world
            .feed(&effect, now_unix)
            .map_err(|e| e.to_string())?;
        let msg = if graph == "drink" {
            format!("Gulp gulp... ({label})")
        } else {
            format!("Yum! ({label})")
        };
        self.anim.show_bubble(msg, 3_000);
        self.anim.on_feed(
            graph,
            Some(label.clone()),
            icon_path,
            &self.anim_index,
            &mut self.world,
        );
        if gained_levels > 0 {
            self.anim.on_level_up(&self.anim_index, &mut self.world);
        }
        self.sync_tray();
        Ok(label)
    }

    fn start_work_by_name(&mut self, query: Option<&str>) -> std::result::Result<String, String> {
        let work_def = if let Some(q) = query {
            self.pet_cfg
                .works
                .iter()
                .find(|w| w.name.eq_ignore_ascii_case(q) || w.graph.eq_ignore_ascii_case(q))
                .cloned()
                .ok_or_else(|| format!("unknown work/study/play task `{q}`"))?
        } else {
            self.pet_cfg
                .works
                .iter()
                .find(|w| self.world.pet.level >= w.level_limit)
                .cloned()
                .ok_or_else(|| "no available work tasks for current pet level".to_owned())?
        };

        if self.world.pet.level < work_def.level_limit {
            return Err(format!(
                "task `{}` requires level {} (current: {})",
                work_def.name, work_def.level_limit, self.world.pet.level
            ));
        }

        let (money_rate, exp_rate) = match work_def.kind {
            vpet_content::WorkKind::Work => (work_def.money_base, 2.0),
            vpet_content::WorkKind::Study => (-work_def.money_base, 10.0),
            vpet_content::WorkKind::Play => (-work_def.money_base, 4.0),
        };

        let session = vpet_core::WorkSession::new(
            work_def.name.clone(),
            work_def.graph.clone(),
            work_def.time_mins,
            money_rate,
            exp_rate,
            work_def.strength_food,
            work_def.strength_drink,
            work_def.feeling,
            work_def.finish_bonus,
        );

        let name = work_def.name.clone();
        let graph = work_def.graph.clone();
        self.world
            .start_work(session, work_def.level_limit)
            .map_err(|e| e.to_string())?;

        self.anim.request_state(
            crate::anim::ActionState::Working { graph },
            true,
            &self.anim_index,
            &mut self.world,
        );
        self.anim.show_bubble(format!("Started {name}!"), 3_000);
        self.sync_tray();
        Ok(name)
    }

    fn cancel_active_work(&mut self) -> bool {
        if let Some(w) = self.world.cancel_work() {
            self.anim.request_state(
                crate::anim::ActionState::Idle,
                true,
                &self.anim_index,
                &mut self.world,
            );
            self.anim
                .show_bubble(format!("Canceled {}!", w.name), 2_500);
            self.sync_tray();
            true
        } else {
            false
        }
    }

    fn status_snapshot(&self) -> StatusData {
        let (px, py) = self.shell.position();
        StatusData {
            name: self.world.pet.name.clone(),
            level: self.world.pet.level,
            exp: self.world.pet.exp,
            money: self.world.pet.money,
            strength: self.world.pet.strength,
            food: self.world.pet.strength_food,
            drink: self.world.pet.strength_drink,
            feeling: self.world.pet.feeling,
            health: self.world.pet.health,
            likability: self.world.pet.likability,
            mode: self.world.pet.mode.as_str().to_owned(),
            sleeping: self.world.pet.sleeping,
            active_work: self.world.active_work.as_ref().map(|w| w.name.clone()),
            self_care: self.config.simulation.self_care,
            auto_work: self.config.simulation.auto_work,
            ai_enabled: self.config.ai.enabled,
            ai_model: self.config.ai.model.clone(),
            visible: self.shell.visible(),
            click_through: self.shell.click_through(),
            layer: self.shell.layer_mode().as_str().to_owned(),
            output: self.shell.active_output_name().to_owned(),
            position: [px, py],
        }
    }

    fn handle_ipc(&mut self, req: IpcRequest) -> IpcResponse {
        match req {
            IpcRequest::Show => {
                self.shell.set_visible(true);
                self.shell.present();
                self.sync_tray();
                IpcResponse::success(serde_json::json!({ "visible": true }))
            }
            IpcRequest::Hide => {
                self.shell.set_visible(false);
                self.shell.present();
                self.sync_tray();
                IpcResponse::success(serde_json::json!({ "visible": false }))
            }
            IpcRequest::ToggleClickthrough => {
                let next = self.shell.toggle_click_through();
                self.shell.present();
                self.sync_tray();
                IpcResponse::success(serde_json::json!({ "click_through": next }))
            }
            IpcRequest::ToggleSelfCare => {
                self.config.simulation.self_care = !self.config.simulation.self_care;
                self.sync_tray();
                IpcResponse::success(
                    serde_json::json!({ "self_care": self.config.simulation.self_care }),
                )
            }
            IpcRequest::ToggleAutoWork => {
                self.config.simulation.auto_work = !self.config.simulation.auto_work;
                self.sync_tray();
                IpcResponse::success(
                    serde_json::json!({ "auto_work": self.config.simulation.auto_work }),
                )
            }
            IpcRequest::Layer { mode } => {
                let Some(parsed) = LayerMode::parse(&mode) else {
                    return IpcResponse::fail("invalid layer mode");
                };
                self.shell.set_layer_mode(parsed);
                self.shell.present();
                self.sync_tray();
                IpcResponse::success(serde_json::json!({ "layer": parsed.as_str() }))
            }
            IpcRequest::Feed { item } => match self.feed_by_name(&item) {
                Ok(fed_name) => {
                    self.refresh_visuals();
                    self.shell.present();
                    IpcResponse::success(serde_json::json!({
                        "fed": fed_name,
                        "food": self.world.pet.strength_food,
                        "drink": self.world.pet.strength_drink,
                        "money": self.world.pet.money,
                    }))
                }
                Err(msg) => IpcResponse::fail(msg),
            },
            IpcRequest::Buy { item, count } => {
                let Some(food) = self.content.foods.get(&item) else {
                    return IpcResponse::fail(format!("unknown item `{item}`"));
                };
                let total_cost = food.price * f64::from(count);
                if self.world.pet.money < total_cost {
                    return IpcResponse::fail(format!(
                        "insufficient funds: need ${total_cost:.2}, have ${:.2}",
                        self.world.pet.money
                    ));
                }
                self.world.pet.money -= total_cost;
                let in_bag = self.world.inventory.add(&food.name, count);
                self.anim
                    .show_bubble(format!("Bought {count}x {}!", food.name), 3_000);
                self.sync_tray();
                IpcResponse::success(serde_json::json!({
                    "bought": food.name,
                    "count": count,
                    "total_cost": total_cost,
                    "in_bag": in_bag,
                    "money": self.world.pet.money,
                }))
            }
            IpcRequest::ListInventory => {
                let items: Vec<_> = self
                    .world
                    .inventory
                    .to_items()
                    .into_iter()
                    .map(|inv| {
                        let food_info = self.content.foods.get(&inv.id);
                        serde_json::json!({
                            "name": inv.id,
                            "count": inv.count,
                            "price": food_info.map(|f| f.price).unwrap_or(0.0),
                            "category": food_info.map(|f| f.category.as_str()).unwrap_or("Other"),
                        })
                    })
                    .collect();
                IpcResponse::success(serde_json::json!({
                    "money": self.world.pet.money,
                    "total_stacks": items.len(),
                    "items": items,
                }))
            }
            IpcRequest::Dance { loops } => {
                let count = loops.unwrap_or(3).max(1);
                self.anim.on_dance(count, &self.anim_index, &mut self.world);
                self.anim.show_bubble("Dancing to the beat! ~", 4_000);
                self.refresh_visuals();
                self.shell.present();
                IpcResponse::success(serde_json::json!({
                    "dancing": true,
                    "loops": count,
                }))
            }
            IpcRequest::Sleep => {
                if !self.world.pet.sleeping {
                    self.anim.toggle_sleep(&self.anim_index, &mut self.world);
                    self.refresh_visuals();
                    self.shell.present();
                    self.sync_tray();
                }
                IpcResponse::success(serde_json::json!({ "sleeping": true }))
            }
            IpcRequest::Wake => {
                if self.world.pet.sleeping {
                    self.anim.toggle_sleep(&self.anim_index, &mut self.world);
                    self.refresh_visuals();
                    self.shell.present();
                    self.sync_tray();
                }
                IpcResponse::success(serde_json::json!({ "sleeping": false }))
            }
            IpcRequest::Say { text } => {
                self.anim.show_bubble(text, 4_500);
                self.refresh_visuals();
                self.shell.present();
                IpcResponse::success(serde_json::json!({ "speaking": true }))
            }
            IpcRequest::Chat { prompt } => {
                if !self.config.ai.enabled {
                    return IpcResponse::fail("AI chat is disabled in config.toml [ai]");
                }
                self.anim.show_bubble("...", 8_000);
                self.anim.on_think(&self.anim_index, &mut self.world);
                self.refresh_visuals();
                self.shell.present();
                if self.ai_worker.ask(&prompt) {
                    IpcResponse::success(
                        serde_json::json!({ "status": "thinking", "prompt": prompt }),
                    )
                } else {
                    IpcResponse::fail("failed to dispatch prompt to AI worker")
                }
            }
            IpcRequest::PromptInput => {
                spawn_desktop_input_prompt(&self.paths.socket_path);
                IpcResponse::success(serde_json::json!({ "prompt_spawned": true }))
            }
            IpcRequest::ListWorks => {
                let available: Vec<_> = self
                    .pet_cfg
                    .works
                    .iter()
                    .map(|w| {
                        serde_json::json!({
                            "name": w.name,
                            "kind": format!("{:?}", w.kind),
                            "level_required": w.level_limit,
                            "time_mins": w.time_mins,
                            "money_base": w.money_base,
                        })
                    })
                    .collect();
                IpcResponse::success(serde_json::json!({
                    "active": self.world.active_work.as_ref().map(|w| &w.name),
                    "works": available,
                }))
            }
            IpcRequest::Work { name } => match self.start_work_by_name(name.as_deref()) {
                Ok(started) => {
                    self.refresh_visuals();
                    self.shell.present();
                    IpcResponse::success(serde_json::json!({ "started": started }))
                }
                Err(err) => IpcResponse::fail(err),
            },
            IpcRequest::CancelWork => {
                let canceled = self.cancel_active_work();
                self.refresh_visuals();
                self.shell.present();
                IpcResponse::success(serde_json::json!({ "canceled": canceled }))
            }
            IpcRequest::ListFoods { category } => {
                let cat_filter = category.as_deref().map(str::to_ascii_lowercase);
                let money = self.world.pet.money;
                let mut foods: Vec<_> = self
                    .content
                    .foods
                    .all()
                    .into_iter()
                    .filter(|f| {
                        cat_filter
                            .as_deref()
                            .is_none_or(|c| f.category.to_ascii_lowercase() == c)
                    })
                    .map(|f| {
                        serde_json::json!({
                            "name": f.name,
                            "category": f.category,
                            "price": f.price,
                            "food": f.strength_food,
                            "drink": f.strength_drink,
                            "stamina": f.strength,
                            "feeling": f.feeling,
                            "health": f.health,
                            "exp": f.exp,
                            "affordable": f.price <= money,
                        })
                    })
                    .collect();
                foods.sort_by(|a, b| {
                    let pa = a["price"].as_f64().unwrap_or(0.0);
                    let pb = b["price"].as_f64().unwrap_or(0.0);
                    pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
                });
                IpcResponse::success(serde_json::json!({
                    "money": money,
                    "count": foods.len(),
                    "foods": foods,
                }))
            }
            IpcRequest::SideHide { left } => {
                let bounds = self.shell.output_logical_size();
                let sprite_logical = self.shell.sprite_logical_size();
                let max_x = (bounds.0 as i32 - sprite_logical.0 as i32).max(0);
                let (px, py) = self.shell.position();
                let is_left = left.unwrap_or(px < max_x / 2);
                let offset_left = ((219_f32 * sprite_logical.0 as f32) / 500.0).round() as i32;
                let offset_right =
                    (((500.0 - 281_f32) * sprite_logical.0 as f32) / 500.0).round() as i32;
                let target_x = if is_left {
                    -offset_left
                } else {
                    max_x + offset_right
                };
                self.shell.set_position(target_x, py);
                self.world
                    .output_positions
                    .insert(self.shell.active_output_name().to_owned(), (target_x, py));
                self.anim.request_state(
                    crate::anim::ActionState::SideHide {
                        left: is_left,
                        emerging: false,
                    },
                    true,
                    &self.anim_index,
                    &mut self.world,
                );
                self.refresh_visuals();
                self.shell.present();
                IpcResponse::success(serde_json::json!({
                    "side_hide": true,
                    "side": if is_left { "left" } else { "right" },
                    "position": [target_x, py],
                }))
            }
            IpcRequest::Reload => {
                let reloaded = AppConfig::load_or_default(&self.paths.config_file);
                self.apply_reloaded_config(reloaded);
                IpcResponse::success(serde_json::json!({ "reloaded": true }))
            }
            IpcRequest::Status => IpcResponse::success(self.status_snapshot()),
            IpcRequest::Quit => {
                self.start_shutdown();
                IpcResponse::success(serde_json::json!({ "quitting": true }))
            }
        }
    }

    fn handle_tray_cmd(&mut self, cmd: TrayCommand) {
        match cmd {
            TrayCommand::SetVisible(v) => {
                self.shell.set_visible(v);
                self.shell.present();
                self.sync_tray();
            }
            TrayCommand::SetClickThrough(ct) => {
                self.shell.set_click_through(ct);
                self.shell.present();
                self.sync_tray();
            }
            TrayCommand::SetOverlay(ov) => {
                self.shell.set_layer_mode(if ov {
                    LayerMode::Overlay
                } else {
                    LayerMode::Top
                });
                self.shell.present();
                self.sync_tray();
            }
            TrayCommand::ToggleSleep => {
                self.anim.toggle_sleep(&self.anim_index, &mut self.world);
                self.refresh_visuals();
                self.shell.present();
                self.sync_tray();
            }
            TrayCommand::ToggleSelfCare => {
                let _ = self.handle_ipc(IpcRequest::ToggleSelfCare);
            }
            TrayCommand::ToggleAutoWork => {
                let _ = self.handle_ipc(IpcRequest::ToggleAutoWork);
            }
            TrayCommand::FeedSnack => {
                let _ = self.feed_by_name("snack");
                self.refresh_visuals();
                self.shell.present();
            }
            TrayCommand::GiveWater => {
                let _ = self.feed_by_name("water");
                self.refresh_visuals();
                self.shell.present();
            }
            TrayCommand::FeedItem(name) => {
                let _ = self.feed_by_name(&name);
                self.refresh_visuals();
                self.shell.present();
            }
            TrayCommand::Dance => {
                self.anim.on_dance(3, &self.anim_index, &mut self.world);
                self.anim.show_bubble("Dancing to the beat! ~", 4_000);
                self.refresh_visuals();
                self.shell.present();
            }
            TrayCommand::StartWork(name) => {
                let _ = self.start_work_by_name(Some(&name));
                self.refresh_visuals();
                self.shell.present();
            }
            TrayCommand::CancelWork => {
                let _ = self.cancel_active_work();
                self.refresh_visuals();
                self.shell.present();
            }
            TrayCommand::PromptAiChat => {
                spawn_desktop_input_prompt(&self.paths.socket_path);
            }
            TrayCommand::ReloadConfig => {
                let next = AppConfig::load_or_default(&self.paths.config_file);
                self.apply_reloaded_config(next);
            }
            TrayCommand::Quit => {
                self.start_shutdown();
            }
        }
    }

    fn open_context_menu(&mut self, anchor: (i32, i32), page: DesktopMenuPage) {
        let (screen_w, screen_h) = self.shell.output_logical_size();
        let (pet_x, _pet_y) = self.shell.position();
        let (sw, _sh) = self.shell.sprite_logical_size();

        let menu_w = self.menu_style.width as i32;

        let menu_x = if pet_x + sw as i32 + menu_w + 12 <= screen_w as i32 {
            pet_x + sw as i32 + 12
        } else if pet_x - menu_w - 12 >= 0 {
            pet_x - menu_w - 12
        } else {
            (anchor.0 - menu_w / 2).clamp(10, (screen_w as i32 - menu_w - 10).max(10))
        };

        let entries = self.build_menu_entries(page);

        let menu_renderer = self.menu_renderer.get_or_insert_with(|| {
            let mut mr = ContextMenuRenderer::new();
            for font_path in self.theme_fonts.iter().take(2) {
                mr.load_font_file(font_path);
            }
            mr
        });

        let est_h = (entries.len() * 26 + 30) as i32;
        let menu_y = (anchor.1 - 20).clamp(10, (screen_h as i32 - est_h - 10).max(10));

        if let Some(rendered) =
            menu_renderer.render_menu(&entries, &self.menu_style, (menu_x, menu_y))
        {
            self.shell
                .set_overlay_popup(Some((rendered.pixmap.clone(), (menu_x, menu_y))));
            self.shell.set_capture_input(true);
            self.shell.present();
            self.desktop_menu = Some(DesktopMenuState {
                _page: page,
                anchor,
                rendered,
            });
        }
    }

    fn close_context_menu(&mut self) {
        if self.desktop_menu.is_some() {
            self.desktop_menu = None;
            self.shell.set_overlay_popup(None);
            self.shell.set_capture_input(false);
            self.shell.present();
        }
    }

    fn toggle_context_menu(&mut self, anchor: (i32, i32)) {
        if self.desktop_menu.is_some() {
            self.close_context_menu();
        } else {
            self.open_context_menu(anchor, DesktopMenuPage::Main);
        }
    }

    fn build_menu_entries(&self, page: DesktopMenuPage) -> Vec<MenuEntry> {
        let mut entries = Vec::new();
        match page {
            DesktopMenuPage::Main => {
                entries.push(MenuEntry {
                    id: 0,
                    label: format!(
                        "{} – Lv.{} ({})",
                        self.world.pet.name,
                        self.world.pet.level,
                        self.world.pet.mode.as_str()
                    ),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: true,
                });

                entries.push(MenuEntry {
                    id: 2,
                    label: "Show Pet".to_owned(),
                    subtext: None,
                    checked: Some(self.shell.visible()),
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 3,
                    label: "Click-Through".to_owned(),
                    subtext: None,
                    checked: Some(self.shell.click_through()),
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 4,
                    label: "Overlay Above Fullscreen".to_owned(),
                    subtext: None,
                    checked: Some(self.shell.layer_mode() == LayerMode::Overlay),
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 5,
                    label: "Sleeping".to_owned(),
                    subtext: None,
                    checked: Some(self.world.pet.sleeping),
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 6,
                    label: "Self-Care (Auto-Feed)".to_owned(),
                    subtext: None,
                    checked: Some(self.config.simulation.self_care),
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 7,
                    label: "Auto-Work".to_owned(),
                    subtext: None,
                    checked: Some(self.config.simulation.auto_work),
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });

                entries.push(MenuEntry {
                    id: 0,
                    label: String::new(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: true,
                    is_header: false,
                });

                entries.push(MenuEntry {
                    id: 8,
                    label: "Quick Snack".to_owned(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 9,
                    label: "Quick Water".to_owned(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 10,
                    label: "Food Store".to_owned(),
                    subtext: Some(format!("${:.2} >", self.world.pet.money)),
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });

                let bp_count = self
                    .world
                    .inventory
                    .to_items()
                    .iter()
                    .map(|it| it.count)
                    .sum::<u32>();
                entries.push(MenuEntry {
                    id: 11,
                    label: "Backpack".to_owned(),
                    subtext: Some(format!("({bp_count}) >")),
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });

                entries.push(MenuEntry {
                    id: 12,
                    label: "Dance to Music".to_owned(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });

                entries.push(MenuEntry {
                    id: 0,
                    label: String::new(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: true,
                    is_header: false,
                });

                if let Some(w) = &self.world.active_work {
                    let mins = (w.remaining_secs / 60.0).ceil() as u32;
                    entries.push(MenuEntry {
                        id: 0,
                        label: format!("Working: {} ({}m left)", w.name, mins),
                        subtext: None,
                        checked: None,
                        disabled: true,
                        is_separator: false,
                        is_header: false,
                    });
                    entries.push(MenuEntry {
                        id: 14,
                        label: "Cancel Work".to_owned(),
                        subtext: None,
                        checked: None,
                        disabled: false,
                        is_separator: false,
                        is_header: false,
                    });
                } else {
                    entries.push(MenuEntry {
                        id: 13,
                        label: "Work / Study / Play".to_owned(),
                        subtext: Some(">".to_owned()),
                        checked: None,
                        disabled: false,
                        is_separator: false,
                        is_header: false,
                    });
                }

                entries.push(MenuEntry {
                    id: 0,
                    label: String::new(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: true,
                    is_header: false,
                });

                entries.push(MenuEntry {
                    id: 15,
                    label: "Chat with Pet...".to_owned(),
                    subtext: None,
                    checked: None,
                    disabled: !self.config.ai.enabled,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 16,
                    label: "Reload Config".to_owned(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
                entries.push(MenuEntry {
                    id: 17,
                    label: "Quit".to_owned(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: false,
                });
            }
            DesktopMenuPage::FoodStore => {
                entries.push(MenuEntry {
                    id: 1,
                    label: format!("< Back | Food Store (${:.2})", self.world.pet.money),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: true,
                });

                let popular = [
                    "Fruit Juice",
                    "1L Pure Water",
                    "Mineral Water",
                    "Sandwich",
                    "Bread",
                    "Steak",
                    "Cookie",
                    "Cola",
                    "Coffee",
                    "Medicine",
                ];
                for (idx, name) in popular.iter().enumerate() {
                    let price = self.content.foods.get(name).map_or(10.0, |f| f.price);
                    let can_afford = self.world.pet.money >= price;
                    entries.push(MenuEntry {
                        id: 1000 + idx as u32,
                        label: (*name).to_owned(),
                        subtext: Some(format!("${price:.1}")),
                        checked: None,
                        disabled: !can_afford,
                        is_separator: false,
                        is_header: false,
                    });
                }
            }
            DesktopMenuPage::Works => {
                entries.push(MenuEntry {
                    id: 1,
                    label: "< Back | Work & Study".to_owned(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: true,
                });

                for (idx, w) in self.pet_cfg.works.iter().enumerate() {
                    let unlocked = self.world.pet.level >= w.level_limit;
                    let kind_str = match w.kind {
                        vpet_content::WorkKind::Work => "Work",
                        vpet_content::WorkKind::Study => "Study",
                        vpet_content::WorkKind::Play => "Play",
                    };
                    let sub = if unlocked {
                        format!("{}, {}m", kind_str, w.time_mins)
                    } else {
                        format!("Req Lv.{}", w.level_limit)
                    };
                    entries.push(MenuEntry {
                        id: 2000 + idx as u32,
                        label: w.name.clone(),
                        subtext: Some(sub),
                        checked: None,
                        disabled: !unlocked,
                        is_separator: false,
                        is_header: false,
                    });
                }
            }
            DesktopMenuPage::Backpack => {
                entries.push(MenuEntry {
                    id: 1,
                    label: "< Back | Backpack".to_owned(),
                    subtext: None,
                    checked: None,
                    disabled: false,
                    is_separator: false,
                    is_header: true,
                });

                let items = self.world.inventory.to_items();
                if items.is_empty() {
                    entries.push(MenuEntry {
                        id: 0,
                        label: "(Backpack is empty)".to_owned(),
                        subtext: None,
                        checked: None,
                        disabled: true,
                        is_separator: false,
                        is_header: false,
                    });
                } else {
                    for (idx, it) in items.iter().enumerate() {
                        entries.push(MenuEntry {
                            id: 3000 + idx as u32,
                            label: it.id.clone(),
                            subtext: Some(format!("x{}", it.count)),
                            checked: None,
                            disabled: false,
                            is_separator: false,
                            is_header: false,
                        });
                    }
                }
            }
        }
        entries
    }

    fn handle_menu_action(&mut self, action_id: u32) {
        match action_id {
            1 => {
                if let Some(menu) = &self.desktop_menu {
                    let anchor = menu.anchor;
                    self.open_context_menu(anchor, DesktopMenuPage::Main);
                }
            }
            2 => {
                let vis = !self.shell.visible();
                self.handle_tray_cmd(TrayCommand::SetVisible(vis));
                self.close_context_menu();
            }
            3 => {
                let ct = !self.shell.click_through();
                self.handle_tray_cmd(TrayCommand::SetClickThrough(ct));
                self.close_context_menu();
            }
            4 => {
                let ov = self.shell.layer_mode() != LayerMode::Overlay;
                self.handle_tray_cmd(TrayCommand::SetOverlay(ov));
                self.close_context_menu();
            }
            5 => {
                self.handle_tray_cmd(TrayCommand::ToggleSleep);
                self.close_context_menu();
            }
            6 => {
                self.handle_tray_cmd(TrayCommand::ToggleSelfCare);
                self.close_context_menu();
            }
            7 => {
                self.handle_tray_cmd(TrayCommand::ToggleAutoWork);
                self.close_context_menu();
            }
            8 => {
                self.handle_tray_cmd(TrayCommand::FeedSnack);
                self.close_context_menu();
            }
            9 => {
                self.handle_tray_cmd(TrayCommand::GiveWater);
                self.close_context_menu();
            }
            10 => {
                if let Some(menu) = &self.desktop_menu {
                    let anchor = menu.anchor;
                    self.open_context_menu(anchor, DesktopMenuPage::FoodStore);
                }
            }
            11 => {
                if let Some(menu) = &self.desktop_menu {
                    let anchor = menu.anchor;
                    self.open_context_menu(anchor, DesktopMenuPage::Backpack);
                }
            }
            12 => {
                self.handle_tray_cmd(TrayCommand::Dance);
                self.close_context_menu();
            }
            13 => {
                if let Some(menu) = &self.desktop_menu {
                    let anchor = menu.anchor;
                    self.open_context_menu(anchor, DesktopMenuPage::Works);
                }
            }
            14 => {
                self.handle_tray_cmd(TrayCommand::CancelWork);
                self.close_context_menu();
            }
            15 => {
                self.handle_tray_cmd(TrayCommand::PromptAiChat);
                self.close_context_menu();
            }
            16 => {
                self.handle_tray_cmd(TrayCommand::ReloadConfig);
                self.close_context_menu();
            }
            17 => {
                self.handle_tray_cmd(TrayCommand::Quit);
                self.close_context_menu();
            }
            1000..=1999 => {
                let popular = [
                    "Fruit Juice",
                    "1L Pure Water",
                    "Mineral Water",
                    "Sandwich",
                    "Bread",
                    "Steak",
                    "Cookie",
                    "Cola",
                    "Coffee",
                    "Medicine",
                ];
                let idx = (action_id - 1000) as usize;
                if let Some(&name) = popular.get(idx) {
                    let _ = self.feed_by_name(name);
                    self.refresh_visuals();
                    self.shell.present();
                }
                self.close_context_menu();
            }
            2000..=2999 => {
                let idx = (action_id - 2000) as usize;
                if let Some(w) = self.pet_cfg.works.get(idx) {
                    let name = w.name.clone();
                    let _ = self.start_work_by_name(Some(&name));
                    self.refresh_visuals();
                    self.shell.present();
                }
                self.close_context_menu();
            }
            3000..=3999 => {
                let idx = (action_id - 3000) as usize;
                let items = self.world.inventory.to_items();
                if let Some(it) = items.get(idx) {
                    let name = it.id.clone();
                    let _ = self.feed_by_name(&name);
                    self.refresh_visuals();
                    self.shell.present();
                }
                self.close_context_menu();
            }
            _ => {
                self.close_context_menu();
            }
        }
    }

    fn refresh_visuals(&mut self) {
        self.loader.poll_into(&mut self.cache);
        let sprite_px = self.config.display.sprite_px();

        if let Some(active) = self.anim.active_clip() {
            let key = active.cache_key.clone();
            let frame_idx = active.frame_index;
            let source = active.source.clone();

            if !self.cache.contains(&key) {
                self.loader.request(key.clone(), source, sprite_px);
            }
            if let Some(clip) = self.cache.get(&key)
                && !clip.frames.is_empty()
            {
                let safe_idx = frame_idx % clip.frames.len();
                if self.last_clip_key != key || self.last_frame_idx != safe_idx {
                    self.shell
                        .set_sprite((*clip.frames[safe_idx].pixmap).clone());
                    self.last_clip_key = key;
                    self.last_frame_idx = safe_idx;
                }
            }
        }

        let current_bubble = self.anim.bubble_text().map(str::to_owned);
        if current_bubble != self.last_bubble {
            let rendered = current_bubble.as_deref().and_then(|txt| {
                let renderer = self.bubble_renderer.get_or_insert_with(|| {
                    let mut br = BubbleRenderer::new();
                    for font_path in self.theme_fonts.iter().take(2) {
                        br.load_font_file(font_path);
                    }
                    br
                });
                renderer.render_bubble(txt, sprite_px.max(200), &self.bubble_style)
            });
            self.shell.set_bubble(rendered);
            self.last_bubble = current_bubble;
        }
    }
}

pub fn run_daemon(paths: AppPaths, mut config: AppConfig, overrides: RunOverrides) -> Result<()> {
    if let Some(l) = overrides.layer {
        config.display.layer = l.as_str().to_owned();
    }
    if let Some(ct) = overrides.click_through {
        config.display.click_through = ct;
    }
    if let Some(out) = overrides.output {
        config.display.output = out;
    }

    let lock = match InstanceLock::acquire(&paths.lock_path) {
        Ok(l) => l,
        Err(_) => {
            eprintln!("vpet-rs daemon is already running.");
            eprintln!("Use `vpet-rs ctl --help` to control it, or `vpet-rs ctl quit` to stop.");
            return Ok(());
        }
    };
    // Bind Unix socket IPC immediately after holding single-instance flock so CLI clients connect right away.
    let ipc_server = IpcServer::bind(&paths.socket_path)?;
    let save_store = SaveStore::new(&paths.saves_dir);

    let content_root = paths.resolve_content_root(&config.app.content_root);
    let mut mod_loader = ModLoader::new(&content_root);
    mod_loader.add_mod_root(&paths.mods_dir);
    let mut content = mod_loader.load_all();
    content.localize(&config.pet.locale);

    let pet_cfg = content
        .pets
        .get(&config.pet.pet_id)
        .cloned()
        .or_else(|| content.pets.values().next().cloned())
        .unwrap_or_default();

    let anim_index = content
        .animations
        .get(&pet_cfg.id)
        .cloned()
        .unwrap_or_default();

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0x5650_4554, |d| d.as_nanos() as u64);

    let mut world = save_store
        .load_or_recover(&config.app.profile)
        .unwrap_or_else(|| {
            let mut w = World::new(&config.app.profile, &config.pet.name, seed);
            w.pet.mode = config.pet.parsed_start_mode();
            w
        });
    world.pet.money = world.pet.money.max(10.0);
    if config.pet.locale.starts_with("en") && vpet_content::contains_cjk(&world.pet.name) {
        world.pet.name = "Loris".to_owned();
    }

    let sprite_px = config.display.sprite_px();
    let mut cache = FrameCache::new(config.app.cache_mb.max(16) * 1024 * 1024);
    let mut loader = AsyncClipLoader::new();

    // Synchronously decode the initial idle clip so frame 1 shows the real pet immediately,
    // while preloading sleep/touch/raise clips asynchronously in the background worker!
    let fallback_sprite = procedural_mascot_pixmap(sprite_px)?;
    let mut initial_sprite = fallback_sprite.clone();
    if let Some(entry) = anim_index.pick(
        GraphType::Default,
        None,
        AnimatType::Single,
        world.pet.mode,
        world.rng.state(),
    ) {
        let key = clip_cache_key(entry, sprite_px);
        if let Ok(clip) = decode_animation_source(&key, &entry.source, sprite_px) {
            if let Some(first) = clip.frames.first() {
                initial_sprite = (*first.pixmap).clone();
            }
            cache.insert(clip);
        }
    }

    preload_essential_clips(&anim_index, &world, sprite_px, &cache, &mut loader);

    let (theme_fonts, bubble_style) = if let Some(theme) = content
        .themes
        .get("default")
        .or_else(|| content.themes.values().next())
    {
        let style = BubbleStyle {
            bg_rgba: theme.color_or("primarylighter", [252, 253, 255, 240]),
            border_rgba: theme.color_or("primary", [129, 212, 250, 255]),
            text_rgba: theme.color_or("primarytext", [30, 36, 48, 255]),
            font_px: 15.0,
            padding_px: 10,
            corner_radius: 10.0,
        };
        (theme.fonts.clone(), style)
    } else {
        (Vec::new(), BubbleStyle::default())
    };

    let mut event_loop: EventLoop<'static, RuntimeState> =
        EventLoop::try_new().context("failed to initialize calloop event loop")?;
    let loop_handle = event_loop.handle();

    let initial_pos = (config.display.position[0], config.display.position[1]);
    let surface_cfg = SurfaceConfig {
        namespace: "vpet-rs".to_owned(),
        preferred_output: config.display.preferred_output(),
        layer: config.display.layer_mode(),
        click_through: config.display.click_through,
        visible: true,
        initial_position: initial_pos,
    };

    let mut shell =
        WaylandShell::connect_and_insert(surface_cfg, initial_sprite, &loop_handle, |s| {
            &mut s.shell
        })
        .context("failed to initialize Wayland layer-shell")?;

    // Restore per-output saved position if previously recorded for this monitor.
    if let Some(&saved_pos) = world.output_positions.get(shell.active_output_name()) {
        shell.set_position(saved_pos.0, saved_pos.1);
        shell.present();
    }

    // Register pre-bound Unix socket IPC (`0600` + `SO_PEERCRED`) on calloop.
    let ipc_fd = ipc_server
        .raw_fd()
        .try_clone_to_owned()
        .context("failed to duplicate IPC socket fd")?;
    loop_handle
        .insert_source(
            Generic::new(ipc_fd, Interest::READ, Mode::Level),
            move |_, _, state: &mut RuntimeState| {
                ipc_server.accept_pending(|req| state.handle_ipc(req));
                Ok(PostAction::Continue)
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to register IPC source: {e}"))?;

    // Categorize food items for the tray food store once.
    let tray_meals = categorize_food_items(&content.foods, "meal");
    let tray_drinks = categorize_food_items(&content.foods, "drink");
    let tray_snacks = categorize_food_items(&content.foods, "snack");
    let tray_drugs = categorize_food_items(&content.foods, "drug");
    let tray_gifts = categorize_gifts_and_functional(&content.foods);

    // Spawn StatusNotifierItem tray (unless disabled in headless test).
    let (tray_tx, tray_rx) = channel::channel::<TrayCommand>();
    let tray = (!overrides.disable_tray).then(|| {
        TrayController::spawn(
            TraySnapshot {
                title: format!(
                    "{} — Lv.{} ({})",
                    world.pet.name,
                    world.pet.level,
                    world.pet.mode.as_str()
                ),
                pet_level: world.pet.level,
                pet_money: world.pet.money,
                visible: true,
                click_through: shell.click_through(),
                overlay: shell.layer_mode() == LayerMode::Overlay,
                sleeping: world.pet.sleeping,
                self_care: config.simulation.self_care,
                auto_work: config.simulation.auto_work,
                active_work: None,
                available_works: pet_cfg
                    .works
                    .iter()
                    .map(|w| (w.name.clone(), w.level_limit))
                    .collect(),
                backpack: Arc::new(Vec::new()),
                meals: tray_meals.clone(),
                drinks: tray_drinks.clone(),
                snacks: tray_snacks.clone(),
                drugs: tray_drugs.clone(),
                gifts: tray_gifts.clone(),
                ai_enabled: config.ai.enabled,
            },
            tray_tx,
        )
    });
    loop_handle
        .insert_source(tray_rx, |ev, (), state: &mut RuntimeState| {
            if let ChanEvent::Msg(cmd) = ev {
                state.handle_tray_cmd(cmd);
            }
        })
        .map_err(|e| anyhow::anyhow!("failed to register tray channel: {e}"))?;

    // Register UNIX signals (`SIGUSR1` click-through, `SIGUSR2` layer, `SIGINT`/`SIGTERM` clean save+exit).
    let signals = Signals::new(&[
        Signal::SIGUSR1,
        Signal::SIGUSR2,
        Signal::SIGINT,
        Signal::SIGTERM,
    ])?;
    loop_handle
        .insert_source(signals, |ev, (), state: &mut RuntimeState| {
            match ev.signal() {
                Signal::SIGUSR1 => {
                    let _ = state.handle_ipc(IpcRequest::ToggleClickthrough);
                }
                Signal::SIGUSR2 => {
                    let next = match state.shell.layer_mode() {
                        LayerMode::Top => "overlay",
                        LayerMode::Overlay => "top",
                    };
                    let _ = state.handle_ipc(IpcRequest::Layer {
                        mode: next.to_owned(),
                    });
                }
                Signal::SIGINT | Signal::SIGTERM => {
                    info!("received termination signal; starting shutdown");
                    state.start_shutdown();
                }
                _ => {}
            }
        })
        .map_err(|e| anyhow::anyhow!("failed to register signal source: {e}"))?;

    // 10 Hz deterministic simulation + config hot-reload + autosave timer.
    let sim_hz = config.simulation.tick_hz.clamp(1, 60);
    let sim_period = Duration::from_millis((1_000 / u64::from(sim_hz)).max(16));
    loop_handle
        .insert_source(Timer::from_duration(sim_period), move |_, (), state| {
            let dt_secs = 1.0 / f64::from(state.config.simulation.tick_hz.clamp(1, 60));
            let had_work = state.world.active_work.is_some();
            state.world.tick(dt_secs, unix_now());
            if had_work && state.world.active_work.is_none() {
                state.anim.request_state(
                    crate::anim::ActionState::Idle,
                    true,
                    &state.anim_index,
                    &mut state.world,
                );
                state.anim.show_bubble("Work completed! Great job!", 4_000);
                state.sync_tray();
            }

            // Self-care: automatically nourish pet if hunger or thirst drops below 30%
            if state.config.simulation.self_care
                && !state.world.pet.sleeping
                && !matches!(state.anim.state(), crate::anim::ActionState::Eating { .. })
            {
                if state.world.pet.strength_food < 30.0
                    && state.world.pet.store_strength_food < 15.0
                {
                    let _ = state.feed_by_name("snack");
                }
                if state.world.pet.strength_drink < 30.0
                    && state.world.pet.store_strength_drink < 15.0
                {
                    let _ = state.feed_by_name("water");
                }
            }

            // Auto-work: automatically start best unlocked work when well-fed, idle, and healthy
            if state.config.simulation.auto_work
                && state.world.active_work.is_none()
                && !state.world.pet.sleeping
                && state.world.pet.strength_food >= 40.0
                && state.world.pet.strength_drink >= 40.0
                && state.world.pet.strength >= 30.0
                && state.world.pet.mode != vpet_core::ModeType::Ill
                && matches!(state.anim.state(), crate::anim::ActionState::Idle)
            {
                let best = state
                    .pet_cfg
                    .works
                    .iter()
                    .filter(|w| {
                        w.kind == vpet_content::WorkKind::Work
                            && state.world.pet.level >= w.level_limit
                    })
                    .max_by_key(|w| w.level_limit)
                    .or_else(|| {
                        state
                            .pet_cfg
                            .works
                            .iter()
                            .filter(|w| state.world.pet.level >= w.level_limit)
                            .max_by_key(|w| w.level_limit)
                    })
                    .map(|w| w.name.clone());

                if let Some(work_name) = best {
                    let _ = state.start_work_by_name(Some(&work_name));
                }
            }

            if let Some(reloaded) = state.watcher.check_modified() {
                state.apply_reloaded_config(reloaded);
            }

            let autosave_secs = state.config.simulation.autosave_secs.max(10);
            if state.last_autosave.elapsed() >= Duration::from_secs(autosave_secs) {
                if let Err(err) = state.save_store.save_world(&state.world) {
                    warn!(%err, "autosave failed");
                }
                state.last_autosave = Instant::now();
            }

            TimeoutAction::ToDuration(sim_period)
        })
        .map_err(|e| anyhow::anyhow!("failed to register simulation timer: {e}"))?;

    // ~30 Hz animation + pointer interaction timer (only commits to Wayland when pixels/region change).
    let anim_period = Duration::from_millis(33);
    loop_handle
        .insert_source(Timer::from_duration(anim_period), move |_, (), state| {
            if let Some(limit) = state.exit_after
                && state.started_at.elapsed() >= limit
            {
                state.running = false;
                return TimeoutAction::Drop;
            }
            if state.anim.is_shutdown_ready()
                || state
                    .shutdown_deadline
                    .is_some_and(|dl| Instant::now() >= dl)
            {
                state.running = false;
                return TimeoutAction::Drop;
            }

            for ev in state.shell.drain_events() {
                match ev {
                    SurfaceEvent::Clicked { x, y, button } => {
                        let click_pos = (x.round() as i32, y.round() as i32);
                        if button == BTN_RIGHT {
                            state.toggle_context_menu(click_pos);
                        } else if button == BTN_LEFT {
                            if let Some(menu) = &state.desktop_menu {
                                match menu.rendered.hit_test(click_pos.0, click_pos.1) {
                                    MenuHit::Item(action_id) => {
                                        state.handle_menu_action(action_id);
                                    }
                                    MenuHit::InsideNoAction => {}
                                    MenuHit::Outside => {
                                        state.close_context_menu();
                                    }
                                }
                            } else {
                                let (px, py) = state.shell.position();
                                let zone = classify_hit_zone(
                                    &state.pet_cfg,
                                    state.shell.sprite_logical_size(),
                                    x - f64::from(px),
                                    y - f64::from(py),
                                );
                                state.anim.on_click_zone(
                                    zone,
                                    &state.content.texts,
                                    &state.anim_index,
                                    &mut state.world,
                                );
                            }
                        } else if state.desktop_menu.is_some() {
                            state.close_context_menu();
                        }
                    }
                    SurfaceEvent::DragStarted { .. } => {
                        state.close_context_menu();
                        state
                            .anim
                            .on_drag_start(&state.anim_index, &mut state.world);
                    }
                    SurfaceEvent::DragEnded { pet_x, pet_y } => {
                        let out_name = state.shell.active_output_name().to_owned();
                        let bounds = state.shell.output_logical_size();
                        let sprite_logical = state.shell.sprite_logical_size();
                        state.anim.on_drag_end(
                            &out_name,
                            (pet_x, pet_y),
                            bounds,
                            sprite_logical,
                            &state.anim_index,
                            &mut state.world,
                        );
                    }
                    _ => {}
                }
            }

            if let Some(res) = state.ai_worker.poll_reply() {
                match res.reply {
                    Ok(text) => {
                        state.world.pet.add_feeling(2.5);
                        state.world.pet.add_likability(0.8);
                        let _ = state.world.pet.add_exp(2.0);
                        state.anim.show_bubble(text, 8_000);
                        state.anim.request_state(
                            crate::anim::ActionState::Saying,
                            true,
                            &state.anim_index,
                            &mut state.world,
                        );
                    }
                    Err(err) => {
                        warn!(%err, "AI chat query failed");
                        state.anim.show_bubble(format!("Hmm? ({err})"), 4_500);
                        state.anim.request_state(
                            crate::anim::ActionState::Idle,
                            true,
                            &state.anim_index,
                            &mut state.world,
                        );
                    }
                }
                state.sync_tray();
                state.refresh_visuals();
                state.shell.present();
            }

            if let Some(status) = state.audio_monitor.poll_status()
                && status == crate::music::MusicStatus::Playing
                && matches!(state.anim.state(), crate::anim::ActionState::Idle)
                && !state.world.pet.sleeping
                && state.world.active_work.is_none()
            {
                state.anim.on_dance(4, &state.anim_index, &mut state.world);
                state.anim.show_bubble("Dancing to the music! ~", 4_000);
            }

            let mut pos = state.shell.position();
            let out_name = state.shell.active_output_name().to_owned();
            let bounds = state.shell.output_logical_size();
            let sprite_logical = state.shell.sprite_logical_size();

            let durs: Option<Vec<u32>> = state
                .anim
                .active_clip()
                .map(|c| c.cache_key.clone())
                .and_then(|k| state.cache.get(&k))
                .map(|clip| clip.frames.iter().map(|f| f.duration_ms).collect());

            if !state.shell.is_dragging() && state.desktop_menu.is_none() {
                state.anim.advance(
                    33,
                    durs.as_deref(),
                    &mut pos,
                    AdvanceViewport {
                        bounds,
                        sprite_logical,
                        output_name: &out_name,
                    },
                    &state.anim_index,
                    &mut state.world,
                );
                state.shell.set_position(pos.0, pos.1);
            }

            state.refresh_visuals();
            state.shell.present();

            TimeoutAction::ToDuration(anim_period)
        })
        .map_err(|e| anyhow::anyhow!("failed to register animation timer: {e}"))?;

    let mut anim = InteractionController::new(sprite_px, config.display.auto_walk);
    anim.request_state(
        crate::anim::ActionState::StartUp,
        true,
        &anim_index,
        &mut world,
    );
    anim.ensure_clip(&anim_index, &mut world);

    info!(
        output = %shell.active_output_name(),
        size = ?shell.output_logical_size(),
        layer = %shell.layer_mode().as_str(),
        animations = anim_index.len(),
        foods = content.foods.len(),
        socket = %paths.socket_path.display(),
        "vpet-rs daemon ready"
    );

    let watcher = ConfigWatcher::new(paths.config_file.clone());
    let ai_worker = crate::ai::AiWorker::spawn(config.ai.clone());
    let audio_monitor = crate::music::AudioMonitor::spawn();
    let mut state = RuntimeState {
        paths,
        config,
        watcher,
        _lock: lock,
        save_store,
        content,
        pet_cfg,
        anim_index,
        world,
        cache,
        loader,
        bubble_renderer: None,
        theme_fonts,
        bubble_style,
        fallback_sprite,
        menu_renderer: None,
        menu_style: MenuStyle::default(),
        desktop_menu: None,
        anim,
        shell,
        tray,
        tray_meals,
        tray_drinks,
        tray_snacks,
        tray_drugs,
        tray_gifts,
        ai_worker,
        audio_monitor,
        last_clip_key: String::new(),
        last_frame_idx: usize::MAX,
        last_bubble: None,
        last_autosave: Instant::now(),
        started_at: Instant::now(),
        exit_after: overrides.exit_after,
        shutdown_deadline: None,
        running: true,
    };
    state.sync_tray();

    while state.running && !state.shell.is_closed() {
        event_loop.dispatch(None, &mut state)?;
    }

    if let Some(mut tray) = state.tray.take() {
        tray.shutdown();
    }
    state.world.output_positions.insert(
        state.shell.active_output_name().to_owned(),
        state.shell.position(),
    );
    if let Err(err) = state.save_store.save_world(&state.world) {
        warn!(%err, "failed to save world on shutdown");
    } else {
        info!("saved world state cleanly");
    }
    let _ = &state.fallback_sprite;

    Ok(())
}

fn preload_essential_clips(
    index: &AnimationIndex,
    world: &World,
    sprite_px: u32,
    cache: &FrameCache,
    loader: &mut AsyncClipLoader,
) {
    let seed = world.rng.state();
    let mode = world.pet.mode;
    let specs = [
        (GraphType::Sleep, None, AnimatType::AStart),
        (GraphType::Sleep, None, AnimatType::BLoop),
        (GraphType::TouchHead, None, AnimatType::AStart),
        (GraphType::TouchHead, None, AnimatType::BLoop),
        (GraphType::TouchBody, None, AnimatType::AStart),
        (GraphType::RaisedDynamic, None, AnimatType::Single),
        (GraphType::Common, Some("eat"), AnimatType::Single),
        (GraphType::Common, Some("drink"), AnimatType::Single),
        (GraphType::Common, Some("gift"), AnimatType::Single),
        (GraphType::Common, Some("pinch"), AnimatType::Single),
        (GraphType::Common, Some("levelup"), AnimatType::Single),
        (GraphType::Common, Some("music"), AnimatType::AStart),
        (GraphType::Common, Some("music"), AnimatType::BLoop),
        (GraphType::SideHideLeftMain, None, AnimatType::AStart),
        (GraphType::SideHideLeftMain, None, AnimatType::BLoop),
        (GraphType::SideHideLeftMain, None, AnimatType::CEnd),
        (GraphType::SideHideLeftRise, None, AnimatType::AStart),
        (GraphType::SideHideLeftRise, None, AnimatType::BLoop),
        (GraphType::SideHideLeftRise, None, AnimatType::CEnd),
        (GraphType::SideHideRightMain, None, AnimatType::AStart),
        (GraphType::SideHideRightMain, None, AnimatType::BLoop),
        (GraphType::SideHideRightMain, None, AnimatType::CEnd),
        (GraphType::SideHideRightRise, None, AnimatType::AStart),
        (GraphType::SideHideRightRise, None, AnimatType::BLoop),
        (GraphType::SideHideRightRise, None, AnimatType::CEnd),
    ];
    let items = specs.into_iter().filter_map(|(gt, name, animat)| {
        let entry = index.pick(gt, name, animat, mode, seed)?;
        Some((clip_cache_key(entry, sprite_px), entry.source.clone()))
    });
    loader.preload(items, cache, sprite_px);
}

fn food_item_to_effect(item: &FoodItem) -> FoodEffect {
    FoodEffect {
        name: item.name.clone(),
        price: item.price,
        exp: item.exp,
        strength: item.strength,
        strength_food: item.strength_food,
        strength_drink: item.strength_drink,
        feeling: item.feeling,
        health: item.health,
        likability: item.likability,
        buff_id: item.buff.clone(),
        buff_time_mins: item.buff_time_mins,
    }
}

fn categorize_food_items(
    foods: &vpet_content::FoodDatabase,
    category: &str,
) -> Arc<Vec<TrayFoodItem>> {
    let mut items: Vec<_> = foods
        .all()
        .into_iter()
        .filter(|f| f.category.eq_ignore_ascii_case(category) && !f.name.trim().is_empty())
        .map(|f| TrayFoodItem {
            name: f.name.clone(),
            price: f.price,
        })
        .collect();
    items.sort_by(|a, b| {
        a.price
            .partial_cmp(&b.price)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Arc::new(items)
}

fn categorize_gifts_and_functional(foods: &vpet_content::FoodDatabase) -> Arc<Vec<TrayFoodItem>> {
    let mut items: Vec<_> = foods
        .all()
        .into_iter()
        .filter(|f| {
            (f.category.eq_ignore_ascii_case("gift")
                || f.category.eq_ignore_ascii_case("functional"))
                && !f.name.trim().is_empty()
        })
        .map(|f| TrayFoodItem {
            name: f.name.clone(),
            price: f.price,
        })
        .collect();
    items.sort_by(|a, b| {
        a.price
            .partial_cmp(&b.price)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Arc::new(items)
}

fn default_snack_effect() -> FoodEffect {
    FoodEffect {
        name: "Snack".to_owned(),
        price: 0.0,
        exp: 8.0,
        strength: 15.0,
        strength_food: 25.0,
        strength_drink: 15.0,
        feeling: 10.0,
        health: 1.0,
        likability: 0.5,
        buff_id: None,
        buff_time_mins: 0,
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn spawn_desktop_input_prompt(socket_path: &std::path::Path) {
    let sock = socket_path.to_path_buf();
    std::thread::spawn(move || {
        let text_opt = if let Ok(out) = std::process::Command::new("fuzzel")
            .args(["-d", "-p", "Chat with Loris: "])
            .output()
        {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
            } else {
                None
            }
        } else if let Ok(out) = std::process::Command::new("zenity")
            .args([
                "--entry",
                "--title",
                "VPetRS AI Chat",
                "--text",
                "Say something to Loris:",
            ])
            .output()
        {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
            } else {
                None
            }
        } else if let Ok(out) = std::process::Command::new("kdialog")
            .args(["--inputbox", "Say something to Loris:"])
            .output()
        {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
            } else {
                None
            }
        } else {
            warn!("no supported desktop input prompter found (fuzzel, zenity, or kdialog)");
            None
        };

        if let Some(msg) = text_opt.filter(|s| !s.is_empty()) {
            let req = crate::ipc::IpcRequest::Chat { prompt: msg };
            let _ = crate::ipc::send_ipc_request(&sock, &req);
        }
    });
}
