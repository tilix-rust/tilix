#![allow(dead_code)]
#![allow(unused_imports)]

use std::fs;
use std::path::Path;

#[path = "../src/model/mod.rs"]
mod model;

#[path = "../src/pty/mod.rs"]
mod pty;

#[path = "../src/ui/mod.rs"]
mod ui;

#[path = "../src/app.rs"]
mod app;

use app::{parse_cli_args, CliAction};
use model::config::AppConfig;

#[test]
fn test_config_backwards_compatibility_and_defaults() {
    // 1. Partial JSON with only one field
    let partial_json = r#"{"notifications_enabled": false}"#;
    let config: AppConfig = serde_json::from_str(partial_json)
        .expect("partial JSON should deserialize with serde defaults");
    let expected = AppConfig {
        notifications_enabled: false,
        ..Default::default()
    };
    assert_eq!(config, expected);

    // 2. Empty JSON object
    let empty_json = "{}";
    let config_empty: AppConfig = serde_json::from_str(empty_json)
        .expect("empty JSON should deserialize to default config");
    assert_eq!(config_empty, AppConfig::default());

    // 3. JSON with unknown/future fields (forward compatibility)
    let forward_compat_json = r#"{
        "quake_height_percent": 60,
        "future_unknown_setting": "awesome_feature",
        "another_field": 42
    }"#;
    let config_forward: AppConfig = serde_json::from_str(forward_compat_json)
        .expect("JSON with unknown fields should deserialize gracefully");
    assert_eq!(config_forward.quake_height_percent, 60);
    assert!(config_forward.notifications_enabled);

    // 4. Full roundtrip serialization
    let default_config = AppConfig::default();
    let serialized = default_config.to_json().expect("to_json should succeed");
    let deserialized = AppConfig::from_json(&serialized).expect("from_json should succeed");
    assert_eq!(default_config, deserialized);
}

#[test]
fn test_packaging_asset_files_exist() {
    let required_files = [
        "PKGBUILD",
        "Makefile",
        "data/com.github.tilix_rust.desktop",
        "data/com.github.tilix_rust.metainfo.xml",
        "data/com.github.tilix_rust.service",
        "data/icons/scalable/apps/com.github.tilix_rust.svg",
        "data/completions/bash/tilix",
        "data/completions/zsh/_tilix",
        "data/completions/fish/tilix.fish",
        "build-aux/com.github.tilix_rust.json",
    ];

    for file in &required_files {
        let path = Path::new(file);
        assert!(path.exists(), "Required packaging file missing: {}", file);
        let metadata = fs::metadata(path).expect("failed to read metadata");
        assert!(metadata.len() > 0, "File must not be empty: {}", file);
    }
}

#[test]
fn test_pkgbuild_content_specifications() {
    let content = fs::read_to_string("PKGBUILD").expect("failed to read PKGBUILD");
    assert!(content.contains("pkgname=tilix-rust"));
    assert!(content.contains("pkgver="), "PKGBUILD must declare pkgver");
    assert!(content.contains("license=('MPL-2.0')"));
    assert!(content.contains("provides=('tilix')"));
    assert!(content.contains("conflicts=('tilix')"));
    assert!(content.contains("cargo build --release --all-targets"));
    assert!(content.contains("cargo test --release"));
    assert!(content.contains("com.github.tilix_rust.desktop"));
    assert!(content.contains("com.github.tilix_rust.metainfo.xml"));
    assert!(content.contains("com.github.tilix_rust.service"));
    assert!(content.contains("com.github.tilix_rust.svg"));
    assert!(content.contains("completions/bash/tilix"));
    assert!(content.contains("completions/zsh/_tilix"));
    assert!(content.contains("completions/fish/tilix.fish"));
    assert!(content.contains("BUILDDIR="), "PKGBUILD must isolate BUILDDIR to protect src/ from makepkg -C / -c");
}

#[test]
fn test_makefile_content_specifications() {
    let content = fs::read_to_string("Makefile").expect("failed to read Makefile");
    assert!(content.contains("PREFIX ?= /usr/local"));
    assert!(content.contains("BINDIR ?= $(PREFIX)/bin"));
    assert!(content.contains("DATADIR ?= $(PREFIX)/share"));
    assert!(content.contains("all: build"));
    assert!(content.contains("build:"));
    assert!(content.contains("install: build"));
    assert!(content.contains("uninstall:"));
    assert!(content.contains("clean:"));
}

#[test]
fn test_desktop_file_actions_and_cli_consistency() {
    let content = fs::read_to_string("data/com.github.tilix_rust.desktop")
        .expect("failed to read desktop file");

    assert!(content.contains("[Desktop Entry]"));
    assert!(content.contains("Type=Application"));
    assert!(content.contains("Name=Tilix"));
    assert!(content.contains("Icon=com.github.tilix_rust"));
    assert!(content.contains("StartupWMClass=com.github.tilix_rust"));
    assert!(content.contains("Actions=NewWindow;Quake;Preferences;"));

    assert!(content.contains("[Desktop Action NewWindow]"));
    assert!(content.contains("[Desktop Action Quake]"));
    assert!(content.contains("[Desktop Action Preferences]"));

    // Verify Desktop Action Exec lines match CLI parser expectations
    assert_eq!(parse_cli_args(["tilix"]), CliAction::NewWindow);
    assert_eq!(
        parse_cli_args(["tilix", "--quake-toggle"]),
        CliAction::QuakeToggle
    );
    assert_eq!(
        parse_cli_args(["tilix", "--preferences"]),
        CliAction::Preferences
    );
}

#[test]
fn test_shell_completions_flag_coverage() {
    let bash = fs::read_to_string("data/completions/bash/tilix")
        .expect("failed to read bash completion");
    let zsh = fs::read_to_string("data/completions/zsh/_tilix")
        .expect("failed to read zsh completion");
    let fish = fs::read_to_string("data/completions/fish/tilix.fish")
        .expect("failed to read fish completion");

    let required_flags = [
        "-h",
        "--help",
        "-v",
        "--version",
        "-p",
        "--preferences",
        "--quake",
        "--quake-toggle",
    ];

    for flag in &required_flags {
        assert!(
            bash.contains(flag),
            "Bash completion missing flag: {}",
            flag
        );
        assert!(zsh.contains(flag), "Zsh completion missing flag: {}", flag);
        assert!(
            fish.contains(flag.trim_start_matches('-')),
            "Fish completion missing flag: {}",
            flag
        );
    }
}

#[test]
fn test_flatpak_manifest_validity() {
    let content = fs::read_to_string("build-aux/com.github.tilix_rust.json")
        .expect("failed to read Flatpak manifest");
    let val: serde_json::Value =
        serde_json::from_str(&content).expect("Flatpak manifest must be valid JSON");

    assert_eq!(val["app-id"], "com.github.tilix_rust");
    assert_eq!(val["runtime"], "org.gnome.Platform");
    assert_eq!(val["runtime-version"], "47");
    assert_eq!(val["sdk"], "org.gnome.Sdk");

    let finish_args = val["finish-args"]
        .as_array()
        .expect("finish-args should be array");
    let args_str: Vec<&str> = finish_args.iter().filter_map(|v| v.as_str()).collect();

    assert!(args_str.contains(&"--share=ipc"));
    assert!(args_str.contains(&"--socket=wayland"));
    assert!(args_str.contains(&"--socket=fallback-x11"));
    assert!(args_str.contains(&"--filesystem=host"));
    assert!(args_str.contains(&"--device=all"));
    assert!(args_str.contains(&"--talk-name=org.freedesktop.Notifications"));
}

#[test]
fn test_metainfo_xml_validity() {
    let content = fs::read_to_string("data/com.github.tilix_rust.metainfo.xml")
        .expect("failed to read metainfo");
    assert!(content.contains("<component type=\"desktop-application\">"));
    assert!(content.contains("<id>com.github.tilix_rust</id>"));
    assert!(content.contains("<metadata_license>CC0-1.0</metadata_license>"));
    assert!(content.contains("<project_license>MPL-2.0</project_license>"));
    assert!(content.contains("<name>Tilix</name>"));
    assert!(content.contains("<launchable type=\"desktop-id\">com.github.tilix_rust.desktop</launchable>"));
    assert!(content.contains("<releases>"), "Metainfo XML must declare a <releases> block");
    assert!(content.contains("<release version="), "Metainfo XML must declare at least one <release version=...> entry");
}

#[test]
fn test_svg_icon_validity() {
    let content = fs::read_to_string("data/icons/scalable/apps/com.github.tilix_rust.svg")
        .expect("failed to read svg icon");
    assert!(content.contains("<svg"));
    assert!(content.contains("viewBox=\"0 0 128 128\""));
    assert!(content.contains("</svg>"));
}
