use memento::config::AppConfig;

#[test]
fn default_config_schema_version() {
    let config = AppConfig::default();
    assert_eq!(config.schema_version, 1);
}

#[test]
fn default_config_empty_roots() {
    let config = AppConfig::default();
    assert!(config.scan.roots.is_empty());
}

#[test]
fn default_config_image_extensions_count() {
    let config = AppConfig::default();
    // Expect the known set of 20 image extensions
    assert!(config.scan.image_extensions.len() >= 20);
}

#[test]
fn default_config_video_extensions_count() {
    let config = AppConfig::default();
    assert_eq!(config.scan.video_extensions.len(), 12);
}

#[test]
fn default_config_extensions_are_lowercase() {
    let config = AppConfig::default();
    for ext in &config.scan.image_extensions {
        assert_eq!(
            ext,
            &ext.to_lowercase(),
            "extension '{}' should be lowercase",
            ext
        );
    }
    for ext in &config.scan.video_extensions {
        assert_eq!(
            ext,
            &ext.to_lowercase(),
            "extension '{}' should be lowercase",
            ext
        );
    }
}

#[test]
fn default_config_batch_sizes() {
    let config = AppConfig::default();
    assert_eq!(config.scan.metadata.batch_size, 500);
    assert_eq!(config.scan.hash.batch_size, 100);
    assert_eq!(config.scan.hash.parallelism, 0);
}

#[test]
fn config_yaml_roundtrip() {
    let config = AppConfig::default();
    let yaml_str = serde_yml::to_string(&config).unwrap();
    let parsed: AppConfig = serde_yml::from_str(&yaml_str).unwrap();

    assert_eq!(parsed.schema_version, config.schema_version);
    assert_eq!(parsed.scan.roots, config.scan.roots);
    assert_eq!(parsed.scan.image_extensions, config.scan.image_extensions);
    assert_eq!(parsed.scan.video_extensions, config.scan.video_extensions);
    assert_eq!(
        parsed.scan.metadata.batch_size,
        config.scan.metadata.batch_size
    );
    assert_eq!(parsed.scan.hash.batch_size, config.scan.hash.batch_size);
}

#[test]
fn config_yaml_roundtrip_with_roots() {
    let mut config = AppConfig::default();
    config.scan.roots = vec!["/photos".into(), "/backup/photos".into()];

    let yaml_str = serde_yml::to_string(&config).unwrap();
    let parsed: AppConfig = serde_yml::from_str(&yaml_str).unwrap();

    assert_eq!(parsed.scan.roots, vec!["/photos", "/backup/photos"]);
}

#[test]
fn config_partial_yaml_uses_defaults() {
    let yaml_str = r#"
scan:
  roots:
    - /my/photos
"#;
    let parsed: AppConfig = serde_yml::from_str(yaml_str).unwrap();

    assert_eq!(parsed.schema_version, 1);
    assert_eq!(parsed.scan.roots, vec!["/my/photos"]);
    // Defaults should fill in
    assert!(!parsed.scan.image_extensions.is_empty());
    assert!(!parsed.scan.video_extensions.is_empty());
    assert_eq!(parsed.scan.metadata.batch_size, 500);
}

#[test]
fn config_empty_yaml_uses_all_defaults() {
    let parsed: AppConfig = serde_yml::from_str("{}").unwrap();
    let default = AppConfig::default();

    assert_eq!(parsed.schema_version, default.schema_version);
    assert_eq!(
        parsed.scan.image_extensions.len(),
        default.scan.image_extensions.len()
    );
}

#[test]
fn config_windows_paths_unquoted() {
    let yaml_str = r#"
scan:
  roots:
    - D:\Photos\Vacation
    - E:\Backup
"#;
    let parsed: AppConfig = serde_yml::from_str(yaml_str).unwrap();
    assert_eq!(parsed.scan.roots, vec![r"D:\Photos\Vacation", r"E:\Backup"]);
}

#[test]
fn config_known_image_extensions_present() {
    let config = AppConfig::default();
    let exts = &config.scan.image_extensions;

    // Spot check critical formats
    assert!(exts.contains(&"jpg".to_string()));
    assert!(exts.contains(&"jpeg".to_string()));
    assert!(exts.contains(&"png".to_string()));
    assert!(exts.contains(&"heic".to_string()));
    assert!(exts.contains(&"cr2".to_string()));
    assert!(exts.contains(&"dng".to_string()));
    assert!(exts.contains(&"tiff".to_string()));
}

#[test]
fn config_known_video_extensions_present() {
    let config = AppConfig::default();
    let exts = &config.scan.video_extensions;

    assert!(exts.contains(&"mp4".to_string()));
    assert!(exts.contains(&"mov".to_string()));
    assert!(exts.contains(&"mkv".to_string()));
    assert!(exts.contains(&"avi".to_string()));
}
