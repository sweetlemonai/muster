//! The menu bar icon (task 0085): Muster's robot head in the menu bar, idle while nothing waits
//! for the owner and marked while something does, so the count shows even with the Dock hidden.
//! It shows what the Dock badge counts (`badge::dock_count`), from the same reads: each window
//! poll and each hidden read (`lib.rs`) pass their count here, so it adds none of its own.
//! Clicking it brings the window forward; its menu (right-click or Control-click) shows Muster,
//! says the count in words and quits. Only those built-in actions: nothing from a queue runs.
//! Settings › Notifications turns it off (`settings.json` `menuBar`, on unless turned off). The
//! images are template images (black and alpha), so macOS tints them for a light or dark menu bar;
//! the waiting one fills in the antennas' tips, so it isn't told apart by colour alone.
//! The tooltip and the menu name a count, never a task's text.

use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

/// The tray icon's id: there's only ever this one.
pub const ID: &str = "muster";
const SHOW: &str = "show";
const COUNT: &str = "count";
const QUIT: &str = "quit";

/// The owner's template images at 2x (36 px for the menu bar's 18 pt), decoded at compile time.
const IDLE: Image<'static> =
    tauri::include_image!("icons/original/png/menubar-idle-template-2x.png");
const WAITING: Image<'static> =
    tauri::include_image!("icons/original/png/menubar-waiting-template-2x.png");

/// Which of the two images the menu bar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Nothing waits for the owner.
    Idle,
    /// Something does: the head with its antennas marked.
    Waiting,
}

/// The look for `n` things waiting for the owner.
pub fn look(n: usize) -> Look {
    if n > 0 {
        Look::Waiting
    } else {
        Look::Idle
    }
}

/// The count in words, as the menu says it.
pub fn words(n: usize) -> String {
    match n {
        0 => "Nothing waits for you".into(),
        1 => "1 thing waits for you".into(),
        n => format!("{n} things wait for you"),
    }
}

/// The tooltip for `n`, or for no read yet.
pub fn tooltip(n: Option<usize>) -> String {
    match n {
        Some(n) => format!("Muster · {}", words(n).to_lowercase()),
        None => "Muster".into(),
    }
}

/// What the menu says before the first read has a count.
pub const READING: &str = "Reading the queues…";

/// The menu bar as Muster drives it; `Tray` is the real one, tests have their own.
///
/// The icon is made once and then only shown or hidden: making one registers its menu handler
/// for the life of the app (Tauri never removes it), so making it again on each Off then On
/// would run Show and Quit once per toggle.
pub trait Bar {
    /// Has the icon been made?
    fn made(&self) -> bool;
    /// Makes it, in the menu bar, idle and with no count yet; true when it's made.
    fn make(&self) -> bool;
    /// Shows or hides the made icon; true when that took.
    fn show(&self, on: bool) -> bool;
    /// Shows `n`: the look, the tooltip and the menu's count; true when all of it took.
    fn set(&self, n: usize) -> bool;
}

/// The latest count and what the menu bar shows of it.
#[derive(Default)]
pub struct MenuBar {
    seen: Mutex<Seen>,
}

#[derive(Default)]
struct Seen {
    /// The latest count read, whether or not the icon is up.
    latest: Option<usize>,
    /// The count the icon shows, when its last `set` took. Kept while it's hidden: a hidden
    /// icon isn't set, and when it's shown again it still shows this.
    shown: Option<usize>,
    /// Whether the made icon is in the menu bar, as far as Muster set it.
    visible: bool,
}

impl MenuBar {
    /// Brings `bar` in line: shown when `on`, with the latest count (`n` when a read gave one),
    /// and hidden when not. Makes the icon at most once (again only after a failed make), and
    /// sets it only when what it shows changes; a `show` or `set` that fails is tried again on
    /// the next sync. The lock is held throughout, so a poll, a hidden read and a Settings
    /// change never make two.
    pub fn sync(&self, on: bool, n: Option<usize>, bar: &impl Bar) {
        let mut seen = self.seen();
        if n.is_some() {
            seen.latest = n;
        }
        if !bar.made() {
            if !on {
                return;
            }
            seen.shown = None;
            if !bar.make() {
                return;
            }
            seen.visible = true;
        }
        if seen.visible != on {
            if !bar.show(on) {
                return;
            }
            seen.visible = on;
        }
        if !on {
            return;
        }
        let Some(n) = seen.latest else { return };
        if seen.shown != Some(n) {
            seen.shown = bar.set(n).then_some(n);
        }
    }

    /// The state behind the lock. Nothing panics while holding it, but a poisoned lock still
    /// holds plain values, so it's used as is.
    fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The real menu bar, through Tauri's tray icon (its calls run on the main thread themselves).
pub struct Tray<'a>(pub &'a AppHandle);

impl Bar for Tray<'_> {
    fn made(&self) -> bool {
        self.0.tray_by_id(ID).is_some()
    }

    fn make(&self) -> bool {
        let Ok(menu) = menu(self.0, READING) else {
            return false;
        };
        // Tauri registers the handlers only once the icon exists, so a failed make leaves none.
        TrayIconBuilder::with_id(ID)
            .icon(IDLE)
            .icon_as_template(true)
            .tooltip(tooltip(None))
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, e| match e.id().as_ref() {
                SHOW => show_window(app),
                QUIT => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, e| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = e
                {
                    show_window(tray.app_handle());
                }
            })
            .build(self.0)
            .is_ok()
    }

    fn show(&self, on: bool) -> bool {
        self.0
            .tray_by_id(ID)
            .is_some_and(|tray| tray.set_visible(on).is_ok())
    }

    fn set(&self, n: usize) -> bool {
        let Some(tray) = self.0.tray_by_id(ID) else {
            return false;
        };
        let image = match look(n) {
            Look::Idle => IDLE,
            Look::Waiting => WAITING,
        };
        let Ok(menu) = menu(self.0, &words(n)) else {
            return false;
        };
        tray.set_icon_with_as_template(Some(image), true).is_ok()
            && tray.set_tooltip(Some(tooltip(Some(n)))).is_ok()
            && tray.set_menu(Some(menu)).is_ok()
    }
}

/// The menu: Show Muster, the count in words (not a button), Quit Muster.
fn menu(app: &AppHandle, count: &str) -> tauri::Result<Menu<tauri::Wry>> {
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, SHOW, "Show Muster", true, None::<&str>)?,
            &MenuItem::with_id(app, COUNT, count, false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit Muster", true, None::<&str>)?,
        ],
    )
}

/// Brings the window forward: out of the Dock if minimized, shown if hidden, then focused.
fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A menu bar that records what it was asked to do, counts the menu handlers registered
    /// (one per icon made, as Tauri does), and fails when told to.
    #[derive(Default)]
    struct Fake {
        made: RefCell<bool>,
        visible: RefCell<bool>,
        handlers: RefCell<usize>,
        calls: RefCell<Vec<String>>,
        fail_make: RefCell<bool>,
        fail_show: RefCell<bool>,
        fail_set: RefCell<bool>,
    }

    impl Bar for Fake {
        fn made(&self) -> bool {
            *self.made.borrow()
        }
        fn make(&self) -> bool {
            self.calls.borrow_mut().push("make".into());
            if *self.fail_make.borrow() {
                return false;
            }
            *self.made.borrow_mut() = true;
            *self.visible.borrow_mut() = true;
            *self.handlers.borrow_mut() += 1;
            true
        }
        fn show(&self, on: bool) -> bool {
            self.calls
                .borrow_mut()
                .push(if on { "show" } else { "hide" }.into());
            let ok = *self.made.borrow() && !*self.fail_show.borrow();
            if ok {
                *self.visible.borrow_mut() = on;
            }
            ok
        }
        fn set(&self, n: usize) -> bool {
            self.calls.borrow_mut().push(format!("set {n}"));
            !*self.fail_set.borrow()
        }
    }

    impl Fake {
        fn took(&self) -> Vec<String> {
            self.calls.borrow_mut().drain(..).collect()
        }
    }

    #[test]
    fn idle_with_nothing_waiting_and_waiting_from_one() {
        assert_eq!(look(0), Look::Idle);
        assert_eq!(look(1), Look::Waiting);
        assert_eq!(look(120), Look::Waiting);
        assert_eq!(look(usize::MAX), Look::Waiting);
    }

    #[test]
    fn the_count_is_in_words_and_the_tooltip_names_only_a_count() {
        assert_eq!(words(0), "Nothing waits for you");
        assert_eq!(words(1), "1 thing waits for you");
        assert_eq!(words(2), "2 things wait for you");
        assert_eq!(words(77), "77 things wait for you");
        assert_eq!(tooltip(Some(0)), "Muster · nothing waits for you");
        assert_eq!(tooltip(Some(3)), "Muster · 3 things wait for you");
        assert_eq!(tooltip(None), "Muster");
    }

    #[test]
    fn goes_up_idle_then_shows_each_count_once() {
        let (m, bar) = (MenuBar::default(), Fake::default());
        // At launch, before any read: made and up, with no count to show yet.
        m.sync(true, None, &bar);
        assert_eq!(bar.took(), ["make"]);
        m.sync(true, Some(0), &bar);
        assert_eq!(bar.took(), ["set 0"]);
        m.sync(true, Some(0), &bar);
        assert!(bar.took().is_empty(), "the same count isn't set again");
        m.sync(true, Some(2), &bar);
        m.sync(true, Some(2), &bar);
        m.sync(true, Some(0), &bar);
        assert_eq!(bar.took(), ["set 2", "set 0"]);
        // A sync with no new read (a Settings change) keeps the latest count.
        m.sync(true, None, &bar);
        assert!(bar.took().is_empty());
    }

    #[test]
    fn off_hides_it_and_on_again_shows_the_latest_count() {
        let (m, bar) = (MenuBar::default(), Fake::default());
        m.sync(true, Some(1), &bar);
        assert_eq!(bar.took(), ["make", "set 1"]);
        m.sync(false, None, &bar);
        assert_eq!(bar.took(), ["hide"]);
        assert!(!*bar.visible.borrow());
        // Reads go on while it's off; the hidden icon isn't touched.
        m.sync(false, Some(4), &bar);
        assert!(bar.took().is_empty());
        // On again from Settings (no read): shown, with the count read while it was off.
        m.sync(true, None, &bar);
        assert_eq!(bar.took(), ["show", "set 4"]);
        assert!(*bar.visible.borrow());
        // Off then on with the count it still shows: shown, not set again.
        m.sync(false, None, &bar);
        m.sync(true, None, &bar);
        assert_eq!(bar.took(), ["hide", "show"]);
    }

    #[test]
    fn off_at_launch_makes_nothing_until_on() {
        let (m, bar) = (MenuBar::default(), Fake::default());
        m.sync(false, None, &bar);
        m.sync(false, Some(2), &bar);
        assert!(bar.took().is_empty(), "off from the start: no icon made");
        m.sync(true, None, &bar);
        assert_eq!(bar.took(), ["make", "set 2"]);
    }

    #[test]
    fn toggling_many_times_leaves_one_menu_handler() {
        // Every icon made registers a menu handler for good, so Show and Quit would run once
        // per handler: Off then On only hides and shows the one icon (review R1).
        let (m, bar) = (MenuBar::default(), Fake::default());
        m.sync(true, Some(1), &bar);
        for i in 0..50 {
            m.sync(false, None, &bar);
            m.sync(true, Some(i % 3), &bar);
        }
        assert_eq!(*bar.handlers.borrow(), 1);
        assert_eq!(bar.took().iter().filter(|c| *c == "make").count(), 1);
        assert!(*bar.visible.borrow());
    }

    #[test]
    fn a_failed_make_show_or_set_is_tried_again_next_time() {
        let (m, bar) = (MenuBar::default(), Fake::default());
        *bar.fail_make.borrow_mut() = true;
        m.sync(true, Some(3), &bar);
        assert_eq!(
            bar.took(),
            ["make"],
            "nothing set on an icon that isn't made"
        );
        assert_eq!(*bar.handlers.borrow(), 0, "a failed make registers nothing");
        *bar.fail_make.borrow_mut() = false;
        *bar.fail_set.borrow_mut() = true;
        m.sync(true, Some(3), &bar);
        assert_eq!(bar.took(), ["make", "set 3"]);
        *bar.fail_set.borrow_mut() = false;
        m.sync(true, Some(3), &bar);
        assert_eq!(
            bar.took(),
            ["set 3"],
            "the same count again after a failed set"
        );
        m.sync(true, Some(3), &bar);
        assert!(bar.took().is_empty());
        // A hide that fails is tried again, and so is the show after it.
        *bar.fail_show.borrow_mut() = true;
        m.sync(false, None, &bar);
        assert_eq!(bar.took(), ["hide"]);
        assert!(*bar.visible.borrow());
        *bar.fail_show.borrow_mut() = false;
        m.sync(false, None, &bar);
        assert_eq!(bar.took(), ["hide"]);
        assert!(!*bar.visible.borrow());
        *bar.fail_show.borrow_mut() = true;
        m.sync(true, None, &bar);
        assert_eq!(bar.took(), ["show"], "not set while it stays hidden");
        *bar.fail_show.borrow_mut() = false;
        m.sync(true, None, &bar);
        assert_eq!(bar.took(), ["show"]);
        assert!(*bar.visible.borrow());
        assert_eq!(*bar.handlers.borrow(), 1);
    }

    #[test]
    fn the_window_cant_reach_the_tray() {
        // With the `tray-icon` feature, `core:default` would let the window make and change
        // tray icons; the menu bar icon is the core's alone, so the window lists the rest.
        let cap: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let perms: Vec<&str> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        assert!(perms.contains(&"core:window:default"), "{perms:?}");
        for p in &perms {
            assert!(*p != "core:default" && !p.starts_with("core:tray"), "{p}");
        }
    }

    #[test]
    fn the_images_are_template_images_and_waiting_is_marked_not_coloured() {
        for (name, img) in [("idle", &IDLE), ("waiting", &WAITING)] {
            assert_eq!((img.width(), img.height()), (36, 36), "{name}: 18 pt at 2x");
            let px: Vec<&[u8]> = img.rgba().chunks(4).collect();
            assert!(
                px.iter().all(|p| p[..3] == [0, 0, 0] || p[3] == 0),
                "{name}: black and alpha only, so macOS can tint it"
            );
            assert!(px.iter().any(|p| p[3] > 0), "{name}: not blank");
        }
        // The waiting head has more ink (the antennas' tips filled in), not another colour.
        let ink = |img: &Image<'_>| img.rgba().chunks(4).filter(|p| p[3] > 127).count();
        assert!(
            ink(&WAITING) > ink(&IDLE),
            "{} vs {}",
            ink(&WAITING),
            ink(&IDLE)
        );
    }
}
