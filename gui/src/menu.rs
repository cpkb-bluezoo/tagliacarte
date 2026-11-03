//! The native menu bar.
//!
//! Rust owns the structure and the shortcuts; the page owns the words. The
//! menu starts with English labels, and the page sends its own (from the same
//! ARB strings as the rest of the UI, in the language it is showing) once it
//! has them, and again if the language changes. A choice of one of the page's
//! own actions (`ACTIONS`) goes back as a `menuAction` event; the rest are the
//! system's (Hide, Quit, Copy and so on) and never reach the page. Which
//! message actions can be used right now is also the page's to say.
//!
//! macOS only for now: elsewhere the menu is built but not attached to a window.

use std::collections::HashMap;

use muda::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu, accelerator::Accelerator};

/// One menu item the page acts on.
pub struct Action {
    pub id: &'static str,
    /// English, until the page sends the real label.
    pub label: &'static str,
    pub accelerator: Option<&'static str>,
    /// Starts disabled, and is enabled by the page when it can be used.
    pub dynamic: bool,
}

pub const ACTIONS: &[Action] = &[
    Action { id: "settings", label: "Settings\u{2026}", accelerator: Some("CmdOrCtrl+,"), dynamic: false },
    Action { id: "newMessage", label: "New Message", accelerator: Some("CmdOrCtrl+N"), dynamic: false },
    Action { id: "getNewMail", label: "Get New Mail", accelerator: Some("CmdOrCtrl+Shift+N"), dynamic: true },
    Action { id: "reply", label: "Reply", accelerator: Some("CmdOrCtrl+R"), dynamic: true },
    Action { id: "replyAll", label: "Reply All", accelerator: Some("CmdOrCtrl+Shift+R"), dynamic: true },
    Action { id: "forward", label: "Forward", accelerator: Some("CmdOrCtrl+Shift+F"), dynamic: true },
];

/// Items whose words the page may set but that it does not act on: the
/// system's own commands, and the menus' titles.
pub const SYSTEM_LABELS: &[&str] = &[
    "about", "hide", "hideOthers", "showAll", "quit", "closeWindow", "undo", "redo", "cut", "copy", "paste",
    "selectAll", "minimize", "zoom", "fullscreen", "bringAllToFront", "services", "menuFile", "menuEdit",
    "menuView", "menuMessage", "menuWindow", "menuHelp",
];

fn action(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|a| a.id == id)
}

/// A choice that is the page's to act on.
pub fn is_action(id: &str) -> bool {
    action(id).is_some()
}

/// `ids` and `labels` pair up, and every id is one the menu has.
pub fn check_labels(ids: &[String], labels: &[String]) -> Result<Vec<(String, String)>, String> {
    if ids.len() != labels.len() {
        return Err(format!("{} ids but {} labels", ids.len(), labels.len()));
    }
    for id in ids {
        if !is_action(id) && !SYSTEM_LABELS.contains(&id.as_str()) {
            return Err(format!("no menu item '{id}'"));
        }
    }
    Ok(ids.iter().cloned().zip(labels.iter().cloned()).collect())
}

/// Every id names an item the page can enable; the rest are always available.
pub fn check_enabled(ids: &[String]) -> Result<Vec<String>, String> {
    for id in ids {
        if !action(id).is_some_and(|a| a.dynamic) {
            return Err(format!("'{id}' is not a menu item that is enabled and disabled"));
        }
    }
    Ok(ids.to_vec())
}

enum Handle {
    Item(MenuItem),
    System(PredefinedMenuItem),
    Menu(Submenu),
}

/// The menu bar and the items whose words and state change.
pub struct MenuBar {
    pub menu: Menu,
    items: HashMap<&'static str, Handle>,
}

impl MenuBar {
    /// Must be called on the main thread (AppKit requires it).
    pub fn build() -> Result<MenuBar, muda::Error> {
        let mut items: HashMap<&'static str, Handle> = HashMap::new();

        let custom = |id: &'static str, items: &mut HashMap<&'static str, Handle>| -> MenuItem {
            let a = action(id).expect("an id from ACTIONS");
            let accel = a.accelerator.map(|s| s.parse::<Accelerator>().expect("a valid accelerator"));
            let item = MenuItem::with_id(a.id, a.label, !a.dynamic, accel);
            items.insert(a.id, Handle::Item(item.clone()));
            item
        };
        macro_rules! system {
            ($id:expr, $item:expr) => {{
                let item = $item;
                items.insert($id, Handle::System(item.clone()));
                item
            }};
        }

        let about = system!(
            "about",
            PredefinedMenuItem::about(
                None,
                Some(AboutMetadata {
                    name: Some("Tagliacarte".to_owned()),
                    version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                    copyright: Some("Copyright \u{a9} 2026 Chris Burdess".to_owned()),
                    ..Default::default()
                }),
            )
        );
        let settings = custom("settings", &mut items);
        let app = Submenu::with_items(
            "Tagliacarte",
            true,
            &[
                &about,
                &PredefinedMenuItem::separator(),
                &settings,
                &PredefinedMenuItem::separator(),
                &system!("services", PredefinedMenuItem::services(None)),
                &PredefinedMenuItem::separator(),
                &system!("hide", PredefinedMenuItem::hide(None)),
                &system!("hideOthers", PredefinedMenuItem::hide_others(None)),
                &system!("showAll", PredefinedMenuItem::show_all(None)),
                &PredefinedMenuItem::separator(),
                &system!("quit", PredefinedMenuItem::quit(None)),
            ],
        )?;

        let new_message = custom("newMessage", &mut items);
        let get_new_mail = custom("getNewMail", &mut items);
        let file = Submenu::with_items(
            "File",
            true,
            &[
                &new_message,
                &get_new_mail,
                &PredefinedMenuItem::separator(),
                &system!("closeWindow", PredefinedMenuItem::close_window(None)),
            ],
        )?;
        items.insert("menuFile", Handle::Menu(file.clone()));

        // WKWebView only honours Cmd+C/V/X/A/Z when the app has an Edit menu.
        let edit = Submenu::with_items(
            "Edit",
            true,
            &[
                &system!("undo", PredefinedMenuItem::undo(None)),
                &system!("redo", PredefinedMenuItem::redo(None)),
                &PredefinedMenuItem::separator(),
                &system!("cut", PredefinedMenuItem::cut(None)),
                &system!("copy", PredefinedMenuItem::copy(None)),
                &system!("paste", PredefinedMenuItem::paste(None)),
                &system!("selectAll", PredefinedMenuItem::select_all(None)),
            ],
        )?;
        items.insert("menuEdit", Handle::Menu(edit.clone()));

        let view = Submenu::with_items("View", true, &[&system!("fullscreen", PredefinedMenuItem::fullscreen(None))])?;
        items.insert("menuView", Handle::Menu(view.clone()));

        let reply = custom("reply", &mut items);
        let reply_all = custom("replyAll", &mut items);
        let forward = custom("forward", &mut items);
        let message = Submenu::with_items("Message", true, &[&reply, &reply_all, &forward])?;
        items.insert("menuMessage", Handle::Menu(message.clone()));

        let window = Submenu::with_items(
            "Window",
            true,
            &[
                &system!("minimize", PredefinedMenuItem::minimize(None)),
                &system!("zoom", PredefinedMenuItem::zoom(None)),
                &PredefinedMenuItem::separator(),
                &system!("bringAllToFront", PredefinedMenuItem::bring_all_to_front(None)),
            ],
        )?;
        items.insert("menuWindow", Handle::Menu(window.clone()));

        // No help to point at yet; the menu still gives macOS somewhere to put
        // its search field.
        let help = Submenu::with_items("Help", true, &[])?;
        items.insert("menuHelp", Handle::Menu(help.clone()));

        let menu = Menu::new();
        menu.append_items(&[&app, &file, &edit, &view, &message, &window, &help])?;
        #[cfg(target_os = "macos")]
        {
            window.set_as_windows_menu_for_nsapp();
            help.set_as_help_menu_for_nsapp();
        }
        Ok(MenuBar { menu, items })
    }

    /// Words from the page. Ids were checked with `check_labels`.
    pub fn set_labels(&self, labels: &[(String, String)]) {
        for (id, text) in labels {
            match self.items.get(id.as_str()) {
                Some(Handle::Item(i)) => i.set_text(text),
                Some(Handle::System(i)) => i.set_text(text),
                Some(Handle::Menu(m)) => m.set_text(text),
                None => {}
            }
        }
    }

    /// Exactly the page's dynamic items in `enabled` can be used.
    pub fn set_enabled(&self, enabled: &[String]) {
        for a in ACTIONS.iter().filter(|a| a.dynamic) {
            if let Some(Handle::Item(i)) = self.items.get(a.id) {
                i.set_enabled(enabled.iter().any(|e| e == a.id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_id_is_unique_and_every_shortcut_parses() {
        let mut seen = std::collections::HashSet::new();
        for a in ACTIONS {
            assert!(seen.insert(a.id), "duplicate id {}", a.id);
            if let Some(s) = a.accelerator {
                s.parse::<Accelerator>().unwrap_or_else(|e| panic!("{} has a bad shortcut {s}: {e}", a.id));
            }
        }
        for id in SYSTEM_LABELS {
            assert!(seen.insert(id), "duplicate id {id}");
        }
        let shortcuts: Vec<_> = ACTIONS.iter().filter_map(|a| a.accelerator).collect();
        let distinct: std::collections::HashSet<_> = shortcuts.iter().collect();
        assert_eq!(shortcuts.len(), distinct.len(), "two items share a shortcut");
    }

    #[test]
    fn the_page_may_label_known_items_only_and_in_pairs() {
        let ok = check_labels(&strings(&["reply", "quit", "menuFile"]), &strings(&["Antworten", "Beenden", "Datei"])).unwrap();
        assert_eq!(ok[0], ("reply".to_owned(), "Antworten".to_owned()));
        assert_eq!(ok.len(), 3);
        assert!(check_labels(&strings(&["reply"]), &strings(&[])).unwrap_err().contains("1 ids but 0"));
        assert!(check_labels(&strings(&["nope"]), &strings(&["x"])).unwrap_err().contains("'nope'"));
        assert!(check_labels(&[], &[]).unwrap().is_empty());
    }

    #[test]
    fn only_the_items_that_come_and_go_can_be_enabled() {
        assert_eq!(check_enabled(&strings(&["reply", "forward"])).unwrap().len(), 2);
        assert!(check_enabled(&strings(&["settings"])).is_err(), "Settings is always available");
        assert!(check_enabled(&strings(&["quit"])).is_err());
        assert!(check_enabled(&strings(&["nope"])).is_err());
        assert!(check_enabled(&[]).unwrap().is_empty(), "nothing enabled is a valid answer");
    }

    #[test]
    fn a_choice_is_the_pages_only_for_its_own_actions() {
        assert!(is_action("newMessage") && is_action("reply"));
        assert!(!is_action("quit") && !is_action("menuFile") && !is_action(""));
    }
}
