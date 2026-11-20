use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_skillguard");

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn scan_path(path: PathBuf, extra: &[&str]) -> (i32, Value, String) {
    let mut cmd = Command::new(BIN);
    cmd.arg("scan")
        .arg(path)
        .arg("--format")
        .arg("json")
        .arg("--no-color");
    for arg in extra {
        cmd.arg(arg);
    }
    let output = cmd.output().expect("failed to run skillguard");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("invalid JSON ({e})\nstdout: {stdout}\nstderr: {stderr}"));
    (output.status.code().unwrap_or(-1), value, stderr)
}

fn scan(fixture_name: &str, extra: &[&str]) -> (i32, Value, String) {
    scan_path(fixture(fixture_name), extra)
}

fn has_rule(value: &Value, rule_id: &str) -> bool {
    value["findings"]
        .as_array()
        .map(|items| items.iter().any(|f| f["rule_id"] == rule_id))
        .unwrap_or(false)
}

fn severity_of(value: &Value, rule_id: &str) -> Option<String> {
    value["findings"]
        .as_array()?
        .iter()
        .find(|f| f["rule_id"] == rule_id)
        .and_then(|f| f["severity"].as_str())
        .map(ToOwned::to_owned)
}

#[test]
fn clean_skill_passes() {
    let (code, value, stderr) = scan("clean-skill", &[]);
    assert_eq!(code, 0, "clean skill should pass: {stderr}");
    assert_eq!(value["summary"]["total"].as_u64(), Some(0), "{value}");
}

#[test]
fn detects_prompt_injection() {
    let (code, value, _) = scan("prompt-injection", &[]);
    assert_eq!(code, 1);
    assert!(has_rule(&value, "SG-PI-001"), "{value}");
    assert!(has_rule(&value, "SG-PI-002"), "{value}");
    assert!(has_rule(&value, "SG-PI-003"), "{value}");
}

#[test]
fn detects_exfiltration_combo() {
    let (code, value, _) = scan("exfil-skill", &[]);
    assert_eq!(code, 1);
    assert!(has_rule(&value, "SG-PERM-002"), "{value}");
    assert!(has_rule(&value, "SG-NET-001"), "{value}");
    assert_eq!(
        severity_of(&value, "SG-COMBO-001").as_deref(),
        Some("critical"),
        "{value}"
    );
}

#[test]
fn allowlist_suppresses_known_domain() {
    let (_, value, _) = scan("exfil-skill", &["--allow-domain", "discord.com"]);
    assert!(!has_rule(&value, "SG-NET-001"), "{value}");
    assert!(!has_rule(&value, "SG-COMBO-001"), "{value}");
    assert!(has_rule(&value, "SG-PERM-002"), "{value}");
}

#[test]
fn detects_remote_execution_patterns() {
    let (code, value, _) = scan("dangerous-skill", &[]);
    assert_eq!(code, 1);
    assert!(has_rule(&value, "SG-NET-003"), "{value}");
    assert!(has_rule(&value, "SG-SC-003"), "{value}");
}

#[test]
fn inline_and_file_suppression_work() {
    let (code, value, stderr) = scan("suppressed-skill", &[]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!has_rule(&value, "SG-CMD-001"), "{value}");
    assert!(!has_rule(&value, "SG-HYG-003"), "{value}");
}

#[test]
fn collection_discovers_all_skills() {
    let (_, value, _) = scan("collection", &[]);
    assert_eq!(value["summary"]["skills"].as_u64(), Some(2), "{value}");
    assert!(has_rule(&value, "SG-PI-001"), "{value}");
}

#[test]
fn ignore_rule_works() {
    let (_, value, _) = scan("exfil-skill", &["--ignore", "SG-COMBO-001"]);
    assert!(!has_rule(&value, "SG-COMBO-001"), "{value}");
}

#[test]
fn fail_on_none_never_fails() {
    let (code, value, _) = scan("prompt-injection", &["--fail-on", "none"]);
    assert_eq!(code, 0, "{value}");
    assert!(has_rule(&value, "SG-PI-001"), "{value}");
}

#[test]
fn sarif_output_is_valid() {
    let output = Command::new(BIN)
        .arg("scan")
        .arg(fixture("prompt-injection"))
        .arg("--format")
        .arg("sarif")
        .output()
        .expect("failed to run skillguard");
    let value: Value = serde_json::from_slice(&output.stdout).expect("valid SARIF JSON");
    assert_eq!(value["version"], "2.1.0");
    assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "SkillGuard");
    assert!(!value["runs"][0]["results"]
        .as_array()
        .expect("results array")
        .is_empty());
}

#[test]
fn text_output_has_result_and_no_ansi_when_disabled() {
    let output = Command::new(BIN)
        .arg("scan")
        .arg(fixture("prompt-injection"))
        .arg("--no-color")
        .output()
        .expect("failed to run skillguard");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Result: FAIL"), "{stdout}");
    assert!(!stdout.contains('\u{1b}'), "ANSI codes should be disabled");
}

#[test]
fn config_exclude_applies_during_skill_discovery() {
    let (code, value, stderr) = scan("exclude-root", &[]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(value["summary"]["skills"].as_u64(), Some(1), "{value}");
    assert!(!has_rule(&value, "SG-PI-001"), "{value}");
}

#[test]
fn config_exclude_skips_paths() {
    let (code, value, stderr) = scan("config-skill", &[]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!has_rule(&value, "SG-NET-003"), "{value}");
    assert_eq!(value["summary"]["total"].as_u64(), Some(0), "{value}");
}

#[test]
fn external_rule_pack_loads() {
    let rules_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("extra-rules");
    let (code, value, _) = scan(
        "dangerous-skill",
        &["--rules", rules_dir.to_str().expect("utf-8 path")],
    );
    assert_eq!(code, 1);
    assert!(has_rule(&value, "ACME-NET-001"), "{value}");
}

#[test]
fn secret_excerpt_is_masked() {
    let (_, value, _) = scan("secret-skill", &["--fail-on", "none"]);
    assert!(has_rule(&value, "SG-SEC-002"), "{value}");
    let serialized = value.to_string();
    assert!(
        !serialized.contains("abcdefghijklmnopqrstuvwxyz123456"),
        "full secret leaked in output: {serialized}"
    );
    assert!(
        serialized.contains('…'),
        "masked excerpt expected: {serialized}"
    );
}

#[test]
fn rules_list_prints_builtin_pack() {
    let output = Command::new(BIN)
        .args(["rules", "list"])
        .output()
        .expect("failed to run skillguard");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SG-PI-001"), "{stdout}");
    assert!(stdout.contains("SG-COMBO-001"), "{stdout}");
}
#[test]
fn detects_chinese_prompt_injection() {
    let (code, value, _) = scan_path(repo_path("bench/samples/zh/zh-pi-ignore"), &[]);
    assert_eq!(code, 1);
    assert!(has_rule(&value, "SG-PI-001"), "{value}");
}

#[test]
fn detects_container_embedded_script() {
    let (code, value, _) = scan_path(repo_path("bench/samples/zh/zh-docx-script"), &[]);
    assert_eq!(code, 1);
    assert_eq!(
        severity_of(&value, "SG-CONT-001").as_deref(),
        Some("high"),
        "{value}"
    );
}

#[test]
fn detects_bytecode_source_mismatch() {
    let (code, value, _) = scan_path(repo_path("bench/samples/zh/zh-pyc-mismatch"), &[]);
    assert_eq!(code, 1);
    assert!(has_rule(&value, "SG-PYC-001"), "{value}");
}

#[test]
fn detects_whitespace_padding_payload() {
    let (code, value, _) = scan("padding-skill", &[]);
    assert_eq!(code, 1);
    assert!(has_rule(&value, "SG-OBF-003"), "{value}");
    assert!(has_rule(&value, "SG-DATA-002"), "{value}");
}

#[test]
fn detects_registry_hijack_combo() {
    let (code, value, _) = scan_path(repo_path("bench/samples/zh/zh-npm-registry"), &[]);
    assert_eq!(code, 1);
    assert!(has_rule(&value, "SG-SUPPLY-004"), "{value}");
    assert_eq!(
        severity_of(&value, "SG-COMBO-004").as_deref(),
        Some("critical"),
        "{value}"
    );
}
#[test]
fn benign_curl_pipe_json_is_not_download_exec() {
    let (code, value, _) = scan("curl-json-benign", &[]);
    assert!(!has_rule(&value, "SG-NET-004"), "{value}");
    assert_ne!(code, 1, "{value}");
}

#[test]
fn curl_pipe_python_exec_still_flags() {
    let (code, value, _) = scan("curl-python-exec", &[]);
    assert_eq!(code, 1);
    assert!(has_rule(&value, "SG-NET-004"), "{value}");
}
