//! Pointer hit, click, and implicit-grab drag state machine.

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;
pub const BTN_MIDDLE: u32 = 0x112;

const DRAG_THRESHOLD_PX: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceEvent {
    HoverChanged(bool),
    PointerMotion { x: f64, y: f64 },
    Clicked { x: f64, y: f64, button: u32 },
    DragStarted { pet_x: i32, pet_y: i32 },
    Dragged { pet_x: i32, pet_y: i32 },
    DragEnded { pet_x: i32, pet_y: i32 },
    Axis { x: f64, y: f64, dy: f64 },
}

#[derive(Debug, Default)]
pub struct PointerTracker {
    hovered: bool,
    cursor_pos: (f64, f64),
    grab: Option<GrabState>,
}

#[derive(Debug, Clone, Copy)]
struct GrabState {
    press_pos: (f64, f64),
    offset_x: f64,
    offset_y: f64,
    dragging: bool,
}

impl PointerTracker {
    pub const fn is_dragging(&self) -> bool {
        match self.grab {
            Some(g) => g.dragging,
            None => false,
        }
    }

    pub const fn is_hovered(&self) -> bool {
        self.hovered
    }

    pub fn on_enter(&mut self, pos: (f64, f64)) -> Option<SurfaceEvent> {
        self.cursor_pos = pos;
        if !self.hovered {
            self.hovered = true;
            return Some(SurfaceEvent::HoverChanged(true));
        }
        None
    }

    pub fn on_leave(&mut self) -> Option<SurfaceEvent> {
        if self.hovered {
            self.hovered = false;
            return Some(SurfaceEvent::HoverChanged(false));
        }
        None
    }

    pub fn on_press(
        &mut self,
        button: u32,
        pos: (f64, f64),
        pet_pos: (i32, i32),
    ) -> Option<SurfaceEvent> {
        self.cursor_pos = pos;
        if button == BTN_LEFT {
            self.grab = Some(GrabState {
                press_pos: pos,
                offset_x: pos.0 - f64::from(pet_pos.0),
                offset_y: pos.1 - f64::from(pet_pos.1),
                dragging: false,
            });
            None
        } else {
            Some(SurfaceEvent::Clicked {
                x: pos.0,
                y: pos.1,
                button,
            })
        }
    }

    pub fn on_motion(
        &mut self,
        pos: (f64, f64),
        bounds: (u32, u32),
        sprite_logical: (u32, u32),
    ) -> Option<SurfaceEvent> {
        self.cursor_pos = pos;
        if let Some(grab) = self.grab.as_mut() {
            let dx = pos.0 - grab.press_pos.0;
            let dy = pos.1 - grab.press_pos.1;
            if !grab.dragging && dx.hypot(dy) < DRAG_THRESHOLD_PX {
                return None;
            }

            let max_x = (bounds.0 as i32 - sprite_logical.0 as i32).max(0);
            let max_y = (bounds.1 as i32 - sprite_logical.1 as i32).max(0);
            let pet_x = ((pos.0 - grab.offset_x).round() as i32).clamp(0, max_x);
            let pet_y = ((pos.1 - grab.offset_y).round() as i32).clamp(0, max_y);

            if !grab.dragging {
                grab.dragging = true;
                Some(SurfaceEvent::DragStarted { pet_x, pet_y })
            } else {
                Some(SurfaceEvent::Dragged { pet_x, pet_y })
            }
        } else {
            Some(SurfaceEvent::PointerMotion { x: pos.0, y: pos.1 })
        }
    }

    pub fn on_release(
        &mut self,
        button: u32,
        pos: (f64, f64),
        pet_pos: (i32, i32),
    ) -> Option<SurfaceEvent> {
        self.cursor_pos = pos;
        if button != BTN_LEFT {
            return None;
        }
        let grab = self.grab.take()?;
        if grab.dragging {
            Some(SurfaceEvent::DragEnded {
                pet_x: pet_pos.0,
                pet_y: pet_pos.1,
            })
        } else {
            Some(SurfaceEvent::Clicked {
                x: pos.0,
                y: pos.1,
                button: BTN_LEFT,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_press_release_emits_click_not_drag() {
        let mut pt = PointerTracker::default();
        assert_eq!(pt.on_press(BTN_LEFT, (100.0, 120.0), (80, 90)), None);
        assert_eq!(pt.on_motion((101.5, 121.0), (1920, 1080), (250, 250)), None);
        assert_eq!(
            pt.on_release(BTN_LEFT, (101.5, 121.0), (80, 90)),
            Some(SurfaceEvent::Clicked {
                x: 101.5,
                y: 121.0,
                button: BTN_LEFT,
            })
        );
    }

    #[test]
    fn motion_past_threshold_drags_and_clamps_to_output() {
        let mut pt = PointerTracker::default();
        pt.on_press(BTN_LEFT, (50.0, 50.0), (40, 40));
        assert_eq!(
            pt.on_motion((200.0, 300.0), (1920, 1080), (250, 250)),
            Some(SurfaceEvent::DragStarted {
                pet_x: 190,
                pet_y: 290,
            })
        );
        // Dragging past far bottom-right clamps cleanly inside 1920x1080 - 250x250.
        assert_eq!(
            pt.on_motion((5000.0, 5000.0), (1920, 1080), (250, 250)),
            Some(SurfaceEvent::Dragged {
                pet_x: 1670,
                pet_y: 830,
            })
        );
        assert_eq!(
            pt.on_release(BTN_LEFT, (5000.0, 5000.0), (1670, 830)),
            Some(SurfaceEvent::DragEnded {
                pet_x: 1670,
                pet_y: 830,
            })
        );
    }
}
