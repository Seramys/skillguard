mod config;
mod engine;
mod frontmatter;
mod guard;
mod install;
mod model;
mod pyc;
mod report;
mod rules;

use engine::ScanOptions;
use model::Severity;
use regex::Regex;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BOOL_FLAGS: &[&str] = &[
    "no-color",
    "quiet",
    "json-pretty",
    "json",
    "force",
    "yes",
    "dry-run",
    "keep-quarantine",
    "help",
];

fn main() {
    process::exit(run());
}

fn run() -> i32 {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        print_help();
        return 0;
    };
    let rest = &args[1..];
    let result = match command {
        "scan" => cmd_scan(rest),
        "lock" => cmd_lock(rest),
        "install" => cmd_install(rest),
        "verify" => cmd_verify(rest),
        "policy" => cmd_policy(rest),
        "rules" => cmd_rules(rest),
        "init" => cmd_init(rest),
        "version" | "--version" | "-V" => {
            println!("skillguard {VERSION}");
            Ok(0)
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(0)
        }
        other => Err(format!(
            "unknown command: {other}\nRun `skillguard help` for usage."
        )),
    };
    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            2
        }
    }
}

fn print_help() {
    println!(
        r#"SkillGuard {VERSION} - AI Agent Skill supply-chain security scanner

Usage:
  skillguard scan <path> [options]
  skillguard lock <path> [--output skillguard.lock]
  skillguard verify <path> [--lock skillguard.lock]
  skillguard policy validate <file.json>
  skillguard install <source> [options]   (experimental: P1 install rail)
  skillguard rules list
  skillguard rules validate <file.json|dir>
  skillguard init [dir] [--force]
  skillguard version

Scan options:
  --format text|json|sarif       Output format (default: text)
  --fail-on none|low|medium|high|critical
                                 Exit 1 when a finding reaches this severity (default: high)
  --min-severity low|medium|high|critical
                                 Hide findings below this severity (default: low)
  --policy <file>                Policy gate (default: .skillguard-policy.json if present)
  --baseline <file>              Hide findings recorded in a baseline
  --write-baseline <file>        Write current findings as a baseline
  --audit <file>                 Append a JSONL audit record
  --ignore SG-XX-001[,SG-YY-*]   Ignore rules by id or prefix
  --allow-domain <domain>        Allow network findings for a domain (repeatable/comma)
  --rules <dir>                  Load extra JSON rule packs (overrides by id)
  --config <file>                Config file (default: .skillguard.json)
  --max-file-size <kb>           Skip text scanning above this size (default: 512)
  --no-color                     Disable ANSI colors
  --quiet                        Hide header and summary
  --json-pretty                  Pretty-print JSON output

Lock / verify options:
  --output <file>                Lockfile path (default: skillguard.lock)
  --source <label>               Source label stored in the lockfile
  --lock <file>                  Verify against a lockfile instead of an install receipt
  --json                         Machine-readable result
  --audit <file>                 Append a JSONL audit record

Policy file:
  {{
    "profile": "team",
    "fail_on": "high",
    "max_medium": 10,
    "deny": ["SG-COMBO-001"],
    "allow": [{{"rule": "SG-PERM-001", "owner": "alice",
               "reason": "approved", "expires": "2026-12-31"}}]
  }}

Examples:
  skillguard scan ./skills --policy .skillguard-policy.json --audit .skillguard-audit.jsonl
  skillguard scan ./skills --write-baseline baseline.json
  skillguard lock ./my-skill --output skillguard.lock
  skillguard verify ./my-skill --lock skillguard.lock
  skillguard install https://github.com/owner/repo --target claude --yes
"#
    );
}

struct ParsedArgs {
    positional: Vec<String>,
    flags: BTreeMap<String, Vec<String>>,
}

impl ParsedArgs {
    fn has(&self, key: &str) -> bool {
        self.flags.contains_key(key)
    }

    fn one(&self, key: &str) -> Option<&str> {
        self.flags
            .get(key)
            .and_then(|values| values.last())
            .map(String::as_str)
    }

    fn all(&self, key: &str) -> Vec<String> {
        self.flags.get(key).cloned().unwrap_or_default()
    }
}

fn parse_args(args: &[String], known: &[&str]) -> Result<ParsedArgs, String> {
    let mut positional = Vec::new();
    let mut flags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut index = 0usize;
    let mut after_separator = false;

    while index < args.len() {
        let arg = &args[index];
        if after_separator {
            positional.push(arg.clone());
            index += 1;
            continue;
        }
        if arg == "--" {
            after_separator = true;
            index += 1;
            continue;
        }
        if arg == "-h" || arg == "--help" {
            flags
                .entry("help".to_string())
                .or_default()
                .push("true".into());
            index += 1;
            continue;
        }
        if let Some(rest) = arg.strip_prefix("--") {
            let (key, inline) = match rest.split_once('=') {
                Some((k, v)) => (k.to_string(), Some(v.to_string())),
                None => (rest.to_string(), None),
            };
            if !known.contains(&key.as_str()) {
                return Err(format!("unknown option --{key}"));
            }
            if let Some(value) = inline {
                flags.entry(key).or_default().push(value);
            } else if BOOL_FLAGS.contains(&key.as_str()) {
                flags.entry(key).or_default().push("true".into());
            } else if index + 1 < args.len() && !args[index + 1].starts_with("--") {
                flags.entry(key).or_default().push(args[index + 1].clone());
                index += 1;
            } else {
                return Err(format!("option --{key} requires a value"));
            }
        } else if arg.starts_with('-') && arg != "-" {
            return Err(format!("unknown option {arg}"));
        } else {
            positional.push(arg.clone());
        }
        index += 1;
    }

    Ok(ParsedArgs { positional, flags })
}

fn cmd_lock(args: &[String]) -> Result<i32, String> {
    let parsed = parse_args(args, &["output", "source", "json", "audit", "help"])?;
    if parsed.has("help") {
        print_help();
        return Ok(0);
    }
    let target = parsed.positional.first().map(String::as_str).unwrap_or(".");
    let root = PathBuf::from(target);
    if !root.exists() {
        return Err(format!("path not found: {}", root.display()));
    }
    let output = PathBuf::from(parsed.one("output").unwrap_or("skillguard.lock"));
    let source = parsed.one("source").unwrap_or(target);
    let findings = scan_for_guard(&root)?;
    let lock = guard::write_lock(&root, &output, source, &findings)?;
    if let Some(audit) = parsed.one("audit") {
        let entry = guard::audit_entry(
            "lock",
            &root.display().to_string(),
            guard::Decision::Allow,
            &findings,
            None,
            Some(&lock.fingerprint),
        );
        guard::append_audit(Path::new(audit), &entry)?;
    }
    if parsed.has("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&lock)
                .map_err(|error| format!("cannot serialize lockfile: {error}"))?
        );
    } else {
        println!("SkillGuard {VERSION}  |  lock");
        println!("skill: {}", lock.skill);
        println!("source: {}", lock.source);
        println!("files: {}", lock.files.len());
        println!("fingerprint: {}", lock.fingerprint);
        println!("findings: {} total", lock.findings.total);
        println!("lockfile: {}", output.display());
    }
    Ok(0)
}

fn cmd_policy(args: &[String]) -> Result<i32, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("validate");
    match subcommand {
        "validate" => {
            let Some(path) = args.get(1) else {
                return Err("usage: skillguard policy validate <file.json>".into());
            };
            let policy = guard::load_policy(Path::new(path))?;
            println!(
                "OK: profile={} fail_on={} max_medium={} deny={} exceptions={}",
                if policy.profile.is_empty() {
                    "default"
                } else {
                    policy.profile.as_str()
                },
                policy.fail_on.as_deref().unwrap_or("high"),
                policy
                    .max_medium
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                policy.deny.len(),
                policy.allow.len()
            );
            Ok(0)
        }
        other => Err(format!("unknown policy subcommand: {other}")),
    }
}

fn scan_for_guard(root: &Path) -> Result<Vec<model::Finding>, String> {
    let compiled = rules::compile_all(rules::load_builtin()?)?;
    let opts = ScanOptions {
        min_severity: Severity::Low,
        fail_on: Some(Severity::High),
        ignore_rules: Vec::new(),
        allow_domains: Vec::new(),
        max_file_size: 512 * 1024,
        exclude: Vec::new(),
    };
    Ok(engine::scan(root, &compiled, &opts)?.findings)
}

fn cmd_install(args: &[String]) -> Result<i32, String> {
    let known = [
        "dest",
        "target",
        "ref",
        "path",
        "rules",
        "allow-domain",
        "ignore",
        "yes",
        "force",
        "dry-run",
        "keep-quarantine",
        "json",
        "help",
    ];
    let parsed = parse_args(args, &known)?;
    if parsed.has("help") {
        print_help();
        return Ok(0);
    }
    let Some(source) = parsed.positional.first().cloned() else {
        return Err("usage: skillguard install <source> --dest <dir>|--target <agent>".into());
    };
    if parsed.positional.len() > 1 {
        return Err("install 只接受一个 source 参数".into());
    }
    let opts = install::InstallOptions {
        source,
        git_ref: parsed.one("ref").map(ToOwned::to_owned),
        subpath: parsed.one("path").map(ToOwned::to_owned),
        dest: parsed.one("dest").map(PathBuf::from),
        target: parsed.one("target").map(ToOwned::to_owned),
        rules_dir: parsed.one("rules").map(PathBuf::from),
        allow_domains: split_csv(&parsed.all("allow-domain")),
        ignore_rules: split_csv(&parsed.all("ignore")),
        yes: parsed.has("yes"),
        force: parsed.has("force"),
        dry_run: parsed.has("dry-run"),
        keep_quarantine: parsed.has("keep-quarantine"),
    };
    let outcome = install::install(opts)?;
    if parsed.has("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&outcome)
                .map_err(|error| format!("cannot serialize install result: {error}"))?
        );
    } else {
        println!(
            "SkillGuard {VERSION}  |  install\nsource: {}\nskill: {}",
            outcome.source, outcome.skill
        );
        if let Some(reference) = &outcome.resolved_ref {
            println!("resolved: {reference}");
        }
        println!(
            "scan: {} ({} critical, {} high, {} medium, {} low)",
            outcome.verdict,
            outcome.counts.critical,
            outcome.counts.high,
            outcome.counts.medium,
            outcome.counts.low
        );
        for finding in &outcome.findings {
            println!(
                "  [{}] {} {}:{}  {}",
                finding.severity.to_uppercase(),
                finding.rule_id,
                finding.file,
                finding.line,
                finding.title
            );
        }
        if outcome.dry_run {
            println!("dry-run: no files written");
        } else {
            if let Some(destination) = &outcome.destination {
                println!("destination: {}", destination.display());
            }
            println!("fingerprint: {}", outcome.fingerprint);
            println!("result: INSTALLED");
        }
    }
    Ok(0)
}

fn cmd_verify(args: &[String]) -> Result<i32, String> {
    let parsed = parse_args(args, &["lock", "json", "audit", "help"])?;
    if parsed.has("help") {
        print_help();
        return Ok(0);
    }
    let Some(path) = parsed.positional.first() else {
        return Err("usage: skillguard verify <path> [--lock skillguard.lock]".into());
    };
    let root = if Path::new(path).is_file() {
        Path::new(path)
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf()
    } else {
        PathBuf::from(path)
    };

    if let Some(lock) = parsed.one("lock") {
        let outcome = guard::verify_lock(&root, Path::new(lock))?;
        if let Some(audit) = parsed.one("audit") {
            let entry = guard::audit_entry(
                "verify-lock",
                &root.display().to_string(),
                if outcome.ok {
                    guard::Decision::Allow
                } else {
                    guard::Decision::Block
                },
                &[],
                None,
                Some(&outcome.actual),
            );
            guard::append_audit(Path::new(audit), &entry)?;
        }
        if parsed.has("json") {
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome)
                    .map_err(|error| format!("cannot serialize verify result: {error}"))?
            );
        } else {
            println!("SkillGuard {VERSION}  |  verify --lock");
            println!("skill: {}", outcome.skill);
            println!("path: {}", outcome.path.display());
            println!("expected: {}", outcome.expected);
            println!("actual:   {}", outcome.actual);
            if outcome.ok {
                println!("result: PASS (lockfile matches)");
            } else {
                println!("result: FAIL ({} drift item(s))", outcome.drift.len());
                for item in &outcome.drift {
                    println!("  {}  {}", item.kind, item.path);
                }
            }
        }
        return Ok(if outcome.ok { 0 } else { 1 });
    }

    let outcome = install::verify(&root)?;
    if let Some(audit) = parsed.one("audit") {
        let entry = guard::audit_entry(
            "verify",
            &root.display().to_string(),
            if outcome.ok {
                guard::Decision::Allow
            } else {
                guard::Decision::Block
            },
            &[],
            None,
            Some(&outcome.actual),
        );
        guard::append_audit(Path::new(audit), &entry)?;
    }
    if parsed.has("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&outcome)
                .map_err(|error| format!("cannot serialize verify result: {error}"))?
        );
    } else {
        println!("SkillGuard {VERSION}  |  verify");
        println!("skill: {}", outcome.skill);
        println!("path: {}", outcome.path.display());
        println!("expected: {}", outcome.expected);
        println!("actual:   {}", outcome.actual);
        if outcome.ok {
            println!("result: PASS (no drift)");
        } else {
            println!("result: FAIL ({} drift item(s))", outcome.drift.len());
            for item in &outcome.drift {
                println!("  {}  {}", item.kind, item.path);
            }
        }
    }
    Ok(if outcome.ok { 0 } else { 1 })
}

fn cmd_scan(args: &[String]) -> Result<i32, String> {
    let known = [
        "format",
        "fail-on",
        "min-severity",
        "ignore",
        "allow-domain",
        "rules",
        "config",
        "max-file-size",
        "no-color",
        "quiet",
        "json-pretty",
        "policy",
        "baseline",
        "write-baseline",
        "audit",
        "help",
    ];
    let parsed = parse_args(args, &known)?;
    if parsed.has("help") {
        print_help();
        return Ok(0);
    }

    let target = parsed.positional.first().map(String::as_str).unwrap_or(".");
    let root = PathBuf::from(target);
    if !root.exists() {
        return Err(format!("path not found: {}", root.display()));
    }

    let cfg = match parsed.one("config") {
        Some(path) => config::load(Path::new(path))?,
        None => match config::discover(&root) {
            Some(path) => config::load(&path)?,
            None => config::Config::default(),
        },
    };

    let format = parsed.one("format").unwrap_or("text").to_ascii_lowercase();
    if !matches!(format.as_str(), "text" | "json" | "sarif") {
        return Err(format!("invalid --format: {format}"));
    }

    let fail_on = match parsed.one("fail-on").or(cfg.fail_on.as_deref()) {
        Some("none") => None,
        Some(value) => {
            Some(Severity::parse(value).ok_or_else(|| format!("invalid --fail-on: {value}"))?)
        }
        None => Some(Severity::High),
    };

    let min_severity = match parsed.one("min-severity").or(cfg.min_severity.as_deref()) {
        Some(value) => {
            Severity::parse(value).ok_or_else(|| format!("invalid --min-severity: {value}"))?
        }
        None => Severity::Low,
    };

    let mut ignore_rules = cfg.ignore_rules.clone();
    ignore_rules.extend(split_csv(&parsed.all("ignore")));
    ignore_rules.sort();
    ignore_rules.dedup();

    let mut allow_domains = cfg.allow_domains.clone();
    allow_domains.extend(split_csv(&parsed.all("allow-domain")));
    allow_domains.sort();
    allow_domains.dedup();

    let max_kb = match parsed.one("max-file-size") {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| format!("invalid --max-file-size: {value}"))?,
        None => cfg.max_file_size_kb.unwrap_or(512),
    };

    let rules_dir = parsed
        .one("rules")
        .map(PathBuf::from)
        .or_else(|| cfg.rules_dir.as_ref().map(PathBuf::from));

    let builtin = rules::load_builtin()?;
    let external = match &rules_dir {
        Some(dir) => rules::load_dir(dir)?,
        None => Vec::new(),
    };
    let (merged, warnings) = rules::merge(builtin, external);
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
    if merged.is_empty() {
        return Err("no rules loaded".into());
    }
    let compiled = rules::compile_all(merged)?;

    let opts = ScanOptions {
        min_severity,
        fail_on,
        ignore_rules,
        allow_domains,
        max_file_size: max_kb.saturating_mul(1024),
        exclude: compile_excludes(&cfg.exclude)?,
    };

    let mut outcome = engine::scan(&root, &compiled, &opts)?;

    if let Some(path) = parsed.one("write-baseline") {
        guard::write_baseline(Path::new(path), &outcome.findings)?;
    }
    if let Some(path) = parsed.one("baseline") {
        let baseline = guard::load_baseline(Path::new(path))?;
        outcome.findings = guard::filter_baseline(outcome.findings, &baseline);
    }

    let policy_path = parsed.one("policy").map(PathBuf::from).or_else(|| {
        let local = root.join(".skillguard-policy.json");
        if local.is_file() {
            Some(local)
        } else {
            None
        }
    });
    let policy_result = match &policy_path {
        Some(path) => {
            let policy = guard::load_policy(path)?;
            Some(guard::evaluate(&outcome.findings, &policy))
        }
        None => None,
    };

    let threshold = opts.fail_on;
    let color = report::use_color(parsed.has("no-color"));
    let summary = report::summary(&outcome);
    let failed = report::failed(&summary, threshold)
        || policy_result
            .as_ref()
            .map(|result| result.decision == guard::Decision::Block)
            .unwrap_or(false);

    if let Some(result) = &policy_result {
        if !parsed.has("quiet") {
            eprintln!(
                "policy: {} (profile {}){}",
                result.decision.as_str(),
                result.profile,
                if result.reasons.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", result.reasons.join("; "))
                }
            );
        }
    }

    if let Some(audit) = parsed.one("audit") {
        let decision = policy_result
            .as_ref()
            .map(|result| result.decision)
            .unwrap_or(if failed {
                guard::Decision::Block
            } else {
                guard::Decision::Allow
            });
        let entry = guard::audit_entry(
            "scan",
            &root.display().to_string(),
            decision,
            &outcome.findings,
            policy_path
                .as_ref()
                .map(|path| path.display().to_string())
                .as_deref(),
            None,
        );
        guard::append_audit(Path::new(audit), &entry)?;
    }

    match format.as_str() {
        "json" => println!(
            "{}",
            report::to_json(&outcome, VERSION, parsed.has("json-pretty"))
        ),
        "sarif" => println!("{}", report::to_sarif(&outcome, VERSION, &compiled)),
        _ => {
            report::print_text(&outcome, VERSION, threshold, color, parsed.has("quiet"));
        }
    }

    Ok(if failed { 1 } else { 0 })
}

fn cmd_rules(args: &[String]) -> Result<i32, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("list");
    match subcommand {
        "list" => {
            let compiled = rules::compile_all(rules::load_builtin()?)?;
            println!("{:<14} {:<9} {:<18} TITLE", "ID", "SEVERITY", "CATEGORY");
            for cr in compiled {
                println!(
                    "{:<14} {:<9} {:<18} {}",
                    cr.rule.id,
                    cr.rule.severity.as_str(),
                    cr.rule.category,
                    cr.rule.title
                );
            }
            Ok(0)
        }
        "validate" => {
            let Some(path) = args.get(1) else {
                return Err("usage: skillguard rules validate <file.json|dir>".into());
            };
            let path = Path::new(path);
            let loaded = if path.is_dir() {
                rules::load_dir(path)?
            } else {
                rules::load_file(path)?
            };
            let compiled = rules::compile_all(loaded)?;
            println!("OK: {} rule(s) loaded and compiled", compiled.len());
            Ok(0)
        }
        other => Err(format!("unknown rules subcommand: {other}")),
    }
}

fn cmd_init(args: &[String]) -> Result<i32, String> {
    let parsed = parse_args(args, &["force", "help"])?;
    if parsed.has("help") {
        println!(
            "Usage: skillguard init [dir] [--force]\nWrites .skillguard.json and a GitHub Actions workflow."
        );
        return Ok(0);
    }
    let dir = parsed
        .positional
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let force = parsed.has("force");

    let config_path = dir.join(".skillguard.json");
    if config_path.exists() && !force {
        eprintln!("skip existing {}", config_path.display());
    } else {
        let body = r#"{
  "fail_on": "high",
  "min_severity": "low",
  "ignore_rules": [],
  "allow_domains": [],
  "max_file_size_kb": 512,
  "exclude": ["node_modules", ".git", "dist", "vendor"]
}
"#;
        fs::write(&config_path, body)
            .map_err(|e| format!("cannot write {}: {e}", config_path.display()))?;
        println!("wrote {}", config_path.display());
    }

    let workflow_dir = dir.join(".github").join("workflows");
    fs::create_dir_all(&workflow_dir)
        .map_err(|e| format!("cannot create {}: {e}", workflow_dir.display()))?;
    let workflow_path = workflow_dir.join("skillguard.yml");
    if workflow_path.exists() && !force {
        eprintln!("skip existing {}", workflow_path.display());
    } else {
        let body = r#"name: SkillGuard
on:
  push:
  pull_request:
jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - name: Scan agent skills
        run: npx --yes seramys-skillguard@0.1.0 scan . --format sarif --fail-on high > skillguard.sarif
      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: skillguard.sarif
"#;
        fs::write(&workflow_path, body)
            .map_err(|e| format!("cannot write {}: {e}", workflow_path.display()))?;
        println!("wrote {}", workflow_path.display());
    }

    println!("done. Run: skillguard scan .");
    Ok(0)
}

fn split_csv(values: &[String]) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| value.split(','))
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn compile_excludes(patterns: &[String]) -> Result<Vec<Regex>, String> {
    patterns
        .iter()
        .map(|pattern| {
            let mut regex = String::from(".*");
            for ch in pattern.chars() {
                match ch {
                    '*' => regex.push_str(".*"),
                    '?' => regex.push('.'),
                    _ => regex.push_str(&regex::escape(&ch.to_string())),
                }
            }
            regex.push_str(".*");
            Regex::new(&regex).map_err(|e| format!("invalid exclude pattern {pattern}: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags_and_positionals() {
        let args = vec![
            "skills".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--fail-on=high".to_string(),
            "--ignore".to_string(),
            "SG-A,SG-B".to_string(),
            "--quiet".to_string(),
        ];
        let parsed = parse_args(&args, &["format", "fail-on", "ignore", "quiet"]).unwrap();
        assert_eq!(parsed.positional, vec!["skills"]);
        assert_eq!(parsed.one("format"), Some("json"));
        assert_eq!(parsed.one("fail-on"), Some("high"));
        assert!(parsed.has("quiet"));
    }

    #[test]
    fn rejects_unknown_flag() {
        let args = vec!["--nope".to_string()];
        assert!(parse_args(&args, &["format"]).is_err());
    }

    #[test]
    fn splits_csv_values() {
        assert_eq!(
            split_csv(&["a, b".to_string(), "c".to_string()]),
            vec!["a", "b", "c"]
        );
    }
}
