use std::fs;
use std::path::{Path, PathBuf};
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
        "skillguard-install-test-{name}-{}-{stamp}",
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

fn install(source: &Path, dest: &Path, extra: &[&str]) -> (i32, String, String) {
    let mut args = vec![
        "install",
        source.to_str().unwrap(),
        "--dest",
        dest.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    run(&args)
}

fn verify(path: &Path) -> (i32, String, String) {
    run(&["verify", path.to_str().unwrap()])
}

#[test]
fn installs_clean_skill_and_verifies_fingerprint() {
    let root = temp_dir("clean");
    let dest = root.join("installed");
    let (code, stdout, stderr) = install(&fixture("clean-skill"), &dest, &["--json"]);
    assert_eq!(
        code, 0,
        "install failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(dest.join(".skillguard-receipt.json").is_file());
    let receipt = fs::read_to_string(dest.join(".skillguard-receipt.json")).unwrap();
    assert!(receipt.contains("\"schema\": \"skillguard-receipt/v1\""));
    assert!(receipt.contains("\"verdict\": \"PASS\""));
    assert!(receipt.contains("\"fingerprint\""));

    let (code, stdout, stderr) = verify(&dest);
    assert_eq!(code, 0, "verify failed\nstdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("PASS (no drift)"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn verify_detects_modified_file() {
    let root = temp_dir("tamper");
    let dest = root.join("installed");
    let (code, _, stderr) = install(&fixture("clean-skill"), &dest, &[]);
    assert_eq!(code, 0, "{stderr}");
    let skill_md = dest.join("SKILL.md");
    let mut body = fs::read_to_string(&skill_md).unwrap();
    body.push_str("\n<!-- changed after install -->\n");
    fs::write(&skill_md, body).unwrap();

    let (code, stdout, stderr) = verify(&dest);
    assert_eq!(
        code, 1,
        "drift should fail\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("modified"), "{stdout}");
    assert!(stdout.contains("SKILL.md"), "{stdout}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn install_blocks_high_severity_without_force() {
    let root = temp_dir("block");
    let dest = root.join("blocked");
    let (code, stdout, stderr) = install(&fixture("dangerous-skill"), &dest, &[]);
    assert_ne!(code, 0, "high severity must be blocked\nstdout: {stdout}");
    assert!(stderr.contains("安装被阻止"), "{stderr}");
    assert!(!dest.exists(), "destination must not be created");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn review_requires_yes() {
    let root = temp_dir("review");
    let dest = root.join("reviewed");
    let (code, stdout, stderr) = install(&fixture("review-skill"), &dest, &[]);
    assert_ne!(code, 0, "review must require --yes\nstdout: {stdout}");
    assert!(stderr.contains("--yes"), "{stderr}");
    assert!(!dest.exists());

    let (code, stdout, stderr) = install(&fixture("review-skill"), &dest, &["--yes", "--json"]);
    assert_eq!(
        code, 0,
        "review install failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    let receipt = fs::read_to_string(dest.join(".skillguard-receipt.json")).unwrap();
    assert!(receipt.contains("\"verdict\": \"REVIEW\""), "{receipt}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn existing_destination_requires_force() {
    let root = temp_dir("force");
    let dest = root.join("installed");
    let (code, _, stderr) = install(&fixture("clean-skill"), &dest, &[]);
    assert_eq!(code, 0, "{stderr}");
    let (code, _, stderr) = install(&fixture("clean-skill"), &dest, &[]);
    assert_ne!(
        code, 0,
        "existing destination must require --force\n{stderr}"
    );
    let (code, stdout, stderr) = install(&fixture("clean-skill"), &dest, &["--force"]);
    assert_eq!(
        code, 0,
        "force install failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn dry_run_writes_nothing() {
    let root = temp_dir("dry");
    let dest = root.join("dry-run-dest");
    let (code, stdout, stderr) = install(&fixture("clean-skill"), &dest, &["--dry-run"]);
    assert_eq!(
        code, 0,
        "dry run failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("dry-run: no files written"), "{stdout}");
    assert!(!dest.exists());
    let _ = fs::remove_dir_all(root);
}
