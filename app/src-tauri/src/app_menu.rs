//! Muster's menus in the Mac's menu bar (mu-147): Tauri's default ones (the app menu, File, Edit,
//! View, Window, Help), made here so their titles are in the language in effect (`i18n`,
//! `native.json`'s `appMenu`) and set again when it changes (`set`). Every item is one of macOS's
//! own actions (`PredefinedMenuItem`), given only its title; Muster's name stays "Muster".
//!
//! View also starts with Muster's own three (mu-167): Hide Sidebar ⌃⌘S, Hide Recently Finished
//! ⌃⌘R and Hide Filters ⌥⌘F, each saying what a click will do (Show… while it's hidden). A click
//! or the key sends the main window a `view-toggle` event with the region (`on_event`), the key's
//! one path; the window tells the menu what shows (`set_view_menu`, three booleans). Sidebar is
//! wired; the other two stay disabled until the window toggles them (mu-169).

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Deserialize;
use tauri::menu::{
    AboutMetadata, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
    WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Emitter, Wry};

use crate::i18n;

/// What the window shows (true) or has hidden, as it says through `set_view_menu`: three
/// booleans and nothing else: an object with exactly these three names (a derived struct would
/// take a list of three too, so it's read as a map first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "BTreeMap<String, bool>")]
pub struct ViewShown {
    pub sidebar: bool,
    pub recent: bool,
    pub filters: bool,
}

impl TryFrom<BTreeMap<String, bool>> for ViewShown {
    type Error = String;
    fn try_from(mut m: BTreeMap<String, bool>) -> Result<Self, String> {
        let mut take = |name: &str| m.remove(name).ok_or(format!("no {name}"));
        let shown = ViewShown {
            sidebar: take("sidebar")?,
            recent: take("recent")?,
            filters: take("filters")?,
        };
        match m.keys().next() {
            Some(other) => Err(format!("unknown field {other}")),
            None => Ok(shown),
        }
    }
}

impl Default for ViewShown {
    fn default() -> Self {
        ViewShown {
            sidebar: true,
            recent: true,
            filters: true,
        }
    }
}

/// What the window last said it shows; everything until it first says.
static VIEW: Mutex<ViewShown> = Mutex::new(ViewShown {
    sidebar: true,
    recent: true,
    filters: true,
});

fn view_now() -> ViewShown {
    VIEW.lock().map(|v| *v).unwrap_or_default()
}

/// One of macOS's own menu actions, a separator, or one of Muster's View toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ToggleSidebar,
    ToggleRecent,
    ToggleFilters,
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    CloseWindow,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Fullscreen,
    Minimize,
    Zoom,
    Separator,
}

/// A View toggle's menu id, the region its event names, its key and whether the window wires it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toggle {
    pub id: &'static str,
    pub region: &'static str,
    pub accelerator: &'static str,
    pub enabled: bool,
}

const TOGGLES: [(Action, Toggle); 3] = [
    (
        Action::ToggleSidebar,
        Toggle {
            id: "view-sidebar",
            region: "sidebar",
            accelerator: "Ctrl+Cmd+S",
            enabled: true,
        },
    ),
    (
        Action::ToggleRecent,
        Toggle {
            id: "view-recent",
            region: "recent",
            accelerator: "Ctrl+Cmd+R",
            enabled: false,
        },
    ),
    (
        Action::ToggleFilters,
        Toggle {
            id: "view-filters",
            region: "filters",
            accelerator: "Alt+Cmd+F",
            enabled: false,
        },
    ),
];

impl Action {
    /// Its View toggle, for Muster's own three items.
    pub fn toggle(self) -> Option<Toggle> {
        TOGGLES.iter().find(|(a, _)| *a == self).map(|(_, t)| *t)
    }

    /// Its title in `native.json`, for what `view` shows (a toggle says what a click will do); a
    /// separator has none.
    fn key(self, view: ViewShown) -> Option<&'static str> {
        let pick = |shown: bool, hide: &'static str, show: &'static str| {
            if shown {
                hide
            } else {
                show
            }
        };
        Some(match self {
            Action::ToggleSidebar => pick(
                view.sidebar,
                "appMenu.item.hideSidebar",
                "appMenu.item.showSidebar",
            ),
            Action::ToggleRecent => pick(
                view.recent,
                "appMenu.item.hideRecent",
                "appMenu.item.showRecent",
            ),
            Action::ToggleFilters => pick(
                view.filters,
                "appMenu.item.hideFilters",
                "appMenu.item.showFilters",
            ),
            Action::About => "appMenu.item.about",
            Action::Services => "appMenu.item.services",
            Action::Hide => "appMenu.item.hide",
            Action::HideOthers => "appMenu.item.hideOthers",
            Action::ShowAll => "appMenu.item.showAll",
            Action::Quit => "appMenu.item.quit",
            Action::CloseWindow => "appMenu.item.closeWindow",
            Action::Undo => "appMenu.item.undo",
            Action::Redo => "appMenu.item.redo",
            Action::Cut => "appMenu.item.cut",
            Action::Copy => "appMenu.item.copy",
            Action::Paste => "appMenu.item.paste",
            Action::SelectAll => "appMenu.item.selectAll",
            Action::Fullscreen => "appMenu.item.fullscreen",
            Action::Minimize => "appMenu.item.minimize",
            Action::Zoom => "appMenu.item.zoom",
            Action::Separator => return None,
        })
    }
}

/// One menu of the menu bar: Tauri's id for the two macOS fills in itself (Window's list of
/// windows, Help's search), its title, and its items with their titles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sub {
    pub id: Option<&'static str>,
    pub title: String,
    pub items: Vec<(Action, String)>,
}

/// The menus, in the language in effect, View's toggles worded for what `view` shows; the first,
/// the app menu, titled `app` (macOS shows the app's own name there whatever it's given).
pub fn menus(app: &str, view: ViewShown) -> Vec<Sub> {
    use Action::*;
    let sub = |id: Option<&'static str>, title: String, items: &[Action]| Sub {
        id,
        title,
        items: items
            .iter()
            .map(|a| (*a, a.key(view).map(i18n::t).unwrap_or_default()))
            .collect(),
    };
    vec![
        sub(
            None,
            app.to_string(),
            &[
                About, Separator, Services, Separator, Hide, HideOthers, ShowAll, Separator, Quit,
            ],
        ),
        sub(None, i18n::t("appMenu.menu.file"), &[CloseWindow]),
        sub(
            None,
            i18n::t("appMenu.menu.edit"),
            &[Undo, Redo, Separator, Cut, Copy, Paste, SelectAll],
        ),
        sub(
            None,
            i18n::t("appMenu.menu.view"),
            &[
                ToggleSidebar,
                ToggleRecent,
                ToggleFilters,
                Separator,
                Fullscreen,
            ],
        ),
        sub(
            Some(WINDOW_SUBMENU_ID),
            i18n::t("appMenu.menu.window"),
            &[Minimize, Zoom, Separator, CloseWindow],
        ),
        sub(Some(HELP_SUBMENU_ID), i18n::t("appMenu.menu.help"), &[]),
    ]
}

/// Sets the menus in the language in effect: at launch, and again when the language changes.
/// Menus that can't be made or set leave the ones there, still working, in the language before.
pub fn set(app: &AppHandle) {
    if let Ok(menu) = build(app) {
        let _ = app.set_menu(menu);
    }
}

/// What the window shows now (mu-167): kept, and the menus set again so View's toggles say what
/// a click will do. The argument is three booleans and nothing else (`ViewShown`).
#[tauri::command]
pub fn set_view_menu(app: AppHandle, shown: ViewShown) {
    if remember(shown) {
        set(&app);
    }
}

/// Keeps `shown`; whether it changed what the menu says.
fn remember(shown: ViewShown) -> bool {
    match VIEW.lock() {
        Ok(mut v) if *v != shown => {
            *v = shown;
            true
        }
        Ok(_) => false,
        // A poisoned lock still holds a whole ViewShown: take it over.
        Err(e) => {
            *e.into_inner() = shown;
            true
        }
    }
}

/// The region a menu item's id toggles: one of View's wired toggles, or none.
pub fn region_of(id: &str) -> Option<&'static str> {
    TOGGLES
        .iter()
        .find(|(_, t)| t.id == id && t.enabled)
        .map(|(_, t)| t.region)
}

/// A menu click (or its key): a View toggle sends the main window `view-toggle` with its region,
/// a fixed word from this table; anything else is someone else's.
pub fn on_event(app: &AppHandle, id: &str) {
    if let Some(region) = region_of(id) {
        let _ = app.emit_to("main", VIEW_TOGGLE, region);
    }
}

/// The event the window listens for (App.tsx).
pub const VIEW_TOGGLE: &str = "view-toggle";

/// The menu bar's menus (`menus`), as Tauri's default makes them.
fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let info = app.package_info();
    let bundle = &app.config().bundle;
    let about = AboutMetadata {
        name: Some(info.name.clone()),
        version: Some(info.version.to_string()),
        copyright: bundle.copyright.clone(),
        authors: bundle.publisher.clone().map(|p| vec![p]),
        ..Default::default()
    };
    let subs = menus(&info.name, view_now())
        .into_iter()
        .map(|s| {
            let items = s
                .items
                .iter()
                .map(|(a, text)| item(app, *a, text, &about))
                .collect::<tauri::Result<Vec<_>>>()?;
            let items: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(Item::as_item).collect();
            match s.id {
                Some(id) => Submenu::with_id_and_items(app, id, &s.title, true, &items),
                None => Submenu::with_items(app, &s.title, true, &items),
            }
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let subs: Vec<&dyn IsMenuItem<Wry>> = subs.iter().map(|s| s as &dyn IsMenuItem<Wry>).collect();
    Menu::with_items(app, &subs)
}

/// An item as made: one of macOS's own, or a View toggle of Muster's.
enum Item {
    Own(PredefinedMenuItem<Wry>),
    Toggle(MenuItem<Wry>),
}

impl Item {
    fn as_item(&self) -> &dyn IsMenuItem<Wry> {
        match self {
            Item::Own(i) => i,
            Item::Toggle(i) => i,
        }
    }
}

/// The item for `action`, titled `text`: macOS's own, or a View toggle with its id and key.
fn item(app: &AppHandle, action: Action, text: &str, about: &AboutMetadata) -> tauri::Result<Item> {
    if let Some(t) = action.toggle() {
        return MenuItem::with_id(app, t.id, text, t.enabled, Some(t.accelerator))
            .map(Item::Toggle);
    }
    let text = Some(text);
    match action {
        Action::About => PredefinedMenuItem::about(app, text, Some(about.clone())),
        Action::Services => PredefinedMenuItem::services(app, text),
        Action::Hide => PredefinedMenuItem::hide(app, text),
        Action::HideOthers => PredefinedMenuItem::hide_others(app, text),
        Action::ShowAll => PredefinedMenuItem::show_all(app, text),
        Action::Quit => PredefinedMenuItem::quit(app, text),
        Action::CloseWindow => PredefinedMenuItem::close_window(app, text),
        Action::Undo => PredefinedMenuItem::undo(app, text),
        Action::Redo => PredefinedMenuItem::redo(app, text),
        Action::Cut => PredefinedMenuItem::cut(app, text),
        Action::Copy => PredefinedMenuItem::copy(app, text),
        Action::Paste => PredefinedMenuItem::paste(app, text),
        Action::SelectAll => PredefinedMenuItem::select_all(app, text),
        Action::Fullscreen => PredefinedMenuItem::fullscreen(app, text),
        Action::Minimize => PredefinedMenuItem::minimize(app, text),
        Action::Zoom => PredefinedMenuItem::maximize(app, text),
        Action::Separator => PredefinedMenuItem::separator(app),
        // Not reached: TOGGLES gives each of these its item above.
        Action::ToggleSidebar | Action::ToggleRecent | Action::ToggleFilters => {
            PredefinedMenuItem::separator(app)
        }
    }
    .map(Item::Own)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Action::*;

    /// Each menu's title and its items' titles, separators as "-".
    fn said(subs: &[Sub]) -> Vec<(String, Vec<String>)> {
        subs.iter()
            .map(|s| {
                let items = s
                    .items
                    .iter()
                    .map(|(a, t)| {
                        if *a == Separator {
                            "-".into()
                        } else {
                            t.clone()
                        }
                    })
                    .collect();
                (s.title.clone(), items)
            })
            .collect()
    }

    fn owned(menus: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
        menus
            .iter()
            .map(|(t, items)| (t.to_string(), items.iter().map(|i| i.to_string()).collect()))
            .collect()
    }

    /// Tauri's default menus (`Menu::default` on macOS), with Show All beside Hide Others.
    #[test]
    fn the_menus_are_tauris_default_in_english() {
        crate::i18n::stub::app("en", || {
            assert_eq!(
                said(&menus("Muster", ViewShown::default())),
                owned(&[
                    (
                        "Muster",
                        &[
                            "About Muster",
                            "-",
                            "Services",
                            "-",
                            "Hide Muster",
                            "Hide Others",
                            "Show All",
                            "-",
                            "Quit Muster",
                        ],
                    ),
                    ("File", &["Close Window"]),
                    (
                        "Edit",
                        &["Undo", "Redo", "-", "Cut", "Copy", "Paste", "Select All"],
                    ),
                    (
                        "View",
                        &[
                            "Hide Sidebar",
                            "Hide Recently Finished",
                            "Hide Filters",
                            "-",
                            "Toggle Full Screen",
                        ],
                    ),
                    ("Window", &["Minimize", "Zoom", "-", "Close Window"]),
                    ("Help", &[]),
                ])
            );
        });
    }

    #[test]
    fn the_menus_speak_persian_and_muster_keeps_its_name() {
        crate::i18n::stub::app("fa", || {
            assert_eq!(
                said(&menus("Muster", ViewShown::default())),
                owned(&[
                    (
                        "Muster",
                        &[
                            "درباره‌ی Muster",
                            "-",
                            "خدمات",
                            "-",
                            "پنهان کردن Muster",
                            "پنهان کردن بقیه",
                            "نمایش همه",
                            "-",
                            "خروج از Muster",
                        ],
                    ),
                    ("فایل", &["بستن پنجره"]),
                    (
                        "ویرایش",
                        &["واگرد", "ازنو", "-", "برش", "کپی", "چسباندن", "انتخاب همه"],
                    ),
                    (
                        "نمایش",
                        &[
                            "پنهان کردن نوار کناری",
                            "پنهان کردن «اخیراً انجام شد»",
                            "پنهان کردن نوار فیلتر",
                            "-",
                            "تغییر حالت تمام‌صفحه",
                        ],
                    ),
                    ("پنجره", &["کوچک کردن", "بزرگ کردن", "-", "بستن پنجره"]),
                    ("راهنما", &[]),
                ])
            );
            // No English word but Muster's name.
            for (title, items) in said(&menus("Muster", ViewShown::default())).iter().skip(1) {
                for text in std::iter::once(title).chain(items) {
                    let rest = text.replace("Muster", "");
                    assert!(!rest.chars().any(|c| c.is_ascii_alphabetic()), "{text}");
                }
            }
        });
    }

    /// The same actions in every language, and Window and Help carry Tauri's ids, so macOS
    /// lists the windows in one and puts its search in the other.
    #[test]
    fn the_actions_and_ids_are_the_same_in_every_language() {
        let actions = |l: &'static str| {
            crate::i18n::stub::app(l, || {
                menus("Muster", ViewShown::default())
                    .into_iter()
                    .map(|s| {
                        (
                            s.id,
                            s.items.into_iter().map(|(a, _)| a).collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
        };
        let en = actions("en");
        assert_eq!(en, actions("fa"));
        let ids: Vec<Option<&str>> = en.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            [
                None,
                None,
                None,
                None,
                Some(WINDOW_SUBMENU_ID),
                Some(HELP_SUBMENU_ID)
            ]
        );
        assert_eq!(en[0].1.last(), Some(&Quit));
        assert_eq!(
            en[2].1,
            [Undo, Redo, Separator, Cut, Copy, Paste, SelectAll]
        );
    }

    /// The app menu takes the name it's given; separators have no title.
    #[test]
    fn the_app_menu_is_titled_with_the_apps_name_and_separators_are_blank() {
        let subs = menus("Muster Dev", ViewShown::default());
        assert_eq!(subs[0].title, "Muster Dev");
        for s in &subs {
            for (a, t) in &s.items {
                assert_eq!(*a == Separator, t.is_empty(), "{a:?}");
            }
        }
    }

    /// View's items (mu-167), as worded for `view`.
    fn view_menu(lang: &'static str, view: ViewShown) -> Vec<String> {
        crate::i18n::stub::app(lang, || said(&menus("Muster", view))[3].1.clone())
    }

    /// Each toggle says what a click will do: Hide while it shows, Show while it's hidden.
    #[test]
    fn view_toggles_say_what_a_click_will_do() {
        let hidden = ViewShown {
            sidebar: false,
            recent: true,
            filters: false,
        };
        assert_eq!(
            view_menu("en", hidden),
            [
                "Show Sidebar",
                "Hide Recently Finished",
                "Show Filters",
                "-",
                "Toggle Full Screen"
            ]
        );
        let none = ViewShown {
            sidebar: false,
            recent: false,
            filters: false,
        };
        assert_eq!(
            view_menu("fa", none),
            [
                "نمایش نوار کناری",
                "نمایش «اخیراً انجام شد»",
                "نمایش نوار فیلتر",
                "-",
                "تغییر حالت تمام‌صفحه"
            ]
        );
    }

    /// The toggles' ids, keys and regions: ⌃⌘S, ⌃⌘R and ⌥⌘F, each a fixed region word; the
    /// sidebar wired, the other two disabled until the window toggles them (mu-169).
    #[test]
    fn view_toggles_have_their_ids_keys_and_regions() {
        let got: Vec<_> = [ToggleSidebar, ToggleRecent, ToggleFilters]
            .iter()
            .map(|a| a.toggle().expect("a View toggle has its row in TOGGLES"))
            .map(|t| (t.id, t.region, t.accelerator, t.enabled))
            .collect();
        assert_eq!(
            got,
            [
                ("view-sidebar", "sidebar", "Ctrl+Cmd+S", true),
                ("view-recent", "recent", "Ctrl+Cmd+R", false),
                ("view-filters", "filters", "Alt+Cmd+F", false),
            ]
        );
        for a in [About, Quit, Fullscreen, Separator] {
            assert_eq!(a.toggle(), None, "{a:?}");
        }
        // Not ⌃⌘F, macOS's Full Screen, nor anything another item has.
        let keys: std::collections::HashSet<_> = got.iter().map(|g| g.2).collect();
        assert_eq!(keys.len(), 3);
        assert!(!keys.contains("Ctrl+Cmd+F"));
    }

    /// Tauri drops a key it can't read without a word (`accelerator.parse().ok()`), so each is
    /// checked against muda's grammar: modifiers it names, then one letter.
    #[test]
    fn view_toggle_keys_are_ones_muda_reads() {
        for (_, t) in TOGGLES {
            let parts: Vec<&str> = t.accelerator.split('+').collect();
            let (key, mods) = parts.split_last().expect("an accelerator has a key");
            assert!(
                key.len() == 1 && key.chars().all(|c| c.is_ascii_uppercase()),
                "{}",
                t.accelerator
            );
            for m in mods {
                assert!(
                    matches!(
                        m.to_uppercase().as_str(),
                        "CTRL" | "CONTROL" | "CMD" | "COMMAND" | "SUPER" | "ALT" | "OPTION"
                    ),
                    "{}",
                    t.accelerator
                );
            }
        }
    }

    /// A click on a wired toggle names its region; a disabled one, another menu's item or an
    /// unknown id names none, so the window hears only `sidebar` today.
    #[test]
    fn a_click_names_its_region_and_nothing_else() {
        assert_eq!(region_of("view-sidebar"), Some("sidebar"));
        assert_eq!(region_of("view-recent"), None);
        assert_eq!(region_of("view-filters"), None);
        for id in [
            "",
            "sidebar",
            "show",
            "quit",
            "view-sidebar ",
            "VIEW-SIDEBAR",
        ] {
            assert_eq!(region_of(id), None, "{id:?}");
        }
        assert_eq!(VIEW_TOGGLE, "view-toggle");
    }

    /// `set_view_menu` takes three booleans and nothing else.
    #[test]
    fn set_view_menu_takes_three_booleans_and_nothing_else() {
        let read = |s: &str| serde_json::from_str::<ViewShown>(s);
        assert_eq!(
            read(r#"{"sidebar":false,"recent":true,"filters":true}"#).ok(),
            Some(ViewShown {
                sidebar: false,
                recent: true,
                filters: true
            })
        );
        for bad in [
            r#"{"sidebar":false,"recent":true}"#,
            r#"{"sidebar":false,"recent":true,"filters":true,"path":"/etc"}"#,
            r#"{"sidebar":"false","recent":true,"filters":true}"#,
            r#"{"sidebar":0,"recent":true,"filters":true}"#,
            r#"{"sidebar":null,"recent":true,"filters":true}"#,
            r#"[false,true,true]"#,
            r#""sidebar""#,
            "{}",
        ] {
            assert!(read(bad).is_err(), "{bad}");
        }
    }

    /// The menu is set again only when what the window shows changed; until it says, everything
    /// shows.
    #[test]
    fn the_menu_is_set_again_only_on_a_change() {
        assert_eq!(ViewShown::default(), view_now());
        let hidden = ViewShown {
            sidebar: false,
            ..ViewShown::default()
        };
        assert!(remember(hidden));
        assert!(!remember(hidden));
        assert_eq!(view_now(), hidden);
        assert!(remember(ViewShown::default()));
        assert_eq!(view_now(), ViewShown::default());
    }

    /// The core listens for the menu and takes the window's word (lib.rs).
    #[test]
    fn the_core_wires_the_event_and_the_command() {
        let lib = include_str!("lib.rs");
        assert!(lib.contains(".on_menu_event(|app, e| app_menu::on_event(app, e.id().as_ref()))"));
        assert!(lib.contains("app_menu::set_view_menu,"));
        let src = include_str!("app_menu.rs");
        assert!(src.contains(r#"app.emit_to("main", VIEW_TOGGLE, region)"#));
    }
}
