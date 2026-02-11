use silicube::config::Config;

use super::FIXTURES_PATH;

#[test]
fn test_load_valid_config() {
    let path = format!("{}/configs/valid_full.toml", FIXTURES_PATH);
    let config = Config::from_file(&path).expect("Failed to load config");

    assert!(config.languages.contains_key("cpp17"));
    assert!(config.languages.contains_key("python3"));
    assert_eq!(config.default_limits.time_limit, Some(2.0));
}

#[test]
fn test_load_minimal_config() {
    let path = format!("{}/configs/valid_minimal.toml", FIXTURES_PATH);
    let config = Config::from_file(&path).expect("Failed to load config");

    assert!(config.languages.contains_key("test"));
}

#[test]
fn test_load_invalid_empty_name() {
    let path = format!("{}/configs/invalid_empty_name.toml", FIXTURES_PATH);
    let result = Config::from_file(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_invalid_empty_extension() {
    let path = format!("{}/configs/invalid_empty_extension.toml", FIXTURES_PATH);
    let result = Config::from_file(&path);
    assert!(result.is_err());
}

#[test]
fn test_load_invalid_empty_run_command() {
    let path = format!("{}/configs/invalid_empty_run_command.toml", FIXTURES_PATH);
    let result = Config::from_file(&path);
    assert!(result.is_err());
}
