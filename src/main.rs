mod config;
mod engine;
mod frontmatter;
mod model;
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
const BOOL_FLAGS: &[&str] = &["no-color", "quiet", "json-pretty", "force", "help"];

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
  --ignore SG-XX-001[,SG-YY-*]   Ignore rules by id or prefix
  --allow-domain <domain>        Allow network findings for a domain (repeatable/comma)
  --rules <dir>                  Load extra JSON rule packs (overrides by id)
  --config <file>                Config file (default: .skillguard.json)
  --max-file-size <kb>           Skip text scanning above this size (default: 512)
  --no-color                     Disable ANSI colors
  --quiet                        Hide header and summary
  --json-pretty                  Pretty-print JSON output

Examples:
  skillguard scan ./skills
  skillguard scan ./skills --format sarif --fail-on high > skillguard.sarif
  skillguard scan . --ignore SG-SC-001 --allow-domain github.com
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

    let outcome = engine::scan(&root, &compiled, &opts)?;
    let threshold = opts.fail_on;
    let color = report::use_color(parsed.has("no-color"));
    let summary = report::summary(&outcome);
    let failed = report::failed(&summary, threshold);

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
        run: npx --yes skillguard-cli@0.1.0 scan . --format sarif --fail-on high > skillguard.sarif
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
