use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    pub fields: BTreeMap<String, String>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

/// Parse a leading YAML-ish frontmatter block without a YAML dependency.
/// Supports `key: value`, quoted scalars, inline arrays, `- item` lists,
/// simple nested keys and `|` / `>` block scalars. Unknown syntax is kept
/// as a best-effort scalar rather than failing the scan.
pub fn parse(content: &str) -> Frontmatter {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    if let Some((raw, _body)) = split(&normalized) {
        Frontmatter {
            fields: parse_fields(raw),
        }
    } else {
        Frontmatter::default()
    }
}

fn split(n: &str) -> Option<(&str, &str)> {
    let mut offset = 0usize;
    let mut it = n.split_inclusive('\n');
    let first = it.next()?;
    if !is_fence(first) {
        return None;
    }
    offset += first.len();
    for line in it {
        offset += line.len();
        if is_fence(line) {
            let raw_start = first.len();
            let raw_end = offset - line.len();
            return Some((&n[raw_start..raw_end], &n[offset..]));
        }
    }
    None
}

fn is_fence(line: &str) -> bool {
    line.trim_end_matches(['\n', '\r', ' ', '\t']).trim() == "---"
}

fn parse_fields(raw: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    let mut current_top: Option<String> = None;
    let mut block: Option<(String, bool, Vec<String>)> = None;

    for line in raw.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            if let Some((_, _, buf)) = block.as_mut() {
                buf.push(String::new());
            }
            continue;
        }
        let indent = trimmed.len() - trimmed.trim_start().len();

        let mut flush: Option<(String, bool, Vec<String>)> = None;
        if let Some((key, folded, buf)) = block.as_mut() {
            if indent >= 2 {
                buf.push(trimmed.trim_start().to_string());
                continue;
            }
            flush = Some((key.clone(), *folded, std::mem::take(buf)));
        }
        if let Some((key, folded, buf)) = flush {
            fields.insert(key, finish_block(folded, &buf));
            block = None;
        }

        let t = trimmed.trim_start();
        if let Some(item) = t.strip_prefix("- ") {
            if let Some(key) = &current_top {
                let clean = clean_scalar(item);
                let entry = fields.entry(key.clone()).or_default();
                if !entry.is_empty() {
                    entry.push_str(", ");
                }
                entry.push_str(&clean);
            }
            continue;
        }

        if let Some((k, v)) = t.split_once(':') {
            let key = k.trim();
            if key.is_empty() {
                continue;
            }
            let full_key = if indent >= 2 {
                match &current_top {
                    Some(parent) => format!("{parent}.{key}"),
                    None => key.to_string(),
                }
            } else {
                current_top = Some(key.to_string());
                key.to_string()
            };
            let value = v.trim();
            if value == "|" || value == ">" {
                block = Some((full_key, value == ">", Vec::new()));
            } else {
                fields.insert(full_key, clean_scalar(value));
            }
        }
    }

    if let Some((key, folded, buf)) = block {
        fields.insert(key, finish_block(folded, &buf));
    }
    fields
}

fn finish_block(folded: bool, lines: &[String]) -> String {
    if folded {
        lines.join(" ").trim().to_string()
    } else {
        lines.join("\n").trim().to_string()
    }
}

fn clean_scalar(v: &str) -> String {
    let v = v.trim();
    if v.starts_with('[') && v.ends_with(']') && v.len() >= 2 {
        return v[1..v.len() - 1]
            .split(',')
            .map(|x| unquote(x.trim()))
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
    }
    unquote(v).to_string()
}

fn unquote(v: &str) -> &str {
    let bytes = v.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &v[1..v.len() - 1];
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_frontmatter() {
        let fm = parse("---\nname: weather-helper\ndescription: \"Looks up weather\"\nversion: 1.0.0\n---\nbody\n");
        assert_eq!(fm.get("name"), Some("weather-helper"));
        assert_eq!(fm.get("description"), Some("Looks up weather"));
        assert_eq!(fm.get("version"), Some("1.0.0"));
    }

    #[test]
    fn parses_lists_and_inline_arrays() {
        let fm = parse("---\nallowed-tools:\n  - bash\n  - read_file\ntags: [a, b, c]\n---\n"); // skillguard-disable-line SG-PERM-001
        assert_eq!(fm.get("allowed-tools"), Some("bash, read_file")); // skillguard-disable-line SG-PERM-001
        assert_eq!(fm.get("tags"), Some("a, b, c"));
    }

    #[test]
    fn parses_block_scalars() {
        let fm = parse("---\ndescription: |\n  line one\n  line two\n---\n");
        assert_eq!(fm.get("description"), Some("line one\nline two"));
    }

    #[test]
    fn no_frontmatter() {
        let fm = parse("# just markdown\n");
        assert!(fm.fields.is_empty());
    }
}
