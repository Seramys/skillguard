use crate::engine::{self, ScanOptions};
use crate::frontmatter;
use crate::model::{Finding, Severity};
use crate::rules;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use zip::ZipArchive;

const RECEIPT: &str = ".skillguard-receipt.json";
const MAX_TREE_FILES: usize = 10_000;
const MAX_TREE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DOWNLOAD_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ZIP_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ZIP_TOTAL_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub source: String,
    pub git_ref: Option<String>,
    pub subpath: Option<String>,
    pub dest: Option<PathBuf>,
    pub target: Option<String>,
    pub rules_dir: Option<PathBuf>,
    pub allow_domains: Vec<String>,
    pub ignore_rules: Vec<String>,
    pub yes: bool,
    pub force: bool,
    pub dry_run: bool,
    pub keep_quarantine: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Counts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub total: usize,
}

impl Counts {
    pub fn from_findings(findings: &[Finding]) -> Self {
        let mut counts = Self {
            total: findings.len(),
            ..Self::default()
        };
        for finding in findings {
            match finding.severity {
                Severity::Critical => counts.critical += 1,
                Severity::High => counts.high += 1,
                Severity::Medium => counts.medium += 1,
                Severity::Low => counts.low += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingBrief {
    pub rule_id: String,
    pub severity: String,
    pub title: String,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Serialize)]
pub struct InstallOutcome {
    pub skill: String,
    pub source: String,
    pub resolved_ref: Option<String>,
    pub destination: Option<PathBuf>,
    pub verdict: String,
    pub fingerprint: String,
    pub counts: Counts,
    pub findings: Vec<FindingBrief>,
    pub dry_run: bool,
}

#[derive(Debug, Serialize)]
pub struct Drift {
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyOutcome {
    pub skill: String,
    pub path: PathBuf,
    pub ok: bool,
    pub expected: String,
    pub actual: String,
    pub drift: Vec<Drift>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Receipt {
    schema: String,
    tool_version: String,
    skill: String,
    source: String,
    resolved_ref: Option<String>,
    installed_at: u64,
    verdict: String,
    fingerprint: String,
    files: Vec<FileEntry>,
    findings: Counts,
}

#[derive(Debug)]
struct RemoteInput {
    owner: String,
    repo: String,
    git_ref: Option<String>,
    subpath: Option<String>,
}

pub fn install(opts: InstallOptions) -> Result<InstallOutcome, String> {
    let quarantine = make_quarantine()?;
    let result = install_inner(&opts, &quarantine);
    if !opts.keep_quarantine {
        let _ = fs::remove_dir_all(&quarantine);
    }
    result
}

fn install_inner(opts: &InstallOptions, quarantine: &Path) -> Result<InstallOutcome, String> {
    let (skill_root, source_label, resolved_ref) = prepare_source(opts, quarantine)?;
    let (findings, counts) = scan_root(
        &skill_root,
        opts.rules_dir.as_deref(),
        &opts.allow_domains,
        &opts.ignore_rules,
    )?;
    let skill = read_skill_name(&skill_root);
    let verdict = verdict(&counts);
    let briefs: Vec<FindingBrief> = findings
        .iter()
        .take(20)
        .map(|finding| FindingBrief {
            rule_id: finding.rule_id.clone(),
            severity: finding.severity.as_str().to_string(),
            title: finding.title.clone(),
            file: finding.file.clone(),
            line: finding.line,
        })
        .collect();

    if opts.dry_run {
        return Ok(InstallOutcome {
            skill,
            source: source_label,
            resolved_ref,
            destination: None,
            verdict,
            fingerprint: String::new(),
            counts,
            findings: briefs,
            dry_run: true,
        });
    }

    if verdict == "BLOCK" && !opts.force {
        return Err(format!(
            "安装被阻止：{} critical, {} high。审查后如确需安装，使用 --force --yes。",
            counts.critical, counts.high
        ));
    }
    if (verdict == "BLOCK" || verdict == "REVIEW") && !opts.yes {
        return Err(format!(
            "安装需要确认：verdict={verdict}（{} critical, {} high, {} medium）。确认后使用 --yes。",
            counts.critical, counts.high, counts.medium
        ));
    }

    let dest = resolve_destination(opts, &skill)?;
    if dest.exists() && !opts.force {
        return Err(format!(
            "目标已存在：{}；使用 --force 覆盖（会先备份旧版本）。",
            dest.display()
        ));
    }
    let parent = dest
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    let stage = parent.join(format!(
        ".skillguard-stage-{}-{}",
        std::process::id(),
        now_unix()
    ));
    if stage.exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    copy_tree(&skill_root, &stage)?;
    let (fingerprint, files) = fingerprint_tree(&stage)?;
    let final_verdict = if verdict == "BLOCK" {
        "BLOCK-FORCED".to_string()
    } else {
        verdict
    };
    let receipt = Receipt {
        schema: "skillguard-receipt/v1".to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        skill: skill.clone(),
        source: source_label.clone(),
        resolved_ref: resolved_ref.clone(),
        installed_at: now_unix(),
        verdict: final_verdict.clone(),
        fingerprint: fingerprint.clone(),
        files,
        findings: counts.clone(),
    };
    write_receipt(&stage, &receipt)?;
    atomic_install(&stage, &dest, opts.force)?;

    Ok(InstallOutcome {
        skill,
        source: source_label,
        resolved_ref,
        destination: Some(dest),
        verdict: final_verdict,
        fingerprint,
        counts,
        findings: briefs,
        dry_run: false,
    })
}

pub fn verify(path: &Path) -> Result<VerifyOutcome, String> {
    let root = if path.is_file() {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(path)
            .to_path_buf()
    } else {
        path.to_path_buf()
    };
    let receipt_path = root.join(RECEIPT);
    let text = fs::read_to_string(&receipt_path)
        .map_err(|e| format!("缺少 receipt {}: {e}", receipt_path.display()))?;
    let receipt: Receipt = serde_json::from_str(&text)
        .map_err(|e| format!("receipt 解析失败 {}: {e}", receipt_path.display()))?;
    if receipt.schema != "skillguard-receipt/v1" {
        return Err(format!("不支持的 receipt schema: {}", receipt.schema));
    }
    let (actual, files) = fingerprint_tree(&root)?;
    let old: BTreeMap<String, FileEntry> = receipt
        .files
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let new: BTreeMap<String, FileEntry> = files
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut drift = Vec::new();
    for path in old.keys() {
        if !new.contains_key(path) {
            drift.push(Drift {
                kind: "missing".to_string(),
                path: path.clone(),
            });
        }
    }
    for (path, entry) in &new {
        match old.get(path) {
            None => drift.push(Drift {
                kind: "added".to_string(),
                path: path.clone(),
            }),
            Some(expected) if expected.sha256 != entry.sha256 => drift.push(Drift {
                kind: "modified".to_string(),
                path: path.clone(),
            }),
            _ => {}
        }
    }
    Ok(VerifyOutcome {
        skill: receipt.skill,
        path: root,
        ok: drift.is_empty() && receipt.fingerprint == actual,
        expected: receipt.fingerprint,
        actual,
        drift,
    })
}

fn prepare_source(
    opts: &InstallOptions,
    quarantine: &Path,
) -> Result<(PathBuf, String, Option<String>), String> {
    if let Some(remote) = parse_remote(&opts.source) {
        let (sha, _ref_name) = resolve_remote(&remote, opts.git_ref.as_deref())?;
        let data = download_github_zip(&remote.owner, &remote.repo, &sha)?;
        let extract_root = quarantine.join("source");
        fs::create_dir_all(&extract_root)
            .map_err(|e| format!("cannot create {}: {e}", extract_root.display()))?;
        extract_zip(&data, &extract_root)?;
        let subpath = opts.subpath.as_deref().or(remote.subpath.as_deref());
        let root = select_skill_root(&extract_root, subpath)?;
        return Ok((
            root,
            format!("https://github.com/{}/{}", remote.owner, remote.repo),
            Some(sha),
        ));
    }

    let source = PathBuf::from(&opts.source);
    if !source.exists() {
        return Err(format!("source not found: {}", source.display()));
    }
    let root = if source.is_file() {
        source
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(&source)
            .to_path_buf()
    } else {
        source
    };
    let quarantine_root = quarantine.join("source");
    if quarantine_root.exists() {
        let _ = fs::remove_dir_all(&quarantine_root);
    }
    copy_tree(&root, &quarantine_root)?;
    let selected = select_skill_root(&quarantine_root, opts.subpath.as_deref())?;
    Ok((selected, opts.source.clone(), None))
}

fn scan_root(
    root: &Path,
    rules_dir: Option<&Path>,
    allow_domains: &[String],
    ignore_rules: &[String],
) -> Result<(Vec<Finding>, Counts), String> {
    let builtin = rules::load_builtin()?;
    let external = match rules_dir {
        Some(dir) => rules::load_dir(dir)?,
        None => Vec::new(),
    };
    let (merged, warnings) = rules::merge(builtin, external);
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
    let compiled = rules::compile_all(merged)?;
    let opts = ScanOptions {
        min_severity: Severity::Low,
        fail_on: Some(Severity::High),
        ignore_rules: ignore_rules.to_vec(),
        allow_domains: allow_domains.to_vec(),
        max_file_size: 512 * 1024,
        exclude: Vec::new(),
    };
    let outcome = engine::scan(root, &compiled, &opts)?;
    let counts = Counts::from_findings(&outcome.findings);
    Ok((outcome.findings, counts))
}

fn verdict(counts: &Counts) -> String {
    if counts.critical > 0 || counts.high > 0 {
        "BLOCK".to_string()
    } else if counts.medium > 0 {
        "REVIEW".to_string()
    } else {
        "PASS".to_string()
    }
}

fn resolve_destination(opts: &InstallOptions, skill: &str) -> Result<PathBuf, String> {
    if opts.dest.is_some() && opts.target.is_some() {
        return Err("--dest 和 --target 只能二选一".to_string());
    }
    if let Some(dest) = &opts.dest {
        return Ok(dest.clone());
    }
    let target = opts.target.as_deref().ok_or_else(|| {
        "必须指定 --dest <dir> 或 --target <claude|codex|cursor|openclaw>".to_string()
    })?;
    let home = home_dir().ok_or_else(|| "无法确定用户目录；请使用 --dest".to_string())?;
    let root = match target {
        "claude" => home.join(".claude").join("skills"),
        "codex" => home.join(".codex").join("skills"),
        "cursor" => home.join(".cursor").join("skills"),
        "openclaw" => home.join(".openclaw").join("skills"),
        "custom" => {
            return Err("--target custom 需要配合 --dest <dir>".to_string());
        }
        other => {
            return Err(format!(
                "未知 target: {other}（claude/codex/cursor/openclaw）"
            ))
        }
    };
    Ok(root.join(skill))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

pub fn read_skill_name(root: &Path) -> String {
    if let Ok(content) = fs::read_to_string(root.join("SKILL.md")) {
        let parsed = frontmatter::parse(&content);
        if let Some(name) = parsed.get("name") {
            let clean = sanitize_name(name);
            if !clean.is_empty() {
                return clean;
            }
        }
    }
    root.file_name()
        .and_then(|name| name.to_str())
        .map(sanitize_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "skill".to_string())
}

fn sanitize_name(input: &str) -> String {
    let mut cleaned: String = input
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .collect();
    while cleaned.contains("..") {
        cleaned = cleaned.replace("..", "");
    }
    cleaned.trim_matches('.').to_string()
}

fn make_quarantine() -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!(
        "skillguard-quarantine-{}-{}",
        std::process::id(),
        now_unix()
    ));
    fs::create_dir_all(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    Ok(path)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn parse_remote(input: &str) -> Option<RemoteInput> {
    let trimmed = input.trim();
    let rest = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))?;
    let mut parts = rest.trim_end_matches('/').split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    let tail: Vec<&str> = parts.collect();
    let mut git_ref = None;
    let mut subpath = None;
    if tail.first() == Some(&"tree") {
        git_ref = tail.get(1).map(|value| value.to_string());
        if tail.len() > 2 {
            subpath = Some(tail[2..].join("/"));
        }
    }
    Some(RemoteInput {
        owner,
        repo,
        git_ref,
        subpath,
    })
}

fn resolve_remote(
    remote: &RemoteInput,
    requested_ref: Option<&str>,
) -> Result<(String, String), String> {
    let api = format!(
        "https://api.github.com/repos/{}/{}",
        remote.owner, remote.repo
    );
    let info = http_json(&api)?;
    let default_branch = info
        .get("default_branch")
        .and_then(|value| value.as_str())
        .unwrap_or("main")
        .to_string();
    let reference = requested_ref
        .or(remote.git_ref.as_deref())
        .unwrap_or(&default_branch)
        .to_string();
    if is_commit_sha(&reference) {
        return Ok((reference.clone(), reference));
    }
    let commits = http_json(&format!(
        "https://api.github.com/repos/{}/{}/commits?sha={}&per_page=1",
        remote.owner,
        remote.repo,
        url_encode(&reference)
    ))?;
    let sha = commits
        .get(0)
        .and_then(|value| value.get("sha"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("无法解析 GitHub ref: {reference}"))?
        .to_string();
    Ok((sha, reference))
}

fn download_github_zip(owner: &str, repo: &str, sha: &str) -> Result<Vec<u8>, String> {
    let url = format!("https://codeload.github.com/{owner}/{repo}/zip/{sha}");
    let response = ureq::get(&url)
        .set("User-Agent", "skillguard")
        .call()
        .map_err(|error| format!("下载失败 {url}: {error}"))?;
    let mut data = Vec::new();
    response
        .into_reader()
        .take(MAX_DOWNLOAD_BYTES)
        .read_to_end(&mut data)
        .map_err(|error| format!("读取下载内容失败: {error}"))?;
    if data.is_empty() {
        return Err("下载内容为空".to_string());
    }
    Ok(data)
}

fn http_json(url: &str) -> Result<serde_json::Value, String> {
    let response = ureq::get(url)
        .set("User-Agent", "skillguard")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| format!("GitHub API 请求失败 {url}: {error}"))?;
    let mut body = String::new();
    response
        .into_reader()
        .read_to_string(&mut body)
        .map_err(|error| format!("读取 GitHub API 响应失败: {error}"))?;
    serde_json::from_str(&body).map_err(|error| format!("GitHub API 返回非 JSON: {error}"))
}

fn url_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn extract_zip(data: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive =
        ZipArchive::new(Cursor::new(data)).map_err(|error| format!("ZIP 解析失败: {error}"))?;
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| format!("ZIP 条目读取失败: {error}"))?;
        if entry.is_dir() {
            continue;
        }
        if entry.unix_mode().map(|mode| mode & 0o170000 == 0o120000) == Some(true) {
            return Err(format!("ZIP 包含符号链接，拒绝安装: {}", entry.name()));
        }
        let raw_name = entry.name().replace('\\', "/");
        if raw_name.starts_with('/') || raw_name.split('/').any(|part| part == "..") {
            return Err(format!("ZIP 包含不安全路径: {raw_name}"));
        }
        let size = entry.size();
        if size > MAX_ZIP_ENTRY_BYTES || total.saturating_add(size) > MAX_ZIP_TOTAL_BYTES {
            return Err(format!("ZIP 条目过大或总大小超限: {raw_name}"));
        }
        let out_path = dest.join(&raw_name);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let mut file = fs::File::create(&out_path)
            .map_err(|e| format!("cannot create {}: {e}", out_path.display()))?;
        let mut limited = entry.take(MAX_ZIP_ENTRY_BYTES + 1);
        let written = std::io::copy(&mut limited, &mut file)
            .map_err(|e| format!("cannot write {}: {e}", out_path.display()))?;
        if written > MAX_ZIP_ENTRY_BYTES {
            return Err(format!("ZIP 条目解压后超限: {raw_name}"));
        }
        total += written;
    }
    Ok(())
}

fn select_skill_root(extracted: &Path, subpath: Option<&str>) -> Result<PathBuf, String> {
    if let Some(subpath) = subpath {
        if subpath.split('/').any(|part| part == "..") {
            return Err("--path 不允许包含 ..".to_string());
        }
        let root = extracted.join(subpath);
        if !root.exists() {
            return Err(format!("--path 不存在: {}", root.display()));
        }
        return Ok(root);
    }
    if extracted.join("SKILL.md").is_file() {
        return Ok(extracted.to_path_buf());
    }
    let mut roots = Vec::new();
    find_skill_roots(extracted, extracted, 0, &mut roots)?;
    match roots.len() {
        0 => Ok(extracted.to_path_buf()),
        1 => Ok(roots.remove(0)),
        _ => {
            let names: Vec<String> = roots
                .iter()
                .map(|path| {
                    path.strip_prefix(extracted)
                        .unwrap_or(path)
                        .display()
                        .to_string()
                })
                .collect();
            Err(format!(
                "发现多个 Skill，请用 --path 指定一个：{}",
                names.join(", ")
            ))
        }
    }
}

fn find_skill_roots(
    root: &Path,
    current: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if depth > 8 {
        return Ok(());
    }
    let entries =
        fs::read_dir(current).map_err(|e| format!("cannot read {}: {e}", current.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read entry: {e}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(name.as_str(), ".git" | "node_modules" | "target" | "dist") {
                continue;
            }
            if path.join("SKILL.md").is_file() {
                out.push(path);
            } else {
                find_skill_roots(root, &path, depth + 1, out)?;
            }
        }
    }
    let _ = root;
    Ok(())
}

fn copy_tree(source: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("cannot create {}: {e}", dest.display()))?;
    let mut files = 0usize;
    let mut bytes = 0u64;
    copy_tree_inner(source, dest, &mut files, &mut bytes)?;
    Ok(())
}

fn copy_tree_inner(
    source: &Path,
    dest: &Path,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), String> {
    for entry in
        fs::read_dir(source).map_err(|e| format!("cannot read {}: {e}", source.display()))?
    {
        let entry = entry.map_err(|e| format!("cannot read entry: {e}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
        if file_type.is_symlink() {
            return Err(format!("发现符号链接，拒绝安装: {}", path.display()));
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == RECEIPT {
            continue;
        }
        let target = dest.join(&name);
        if file_type.is_dir() {
            fs::create_dir_all(&target)
                .map_err(|e| format!("cannot create {}: {e}", target.display()))?;
            copy_tree_inner(&path, &target, files, bytes)?;
        } else if file_type.is_file() {
            let size = entry
                .metadata()
                .map_err(|e| format!("cannot stat {}: {e}", path.display()))?
                .len();
            *files += 1;
            *bytes = bytes.saturating_add(size);
            if *files > MAX_TREE_FILES || *bytes > MAX_TREE_BYTES {
                return Err("Skill 文件数或总大小超限".to_string());
            }
            fs::copy(&path, &target).map_err(|e| format!("cannot copy {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

pub fn fingerprint_tree(root: &Path) -> Result<(String, Vec<FileEntry>), String> {
    fingerprint_tree_excluding(root, &[])
}

pub fn fingerprint_tree_excluding(
    root: &Path,
    excludes: &[&Path],
) -> Result<(String, Vec<FileEntry>), String> {
    let exclude_set: BTreeSet<PathBuf> = excludes
        .iter()
        .map(|path| normalize_absolute(path))
        .collect();
    let mut paths = Vec::new();
    collect_paths(root, root, 0, &mut paths, &exclude_set)?;
    paths.sort();
    let mut entries = Vec::new();
    let mut aggregate = Sha256::new();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let data = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let digest = Sha256::digest(&data);
        let hex = hex(&digest);
        aggregate.update(relative.as_bytes());
        aggregate.update([0]);
        aggregate.update(digest.as_slice());
        aggregate.update(b"\n");
        entries.push(FileEntry {
            path: relative,
            sha256: hex,
            size: data.len() as u64,
        });
    }
    Ok((hex(&aggregate.finalize()), entries))
}

fn normalize_absolute(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn is_metadata_name(name: &str) -> bool {
    matches!(
        name,
        ".skillguard-receipt.json"
            | "skillguard.lock"
            | ".skillguard.lock"
            | "baseline.json"
            | "skillguard-baseline.json"
            | ".skillguard-baseline.json"
            | "audit.jsonl"
            | "skillguard-audit.jsonl"
            | ".skillguard-audit.jsonl"
    )
}

fn collect_paths(
    root: &Path,
    current: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
    excludes: &BTreeSet<PathBuf>,
) -> Result<(), String> {
    if depth > 16 {
        return Err("目录层级过深".to_string());
    }
    for entry in
        fs::read_dir(current).map_err(|e| format!("cannot read {}: {e}", current.display()))?
    {
        let entry = entry.map_err(|e| format!("cannot read entry: {e}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("cannot stat {}: {e}", path.display()))?;
        if file_type.is_symlink() {
            return Err(format!("发现符号链接，拒绝校验: {}", path.display()));
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || is_metadata_name(&name) {
            continue;
        }
        if excludes.contains(&normalize_absolute(&path)) {
            continue;
        }
        if file_type.is_dir() {
            collect_paths(root, &path, depth + 1, out, excludes)?;
        } else if file_type.is_file() {
            out.push(path);
            if out.len() > MAX_TREE_FILES {
                return Err("文件数超限".to_string());
            }
        }
    }
    let _ = root;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn write_receipt(stage: &Path, receipt: &Receipt) -> Result<(), String> {
    let path = stage.join(RECEIPT);
    let body =
        serde_json::to_string_pretty(receipt).map_err(|e| format!("receipt 序列化失败: {e}"))?;
    fs::write(&path, format!("{body}\n"))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn atomic_install(stage: &Path, dest: &Path, force: bool) -> Result<(), String> {
    if !dest.exists() {
        return fs::rename(stage, dest)
            .map_err(|e| format!("cannot move into {}: {e}", dest.display()));
    }
    if !force {
        return Err(format!("目标已存在: {}", dest.display()));
    }
    let name = dest
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("skill");
    let backup = dest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{name}.bak-{}", now_unix()));
    fs::rename(dest, &backup).map_err(|e| format!("cannot backup {}: {e}", dest.display()))?;
    match fs::rename(stage, dest) {
        Ok(()) => {
            let _ = fs::remove_dir_all(&backup);
            Ok(())
        }
        Err(error) => {
            let _ = fs::rename(&backup, dest);
            Err(format!("cannot move into {}: {error}", dest.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_sources() {
        let remote = parse_remote("https://github.com/owner/repo/tree/main/skills/demo").unwrap();
        assert_eq!(remote.owner, "owner");
        assert_eq!(remote.repo, "repo");
        assert_eq!(remote.git_ref.as_deref(), Some("main"));
        assert_eq!(remote.subpath.as_deref(), Some("skills/demo"));
    }

    #[test]
    fn sanitizes_skill_names() {
        assert_eq!(sanitize_name("my skill/../x"), "myskillx");
    }

    #[test]
    fn detects_commit_shas() {
        assert!(is_commit_sha("4ffbf9461ef0505f9ce76a0d3694a18ec33ea531"));
        assert!(!is_commit_sha("main"));
    }
}
