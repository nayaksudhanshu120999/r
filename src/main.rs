#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod engine;
mod executor;
mod extensions;
mod icon;
mod indexer;
mod models;
mod system;

use config::{AppConfig, CustomCommandItem, CustomSettingItem, CustomThemeConfig};
use engine::{QueryEngine, BUILTIN_COMMANDS, BUILTIN_SETTINGS};
use executor::execute_candidate;
use extensions::{
    ensure_extensions_guide_file, open_extension_target_file, open_extensions_guide,
    ExtensionEntry, ExtensionKind, BUILTIN_NOTEPAD_ID,
};
use indexer::{build_custom_files_index, build_start_menu_index};
use models::{CandidateItem, ExecutionOutcome, ShelfCategory};
use slint::{Color, ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use system::SystemEvent;

slint::include_modules!();

const CONFIG_PATH: &str = "config.toml";
const PALETTE_WIN_TITLE: &str = "Command Palette";
const SETTINGS_WIN_TITLE: &str = "Command Palette Settings";

struct RuntimeState {
    engine: QueryEngine,
    candidates: Vec<CandidateItem>,
}

fn main() -> Result<(), slint::PlatformError> {
    if !system::ensure_single_instance_or_activate_existing() {
        return Ok(());
    }

    // Always ensure EXTENSIONS_GUIDE.md exists next to the executable/config so it never gets lost
    let _ = ensure_extensions_guide_file();

    let config = AppConfig::load_or_create(CONFIG_PATH);
    let sys_rx = system::init_system_integration(config.run_on_startup, &config.hotkey);

    let apps = build_start_menu_index();
    let custom_files = build_custom_files_index(&config.custom_file_scan_paths);

    let state = Rc::new(RefCell::new(RuntimeState {
        engine: QueryEngine::new(config, apps, custom_files),
        candidates: Vec::new(),
    }));

    let palette = PaletteWindow::new()?;
    let settings = SettingsWindow::new()?;

    let app_icon = icon::create_slint_app_icon();
    palette.set_app_icon(app_icon.clone());
    settings.set_app_icon(app_icon);

    let palette_visible = Rc::new(Cell::new(false));
    let focus_grace_ticks = Rc::new(Cell::new(0u8));

    center_palette_on_screen(&palette);
    sync_settings_ui(&settings, &palette, &state.borrow().engine);

    // 1. Live Search Input + Automatic Bang Pill Chip Extraction (including `note `)
    {
        let state_ref = Rc::clone(&state);
        let palette_weak = palette.as_weak();
        palette.on_query_edited(move |text| {
            let Some(ui) = palette_weak.upgrade() else {
                return;
            };
            let current_bang = ui.get_active_bang().to_string();

            if current_bang.is_empty() {
                let maybe_bang = {
                    let st = state_ref.borrow();
                    st.engine.extract_bang_prefix(text.as_str())
                };
                if let Some((bang_key, bang_label, remainder)) = maybe_bang {
                    ui.set_active_bang(SharedString::from(bang_key.as_str()));
                    ui.set_active_bang_label(SharedString::from(bang_label.as_str()));
                    ui.set_query_text(SharedString::from(remainder.as_str()));
                    refresh_palette_results(&ui, &state_ref, &bang_key, &remainder);
                    return;
                }
            }

            refresh_palette_results(&ui, &state_ref, &current_bang, text.as_str());
        });
    }

    // 2. Clear Bang Chip
    {
        let state_ref = Rc::clone(&state);
        let palette_weak = palette.as_weak();
        palette.on_clear_active_bang(move || {
            if let Some(ui) = palette_weak.upgrade() {
                ui.set_active_bang(SharedString::default());
                ui.set_active_bang_label(SharedString::default());
                let current_q = ui.get_query_text().to_string();
                refresh_palette_results(&ui, &state_ref, "", &current_q);
            }
        });
    }

    // 3. Up/Down Keyboard Selection
    {
        let palette_weak = palette.as_weak();
        palette.on_move_selection(move |delta| {
            if let Some(ui) = palette_weak.upgrade() {
                let total = ui.get_total_candidates();
                if total <= 0 {
                    ui.set_selected_candidate_index(0);
                    return;
                }
                let current = ui.get_selected_candidate_index();
                let next = (current + delta).rem_euclid(total);
                ui.set_selected_candidate_index(next);
            }
        });
    }

    // 4. Execute Selected Candidate
    {
        let state_ref = Rc::clone(&state);
        let palette_weak = palette.as_weak();
        let settings_weak = settings.as_weak();
        let vis_ref = Rc::clone(&palette_visible);
        palette.on_trigger_selected(move |run_as_admin| {
            if let Some(ui) = palette_weak.upgrade() {
                let idx = ui.get_selected_candidate_index();
                execute_and_handle_outcome(
                    &ui,
                    &settings_weak,
                    &state_ref,
                    &vis_ref,
                    idx,
                    run_as_admin,
                );
            }
        });
    }

    // 5. Execute Candidate by Mouse Click
    {
        let state_ref = Rc::clone(&state);
        let palette_weak = palette.as_weak();
        let settings_weak = settings.as_weak();
        let vis_ref = Rc::clone(&palette_visible);
        palette.on_trigger_candidate_at(move |idx, run_as_admin| {
            if let Some(ui) = palette_weak.upgrade() {
                execute_and_handle_outcome(
                    &ui,
                    &settings_weak,
                    &state_ref,
                    &vis_ref,
                    idx,
                    run_as_admin,
                );
            }
        });
    }

    // 6. Open Settings Window
    {
        let state_ref = Rc::clone(&state);
        let palette_weak = palette.as_weak();
        let settings_weak = settings.as_weak();
        let vis_ref = Rc::clone(&palette_visible);
        palette.on_open_settings_window(move || {
            if let Some(p_ui) = palette_weak.upgrade() {
                reset_and_hide_palette(&p_ui, &vis_ref);
                if let Some(s_ui) = settings_weak.upgrade() {
                    sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                    let _ = s_ui.show();
                    system::enable_window_maximize(SETTINGS_WIN_TITLE);
                    system::activate_window_by_title(SETTINGS_WIN_TITLE);
                }
            }
        });
    }

    // 7. Escape Hides Palette
    {
        let palette_weak = palette.as_weak();
        let vis_ref = Rc::clone(&palette_visible);
        palette.on_hide_requested(move || {
            if let Some(ui) = palette_weak.upgrade() {
                reset_and_hide_palette(&ui, &vis_ref);
            }
        });
    }

    // 8. Close Event on Settings Window hides instead of quitting tray app
    {
        let settings_weak = settings.as_weak();
        settings.window().on_close_requested(move || {
            if let Some(s_ui) = settings_weak.upgrade() {
                let _ = s_ui.hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }

    wire_settings_callbacks(&settings, &palette, &state);

    // 9. Poll Tray, Global Hotkey, Outside Click, and Config Reload Events
    let poll_timer = Timer::default();
    {
        let palette_weak = palette.as_weak();
        let settings_weak = settings.as_weak();
        let state_ref = Rc::clone(&state);
        let vis_ref = Rc::clone(&palette_visible);
        let grace_ref = Rc::clone(&focus_grace_ticks);

        poll_timer.start(TimerMode::Repeated, Duration::from_millis(30), move || {
            while let Ok(ev) = sys_rx.try_recv() {
                match ev {
                    SystemEvent::ShowPalette => {
                        if let Some(p_ui) = palette_weak.upgrade() {
                            summon_palette(&p_ui, &state_ref, &vis_ref, &grace_ref);
                        }
                    }
                    SystemEvent::TogglePalette => {
                        if let Some(p_ui) = palette_weak.upgrade() {
                            if vis_ref.get() {
                                reset_and_hide_palette(&p_ui, &vis_ref);
                            } else {
                                summon_palette(&p_ui, &state_ref, &vis_ref, &grace_ref);
                            }
                        }
                    }
                    SystemEvent::OpenSettings => {
                        if let (Some(s_ui), Some(p_ui)) =
                            (settings_weak.upgrade(), palette_weak.upgrade())
                        {
                            reset_and_hide_palette(&p_ui, &vis_ref);
                            sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                            let _ = s_ui.show();
                            system::enable_window_maximize(SETTINGS_WIN_TITLE);
                            system::activate_window_by_title(SETTINGS_WIN_TITLE);
                        }
                    }
                    SystemEvent::ReloadConfig => {
                        if let (Some(s_ui), Some(p_ui)) =
                            (settings_weak.upgrade(), palette_weak.upgrade())
                        {
                            reload_engine_and_ui(&state_ref, &p_ui, &s_ui, "Reloaded configuration");
                        }
                    }
                    SystemEvent::QuitApp => {
                        system::shutdown_system_integration();
                        let _ = slint::quit_event_loop();
                        return;
                    }
                }
            }

            if vis_ref.get() {
                let g = grace_ref.get();
                if g > 0 {
                    grace_ref.set(g - 1);
                } else if system::check_outside_click_or_focus_loss(PALETTE_WIN_TITLE) {
                    if let Some(p_ui) = palette_weak.upgrade() {
                        reset_and_hide_palette(&p_ui, &vis_ref);
                    }
                }
            }
        });
    }

    summon_palette(&palette, &state, &palette_visible, &focus_grace_ticks);
    slint::run_event_loop_until_quit()?;
    system::shutdown_system_integration();
    Ok(())
}

fn wire_settings_callbacks(
    settings: &SettingsWindow,
    palette: &PaletteWindow,
    state: &Rc<RefCell<RuntimeState>>,
) {
    // Open Bundled EXTENSIONS_GUIDE.md with 1 click
    {
        let s_weak = settings.as_weak();
        settings.on_open_extensions_guide(move || {
            let result = open_extensions_guide();
            if let Some(s_ui) = s_weak.upgrade() {
                if result.is_ok() {
                    s_ui.set_status_message(SharedString::from(
                        "Opened EXTENSIONS_GUIDE.md in default viewer",
                    ));
                } else {
                    s_ui.set_status_message(SharedString::from(
                        "Failed to open EXTENSIONS_GUIDE.md",
                    ));
                }
            }
        });
    }

    // Save & Open Selected Notepad (.txt / .md) File with 1 click
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_open_notepad_file(move |file_path| {
            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.set_notepad_file_path(file_path.as_str());
                st.engine.config.save(CONFIG_PATH);
            }
            let open_res = open_extension_target_file(file_path.as_str());
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                if let Ok(resolved) = open_res {
                    s_ui.set_status_message(SharedString::from(format!(
                        "Opened {}",
                        resolved.display()
                    )));
                } else {
                    s_ui.set_status_message(SharedString::from("Could not open note file"));
                }
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Open Any File-Backed Extension's .txt/.md File with 1 click
    {
        let s_weak = settings.as_weak();
        settings.on_open_extension_file(move |target_file| {
            let open_res = open_extension_target_file(target_file.as_str());
            if let Some(s_ui) = s_weak.upgrade() {
                if let Ok(resolved) = open_res {
                    s_ui.set_status_message(SharedString::from(format!(
                        "Opened {}",
                        resolved.display()
                    )));
                } else {
                    s_ui.set_status_message(SharedString::from("Could not open extension file"));
                }
            }
        });
    }

    // Toggle Show Apps
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_toggle_show_apps(move |enabled| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.show_apps = enabled;
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                s_ui.set_status_message(SharedString::from("Saved app visibility setting"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Toggle Startup
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        settings.on_toggle_startup(move |enabled| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.run_on_startup = enabled;
            st.engine.config.save(CONFIG_PATH);
            if let Some(s_ui) = s_weak.upgrade() {
                s_ui.set_status_message(SharedString::from("Updated Windows Startup registry"));
            }
        });
    }

    // Select Global Hotkey
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        settings.on_select_hotkey(move |hk| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.hotkey = hk.to_string();
            st.engine.config.save(CONFIG_PATH);
            let active_hk = st.engine.config.hotkey.clone();
            drop(st);
            if let Some(s_ui) = s_weak.upgrade() {
                s_ui.set_hotkey(SharedString::from(active_hk.as_str()));
                s_ui.set_status_message(SharedString::from("Updated global hotkey"));
            }
        });
    }

    // Toggle CmdPal Own Shortcuts
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_toggle_cmdpal_shortcuts(move |enabled| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.enable_cmdpal_shortcuts = enabled;
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated CmdPal shortcuts visibility"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Toggle All Inbuilt System Commands
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_toggle_all_builtin_cmds(move |enabled| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.enable_builtin_cmds = enabled;
            if enabled {
                st.engine.config.disabled_builtin_cmds.clear();
            }
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated inbuilt system commands"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Toggle Individual Inbuilt System Command
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_toggle_builtin_cmd_item(move |id, enabled| {
            let mut st = state_ref.borrow_mut();
            if enabled {
                st.engine.config.enable_builtin_cmds = true;
            }
            st.engine.config.set_builtin_cmd_enabled(id.as_str(), enabled);
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated command shortcut"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Toggle All Inbuilt Windows System Settings
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_toggle_all_builtin_settings(move |enabled| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.enable_builtin_settings = enabled;
            if enabled {
                st.engine.config.disabled_builtin_settings.clear();
            }
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated inbuilt Windows settings"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Toggle Individual Inbuilt Windows System Setting
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_toggle_builtin_setting_item(move |id, enabled| {
            let mut st = state_ref.borrow_mut();
            if enabled {
                st.engine.config.enable_builtin_settings = true;
            }
            st.engine
                .config
                .set_builtin_setting_enabled(id.as_str(), enabled);
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated Windows setting shortcut"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Select Default Search Engine
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_select_default_search_engine(move |engine_id| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.default_search_engine = engine_id.to_string();
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated default search engine"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Save Custom Search Engine URL
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_save_custom_search_url(move |custom_url| {
            let trimmed = custom_url.trim();
            if trimmed.is_empty() {
                return;
            }
            let mut st = state_ref.borrow_mut();
            st.engine.config.custom_search_url = trimmed.to_string();
            st.engine.config.default_search_engine = "custom".to_string();
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Saved custom search engine URL"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Save Notepad Target .txt or .md File Path
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_save_notepad_file_path(move |file_path| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.set_notepad_file_path(file_path.as_str());
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from(
                    "Saved Notepad (.txt/.md) target file!",
                ));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Toggle Extension Enabled/Disabled
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_toggle_extension_item(move |ext_id, enabled| {
            let mut st = state_ref.borrow_mut();
            if let Some(ext) = st
                .engine
                .config
                .extensions
                .iter_mut()
                .find(|e| e.id.eq_ignore_ascii_case(ext_id.as_str()))
            {
                ext.enabled = enabled;
            }
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated extension status"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Remove Extension
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_remove_extension_item(move |ext_id| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.remove_extension_by_id(ext_id.as_str());
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Removed extension"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Restore Built-in Notepad Extension
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_restore_builtin_notepad(move || {
            let mut st = state_ref.borrow_mut();
            st.engine.config.restore_builtin_notepad();
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from(
                    "Restored Quick Notepad ('note') extension!",
                ));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Add Custom Extension
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_add_custom_extension(move |name, bang, kind_str, target| {
            let clean_name = name.trim();
            let clean_bang = bang
                .trim()
                .trim_start_matches('!')
                .to_ascii_lowercase();
            let clean_target = target.trim();
            if clean_name.is_empty() || clean_bang.is_empty() || clean_target.is_empty() {
                return;
            }

            let kind = ExtensionKind::from_label(kind_str.as_str());
            let icon = match kind {
                ExtensionKind::NotepadFile => "📝",
                ExtensionKind::QuickAppendFile => "📌",
                ExtensionKind::ShellCommand => "⚡",
                ExtensionKind::WebTemplate => "🌐",
            };

            let (target_file, command_template) = match kind {
                ExtensionKind::NotepadFile | ExtensionKind::QuickAppendFile => {
                    (clean_target.to_string(), String::new())
                }
                ExtensionKind::ShellCommand | ExtensionKind::WebTemplate => {
                    (String::new(), clean_target.to_string())
                }
            };

            let ext_id = format!("ext_{}", clean_bang);
            let new_ext = ExtensionEntry {
                id: ext_id.clone(),
                name: clean_name.to_string(),
                description: format!("Custom {} extension", kind.as_str()),
                bang: clean_bang,
                icon: icon.to_string(),
                enabled: true,
                kind,
                target_file,
                command_template,
            };

            let mut st = state_ref.borrow_mut();
            st.engine.config.extensions.retain(|e| {
                !e.id.eq_ignore_ascii_case(&ext_id)
                    && !e.bang.eq_ignore_ascii_case(&new_ext.bang)
            });
            st.engine.config.extensions.push(new_ext);
            st.engine.config.save(CONFIG_PATH);
            drop(st);

            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Added new extension!"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Select Theme Mode
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_select_theme_mode(move |mode| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.theme = mode.to_string();
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated theme mode"));
            }
        });
    }

    // Apply Custom Theme
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_apply_custom_theme(
            move |win_bg, surf_bg, card_bg, sel_bg, border, accent, txt_pri, txt_mut| {
                let mut st = state_ref.borrow_mut();
                st.engine.config.theme = "custom".to_string();
                st.engine.config.custom_theme = CustomThemeConfig {
                    window_bg: win_bg.trim().to_string(),
                    surface_bg: surf_bg.trim().to_string(),
                    card_bg: card_bg.trim().to_string(),
                    selected_bg: sel_bg.trim().to_string(),
                    border: border.trim().to_string(),
                    accent: accent.trim().to_string(),
                    text_primary: txt_pri.trim().to_string(),
                    text_muted: txt_mut.trim().to_string(),
                };
                st.engine.config.save(CONFIG_PATH);
                drop(st);
                if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                    sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                    s_ui.set_status_message(SharedString::from("Applied custom theme colors"));
                }
            },
        );
    }

    // Reset Custom Theme
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_reset_custom_theme(move || {
            let mut st = state_ref.borrow_mut();
            st.engine.config.theme = "powertoys".to_string();
            st.engine.config.custom_theme = CustomThemeConfig::default();
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Reset to PowerToys dark theme"));
            }
        });
    }

    // Move Category Order
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_move_category(move |index, delta| {
            let mut st = state_ref.borrow_mut();
            let len = st.engine.config.category_order.len() as i32;
            let target = index + delta;
            if index >= 0 && index < len && target >= 0 && target < len {
                st.engine
                    .config
                    .category_order
                    .swap(index as usize, target as usize);
                st.engine.config.save(CONFIG_PATH);
            }
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated category priority order"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Reset Category Order
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_reset_category_order(move || {
            let mut st = state_ref.borrow_mut();
            st.engine.config.category_order = ShelfCategory::all_default_order()
                .iter()
                .map(|c| c.as_str().to_string())
                .collect();
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Reset category order"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Toggle Individual App Visibility
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_toggle_app_item(move |app_name, visible| {
            let mut st = state_ref.borrow_mut();
            st.engine
                .config
                .set_app_visibility(app_name.as_str(), visible);
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Updated application visibility"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Add Custom File Scan Path
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_add_scan_path(move |new_path| {
            let trimmed = new_path.trim();
            if trimmed.is_empty() {
                return;
            }
            let mut st = state_ref.borrow_mut();
            st.engine
                .config
                .custom_file_scan_paths
                .push(trimmed.to_string());
            st.engine.config.save(CONFIG_PATH);
            st.engine.custom_files =
                build_custom_files_index(&st.engine.config.custom_file_scan_paths);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Added custom scan path"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Remove Custom File Scan Path
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_remove_scan_path(move |idx| {
            let mut st = state_ref.borrow_mut();
            let i = idx as usize;
            if i < st.engine.config.custom_file_scan_paths.len() {
                st.engine.config.custom_file_scan_paths.remove(i);
                st.engine.config.save(CONFIG_PATH);
                st.engine.custom_files =
                    build_custom_files_index(&st.engine.config.custom_file_scan_paths);
            }
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Removed custom scan path"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Save System Paths
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_save_system_paths(move |cmd_path, settings_path| {
            let mut st = state_ref.borrow_mut();
            st.engine.config.custom_cmd_path = cmd_path.trim().to_string();
            st.engine.config.custom_system_settings_path = settings_path.trim().to_string();
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Saved system paths"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Add Custom Command
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_add_custom_command(move |name, command| {
            let n = name.trim();
            let c = command.trim();
            if n.is_empty() || c.is_empty() {
                return;
            }
            let mut st = state_ref.borrow_mut();
            st.engine.config.custom_commands.push(CustomCommandItem {
                name: n.to_string(),
                command: c.to_string(),
            });
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Added custom command"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Remove Custom Command
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_remove_custom_command(move |idx| {
            let mut st = state_ref.borrow_mut();
            let i = idx as usize;
            if i < st.engine.config.custom_commands.len() {
                st.engine.config.custom_commands.remove(i);
                st.engine.config.save(CONFIG_PATH);
            }
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Removed custom command"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Add Custom Setting
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_add_custom_setting(move |name, path| {
            let n = name.trim();
            let p = path.trim();
            if n.is_empty() || p.is_empty() {
                return;
            }
            let mut st = state_ref.borrow_mut();
            st.engine.config.custom_settings.push(CustomSettingItem {
                name: n.to_string(),
                path: p.to_string(),
            });
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Added custom setting"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Remove Custom Setting
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_remove_custom_setting(move |idx| {
            let mut st = state_ref.borrow_mut();
            let i = idx as usize;
            if i < st.engine.config.custom_settings.len() {
                st.engine.config.custom_settings.remove(i);
                st.engine.config.save(CONFIG_PATH);
            }
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Removed custom setting"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Add Bang Rule
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_add_bang_rule(move |trigger, url| {
            let clean_trig = trigger
                .trim()
                .trim_start_matches('!')
                .to_ascii_lowercase();
            let clean_url = url.trim().to_string();
            if clean_trig.is_empty() || clean_url.is_empty() {
                return;
            }
            let mut st = state_ref.borrow_mut();
            st.engine.config.bangs.insert(clean_trig, clean_url);
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Saved bang shortcut"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Remove Bang Rule
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_remove_bang_rule(move |trigger| {
            let key = trigger.trim().to_ascii_lowercase();
            let mut st = state_ref.borrow_mut();
            st.engine.config.bangs.remove(&key);
            st.engine.config.save(CONFIG_PATH);
            drop(st);
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                sync_settings_ui(&s_ui, &p_ui, &state_ref.borrow().engine);
                s_ui.set_status_message(SharedString::from("Removed bang shortcut"));
                refresh_palette_from_ui(&p_ui, &state_ref);
            }
        });
    }

    // Reload All Indexes & Config
    {
        let state_ref = Rc::clone(state);
        let s_weak = settings.as_weak();
        let p_weak = palette.as_weak();
        settings.on_reload_all(move || {
            let _ = ensure_extensions_guide_file();
            if let (Some(s_ui), Some(p_ui)) = (s_weak.upgrade(), p_weak.upgrade()) {
                reload_engine_and_ui(
                    &state_ref,
                    &p_ui,
                    &s_ui,
                    "Rescanned apps, files, extensions & config",
                );
            }
        });
    }
}

fn execute_and_handle_outcome(
    palette: &PaletteWindow,
    settings_weak: &slint::Weak<SettingsWindow>,
    state: &Rc<RefCell<RuntimeState>>,
    palette_visible: &Rc<Cell<bool>>,
    candidate_index: i32,
    run_as_admin: bool,
) {
    let (candidate_opt, custom_cmd_path) = {
        let st = state.borrow();
        let item = if candidate_index >= 0 {
            st.candidates.get(candidate_index as usize).cloned()
        } else {
            None
        };
        (item, st.engine.config.custom_cmd_path.clone())
    };

    let Some(candidate) = candidate_opt else {
        return;
    };

    let Ok(outcome) = execute_candidate(&candidate, run_as_admin, &custom_cmd_path) else {
        return;
    };

    match outcome {
        ExecutionOutcome::HidePalette => {
            reset_and_hide_palette(palette, palette_visible);
        }
        ExecutionOutcome::KeepOpenAndRefresh { clear_query } => {
            if clear_query {
                palette.set_query_text(SharedString::default());
            }
            let current_bang = palette.get_active_bang().to_string();
            let current_q = palette.get_query_text().to_string();
            refresh_palette_results(palette, state, &current_bang, &current_q);
            palette.invoke_focus_search_input();
        }
        ExecutionOutcome::ActivateBang { bang, label } => {
            palette.set_active_bang(SharedString::from(bang.as_str()));
            palette.set_active_bang_label(SharedString::from(label.as_str()));
            palette.set_query_text(SharedString::default());
            refresh_palette_results(palette, state, &bang, "");
            palette.invoke_focus_search_input();
        }
        ExecutionOutcome::OpenSettingsTab(tab) => {
            reset_and_hide_palette(palette, palette_visible);
            if let Some(s_ui) = settings_weak.upgrade() {
                sync_settings_ui(&s_ui, palette, &state.borrow().engine);
                s_ui.set_active_tab(tab);
                let _ = s_ui.show();
                system::enable_window_maximize(SETTINGS_WIN_TITLE);
                system::activate_window_by_title(SETTINGS_WIN_TITLE);
            }
        }
        ExecutionOutcome::ReloadAll => {
            if let Some(s_ui) = settings_weak.upgrade() {
                reload_engine_and_ui(state, palette, &s_ui, "Reloaded indexes & configuration");
            }
            reset_and_hide_palette(palette, palette_visible);
        }
    }
}

fn reload_engine_and_ui(
    state: &Rc<RefCell<RuntimeState>>,
    palette: &PaletteWindow,
    settings: &SettingsWindow,
    status_msg: &str,
) {
    let new_cfg = AppConfig::load_or_create(CONFIG_PATH);
    let new_apps = build_start_menu_index();
    let new_files = build_custom_files_index(&new_cfg.custom_file_scan_paths);
    {
        let mut st = state.borrow_mut();
        st.engine = QueryEngine::new(new_cfg, new_apps, new_files);
    }
    sync_settings_ui(settings, palette, &state.borrow().engine);
    settings.set_status_message(SharedString::from(status_msg));
    refresh_palette_from_ui(palette, state);
}

fn refresh_palette_from_ui(palette: &PaletteWindow, state: &Rc<RefCell<RuntimeState>>) {
    let bang = palette.get_active_bang().to_string();
    let q = palette.get_query_text().to_string();
    refresh_palette_results(palette, state, &bang, &q);
}

fn sync_settings_ui(settings: &SettingsWindow, palette: &PaletteWindow, engine: &QueryEngine) {
    let cfg = &engine.config;

    apply_theme_to_windows(palette, settings, cfg);

    settings.set_show_apps(cfg.show_apps);
    settings.set_run_on_startup(cfg.run_on_startup);
    settings.set_hotkey(SharedString::from(cfg.hotkey.as_str()));
    settings.set_theme_mode(SharedString::from(cfg.theme.as_str()));

    settings.set_enable_cmdpal_shortcuts(cfg.enable_cmdpal_shortcuts);
    settings.set_enable_builtin_cmds(cfg.enable_builtin_cmds);
    settings.set_enable_builtin_settings(cfg.enable_builtin_settings);

    let builtin_cmds: Vec<BuiltinToggleEntry> = BUILTIN_COMMANDS
        .iter()
        .map(|spec| BuiltinToggleEntry {
            id: SharedString::from(spec.id),
            title: SharedString::from(spec.title),
            subtitle: SharedString::from(spec.subtitle),
            enabled: cfg.is_builtin_cmd_enabled(spec.id),
        })
        .collect();
    settings.set_builtin_cmd_entries(ModelRc::new(VecModel::from(builtin_cmds)));

    let builtin_settings: Vec<BuiltinToggleEntry> = BUILTIN_SETTINGS
        .iter()
        .map(|spec| BuiltinToggleEntry {
            id: SharedString::from(spec.id),
            title: SharedString::from(spec.title),
            subtitle: SharedString::from(spec.subtitle),
            enabled: cfg.is_builtin_setting_enabled(spec.id),
        })
        .collect();
    settings.set_builtin_setting_entries(ModelRc::new(VecModel::from(builtin_settings)));

    settings.set_default_search_engine(SharedString::from(cfg.default_search_engine.as_str()));
    settings.set_custom_search_url(SharedString::from(cfg.custom_search_url.as_str()));

    if let Some(np) = cfg.notepad_extension() {
        settings.set_notepad_installed(true);
        settings.set_notepad_file_path(SharedString::from(np.target_file.as_str()));
    } else {
        settings.set_notepad_installed(false);
    }

    let ext_entries: Vec<ExtensionUiEntry> = cfg
        .extensions
        .iter()
        .map(|ext| {
            let meta = if !ext.target_file.is_empty() {
                format!("File: {} • {}", ext.target_file, ext.description)
            } else if !ext.command_template.is_empty() {
                format!("Template: {} • {}", ext.command_template, ext.description)
            } else {
                ext.description.clone()
            };

            ExtensionUiEntry {
                id: SharedString::from(ext.id.as_str()),
                name: SharedString::from(ext.name.as_str()),
                description: SharedString::from(ext.description.as_str()),
                bang: SharedString::from(ext.bang.as_str()),
                icon: SharedString::from(ext.icon.as_str()),
                kind: SharedString::from(ext.kind.as_str()),
                target_file: SharedString::from(ext.target_file.as_str()),
                target_meta: SharedString::from(meta.as_str()),
                is_file_backed: ext.kind.is_file_backed(),
                enabled: ext.enabled,
                is_builtin: ext.id.eq_ignore_ascii_case(BUILTIN_NOTEPAD_ID),
            }
        })
        .collect();
    settings.set_extension_entries(ModelRc::new(VecModel::from(ext_entries)));

    settings.set_custom_window_bg(SharedString::from(cfg.custom_theme.window_bg.as_str()));
    settings.set_custom_surface_bg(SharedString::from(cfg.custom_theme.surface_bg.as_str()));
    settings.set_custom_card_bg(SharedString::from(cfg.custom_theme.card_bg.as_str()));
    settings.set_custom_selected_bg(SharedString::from(cfg.custom_theme.selected_bg.as_str()));
    settings.set_custom_border(SharedString::from(cfg.custom_theme.border.as_str()));
    settings.set_custom_accent(SharedString::from(cfg.custom_theme.accent.as_str()));
    settings.set_custom_text_primary(SharedString::from(cfg.custom_theme.text_primary.as_str()));
    settings.set_custom_text_muted(SharedString::from(cfg.custom_theme.text_muted.as_str()));

    settings.set_custom_cmd_path(SharedString::from(cfg.custom_cmd_path.as_str()));
    settings.set_custom_system_settings_path(SharedString::from(
        cfg.custom_system_settings_path.as_str(),
    ));

    let cat_entries: Vec<CategoryOrderEntry> = cfg
        .ordered_categories()
        .into_iter()
        .map(|cat| CategoryOrderEntry {
            id: SharedString::from(cat.as_str()),
            display_name: SharedString::from(cat.display_name()),
            description: SharedString::from(cat.description()),
            icon: SharedString::from(cat.icon_glyph()),
        })
        .collect();
    settings.set_category_entries(ModelRc::new(VecModel::from(cat_entries)));

    let app_entries: Vec<AppToggleEntry> = engine
        .apps
        .iter()
        .map(|app| AppToggleEntry {
            name: SharedString::from(app.name.as_str()),
            path: SharedString::from(app.path.to_string_lossy().as_ref()),
            enabled: !cfg.is_app_hidden(&app.name),
        })
        .collect();
    settings.set_app_entries(ModelRc::new(VecModel::from(app_entries)));

    let scan_paths: Vec<SharedString> = cfg
        .custom_file_scan_paths
        .iter()
        .map(|p| SharedString::from(p.as_str()))
        .collect();
    settings.set_file_scan_paths(ModelRc::new(VecModel::from(scan_paths)));

    let custom_cmds: Vec<CustomCmdEntry> = cfg
        .custom_commands
        .iter()
        .map(|c| CustomCmdEntry {
            name: SharedString::from(c.name.as_str()),
            command: SharedString::from(c.command.as_str()),
        })
        .collect();
    settings.set_custom_commands(ModelRc::new(VecModel::from(custom_cmds)));

    let custom_sts: Vec<CustomSettingEntry> = cfg
        .custom_settings
        .iter()
        .map(|s| CustomSettingEntry {
            name: SharedString::from(s.name.as_str()),
            path: SharedString::from(s.path.as_str()),
        })
        .collect();
    settings.set_custom_settings(ModelRc::new(VecModel::from(custom_sts)));

    let mut bang_vec: Vec<BangRuleEntry> = cfg
        .bangs
        .iter()
        .map(|(k, v)| BangRuleEntry {
            trigger: SharedString::from(k.as_str()),
            url_template: SharedString::from(v.as_str()),
        })
        .collect();
    bang_vec.sort_by(|a, b| a.trigger.cmp(&b.trigger));
    settings.set_bang_entries(ModelRc::new(VecModel::from(bang_vec)));
}

fn apply_theme_to_windows(palette: &PaletteWindow, settings: &SettingsWindow, cfg: &AppConfig) {
    let pt_default = CustomThemeConfig::default();
    let active = if cfg.theme == "custom" {
        &cfg.custom_theme
    } else {
        &pt_default
    };

    let win_bg = parse_hex_color(&active.window_bg, Color::from_rgb_u8(0x1f, 0x1f, 0x1f));
    let surf_bg = parse_hex_color(&active.surface_bg, Color::from_rgb_u8(0x2b, 0x2b, 0x2b));
    let card_bg = parse_hex_color(&active.card_bg, Color::from_rgb_u8(0x28, 0x28, 0x28));
    let sel_bg = parse_hex_color(&active.selected_bg, Color::from_rgb_u8(0x38, 0x38, 0x38));
    let border = parse_hex_color(&active.border, Color::from_rgb_u8(0x3d, 0x3d, 0x3d));
    let accent = parse_hex_color(&active.accent, Color::from_rgb_u8(0x60, 0xcd, 0xff));
    let txt_pri = parse_hex_color(&active.text_primary, Color::from_rgb_u8(0xff, 0xff, 0xff));
    let txt_mut = parse_hex_color(&active.text_muted, Color::from_rgb_u8(0x9d, 0x9d, 0x9d));

    palette.set_theme_window_bg(win_bg);
    palette.set_theme_surface_bg(surf_bg);
    palette.set_theme_card_bg(card_bg);
    palette.set_theme_selected_bg(sel_bg);
    palette.set_theme_border(border);
    palette.set_theme_accent(accent);
    palette.set_theme_text_primary(txt_pri);
    palette.set_theme_text_muted(txt_mut);

    settings.set_theme_window_bg(win_bg);
    settings.set_theme_surface_bg(surf_bg);
    settings.set_theme_card_bg(card_bg);
    settings.set_theme_selected_bg(sel_bg);
    settings.set_theme_border(border);
    settings.set_theme_accent(accent);
    settings.set_theme_text_primary(txt_pri);
    settings.set_theme_text_muted(txt_mut);
}

fn parse_hex_color(hex: &str, fallback: Color) -> Color {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return Color::from_rgb_u8(r, g, b);
        }
    }
    fallback
}

fn refresh_palette_results(
    ui: &PaletteWindow,
    state: &Rc<RefCell<RuntimeState>>,
    active_bang: &str,
    query: &str,
) {
    let evaluated = {
        let st = state.borrow();
        st.engine.evaluate_with_bang(active_bang, query)
    };

    let mut ui_entries: Vec<PaletteEntry> = Vec::new();
    let mut current_cat: Option<ShelfCategory> = None;

    for (idx, item) in evaluated.iter().enumerate() {
        if current_cat != Some(item.category) {
            current_cat = Some(item.category);
            ui_entries.push(PaletteEntry {
                is_header: true,
                candidate_index: -1,
                title: SharedString::from(item.category.header_title()),
                subtitle: SharedString::default(),
                badge: SharedString::default(),
                icon: SharedString::from(item.category.icon_glyph()),
                can_elevate: false,
            });
        }

        let row_icon = item
            .icon_override
            .as_deref()
            .unwrap_or_else(|| item.category.icon_glyph());

        ui_entries.push(PaletteEntry {
            is_header: false,
            candidate_index: idx as i32,
            title: SharedString::from(item.title.as_str()),
            subtitle: SharedString::from(item.subtitle.as_str()),
            badge: SharedString::from(item.badge.as_str()),
            icon: SharedString::from(row_icon),
            can_elevate: item.can_elevate,
        });
    }

    let total_candidates = evaluated.len() as i32;
    {
        let mut st = state.borrow_mut();
        st.candidates = evaluated;
    }

    ui.set_total_candidates(total_candidates);
    ui.set_selected_candidate_index(0);
    ui.set_entries(ModelRc::new(VecModel::from(ui_entries)));
}

fn summon_palette(
    ui: &PaletteWindow,
    state: &Rc<RefCell<RuntimeState>>,
    palette_visible: &Rc<Cell<bool>>,
    focus_grace_ticks: &Rc<Cell<u8>>,
) {
    center_palette_on_screen(ui);
    ui.set_active_bang(SharedString::default());
    ui.set_active_bang_label(SharedString::default());
    ui.set_query_text(SharedString::default());
    refresh_palette_results(ui, state, "", "");
    let _ = ui.show();
    palette_visible.set(true);
    focus_grace_ticks.set(8);
    system::activate_window_by_title(PALETTE_WIN_TITLE);
    ui.invoke_focus_search_input();
}

fn reset_and_hide_palette(ui: &PaletteWindow, palette_visible: &Rc<Cell<bool>>) {
    palette_visible.set(false);
    ui.set_active_bang(SharedString::default());
    ui.set_active_bang_label(SharedString::default());
    ui.set_query_text(SharedString::default());
    ui.set_entries(ModelRc::new(VecModel::from(Vec::<PaletteEntry>::new())));
    ui.set_total_candidates(0);
    ui.set_selected_candidate_index(0);
    let _ = ui.hide();
}

fn center_palette_on_screen(ui: &PaletteWindow) {
    let (screen_w, screen_h) = system::primary_screen_size();
    let win_w = 680;
    let x = ((screen_w - win_w) / 2).max(0);
    let y = (screen_h / 5).max(80);
    ui.window()
        .set_position(slint::PhysicalPosition::new(x, y));
}
