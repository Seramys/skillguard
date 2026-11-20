use std::collections::BTreeSet;

#[derive(Debug, Clone)]
enum Value {
    None,
    Bool,
    Int,
    Float,
    Str(String),
    Bytes(Vec<u8>),
    Tuple(Vec<Value>),
    Code(Box<Code>),
}

#[derive(Debug, Clone)]
struct Code {
    names: Vec<String>,
    consts: Vec<Value>,
}

#[derive(Debug)]
pub struct BytecodeReport {
    pub names: BTreeSet<String>,

    pub dangers: Vec<String>,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    interned: Vec<Value>,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            interned: Vec::new(),
        }
    }

    fn u8(&mut self) -> Option<u8> {
        let value = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(value)
    }

    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(length)?;
        let slice = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn i32(&mut self) -> Option<i32> {
        let bytes = self.take(4)?;
        Some(i32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        let bytes = self.take(4)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn value(&mut self) -> Option<Value> {
        let raw = self.u8()?;
        let flagged = raw & 0x80 != 0;
        let tag = raw & 0x7f;
        if tag == b'c' {
            let before = self.interned.len();
            let reserve = if flagged {
                self.interned.push(Value::None);
                Some(before)
            } else {
                None
            };
            let start = self.pos;
            let mut parsed = self.code(true);
            if parsed.is_none() {
                self.pos = start;
                self.interned.truncate(before);
                if flagged {
                    self.interned.push(Value::None);
                }
                parsed = self.code(false);
            }
            let parsed = parsed?;
            let value = Value::Code(Box::new(parsed));
            if let Some(index) = reserve {
                self.interned[index] = value.clone();
            }
            return Some(value);
        }
        let value = match tag {
            b'0' => Value::None,
            b'N' => Value::None,
            b'T' => Value::Bool,
            b'F' => Value::Bool,
            b'i' => {
                self.i32()?;
                Value::Int
            }
            b'I' => {
                let _ = self.take(8)?;
                Value::Int
            }
            b'l' => {
                let count = self.i32()?;
                if !(0..=100_000).contains(&count) {
                    return None;
                }
                for _ in 0..count {
                    let _ = self.take(2)?;
                }
                Value::Int
            }
            b'f' => {
                let _ = self.take(4)?;
                Value::Float
            }
            b'g' => {
                let _ = self.take(8)?;
                Value::Float
            }
            b's' | b't' => {
                let length = self.u32()? as usize;
                Value::Bytes(self.take(length)?.to_vec())
            }
            b'u' | b'a' | b'A' => {
                let length = self.u32()? as usize;
                Value::Str(String::from_utf8_lossy(self.take(length)?).to_string())
            }
            b'z' | b'Z' => {
                let length = self.u8()? as usize;
                Value::Str(String::from_utf8_lossy(self.take(length)?).to_string())
            }
            b'r' => {
                let index = self.i32()?;
                if index < 0 {
                    return None;
                }
                self.interned.get(index as usize)?.clone()
            }
            b'(' => {
                let count = self.i32()?;
                if !(0..=100_000).contains(&count) {
                    return None;
                }
                let mut items = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    items.push(self.value()?);
                }
                Value::Tuple(items)
            }
            b')' => {
                let count = self.u8()? as usize;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(self.value()?);
                }
                Value::Tuple(items)
            }
            b'[' => Value::Tuple(self.collection()?),
            b'{' => Value::Tuple(self.dict_values()?),
            b'<' => Value::Tuple(self.collection()?),
            b'>' => Value::Tuple(self.collection()?),
            b'c' => {
                let start = self.pos;
                let interned_len = self.interned.len();
                let parsed = self.code(true).or_else(|| {
                    self.pos = start;
                    self.interned.truncate(interned_len);
                    self.code(false)
                })?;
                Value::Code(Box::new(parsed))
            }
            _ => return None,
        };
        if flagged {
            self.interned.push(value.clone());
        }
        Some(value)
    }

    fn collection(&mut self) -> Option<Vec<Value>> {
        let mut items = Vec::new();
        loop {
            let next = *self.data.get(self.pos)?;
            if next == b'0' {
                self.pos += 1;
                break;
            }
            items.push(self.value()?);
            if items.len() > 100_000 {
                return None;
            }
        }
        Some(items)
    }

    fn dict_values(&mut self) -> Option<Vec<Value>> {
        let mut items = Vec::new();
        loop {
            let next = *self.data.get(self.pos)?;
            if next == b'0' {
                self.pos += 1;
                break;
            }
            let _key = self.value()?;
            let value = self.value()?;
            items.push(value);
            if items.len() > 100_000 {
                return None;
            }
        }
        Some(items)
    }

    fn code(&mut self, modern: bool) -> Option<Code> {
        let _argcount = self.i32()?;
        let _posonly = self.i32()?;
        let _kwonly = self.i32()?;
        if modern {
            let _stacksize = self.i32()?;
            let _flags = self.i32()?;
        } else {
            let _nlocals = self.i32()?;
            let _stacksize = self.i32()?;
            let _flags = self.i32()?;
        }
        let _bytecode = self.value()?;
        let consts = match self.value()? {
            Value::Tuple(items) => items,
            _ => Vec::new(),
        };
        let names = match self.value()? {
            Value::Tuple(items) => items.iter().filter_map(as_string).collect(),
            _ => Vec::new(),
        };
        if modern {
            let _localsplusnames = self.value()?;
            let _localspluskinds = self.value()?;
        } else {
            let _varnames = self.value()?;
            let _freevars = self.value()?;
            let _cellvars = self.value()?;
        }
        let _filename = self.value()?;
        let _name = self.value()?;
        if modern {
            let _qualname = self.value()?;
        }
        let _firstlineno = self.i32()?;
        let _linetable = self.value()?;
        if modern {
            let _exceptiontable = self.value()?;
        }
        Some(Code { names, consts })
    }
}

fn as_string(value: &Value) -> Option<String> {
    match value {
        Value::Str(text) => Some(text.clone()),
        Value::Bytes(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
        _ => None,
    }
}

impl Code {
    fn walk(&self, names: &mut BTreeSet<String>, strings: &mut BTreeSet<String>) {
        names.extend(self.names.iter().cloned());
        for value in &self.consts {
            walk_value(value, names, strings);
        }
    }
}

fn walk_value(value: &Value, names: &mut BTreeSet<String>, strings: &mut BTreeSet<String>) {
    match value {
        Value::Str(text) => {
            strings.insert(text.clone());
        }
        Value::Bytes(bytes) => {
            if bytes.len() <= 512 {
                let text = String::from_utf8_lossy(bytes).to_string();
                if text.chars().any(|ch| ch.is_ascii_graphic()) {
                    strings.insert(text);
                }
            }
        }
        Value::Tuple(items) => {
            for item in items {
                walk_value(item, names, strings);
            }
        }
        Value::Code(code) => code.walk(names, strings),
        _ => {}
    }
}

pub fn inspect(bytes: &[u8]) -> Option<BytecodeReport> {
    if bytes.len() < 16 {
        return None;
    }
    let mut reader = Reader::new(&bytes[16..]);
    let value = reader.value()?;
    let mut names = BTreeSet::new();
    let mut strings = BTreeSet::new();
    walk_value(&value, &mut names, &mut strings);
    let lower_names: BTreeSet<String> =
        names.iter().map(|name| name.to_ascii_lowercase()).collect();
    let mut dangers = Vec::new();
    if lower_names.contains("environ")
        && (lower_names.contains("urlopen")
            || lower_names.contains("urlretrieve")
            || lower_names.contains("socket")
            || lower_names.contains("subprocess")
            || (lower_names.contains("items") && lower_names.contains("print")))
    {
        dangers.push("读取环境变量并输出/外联".to_string());
    }
    if (lower_names.contains("exec") || lower_names.contains("eval"))
        && (lower_names.contains("b64decode")
            || lower_names.contains("decodebytes")
            || lower_names.contains("fromhex")
            || lower_names.contains("marshal"))
    {
        dangers.push("解码后执行动态代码".to_string());
    }
    if lower_names.contains("subprocess") || lower_names.contains("system") {
        dangers.push("字节码中包含进程执行调用".to_string());
    }
    if strings.iter().any(|text| {
        let lower = text.to_ascii_lowercase();
        lower.contains("http://") || lower.contains("https://") || lower.contains("webhook")
    }) && (lower_names.contains("urlopen") || lower_names.contains("urlretrieve"))
    {
        dangers.push("字节码中包含网络地址与请求调用".to_string());
    }
    Some(BytecodeReport { names, dangers })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_generated_pyc() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("bench/samples/zh/zh-pyc-mismatch/__pycache__/formatter.cpython-313.pyc");
        if !path.is_file() {
            return;
        }
        let bytes = std::fs::read(path).expect("read pyc");
        let report = inspect(&bytes).expect("parse pyc");
        assert!(report.names.contains("format_text"));
    }
}
