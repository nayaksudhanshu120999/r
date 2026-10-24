#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod engine;
mod executor;
mod icon;
mod indexer;
mod models;
mod system;

use config::{AppConfig, CustomEntryConfig};
use engine::QueryEngine;
use executor::execute_candidate;
use icon::create_slint_app_icon;
use indexer::build_start_menu_index;
use models::{CandidateItem, ExecutionTarget};
use slint::{ComponentHandle, LogicalPosition, LogicalSize, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use system::{
    activate_window_by_title, check_outside_click_or_focus_loss, init_system_integration,
    primary_screen_size, SystemEvent,
};

slint::include_modules!();

const PALETTE_TITLE: &str = "Command Palette";
const CONFIG_FILE_PATH: &str = "config.toml";
const WINDOW_WIDTH: f32 = 680.0;
const COMPACT_HEIGHT: f32 = 68.0;
const MAX_EXPANDED_HEIGHT: f32 = 460.0;

struct PaletteState {
    engine: QueryEngine,
    current_candidates: Vec<CandidateItem>,
    palette_visible: bool,
    last_shown_at: Instant,
}

fn main() -> Result<(), slint::PlatformError> {
    let config = AppConfig::load_or_create(CONFIG_FILE_PATH);
    let indexed_apps = build_start_menu_index(&config.custom_app_dirs);
    let engine = QueryEngine::new(config, indexed_apps);

    let app_icon = create_slint_app_icon();

    let palette_ui = PaletteWindow::new()?;
    palette_ui.set_app_icon(app_icon.clone());
    palette_ui.set_dynamic_height(COMPACT_HEIGHT);
    palette_ui
        .window()
        .set_size(LogicalSize::new(WINDOW_WIDTH, COMPACT_HEIGHT));

    let settings_ui = SettingsWindow::new()?;
    settings_ui.set_app_icon(app_icon);

    let state = Rc::new(RefCell::new(PaletteState {
        engine,
        current_candidates: Vec::new(),
        palette_visible: true,
        last_shown_at: Instant::now(),
    }));

    sync_settings_ui(&settings_ui, &state.borrow().engine.config);

    // Query input edited callback
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_query_changed(move |query| {
            if let Some(window) = ui_weak.upgrade() {
                refresh_results(&window, &state_ref, query.as_str());
            }
        });
    }

    // Activate item (Enter or Left-Click) -> Executes task and auto-hides palette
    {
        let ui_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_activate_item(move |index| {
            let candidate = {
                let st = state_ref.borrow();
                st.current_candidates.get(index as usize).cloned()
            };

            if let Some(item) = candidate {
                if matches!(item.target, ExecutionTarget::OpenPaletteSettings) {
                    if let Some(window) = ui_weak.upgrade() {
                        hide_palette(&window, &state_ref);
                    }
                    if let Some(settings_win) = settings_weak.upgrade() {
                        sync_settings_ui(&settings_win, &state_ref.borrow().engine.config);
                        let _ = settings_win.show();
                        activate_window_by_title("Command Palette Settings");
                    }
                } else if execute_candidate(&item, false).is_ok() {
                    if let Some(window) = ui_weak.upgrade() {
                        hide_palette(&window, &state_ref);
                    }
                }
            }
        });
    }

    // Run as Administrator -> Executes elevated task and auto-hides palette
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_run_item_admin(move |index| {
            let candidate = {
                let st = state_ref.borrow();
                st.current_candidates.get(index as usize).cloned()
            };

            if let Some(item) = candidate {
                if execute_candidate(&item, true).is_ok() {
                    if let Some(window) = ui_weak.upgrade() {
                        hide_palette(&window, &state_ref);
                    }
                }
            }
        });
    }

    // Hide App directly from suggestion list / context menu
    {
        let ui_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_hide_app_item(move |index| {
            let app_title = {
                let st = state_ref.borrow();
                st.current_candidates
                    .get(index as usize)
                    .map(|c| c.title.clone())
            };

            if let Some(title) = app_title {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.hidden_apps.push(title);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
                if let Some(settings_win) = settings_weak.upgrade() {
                    sync_settings_ui(&settings_win, &state_ref.borrow().engine.config);
                }
                if let Some(window) = ui_weak.upgrade() {
                    let current_q = window.get_query_text().to_string();
                    refresh_results(&window, &state_ref, &current_q);
                }
            }
        });
    }

    // Open Settings GUI from Palette gear button
    {
        let ui_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_open_settings(move || {
            if let Some(window) = ui_weak.upgrade() {
                hide_palette(&window, &state_ref);
            }
            if let Some(settings_win) = settings_weak.upgrade() {
                sync_settings_ui(&settings_win, &state_ref.borrow().engine.config);
                let _ = settings_win.show();
                activate_window_by_title("Command Palette Settings");
            }
        });
    }

    // Escape key handling: clear query if non-empty, otherwise hide palette
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_escape_pressed(move || {
            if let Some(window) = ui_weak.upgrade() {
                if !window.get_query_text().is_empty() {
                    window.set_query_text(SharedString::from(""));
                    refresh_results(&window, &state_ref, "");
                } else {
                    hide_palette(&window, &state_ref);
                }
            }
        });
    }

    // Wire up SettingsWindow callbacks
    wire_settings_callbacks(&settings_ui, &palette_ui, &state);

    // Initialize Taskbar System Tray & Global Ctrl+Space Shortcut
    let sys_rx = init_system_integration();

    // Background timer: handles Tray/Hotkey events & auto-hide on focus loss / outside click
    let monitor_timer = Timer::default();
    {
        let ui_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(&state);

        monitor_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(35),
            move || {
                // 1. Process System Tray & Global Ctrl+Space Hotkey events
                while let Ok(ev) = sys_rx.try_recv() {
                    match ev {
                        SystemEvent::TogglePalette => {
                            if let Some(window) = ui_weak.upgrade() {
                                let is_vis = state_ref.borrow().palette_visible;
                                if is_vis {
                                    hide_palette(&window, &state_ref);
                                } else {
                                    show_and_focus_palette(&window, &state_ref);
                                }
                            }
                        }
                        SystemEvent::OpenSettings => {
                            if let Some(window) = ui_weak.upgrade() {
                                hide_palette(&window, &state_ref);
                            }
                            if let Some(settings_win) = settings_weak.upgrade() {
                                sync_settings_ui(&settings_win, &state_ref.borrow().engine.config);
                                let _ = settings_win.show();
                                activate_window_by_title("Command Palette Settings");
                            }
                        }
                        SystemEvent::ReloadConfig => {
                            let new_cfg = AppConfig::load_or_create(CONFIG_FILE_PATH);
                            let new_apps = build_start_menu_index(&new_cfg.custom_app_dirs);
                            {
                                let mut st = state_ref.borrow_mut();
                                st.engine = QueryEngine::new(new_cfg, new_apps);
                            }
                            if let Some(settings_win) = settings_weak.upgrade() {
                                sync_settings_ui(&settings_win, &state_ref.borrow().engine.config);
                                settings_win.set_status_message(SharedString::from("Index & config reloaded!"));
                            }
                        }
                        SystemEvent::QuitApp => {
                            let _ = slint::quit_event_loop();
                        }
                    }
                }

                // 2. Auto-hide Palette when user clicks outside or window loses focus
                let (is_vis, elapsed_ms) = {
                    let st = state_ref.borrow();
                    (st.palette_visible, st.last_shown_at.elapsed().as_millis())
                };

                if is_vis && elapsed_ms > 180 {
                    let check_focus = elapsed_ms > 350;
                    if check_outside_click_or_focus_loss(PALETTE_TITLE, check_focus) {
                        if let Some(window) = ui_weak.upgrade() {
                            hide_palette(&window, &state_ref);
                        }
                    }
                }
            },
        );
    }

    // Show & center palette on initial launch, then keep app running in tray indefinitely
    show_and_focus_palette(&palette_ui, &state);
    slint::run_event_loop_until_quit()
}

fn show_and_focus_palette(window: &PaletteWindow, state_ref: &Rc<RefCell<PaletteState>>) {
    let (screen_w, screen_h) = primary_screen_size();
    let x = ((screen_w - WINDOW_WIDTH) * 0.5).max(0.0);
    let y = (screen_h * 0.20).max(60.0);

    window.window().set_position(LogicalPosition::new(x, y));
    window.set_context_menu_visible(false);

    {
        let mut st = state_ref.borrow_mut();
        st.palette_visible = true;
        st.last_shown_at = Instant::now();
    }

    let _ = window.show();
    activate_window_by_title(PALETTE_TITLE);
    window.invoke_focus_input();
}

fn hide_palette(window: &PaletteWindow, state_ref: &Rc<RefCell<PaletteState>>) {
    window.set_context_menu_visible(false);
    window.set_query_text(SharedString::from(""));
    refresh_results(window, state_ref, "");
    state_ref.borrow_mut().palette_visible = false;
    let _ = window.hide();
}

fn refresh_results(window: &PaletteWindow, state_ref: &Rc<RefCell<PaletteState>>, query: &str) {
    let candidates = {
        let state = state_ref.borrow();
        state.engine.evaluate(query)
    };

    let mut header_count = 0usize;
    let mut prev_category: Option<&str> = None;

    let ui_items: Vec<UIResultItem> = candidates
        .iter()
        .map(|c| {
            let cat_str = c.category.as_str();
            let is_first = prev_category != Some(cat_str);
            if is_first {
                header_count += 1;
                prev_category = Some(cat_str);
            }

            UIResultItem {
                title: SharedString::from(c.title.as_str()),
                badge: SharedString::from(c.badge),
                category: SharedString::from(cat_str),
                category_header: SharedString::from(c.category.header_title()),
                icon_text: SharedString::from(c.category.icon_glyph()),
                can_elevate: c.can_elevate,
                is_app: c.category == models::ShelfCategory::Apps,
                is_first_in_category: is_first,
            }
        })
        .collect();

    {
        let mut state = state_ref.borrow_mut();
        state.current_candidates = candidates;
    }

    let target_height = if ui_items.is_empty() {
        COMPACT_HEIGHT
    } else {
        let rows_h = (ui_items.len() as f32) * 45.0;
        let headers_h = (header_count as f32) * 27.0;
        (COMPACT_HEIGHT + rows_h + headers_h + 42.0).min(MAX_EXPANDED_HEIGHT)
    };

    window.set_dynamic_height(target_height);
    window
        .window()
        .set_size(LogicalSize::new(WINDOW_WIDTH, target_height));

    window.set_selected_index(0);
    window.set_results(ModelRc::from(Rc::new(VecModel::from(ui_items))));
}

fn sync_settings_ui(settings_ui: &SettingsWindow, config: &AppConfig) {
    settings_ui.set_show_apps(config.show_apps);
    settings_ui.set_custom_cmd_path(SharedString::from(config.custom_cmd_path.as_str()));
    settings_ui.set_custom_settings_path(SharedString::from(
        config.custom_system_settings_path.as_str(),
    ));

    // Category Preference Order
    let cat_items: Vec<UICategoryOrderItem> = config
        .ordered_categories()
        .into_iter()
        .map(|cat| UICategoryOrderItem {
            id: SharedString::from(cat.as_str()),
            label: SharedString::from(cat.display_name()),
            description: SharedString::from(cat.description()),
        })
        .collect();
    settings_ui.set_category_order(ModelRc::from(Rc::new(VecModel::from(cat_items))));

    // Custom Entries
    let custom_items: Vec<UICustomEntry> = config
        .custom_entries
        .iter()
        .map(|e| UICustomEntry {
            title: SharedString::from(e.title.as_str()),
            category: SharedString::from(e.category.as_str()),
            triggers: SharedString::from(e.triggers.join(", ")),
            target_path: SharedString::from(e.target_path.as_str()),
            args: SharedString::from(e.args.join(" ")),
        })
        .collect();
    settings_ui.set_custom_entries(ModelRc::from(Rc::new(VecModel::from(custom_items))));

    // Hidden Apps
    let hidden: Vec<SharedString> = config
        .hidden_apps
        .iter()
        .map(|s| SharedString::from(s.as_str()))
        .collect();
    settings_ui.set_hidden_apps(ModelRc::from(Rc::new(VecModel::from(hidden))));

    // Custom App Directories
    let dirs: Vec<SharedString> = config
        .custom_app_dirs
        .iter()
        .map(|s| SharedString::from(s.as_str()))
        .collect();
    settings_ui.set_custom_app_dirs(ModelRc::from(Rc::new(VecModel::from(dirs))));

    // Web Bangs
    let mut bangs: Vec<UIBangEntry> = config
        .bangs
        .iter()
        .map(|(k, v)| UIBangEntry {
            key: SharedString::from(k.as_str()),
            url: SharedString::from(v.as_str()),
        })
        .collect();
    bangs.sort_by(|a, b| a.key.cmp(&b.key));
    settings_ui.set_bang_entries(ModelRc::from(Rc::new(VecModel::from(bangs))));
}

fn wire_settings_callbacks(
    settings_ui: &SettingsWindow,
    palette_ui: &PaletteWindow,
    state: &Rc<RefCell<PaletteState>>,
) {
    // Toggle Show Apps
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_toggle_show_apps(move |enabled| {
            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.show_apps = enabled;
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Saved app visibility preference"));
            }
        });
    }

    // Move Category Up in Preference Order
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_move_category_up(move |idx| {
            let i = idx as usize;
            if i > 0 {
                {
                    let mut st = state_ref.borrow_mut();
                    if i < st.engine.config.category_order.len() {
                        st.engine.config.category_order.swap(i, i - 1);
                        st.engine.config.save(CONFIG_FILE_PATH);
                    }
                }
                if let Some(win) = settings_weak.upgrade() {
                    sync_settings_ui(&win, &state_ref.borrow().engine.config);
                    win.set_status_message(SharedString::from("Updated category preference order"));
                }
            }
        });
    }

    // Move Category Down in Preference Order
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_move_category_down(move |idx| {
            let i = idx as usize;
            {
                let mut st = state_ref.borrow_mut();
                if i + 1 < st.engine.config.category_order.len() {
                    st.engine.config.category_order.swap(i, i + 1);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Updated category preference order"));
            }
        });
    }

    // Save Custom Default Paths for CMD & System Settings
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_save_system_paths(move |cmd_path, settings_path| {
            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.custom_cmd_path = cmd_path.to_string();
                st.engine.config.custom_system_settings_path = settings_path.to_string();
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Saved custom CMD & Settings paths!"));
            }
        });
    }

    // Add Custom Command or System Setting Entry
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_add_custom_entry(move |category, title, triggers, target_path, args| {
            let title_str = title.trim().to_string();
            let path_str = target_path.trim().to_string();
            if title_str.is_empty() || path_str.is_empty() {
                return;
            }

            let mut trig_vec: Vec<String> = triggers
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if trig_vec.is_empty() {
                trig_vec.push(title_str.to_ascii_lowercase());
            }

            let args_vec: Vec<String> = args
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.custom_entries.push(CustomEntryConfig {
                    title: title_str,
                    category: category.to_string(),
                    triggers: trig_vec,
                    target_path: path_str,
                    args: args_vec,
                });
                st.engine.config.save(CONFIG_FILE_PATH);
            }

            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Added custom entry!"));
            }
        });
    }

    // Remove Custom Entry
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_remove_custom_entry(move |idx| {
            let i = idx as usize;
            {
                let mut st = state_ref.borrow_mut();
                if i < st.engine.config.custom_entries.len() {
                    st.engine.config.custom_entries.remove(i);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Removed custom entry"));
            }
        });
    }

    // Hide App by Name
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_hide_app_by_name(move |app_name| {
            let trimmed = app_name.trim().to_string();
            if !trimmed.is_empty() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.hidden_apps.push(trimmed);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
                if let Some(win) = settings_weak.upgrade() {
                    sync_settings_ui(&win, &state_ref.borrow().engine.config);
                    win.set_status_message(SharedString::from("Added app to hidden list"));
                }
            }
        });
    }

    // Unhide App
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_unhide_app(move |idx| {
            let i = idx as usize;
            {
                let mut st = state_ref.borrow_mut();
                if i < st.engine.config.hidden_apps.len() {
                    st.engine.config.hidden_apps.remove(i);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Restored application to suggestions"));
            }
        });
    }

    // Add Custom App Scan Directory
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_add_app_dir(move |dir_path| {
            let trimmed = dir_path.trim().to_string();
            if !trimmed.is_empty() {
                {
                    let mut st = state_ref.borrow_mut();
                    if !st.engine.config.custom_app_dirs.contains(&trimmed) {
                        st.engine.config.custom_app_dirs.push(trimmed);
                        st.engine.config.save(CONFIG_FILE_PATH);
                        st.engine.apps = build_start_menu_index(&st.engine.config.custom_app_dirs);
                    }
                }
                if let Some(win) = settings_weak.upgrade() {
                    sync_settings_ui(&win, &state_ref.borrow().engine.config);
                    win.set_status_message(SharedString::from("Added folder & re-indexed apps!"));
                }
            }
        });
    }

    // Remove Custom App Scan Directory
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_remove_app_dir(move |idx| {
            let i = idx as usize;
            {
                let mut st = state_ref.borrow_mut();
                if i < st.engine.config.custom_app_dirs.len() {
                    st.engine.config.custom_app_dirs.remove(i);
                    st.engine.config.save(CONFIG_FILE_PATH);
                    st.engine.apps = build_start_menu_index(&st.engine.config.custom_app_dirs);
                }
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Removed folder & re-indexed apps"));
            }
        });
    }

    // Add Web Bang
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_add_bang(move |key, url| {
            let k = key.trim().to_ascii_lowercase();
            let u = url.trim().to_string();
            if !k.is_empty() && !u.is_empty() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.bangs.insert(k, u);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
                if let Some(win) = settings_weak.upgrade() {
                    sync_settings_ui(&win, &state_ref.borrow().engine.config);
                    win.set_status_message(SharedString::from("Saved web bang shortcut!"));
                }
            }
        });
    }

    // Remove Web Bang
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_remove_bang(move |key| {
            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.bangs.remove(key.as_str());
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Removed web bang shortcut"));
            }
        });
    }

    // Reload Index & Config
    {
        let settings_weak = settings_ui.as_weak();
        let palette_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_reload_index(move || {
            let cfg = AppConfig::load_or_create(CONFIG_FILE_PATH);
            let apps = build_start_menu_index(&cfg.custom_app_dirs);
            {
                let mut st = state_ref.borrow_mut();
                st.engine = QueryEngine::new(cfg, apps);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine.config);
                win.set_status_message(SharedString::from("Rescanned Start Menu & Config!"));
            }
            if let Some(pal) = palette_weak.upgrade() {
                let q = pal.get_query_text().to_string();
                refresh_results(&pal, &state_ref, &q);
            }
        });
    }
}
