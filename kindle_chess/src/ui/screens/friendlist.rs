use std::sync::{Arc, RwLock};

use log::{info, warn};

use crate::i18n::{self, tr};

use crate::{
    models::{
        chess::ChessApp,
        ui::{Display, Screen, Transition},
    },
    ui::{
        events::{AppEvent, RectangleExt, TouchKind},
        widgets::{Button, IconButton, icon_button::ICON_BUTTON_SIZE},
    },
    utils::config::AppConfig,
};

// ─── FriendlistScreen ─────────────────────────────────────────────────────────
/// Opponent picker for the Play flow: paged list of the user's Lichess friends
/// (`/api/rel/following`). Tapping a friend emits `AppEvent::FriendSelected`
/// and pops, handing the username back to the pushing screen (PlayScreen).
pub struct FriendlistScreen {
    pub app: ChessApp,
    pub config: Arc<RwLock<AppConfig>>,
    pub friend_button_0: Button,
    pub friend_button_1: Button,
    pub friend_button_2: Button,
    pub friend_button_3: Button,
    pub prev_page_button: IconButton,
    pub next_page_button: IconButton,
    pub back_button: IconButton,

    // Async fetch state. `friends == None && error == None && !loading` means
    // the screen has not yet kicked off its initial fetch — `render` triggers it.
    pub friends: Option<Arc<Vec<String>>>,
    pub error: Option<String>,
    pub loading: bool,

    // Pagination: 4 friend buttons per page; labels baked by `set_page`.
    pub page_index: usize,
}

impl FriendlistScreen {
    pub fn new(app: ChessApp, config: Arc<RwLock<AppConfig>>) -> Self {
        // Layout: 1072 × 1448. Title strip at top, then 4 stacked 600×96
        // buttons starting at y=400, page icons + back along the bottom.
        const BTN_W: u16 = 600;
        const BTN_H: u16 = 96;
        const GAP: i16 = 28;
        const START_Y: i16 = 400;
        let x = (1072 - BTN_W as i16) / 2;
        let row = |i: i16| START_Y + i * (BTN_H as i16 + GAP);
        let bottom = 1448 - (ICON_BUTTON_SIZE as i16 - 20) - 32;
        Self {
            app,
            friend_button_0: Button::new(x, row(0), BTN_W, BTN_H, "-".to_string(), 40.0, true),
            friend_button_1: Button::new(x, row(1), BTN_W, BTN_H, "-".to_string(), 40.0, true),
            friend_button_2: Button::new(x, row(2), BTN_W, BTN_H, "-".to_string(), 40.0, true),
            friend_button_3: Button::new(x, row(3), BTN_W, BTN_H, "-".to_string(), 40.0, true),
            prev_page_button: IconButton::new(
                1072 / 2 - (ICON_BUTTON_SIZE as i16 + 8),
                bottom,
                "back-page.png",
            ),
            next_page_button: IconButton::new(1072 / 2 + 8, bottom, "next-page.png"),
            back_button: IconButton::new(32 + ICON_BUTTON_SIZE as i16 / 2, bottom, "back-nav.png"),
            config,
            friends: None,
            error: None,
            loading: false,
            page_index: 0,
        }
    }
}

// ─── FriendlistScreen ─────────────────────────────────────────────────────────

const FRIENDS_PER_PAGE: usize = 4;

impl FriendlistScreen {
    /// Spawn the friends fetch onto the tokio runtime. Idempotent w.r.t.
    /// `self.loading` — call from `render` (initial load) only. Result is
    /// delivered as `FriendsLoaded` / `FriendsFailed` over the event channel.
    fn kick_fetch(&mut self, display: &Display) {
        if self.loading {
            return;
        }
        let Some(api) = self.app.online_idle_api() else {
            self.error = Some("Offline backend has no friend list".into());
            return;
        };
        self.loading = true;
        let tx = display.event_tx.clone();
        tokio::spawn(async move {
            match api.get_following().await {
                Ok(names) => {
                    let _ = tx.send(AppEvent::FriendsLoaded(Arc::new(names)));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::FriendsFailed(e.to_string()));
                }
            }
        });
    }

    fn page_count(&self) -> usize {
        match &self.friends {
            Some(f) => f.len().div_ceil(FRIENDS_PER_PAGE),
            None => 0,
        }
    }

    /// Bake labels for `index`'s page of friends into the four buttons and
    /// update `self.page_index`. Slots past the list end get "-" (inert).
    fn set_page(&mut self, index: usize) {
        self.page_index = index;
        let Some(friends) = self.friends.as_ref() else {
            return;
        };
        let buttons: [&mut Button; 4] = [
            &mut self.friend_button_0,
            &mut self.friend_button_1,
            &mut self.friend_button_2,
            &mut self.friend_button_3,
        ];
        let start = index * FRIENDS_PER_PAGE;
        for (k, btn) in buttons.into_iter().enumerate() {
            match friends.get(start + k) {
                Some(name) => btn.label = name.clone(),
                None => btn.label = "-".to_string(),
            }
        }
    }

    /// Which friend `friend_button_{k}` currently maps to, given the page.
    /// `None` for inert "-" slots.
    fn friend_at_slot(&self, k: usize) -> Option<&String> {
        self.friends
            .as_ref()?
            .get(self.page_index * FRIENDS_PER_PAGE + k)
    }
}

impl Screen for FriendlistScreen {
    fn render(&mut self, display: &mut Display) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ui::renderer::DrawColor;

        // First paint after Push: kick off the async fetch once.
        if self.friends.is_none() && self.error.is_none() && !self.loading {
            self.kick_fetch(display);
        }

        display.renderer.clear(DrawColor::White)?;

        // Title strip
        let loc = i18n::locale_of(&self.config);
        let title = tr(&loc, "friends-title");
        let (tw, _) = display.renderer.measure_text(&title, 36.0);
        display
            .renderer
            .draw_text((1072 - tw as i16) / 2, 120, &title, 36.0, DrawColor::Black)?;

        let size_px = 24.0;
        if let Some(err) = &self.error {
            let label = i18n::t(
                &i18n::locale_of(&self.config),
                "common-error",
                &[("error", err)],
            );
            let (tw, th) = display.renderer.measure_text(&label, size_px);
            display.renderer.draw_text(
                (1072 - tw as i16) / 2,
                (1448 - th as i16) / 2,
                &label,
                size_px,
                DrawColor::Black,
            )?;
        } else if let Some(friends) = &self.friends {
            if friends.is_empty() {
                let label = tr(&loc, "friends-empty");
                let (tw, th) = display.renderer.measure_text(&label, size_px);
                display.renderer.draw_text(
                    (1072 - tw as i16) / 2,
                    (1448 - th as i16) / 2,
                    &label,
                    size_px,
                    DrawColor::Black,
                )?;
            } else {
                self.friend_button_0.draw(&mut display.renderer)?;
                self.friend_button_1.draw(&mut display.renderer)?;
                self.friend_button_2.draw(&mut display.renderer)?;
                self.friend_button_3.draw(&mut display.renderer)?;
            }
            self.back_button.draw(&mut display.renderer)?;
            self.next_page_button.draw(&mut display.renderer)?;
            self.prev_page_button.draw(&mut display.renderer)?;
        } else {
            let label = tr(&loc, "friends-loading");
            let (tw, th) = display.renderer.measure_text(&label, size_px);
            display.renderer.draw_text(
                (1072 - tw as i16) / 2,
                (1448 - th as i16) / 2,
                &label,
                size_px,
                DrawColor::Black,
            )?;
            self.back_button.draw(&mut display.renderer)?;
        }

        display.renderer.present()?;
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: AppEvent,
        display: &mut Display,
    ) -> Result<Transition, Box<dyn std::error::Error>> {
        match event {
            AppEvent::FriendsLoaded(names) => {
                info!("Friends loaded: {} entries", names.len());
                self.friends = Some(names);
                self.loading = false;
                self.set_page(self.page_index);
                Ok(Transition::Redraw)
            }

            AppEvent::FriendsFailed(e) => {
                warn!("Friends fetch failed: {}", e);
                self.error = Some(e);
                self.loading = false;
                Ok(Transition::Redraw)
            }

            AppEvent::Touch(touch) => {
                if touch.kind != TouchKind::Up {
                    return Ok(Transition::Stay);
                }

                if self.back_button.rect.contains(touch.x, touch.y) {
                    return Ok(Transition::Pop);
                }

                if self.next_page_button.rect.contains(touch.x, touch.y) {
                    let last = self.page_count().saturating_sub(1);
                    if self.page_index < last {
                        self.set_page(self.page_index + 1);
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                if self.prev_page_button.rect.contains(touch.x, touch.y) {
                    if self.page_index > 0 {
                        self.set_page(self.page_index - 1);
                        return Ok(Transition::Redraw);
                    }
                    return Ok(Transition::Stay);
                }

                let friend_buttons = [
                    &self.friend_button_0,
                    &self.friend_button_1,
                    &self.friend_button_2,
                    &self.friend_button_3,
                ];
                for (k, btn) in friend_buttons.into_iter().enumerate() {
                    if !btn.rect.contains(touch.x, touch.y) {
                        continue;
                    }
                    let Some(name) = self.friend_at_slot(k).cloned() else {
                        return Ok(Transition::Stay);
                    };
                    info!("Friend selected: {}", name);
                    // Bubble the pick to the screen below before popping —
                    // it becomes top-of-stack and receives this event next.
                    let _ = display.event_tx.send(AppEvent::FriendSelected(name));
                    return Ok(Transition::Pop);
                }
                Ok(Transition::Stay)
            }

            AppEvent::Expose => Ok(Transition::Redraw),

            AppEvent::WindowUnmapped => {
                warn!("Window unmapped!");
                Ok(Transition::Stay)
            }

            AppEvent::Quit => Ok(Transition::Quit),

            _ => Ok(Transition::Stay),
        }
    }

    fn on_reveal(&mut self) {
        // Returning to this screen from a pushed one: refetch so the list is
        // current.
        self.friends = None;
        self.error = None;
    }
}

#[cfg(test)]
mod friendlist_tests {
    use super::*;
    use crate::models::chess::ChessApp;
    use crate::ui::screens::test_config;

    fn screen_with_friends(names: &[&str]) -> FriendlistScreen {
        let mut s = FriendlistScreen::new(ChessApp::new_offline(), test_config());
        s.friends = Some(Arc::new(names.iter().map(|n| n.to_string()).collect()));
        s.set_page(0);
        s
    }

    #[test]
    fn pagination_pages_of_four() {
        let s = screen_with_friends(&["a", "b", "c", "d", "e"]);
        assert_eq!(s.page_count(), 2);
        assert_eq!(s.friend_button_0.label, "a");
        assert_eq!(s.friend_button_3.label, "d");
    }

    #[test]
    fn set_page_second_page_and_inert_slots() {
        let mut s = screen_with_friends(&["a", "b", "c", "d", "e"]);
        s.set_page(1);
        assert_eq!(s.friend_button_0.label, "e");
        assert_eq!(s.friend_button_1.label, "-");
        assert_eq!(s.friend_at_slot(0).map(|s| s.as_str()), Some("e"));
        assert_eq!(s.friend_at_slot(1), None);
    }

    #[test]
    fn empty_friend_list_has_one_empty_page() {
        let s = screen_with_friends(&[]);
        assert_eq!(s.page_count(), 0);
    }
}
