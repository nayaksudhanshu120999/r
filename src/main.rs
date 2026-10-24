#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod engine;
mod executor;
mod indexer;
mod models;

use config::AppConfig;
use engine::QueryEngine;
use executor::execute_candidate;
use indexer::build_start_menu_index;
use models::{CandidateItem, ShelfCategory};
use slint::{ComponentHandle, LogicalSize, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

struct PaletteState {
    engine: QueryEngine,
    current_results: Vec<CandidateItem>,
    active_shelf: ShelfCategory,
}

fn main() -> Result<(), slint::PlatformError> {
    let config = AppConfig::load_or_create("config.toml");
    let indexed_apps = build_start_menu_index();
    let engine = QueryEngine::new(config, indexed_apps);

    let state = Rc::new(RefCell::new(PaletteState {
        engine,
        current_results: Vec::new(),
        active_shelf: ShelfCategory::All,
    }));

    let ui = PaletteWindow::new()?;

    // Enforce initial Compact Mode window size (input box only)
    ui.window().set_size(LogicalSize::new(680.0, 64.0));

    // Handle live text input updates
    {
        let ui_weak = ui.as_weak();
        let state_ref = Rc::clone(&state);
        ui.on_query_changed(move |query_text| {
            if let Some(window) = ui_weak.upgrade() {
                refresh_results(&window, &state_ref, query_text.as_str());
            }
        });
    }

    // Handle Quick Access Shelf anchor selection ("Apps", "Cmds", "Web", "Settings")
    {
        let ui_weak = ui.as_weak();
        let state_ref = Rc::clone(&state);
        ui.on_shelf_selected(move |shelf_label| {
            if let Some(window) = ui_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.active_shelf = ShelfCategory::from_label(shelf_label.as_str());
                }
                let current_query = window.get_query_text().to_string();
                refresh_results(&window, &state_ref, &current_query);
            }
        });
    }

    // Handle standard item activation (Enter or Left-Click)
    {
        let ui_weak = ui.as_weak();
        let state_ref = Rc::clone(&state);
        ui.on_activate_item(move |index| {
            if index < 0 {
                return;
            }
            let maybe_item = {
                let st = state_ref.borrow();
                st.current_results.get(index as usize).cloned()
            };

            if let Some(item) = maybe_item {
                if execute_candidate(&item, false).is_ok() {
                    if let Some(window) = ui_weak.upgrade() {
                        reset_or_close(&window, &state_ref);
                    }
                }
            }
        });
    }

    // Handle Context Menu 'Run as Admin' via PowerShell elevation
    {
        let ui_weak = ui.as_weak();
        let state_ref = Rc::clone(&state);
        ui.on_run_item_admin(move |index| {
            if index < 0 {
                return;
            }
            let maybe_item = {
                let st = state_ref.borrow();
                st.current_results.get(index as usize).cloned()
            };

            if let Some(item) = maybe_item {
                if execute_candidate(&item, true).is_ok() {
                    if let Some(window) = ui_weak.upgrade() {
                        reset_or_close(&window, &state_ref);
                    }
                }
            }
        });
    }

    // Handle Escape key: clear input first to shrink to Compact Mode, or exit if already empty
    {
        let ui_weak = ui.as_weak();
        let state_ref = Rc::clone(&state);
        ui.on_escape_pressed(move || {
            if let Some(window) = ui_weak.upgrade() {
                let current_query = window.get_query_text();
                if !current_query.is_empty() {
                    window.set_query_text(SharedString::from(""));
                    window.set_active_shelf(SharedString::from("All"));
                    state_ref.borrow_mut().active_shelf = ShelfCategory::All;
                    refresh_results(&window, &state_ref, "");
                } else {
                    let _ = window.hide();
                }
            }
        });
    }

    ui.run()
}

fn refresh_results(
    window: &PaletteWindow,
    state_ref: &Rc<RefCell<PaletteState>>,
    raw_query: &str,
) {
    let candidates = {
        let mut st = state_ref.borrow_mut();
        let shelf = st.active_shelf;
        let evaluated = st.engine.evaluate(raw_query, shelf);
        st.current_results = evaluated.clone();
        evaluated
    };

    let ui_items: Vec<UIResultItem> = candidates
        .iter()
        .map(|c| UIResultItem {
            title: SharedString::from(c.title.as_str()),
            subtitle: SharedString::from(c.subtitle.as_str()),
            badge: SharedString::from(c.badge),
            category: SharedString::from(c.category.as_str()),
            can_elevate: c.can_elevate,
        })
        .collect();

    let is_compact = raw_query.is_empty() || ui_items.is_empty();
    let model = VecModel::from(ui_items);
    window.set_results(ModelRc::new(model));
    window.set_selected_index(0);
    window.set_context_menu_visible(false);

    // Synchronize native OS window height with Compact Mode vs Default Mode
    let target_height = if is_compact { 64.0 } else { 436.0 };
    window
        .window()
        .set_size(LogicalSize::new(680.0, target_height));
}

fn reset_or_close(window: &PaletteWindow, state_ref: &Rc<RefCell<PaletteState>>) {
    window.set_query_text(SharedString::from(""));
    window.set_active_shelf(SharedString::from("All"));
    state_ref.borrow_mut().active_shelf = ShelfCategory::All;
    refresh_results(window, state_ref, "");
}
