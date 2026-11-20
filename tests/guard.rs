use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const BIN: &str = env!("CARGO_BIN_EXE_skillguard");

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "skillguard-guard-test-{name}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(BIN)
        .args(args)
        .output()
        .expect("failed to run skillguard");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn lock_and_verify_detect_drift() {
    let root = temp_dir("lock");
    let lock = root.join("skillguard.lock");
    let (code, stdout, stderr) = run(&[
        "lock",
        fixture("clean-skill").to_str().unwrap(),
        "--output",
        lock.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "lock failed\nstdout: {stdout}\nstderr: {stderr}");
    assert!(lock.is_file());
    assert!(stdout.contains("fingerprint:"), "{stdout}");

    let (code, stdout, stderr) = run(&[
        "verify",
        fixture("clean-skill").to_str().unwrap(),
        "--lock",
        lock.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "verify failed\nstdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("PASS"), "{stdout}");

    let skill_md = fixture("clean-skill").join("SKILL.md");
    let original = fs::read_to_string(&skill_md).unwrap();
    fs::write(&skill_md, format!("{original}\n<!-- drift -->\n")).unwrap();
    let (code, stdout, stderr) = run(&[
        "verify",
        fixture("clean-skill").to_str().unwrap(),
        "--lock",
        lock.to_str().unwrap(),
    ]);
    fs::write(&skill_md, original).unwrap();
    assert_eq!(
        code, 1,
        "drift should fail\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("modified"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn policy_blocks_high_findings() {
    let root = temp_dir("policy");
    let policy = root.join("policy.json");
    fs::write(
        &policy,
        r#"{"profile":"team","fail_on":"high","max_medium":10,"deny":[],"allow":[]}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "scan",
        fixture("dangerous-skill").to_str().unwrap(),
        "--policy",
        policy.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(
        code, 1,
        "policy should block\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stderr.contains("policy: BLOCK"), "{stderr}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn baseline_hides_known_findings() {
    let root = temp_dir("baseline");
    let baseline = root.join("baseline.json");
    let target = fixture("dangerous-skill");
    let (code, _, stderr) = run(&[
        "scan",
        target.to_str().unwrap(),
        "--write-baseline",
        baseline.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(code, 1, "initial scan should fail\n{stderr}");
    assert!(baseline.is_file());

    let (code, stdout, stderr) = run(&[
        "scan",
        target.to_str().unwrap(),
        "--baseline",
        baseline.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(
        code, 0,
        "baseline should hide findings\nstdout: {stdout}\nstderr: {stderr}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn audit_appends_jsonl() {
    let root = temp_dir("audit");
    let audit = root.join("audit.jsonl");
    let (code, stdout, stderr) = run(&[
        "scan",
        fixture("clean-skill").to_str().unwrap(),
        "--audit",
        audit.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "scan failed\nstdout: {stdout}\nstderr: {stderr}");
    let body = fs::read_to_string(&audit).unwrap();
    assert!(body.contains("skillguard-audit/v1"), "{body}");
    assert!(body.contains("\"action\":\"scan\""), "{body}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mcp_risk_rules_fire() {
    let (code, stdout, stderr) = run(&[
        "scan",
        fixture("mcp-risk").to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(
        code, 1,
        "MCP risk should fail\nstdout: {stdout}\nstderr: {stderr}"
    );
    for rule in ["SG-MCP-002", "SG-MCP-003", "SG-MCP-004", "SG-MCP-005"] {
        assert!(stdout.contains(rule), "missing {rule}\n{stdout}");
    }
}

#[test]
fn mcp_clean_stays_clean() {
    let (code, stdout, stderr) = run(&[
        "scan",
        fixture("mcp-clean").to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(
        code, 0,
        "clean MCP should pass\nstdout: {stdout}\nstderr: {stderr}"
    );
}
