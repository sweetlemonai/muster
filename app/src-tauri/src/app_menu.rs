//! Muster's menus in the Mac's menu bar (mu-147): Tauri's default ones (the app menu, File, Edit,
//! View, Window, Help), made here so their titles are in the language in effect (`i18n`,
//! `native.json`'s `appMenu`) and set again when it changes (`set`). Every item is one of macOS's
//! own actions (`PredefinedMenuItem`), given only its title; Muster's name stays "Muster".

use tauri::menu::{
    AboutMetadata, IsMenuItem, Menu, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
    WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Wry};

use crate::i18n;

/// One of macOS's own menu actions, or a separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
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

impl Action {
    /// Its title in `native.json`; a separator has none.
    fn key(self) -> Option<&'static str> {
        Some(match self {
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

/// The menus, in the language in effect; the first, the app menu, titled `app` (macOS shows the
/// app's own name there whatever it's given).
pub fn menus(app: &str) -> Vec<Sub> {
    use Action::*;
    let sub = |id: Option<&'static str>, title: String, items: &[Action]| Sub {
        id,
        title,
        items: items
            .iter()
            .map(|a| (*a, a.key().map(i18n::t).unwrap_or_default()))
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
        sub(None, i18n::t("appMenu.menu.view"), &[Fullscreen]),
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
    let subs = menus(&info.name)
        .into_iter()
        .map(|s| {
            let items = s
                .items
                .iter()
                .map(|(a, text)| item(app, *a, text, &about))
                .collect::<tauri::Result<Vec<_>>>()?;
            let items: Vec<&dyn IsMenuItem<Wry>> =
                items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
            match s.id {
                Some(id) => Submenu::with_id_and_items(app, id, &s.title, true, &items),
                None => Submenu::with_items(app, &s.title, true, &items),
            }
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let subs: Vec<&dyn IsMenuItem<Wry>> = subs.iter().map(|s| s as &dyn IsMenuItem<Wry>).collect();
    Menu::with_items(app, &subs)
}

/// macOS's own item for `action`, titled `text`.
fn item(
    app: &AppHandle,
    action: Action,
    text: &str,
    about: &AboutMetadata,
) -> tauri::Result<PredefinedMenuItem<Wry>> {
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
    }
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
                said(&menus("Muster")),
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
                    ("View", &["Toggle Full Screen"]),
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
                said(&menus("Muster")),
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
                    ("نمایش", &["تغییر حالت تمام‌صفحه"]),
                    ("پنجره", &["کوچک کردن", "بزرگ کردن", "-", "بستن پنجره"]),
                    ("راهنما", &[]),
                ])
            );
            // No English word but Muster's name.
            for (title, items) in said(&menus("Muster")).iter().skip(1) {
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
                menus("Muster")
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
        let subs = menus("Muster Dev");
        assert_eq!(subs[0].title, "Muster Dev");
        for s in &subs {
            for (a, t) in &s.items {
                assert_eq!(*a == Separator, t.is_empty(), "{a:?}");
            }
        }
    }
}
