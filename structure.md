cmdpal-rs/
├── .github/
│   └── workflows/
│       └── build.yml          # GitHub Actions CI/CD workflow for Windows x64 release
├── .gitignore
├── Cargo.toml                 # Rust package & dependency configuration
├── build.rs                   # Slint UI compiler & procedural Windows .ico generator
├── config.toml                # Default runtime configuration, bangs & extensions
├── README.md                  # Project documentation
├── ui/
│   └── palette.slint          # Slint UI for PaletteWindow & 7-tab SettingsWindow
└── src/
    ├── main.rs                # Application entry point, UI state sync & event loop
    ├── models.rs              # Core data structures, ShelfCategory & ExecutionOutcome
    ├── config.rs              # TOML config loader, normalizer & persistence helpers
    ├── notes.rs               # Notes plugin
    ├── indexer.rs             # Start Menu (.lnk) & custom directory file scanner
    ├── engine.rs              # 7-tier smart ranking engine, calculator & bang parser
    ├── executor.rs            # Process/URI/Admin launcher & clipboard integration
    ├── icon.rs                # Procedural RGBA app & system tray icon renderer
    └── system.rs              # Win32 single-instance mutex, tray icon & global hotkey host
