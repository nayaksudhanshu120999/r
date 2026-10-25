#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod engine;
mod executor;
mod icon;
mod indexer;
mod models;
mod notes;
mod system;

use config::{AppConfig, CustomCommandItem, CustomSettingItem};
use engine::{QueryEngine, BUILTIN_COMMANDS, BUILTIN_SETTINGS};
use executor::execute_candidate;
use indexer::{scan_custom_files, scan_start_menu_apps};
use models::{CandidateItem, ConfigMutation, ExecutionOutcome};
use notes::open_note_target_file;
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use system::{
    check_outside_click_or_focus_loss, ensure_single_instance_or_activate_existing,
    force_focus_window_by_title, get_centered_palette_coords, init_system_integration,
    lock_window_resize_by_title, shutdown_system_integration, SystemEvent,
};

slint::include_modules!();

const CONFIG_FILE: &str = "config.toml";
const PALETTE_TITLE: &str = "Command Palette";
const SETTINGS_TITLE: &str = "Command Palette Settings";
const PALETTE_W: i32 = 680;
const PALETTE_H: i32 = 440;

struct AppState {
    engine: QueryEngine,
    candidates: Vec<CandidateItem>,
    palette_visible: bool,
    settings_visible: bool,
    suppress_outside_close_until: Instant,
    app_filter: String,
    builtin_filter: String,
}

fn parse_hex_color(hex: &str, fallback: Color) -> Color {
    let clean = hex.trim().trim_start_matches('#');
    match clean.len() {
        6 => {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&clean[0..2], 16),
                u8::from_str_radix(&clean[2..4], 16),
                u8::from_str_radix(&clean[4..6], 16),
            ) {
                return Color::from_rgb_u8(r, g, b);
            }
        }
        8 => {
            if let (Ok(r), Ok(g), Ok(b), Ok(a)) = (
                u8::from_str_radix(&clean[0..2], 16),
                u8::from_str_radix(&clean[2..4], 16),
                u8::from_str_radix(&clean[4..6], 16),
                u8::from_str_radix(&clean[6..8], 16),
            ) {
                return Color::from_argb_u8(a, r, g, b);
            }
        }
        _ => {}
    }
    fallback
}

fn apply_theme_to_windows(
    palette: &PaletteWindow,
    settings: &SettingsWindow,
    cfg: &AppConfig,
) {
    let is_custom = cfg.theme == "custom";
    let (win_bg, surf_bg, card_bg, sel_bg, border, accent, txt_pri, txt_mut) = if is_custom {
        (
            parse_hex_color(&cfg.custom_theme.window_bg, Color::from_rgb_u8(0x1f, 0x1f, 0x1f)),
            parse_hex_color(&cfg.custom_theme.surface_bg, Color::from_rgb_u8(0x2b, 0x2b, 0x2b)),
            parse_hex_color(&cfg.custom_theme.card_bg, Color::from_rgb_u8(0x28, 0x28, 0x28)),
            parse_hex_color(&cfg.custom_theme.selected_bg, Color::from_rgb_u8(0x38, 0x38, 0x38)),
            parse_hex_color(&cfg.custom_theme.border, Color::from_rgb_u8(0x3d, 0x3d, 0x3d)),
            parse_hex_color(&cfg.custom_theme.accent, Color::from_rgb_u8(0x60, 0xcd, 0xff)),
            parse_hex_color(&cfg.custom_theme.text_primary, Color::from_rgb_u8(0xff, 0xff, 0xff)),
            parse_hex_color(&cfg.custom_theme.text_muted, Color::from_rgb_u8(0x9d, 0x9d, 0x9d)),
        )
    } else {
        (
            Color::from_rgb_u8(0x1e, 0x1e, 0x1e),
            Color::from_rgb_u8(0x26, 0x26, 0x26),
            Color::from_rgb_u8(0x24, 0x24, 0x24),
            Color::from_rgb_u8(0x32, 0x32, 0x32),
            Color::from_rgb_u8(0x36, 0x36, 0x36),
            Color::from_rgb_u8(0x60, 0xcd, 0xff),
            Color::from_rgb_u8(0xf5, 0xf5, 0xf5),
            Color::from_rgb_u8(0x9a, 0x9a, 0x9a),
        )
    };

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

fn refresh_palette_results(ui: &PaletteWindow, state: &Rc<RefCell<AppState>>) {
    let raw_query = ui.get_search_text().to_string();
    let current_bang = ui.get_active_bang().to_string();

    if current_bang.is_empty() {
        let extracted = {
            let st = state.borrow();
            st.engine.extract_bang_prefix(&raw_query)
        };
        if let Some((bang_key, bang_label, remainder)) = extracted {
            ui.set_active_bang(bang_key.into());
            ui.set_active_bang_label(bang_label.into());
            ui.set_search_text(remainder.into());
            refresh_palette_results(ui, state);
            return;
        }
    }

    let active_bang = ui.get_active_bang().to_string();
    let query = ui.get_search_text().to_string();

    let items = {
        let mut st = state.borrow_mut();
        let evaluated = st.engine.evaluate_with_bang(&active_bang, &query);
        st.candidates = evaluated.clone();
        evaluated
    };

    let mut ui_entries: Vec<PaletteEntry> = Vec::with_capacity(items.len());
    let mut prev_category = None;

    for item in &items {
        let show_header = prev_category != Some(item.category);
        prev_category = Some(item.category);

        let icon_str = item
            .icon_override
            .as_deref()
            .unwrap_or_else(|| item.category.icon_glyph());

        ui_entries.push(PaletteEntry {
            title: item.title.clone().into(),
            subtitle: item.subtitle.clone().into(),
            badge: item.badge.clone().into(),
            category: item.category.as_str().into(),
            section_header: if show_header {
                item.category.header_title().into()
            } else {
                SharedString::default()
            },
            icon: icon_str.into(),
            can_elevate: item.can_elevate,
        });
    }

    ui.set_entries(ModelRc::new(VecModel::from(ui_entries)));
    ui.set_selected_index(0);
}

fn sync_settings_ui(settings_ui: &SettingsWindow, state: &Rc<RefCell<AppState>>) {
    let st = state.borrow();
    if !st.settings_visible {
        return;
    }
    let cfg = &st.engine.config;

    settings_ui.set_show_apps(cfg.show_apps);
    settings_ui.set_run_on_startup(cfg.run_on_startup);
    settings_ui.set_hotkey_str(cfg.hotkey.clone().into());
    settings_ui.set_theme_mode(cfg.theme.clone().into());

    settings_ui.set_enable_cmdpal_shortcuts(cfg.enable_cmdpal_shortcuts);
    settings_ui.set_enable_builtin_cmds(cfg.enable_builtin_cmds);
    settings_ui.set_enable_builtin_settings(cfg.enable_builtin_settings);

    settings_ui.set_default_search_engine(cfg.default_search_engine.clone().into());
    settings_ui.set_custom_search_url(cfg.custom_search_url.clone().into());

    settings_ui.set_custom_window_bg(cfg.custom_theme.window_bg.clone().into());
    settings_ui.set_custom_surface_bg(cfg.custom_theme.surface_bg.clone().into());
    settings_ui.set_custom_card_bg(cfg.custom_theme.card_bg.clone().into());
    settings_ui.set_custom_selected_bg(cfg.custom_theme.selected_bg.clone().into());
    settings_ui.set_custom_border(cfg.custom_theme.border.clone().into());
    settings_ui.set_custom_accent(cfg.custom_theme.accent.clone().into());
    settings_ui.set_custom_text_primary(cfg.custom_theme.text_primary.clone().into());
    settings_ui.set_custom_text_muted(cfg.custom_theme.text_muted.clone().into());

    settings_ui.set_custom_cmd_path(cfg.custom_cmd_path.clone().into());
    settings_ui.set_custom_settings_path(cfg.custom_system_settings_path.clone().into());

    // 1. Category Order
    let ordered_cats = cfg.ordered_categories();
    let total_cats = ordered_cats.len();
    let cat_entries: Vec<CategoryEntry> = ordered_cats
        .into_iter()
        .enumerate()
        .map(|(idx, cat)| CategoryEntry {
            rank: (idx + 1) as i32,
            id: cat.as_str().into(),
            label: cat.display_name().into(),
            description: cat.description().into(),
            icon: cat.icon_glyph().into(),
            is_first: idx == 0,
            is_last: idx + 1 == total_cats,
        })
        .collect();
    settings_ui.set_category_entries(ModelRc::new(VecModel::from(cat_entries)));

    // 2. Built-in Commands & Windows Settings filter
    let bf_lower = st.builtin_filter.trim().to_ascii_lowercase();
    let builtin_cmd_entries: Vec<BuiltinToggleEntry> = BUILTIN_COMMANDS
        .iter()
        .filter(|spec| {
            bf_lower.is_empty()
                || spec.title.to_ascii_lowercase().contains(&bf_lower)
                || spec.subtitle.to_ascii_lowercase().contains(&bf_lower)
                || spec.id.contains(&bf_lower)
        })
        .map(|spec| BuiltinToggleEntry {
            id: spec.id.into(),
            title: spec.title.into(),
            subtitle: spec.subtitle.into(),
            enabled: !cfg.disabled_builtin_cmds.iter().any(|d| d == spec.id),
        })
        .collect();
    settings_ui.set_builtin_cmd_entries(ModelRc::new(VecModel::from(builtin_cmd_entries)));

    let builtin_setting_entries: Vec<BuiltinToggleEntry> = BUILTIN_SETTINGS
        .iter()
        .filter(|spec| {
            bf_lower.is_empty()
                || spec.title.to_ascii_lowercase().contains(&bf_lower)
                || spec.subtitle.to_ascii_lowercase().contains(&bf_lower)
                || spec.id.contains(&bf_lower)
        })
        .map(|spec| BuiltinToggleEntry {
            id: spec.id.into(),
            title: spec.title.into(),
            subtitle: spec.subtitle.into(),
            enabled: !cfg.disabled_builtin_settings.iter().any(|d| d == spec.id),
        })
        .collect();
    settings_ui.set_builtin_setting_entries(ModelRc::new(VecModel::from(builtin_setting_entries)));

    // 3. Apps Visibility List
    let filter_lower = st.app_filter.trim().to_ascii_lowercase();
    let app_entries: Vec<AppToggleEntry> = st
        .engine
        .apps
        .iter()
        .filter(|a| filter_lower.is_empty() || a.name_lower.contains(&filter_lower))
        .map(|a| AppToggleEntry {
            name: a.name.clone().into(),
            path: a.path.to_string_lossy().to_string().into(),
            visible: !cfg.is_app_hidden(&a.name_lower),
        })
        .collect();
    settings_ui.set_app_entries(ModelRc::new(VecModel::from(app_entries)));

    // 4. Custom Scan Folders
    let folder_entries: Vec<SharedString> = cfg
        .custom_file_scan_paths
        .iter()
        .map(|p| p.clone().into())
        .collect();
    settings_ui.set_folder_entries(ModelRc::new(VecModel::from(folder_entries)));

    // 5. Custom Commands
    let cmd_entries: Vec<CustomCmdEntry> = cfg
        .custom_commands
        .iter()
        .map(|c| CustomCmdEntry {
            name: c.name.clone().into(),
            command: c.command.clone().into(),
        })
        .collect();
    settings_ui.set_custom_cmd_entries(ModelRc::new(VecModel::from(cmd_entries)));

    // 6. Custom Windows Settings
    let set_entries: Vec<CustomSettingEntry> = cfg
        .custom_settings
        .iter()
        .map(|s| CustomSettingEntry {
            name: s.name.clone().into(),
            path: s.path.clone().into(),
        })
        .collect();
    settings_ui.set_custom_setting_entries(ModelRc::new(VecModel::from(set_entries)));

    // 7. Web Bangs
    let bang_entries: Vec<BangEntry> = cfg
        .bangs
        .iter()
        .map(|(k, v)| BangEntry {
            trigger: k.clone().into(),
            url: v.clone().into(),
        })
        .collect();
    settings_ui.set_bang_entries(ModelRc::new(VecModel::from(bang_entries)));

    // 8. Multi-file Note Bangs
    let note_entries: Vec<NoteBangEntry> = cfg
        .notes
        .iter()
        .map(|n| NoteBangEntry {
            bang: n.bang.clone().into(),
            name: n.name.clone().into(),
            file_path: n.file_path.clone().into(),
        })
        .collect();
    settings_ui.set_note_entries(ModelRc::new(VecModel::from(note_entries)));
}

fn show_and_focus_palette(ui: &PaletteWindow, state: &Rc<RefCell<AppState>>) {
    let (cx, cy) = get_centered_palette_coords(PALETTE_W, PALETTE_H);
    ui.window()
        .set_position(slint::LogicalPosition::new(cx as f32, cy as f32));
    let _ = ui.show();
    lock_window_resize_by_title(PALETTE_TITLE);
    force_focus_window_by_title(PALETTE_TITLE);
    ui.invoke_focus_input();

    {
        let mut st = state.borrow_mut();
        st.palette_visible = true;
        st.suppress_outside_close_until = Instant::now() + Duration::from_millis(280);
    }

    refresh_palette_results(ui, state);
}

fn reset_and_hide_palette(ui: &PaletteWindow, state: &Rc<RefCell<AppState>>) {
    ui.set_search_text(SharedString::default());
    ui.set_active_bang(SharedString::default());
    ui.set_active_bang_label(SharedString::default());
    ui.set_entries(ModelRc::new(VecModel::from(Vec::<PaletteEntry>::new())));
    ui.set_selected_index(0);
    let _ = ui.hide();

    let mut st = state.borrow_mut();
    st.palette_visible = false;
    st.candidates.clear();
    if st.candidates.capacity() > 64 {
        st.candidates.shrink_to_fit();
    }
}

fn open_settings_window_at_tab(
    palette_ui: &PaletteWindow,
    settings_ui: &SettingsWindow,
    state: &Rc<RefCell<AppState>>,
    tab_index: i32,
) {
    reset_and_hide_palette(palette_ui, state);
    state.borrow_mut().settings_visible = true;
    settings_ui.set_active_tab(tab_index);
    sync_settings_ui(settings_ui, state);
    let _ = settings_ui.show();
    force_focus_window_by_title(SETTINGS_TITLE);
}

fn reload_all_indexes(
    palette_ui: &PaletteWindow,
    settings_ui: &SettingsWindow,
    state: &Rc<RefCell<AppState>>,
) {
    let cfg = AppConfig::load_or_create(CONFIG_FILE);
    system::apply_runtime_system_config(cfg.run_on_startup, &cfg.hotkey);
    let apps = scan_start_menu_apps();
    let custom_files = scan_custom_files(&cfg.custom_file_scan_paths);
    apply_theme_to_windows(palette_ui, settings_ui, &cfg);

    let is_pal_visible = {
        let mut st = state.borrow_mut();
        st.engine = QueryEngine::new(cfg, apps, custom_files);
        st.palette_visible
    };
    sync_settings_ui(settings_ui, state);
    if is_pal_visible {
        refresh_palette_results(palette_ui, state);
    }
}

fn apply_config_mutation(
    palette_ui: &PaletteWindow,
    settings_ui: &SettingsWindow,
    state: &Rc<RefCell<AppState>>,
    mutation: ConfigMutation,
) {
    let cfg_snapshot = {
        let mut st = state.borrow_mut();
        match mutation {
            ConfigMutation::ToggleStartup(val) => st.engine.config.run_on_startup = val,
            ConfigMutation::SetHotkey(hk) => st.engine.config.hotkey = hk,
            ConfigMutation::SetDefaultSearchEngine(eng) => {
                st.engine.config.default_search_engine = eng;
            }
            ConfigMutation::SwitchTheme(th) => st.engine.config.theme = th,
        }
        st.engine.config.save(CONFIG_FILE);
        st.engine.config.clone()
    };
    apply_theme_to_windows(palette_ui, settings_ui, &cfg_snapshot);
    sync_settings_ui(settings_ui, state);
}

fn handle_execution_outcome(
    outcome: ExecutionOutcome,
    palette_ui: &PaletteWindow,
    settings_ui: &SettingsWindow,
    state: &Rc<RefCell<AppState>>,
) {
    match outcome {
        ExecutionOutcome::HidePalette => {
            reset_and_hide_palette(palette_ui, state);
        }
        ExecutionOutcome::KeepOpenAndRefresh { clear_query } => {
            if clear_query {
                palette_ui.set_search_text(SharedString::default());
            }
            refresh_palette_results(palette_ui, state);
            palette_ui.invoke_focus_input();
        }
        ExecutionOutcome::ActivateBang { bang, label } => {
            palette_ui.set_active_bang(bang.into());
            palette_ui.set_active_bang_label(label.into());
            palette_ui.set_search_text(SharedString::default());
            refresh_palette_results(palette_ui, state);
            palette_ui.invoke_focus_input();
        }
        ExecutionOutcome::OpenSettingsTab(tab) => {
            open_settings_window_at_tab(palette_ui, settings_ui, state, tab);
        }
        ExecutionOutcome::ApplyConfigMutation(mutation) => {
            apply_config_mutation(palette_ui, settings_ui, state, mutation);
            reset_and_hide_palette(palette_ui, state);
        }
        ExecutionOutcome::ReloadAll => {
            reload_all_indexes(palette_ui, settings_ui, state);
            reset_and_hide_palette(palette_ui, state);
        }
    }
}

fn main() -> Result<(), slint::PlatformError> {
    if !ensure_single_instance_or_activate_existing() {
        return Ok(());
    }

    let config = AppConfig::load_or_create(CONFIG_FILE);
    let sys_rx = init_system_integration(config.run_on_startup, &config.hotkey);

    let apps = scan_start_menu_apps();
    let custom_files = scan_custom_files(&config.custom_file_scan_paths);
    let engine = QueryEngine::new(config.clone(), apps, custom_files);

    let palette_ui = PaletteWindow::new()?;
    let settings_ui = SettingsWindow::new()?;

    if let Some(app_icon) = icon::create_slint_window_icon() {
        palette_ui.set_window_icon(app_icon.clone());
        settings_ui.set_window_icon(app_icon);
    }

    apply_theme_to_windows(&palette_ui, &settings_ui, &config);

    let state = Rc::new(RefCell::new(AppState {
        engine,
        candidates: Vec::new(),
        palette_visible: true,
        settings_visible: false,
        suppress_outside_close_until: Instant::now() + Duration::from_millis(350),
        app_filter: String::new(),
        builtin_filter: String::new(),
    }));

    settings_ui.window().on_close_requested({
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        move || {
            state_ref.borrow_mut().settings_visible = false;
            if let Some(win) = settings_weak.upgrade() {
                let _ = win.hide();
            }
            slint::CloseRequestResponse::HideWindow
        }
    });

    palette_ui.window().on_close_requested({
        let palette_weak = palette_ui.as_weak();
        let state_ref = state.clone();
        move || {
            if let Some(win) = palette_weak.upgrade() {
                reset_and_hide_palette(&win, &state_ref);
            }
            slint::CloseRequestResponse::HideWindow
        }
    });

    // Query changed
    {
        let palette_weak = palette_ui.as_weak();
        let state_ref = state.clone();
        palette_ui.on_query_changed(move |_| {
            if let Some(ui) = palette_weak.upgrade() {
                refresh_palette_results(&ui, &state_ref);
            }
        });
    }

    // Clear active bang chip
    {
        let palette_weak = palette_ui.as_weak();
        let state_ref = state.clone();
        palette_ui.on_clear_active_bang(move || {
            if let Some(ui) = palette_weak.upgrade() {
                ui.set_active_bang(SharedString::default());
                ui.set_active_bang_label(SharedString::default());
                refresh_palette_results(&ui, &state_ref);
                ui.invoke_focus_input();
            }
        });
    }

    // Selection navigation
    {
        let palette_weak = palette_ui.as_weak();
        palette_ui.on_move_selection(move |delta| {
            if let Some(ui) = palette_weak.upgrade() {
                let count = ui.get_entries().row_count() as i32;
                if count <= 0 {
                    ui.set_selected_index(0);
                    return;
                }
                let current = ui.get_selected_index();
                let next = (current + delta).rem_euclid(count);
                ui.set_selected_index(next);
            }
        });
    }

    // Activate selected item
    {
        let palette_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        palette_ui.on_activate_selected(move |run_as_admin| {
            if let (Some(ui), Some(s_ui)) = (palette_weak.upgrade(), settings_weak.upgrade()) {
                let idx = ui.get_selected_index();
                let maybe_action = {
                    let st = state_ref.borrow();
                    if idx >= 0 && (idx as usize) < st.candidates.len() {
                        Some((
                            st.candidates[idx as usize].clone(),
                            st.engine.config.custom_cmd_path.clone(),
                        ))
                    } else {
                        None
                    }
                };

                if let Some((candidate, cmd_path)) = maybe_action {
                    if let Ok(outcome) = execute_candidate(&candidate, run_as_admin, &cmd_path) {
                        handle_execution_outcome(outcome, &ui, &s_ui, &state_ref);
                    }
                }
            }
        });
    }

    // Activate item by click
    {
        let palette_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        palette_ui.on_activate_index(move |idx, run_as_admin| {
            if let (Some(ui), Some(s_ui)) = (palette_weak.upgrade(), settings_weak.upgrade()) {
                let maybe_action = {
                    let st = state_ref.borrow();
                    if idx >= 0 && (idx as usize) < st.candidates.len() {
                        Some((
                            st.candidates[idx as usize].clone(),
                            st.engine.config.custom_cmd_path.clone(),
                        ))
                    } else {
                        None
                    }
                };

                if let Some((candidate, cmd_path)) = maybe_action {
                    if let Ok(outcome) = execute_candidate(&candidate, run_as_admin, &cmd_path) {
                        handle_execution_outcome(outcome, &ui, &s_ui, &state_ref);
                    }
                }
            }
        });
    }

    // Escape hides palette
    {
        let palette_weak = palette_ui.as_weak();
        let state_ref = state.clone();
        palette_ui.on_close_palette(move || {
            if let Some(ui) = palette_weak.upgrade() {
                reset_and_hide_palette(&ui, &state_ref);
            }
        });
    }

    // Open Settings from Palette gear button or Ctrl+,
    {
        let palette_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        palette_ui.on_open_settings(move || {
            if let (Some(p_ui), Some(s_ui)) = (palette_weak.upgrade(), settings_weak.upgrade()) {
                open_settings_window_at_tab(&p_ui, &s_ui, &state_ref, 0);
            }
        });
    }

    // ====================================================================
    // SETTINGS WINDOW CALLBACKS
    // ====================================================================
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_close_window(move || {
            state_ref.borrow_mut().settings_visible = false;
            if let Some(s_ui) = settings_weak.upgrade() {
                let _ = s_ui.hide();
            }
        });
    }

    {
        let palette_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_rescan_indexes(move || {
            if let (Some(p_ui), Some(s_ui)) = (palette_weak.upgrade(), settings_weak.upgrade()) {
                reload_all_indexes(&p_ui, &s_ui, &state_ref);
            }
        });
    }

    {
        let palette_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_save_general_settings(
            move |show_apps,
                  run_on_startup,
                  hotkey,
                  theme_mode,
                  en_cmdpal,
                  en_cmds,
                  en_settings,
                  def_engine,
                  custom_search_url,
                  w_bg,
                  s_bg,
                  c_bg,
                  sel_bg,
                  bdr,
                  acc,
                  txt_p,
                  txt_m,
                  cmd_path,
                  set_path| {
                if let (Some(p_ui), Some(s_ui)) = (palette_weak.upgrade(), settings_weak.upgrade())
                {
                    let cfg_snapshot = {
                        let mut st = state_ref.borrow_mut();
                        let cfg = &mut st.engine.config;
                        cfg.show_apps = show_apps;
                        cfg.run_on_startup = run_on_startup;
                        cfg.hotkey = hotkey.to_string();
                        cfg.theme = theme_mode.to_string();
                        cfg.enable_cmdpal_shortcuts = en_cmdpal;
                        cfg.enable_builtin_cmds = en_cmds;
                        cfg.enable_builtin_settings = en_settings;
                        cfg.default_search_engine = def_engine.to_string();
                        cfg.custom_search_url = custom_search_url.to_string();
                        cfg.custom_theme.window_bg = w_bg.to_string();
                        cfg.custom_theme.surface_bg = s_bg.to_string();
                        cfg.custom_theme.card_bg = c_bg.to_string();
                        cfg.custom_theme.selected_bg = sel_bg.to_string();
                        cfg.custom_theme.border = bdr.to_string();
                        cfg.custom_theme.accent = acc.to_string();
                        cfg.custom_theme.text_primary = txt_p.to_string();
                        cfg.custom_theme.text_muted = txt_m.to_string();
                        cfg.custom_cmd_path = cmd_path.to_string();
                        cfg.custom_system_settings_path = set_path.to_string();
                        cfg.save(CONFIG_FILE);
                        cfg.clone()
                    };

                    apply_theme_to_windows(&p_ui, &s_ui, &cfg_snapshot);
                    sync_settings_ui(&s_ui, &state_ref);
                }
            },
        );
    }

    // Category ordering
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_move_category_up(move |idx| {
            if let Some(s_ui) = settings_weak.upgrade() {
                let idx = idx as usize;
                {
                    let mut st = state_ref.borrow_mut();
                    if idx > 0 && idx < st.engine.config.category_order.len() {
                        st.engine.config.category_order.swap(idx, idx - 1);
                        st.engine.config.save(CONFIG_FILE);
                    }
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_move_category_down(move |idx| {
            if let Some(s_ui) = settings_weak.upgrade() {
                let idx = idx as usize;
                {
                    let mut st = state_ref.borrow_mut();
                    let len = st.engine.config.category_order.len();
                    if idx + 1 < len {
                        st.engine.config.category_order.swap(idx, idx + 1);
                        st.engine.config.save(CONFIG_FILE);
                    }
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    // Built-in Command & Setting toggles
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_builtin_filter_changed(move |filter_text| {
            if let Some(s_ui) = settings_weak.upgrade() {
                state_ref.borrow_mut().builtin_filter = filter_text.to_string();
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_toggle_builtin_cmd(move |id, enabled| {
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.set_builtin_cmd_enabled(&id, enabled);
                    st.engine.config.save(CONFIG_FILE);
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_toggle_builtin_setting(move |id, enabled| {
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.set_builtin_setting_enabled(&id, enabled);
                    st.engine.config.save(CONFIG_FILE);
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    // App visibility toggles
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_toggle_app_visibility(move |app_name, visible| {
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.set_app_visibility(&app_name, visible);
                    st.engine.config.save(CONFIG_FILE);
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_app_filter_changed(move |filter_text| {
            if let Some(s_ui) = settings_weak.upgrade() {
                state_ref.borrow_mut().app_filter = filter_text.to_string();
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    // Custom scan folders
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_add_scan_folder(move |folder| {
            let trimmed = folder.trim().to_string();
            if trimmed.is_empty() {
                return;
            }
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    if !st.engine.config.custom_file_scan_paths.contains(&trimmed) {
                        st.engine.config.custom_file_scan_paths.push(trimmed);
                        st.engine.config.save(CONFIG_FILE);
                        st.engine.custom_files =
                            scan_custom_files(&st.engine.config.custom_file_scan_paths);
                    }
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_remove_scan_folder(move |idx| {
            if let Some(s_ui) = settings_weak.upgrade() {
                let idx = idx as usize;
                {
                    let mut st = state_ref.borrow_mut();
                    if idx < st.engine.config.custom_file_scan_paths.len() {
                        st.engine.config.custom_file_scan_paths.remove(idx);
                        st.engine.config.save(CONFIG_FILE);
                        st.engine.custom_files =
                            scan_custom_files(&st.engine.config.custom_file_scan_paths);
                    }
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    // Custom Commands & Settings
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_add_custom_cmd(move |name, command| {
            let n = name.trim().to_string();
            let c = command.trim().to_string();
            if n.is_empty() || c.is_empty() {
                return;
            }
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine
                        .config
                        .custom_commands
                        .push(CustomCommandItem { name: n, command: c });
                    st.engine.config.save(CONFIG_FILE);
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_remove_custom_cmd(move |idx| {
            if let Some(s_ui) = settings_weak.upgrade() {
                let idx = idx as usize;
                {
                    let mut st = state_ref.borrow_mut();
                    if idx < st.engine.config.custom_commands.len() {
                        st.engine.config.custom_commands.remove(idx);
                        st.engine.config.save(CONFIG_FILE);
                    }
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_add_custom_setting(move |name, path| {
            let n = name.trim().to_string();
            let p = path.trim().to_string();
            if n.is_empty() || p.is_empty() {
                return;
            }
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine
                        .config
                        .custom_settings
                        .push(CustomSettingItem { name: n, path: p });
                    st.engine.config.save(CONFIG_FILE);
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_remove_custom_setting(move |idx| {
            if let Some(s_ui) = settings_weak.upgrade() {
                let idx = idx as usize;
                {
                    let mut st = state_ref.borrow_mut();
                    if idx < st.engine.config.custom_settings.len() {
                        st.engine.config.custom_settings.remove(idx);
                        st.engine.config.save(CONFIG_FILE);
                    }
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    // Web Bangs
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_add_bang(move |trigger, url| {
            let t = trigger.trim().trim_start_matches('!').to_ascii_lowercase();
            let u = url.trim().to_string();
            if t.is_empty() || !u.contains("{q}") {
                return;
            }
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.bangs.insert(t, u);
                    st.engine.config.save(CONFIG_FILE);
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_remove_bang(move |trigger| {
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.bangs.remove(trigger.as_str());
                    st.engine.config.save(CONFIG_FILE);
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    // Multi-file Note Bangs
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_upsert_note_bang(move |bang, name, file_path| {
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    if st.engine.config.upsert_note_bang(&bang, &name, &file_path) {
                        st.engine.config.save(CONFIG_FILE);
                    }
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();
        settings_ui.on_remove_note_bang(move |bang| {
            if let Some(s_ui) = settings_weak.upgrade() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.remove_note_bang(&bang);
                    st.engine.config.save(CONFIG_FILE);
                }
                sync_settings_ui(&s_ui, &state_ref);
            }
        });
    }

    {
        settings_ui.on_open_note_file(move |file_path| {
            let _ = open_note_target_file(&file_path);
        });
    }

    // Event loop timer (30ms)
    let poll_timer = Timer::default();
    {
        let palette_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = state.clone();

        poll_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(30),
            move || {
                while let Ok(ev) = sys_rx.try_recv() {
                    match ev {
                        SystemEvent::ShowPalette => {
                            if let Some(ui) = palette_weak.upgrade() {
                                show_and_focus_palette(&ui, &state_ref);
                            }
                        }
                        SystemEvent::TogglePalette => {
                            if let Some(ui) = palette_weak.upgrade() {
                                let is_vis = state_ref.borrow().palette_visible;
                                if is_vis {
                                    reset_and_hide_palette(&ui, &state_ref);
                                } else {
                                    show_and_focus_palette(&ui, &state_ref);
                                }
                            }
                        }
                        SystemEvent::OpenSettings => {
                            if let (Some(p_ui), Some(s_ui)) =
                                (palette_weak.upgrade(), settings_weak.upgrade())
                            {
                                open_settings_window_at_tab(&p_ui, &s_ui, &state_ref, 0);
                            }
                        }
                        SystemEvent::ReloadConfig => {
                            if let (Some(p_ui), Some(s_ui)) =
                                (palette_weak.upgrade(), settings_weak.upgrade())
                            {
                                reload_all_indexes(&p_ui, &s_ui, &state_ref);
                            }
                        }
                        SystemEvent::QuitApp => {
                            shutdown_system_integration();
                            let _ = slint::quit_event_loop();
                            return;
                        }
                    }
                }

                let should_check = {
                    let st = state_ref.borrow();
                    st.palette_visible && Instant::now() >= st.suppress_outside_close_until
                };

                if should_check && check_outside_click_or_focus_loss(PALETTE_TITLE) {
                    if let Some(ui) = palette_weak.upgrade() {
                        reset_and_hide_palette(&ui, &state_ref);
                    }
                }
            },
        );
    }

    show_and_focus_palette(&palette_ui, &state);
    let res = slint::run_event_loop_until_quit();
    shutdown_system_integration();
    res
}
