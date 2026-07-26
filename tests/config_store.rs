use betterreview::{
    domain::DiffLayout,
    state::{AppConfig, StatePaths},
};

/// One test on purpose: the config directory comes from an environment
/// variable, and parallel tests would race over it.
#[test]
fn the_config_lives_under_the_config_directory_and_honours_the_legacy_file() {
    let dir = tempfile::tempdir().unwrap();
    let state = StatePaths::new(dir.path().join("state"));
    unsafe { std::env::set_var("BETTERREVIEW_CONFIG_DIR", dir.path()) };

    assert_eq!(
        AppConfig::load(&state),
        AppConfig::default(),
        "a missing config falls back to defaults"
    );

    let config = AppConfig {
        diff_layout: DiffLayout::Split,
        files_hidden: true,
        wrap_lines: true,
    };
    config.save(&state).unwrap();

    assert!(dir.path().join("config.json").exists());
    assert_eq!(AppConfig::load(&state), config);

    std::fs::remove_file(dir.path().join("config.json")).unwrap();
    std::fs::create_dir_all(dir.path().join("state")).unwrap();
    std::fs::write(
        dir.path().join("state/config.json"),
        br#"{"diff_layout":"split","files_hidden":false}"#,
    )
    .unwrap();

    assert_eq!(
        AppConfig::load(&state).diff_layout,
        DiffLayout::Split,
        "a config left in the old state directory is still read"
    );

    unsafe { std::env::remove_var("BETTERREVIEW_CONFIG_DIR") };
}
