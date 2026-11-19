# SkillGuard

AI Agent Skill 供应链安全扫描器。静态扫描 `SKILL.md`、脚本、配置和配套文件，检测提示注入、过度权限、密钥泄露、可疑外联、危险命令和供应链风险。

一句话：**在给 Agent 装 Skill 之前，先跑一遍 SkillGuard。**

- 单文件二进制，无运行时依赖
- Node / Python 两种生态启动器，共用同一套 Rust 扫描引擎
- text / JSON / SARIF 三种输出，可直接接入 CI 和 GitHub Code Scanning
- 只读扫描，不执行被扫描代码，不发起网络请求
- 规则包可扩展，支持项目级、文件级和行级抑制

---

## 快速开始

### 1. 单文件二进制

```bash
# Windows
dist\win32-x64\skillguard.exe scan .\skills

# Linux / macOS
./dist/linux-x64/skillguard scan ./skills
```

自行构建：

```bash
cargo build --release
bash scripts/build.sh          # 输出到 dist/<platform>-<arch>/
```

### 2. Node 生态

发布后：

```bash
npx skillguard-cli scan ./skills --fail-on high
```

当前仓库内验证：

```bash
node packages/npm/bin/skillguard.js scan ./skills --fail-on high
```

本地打包：

```bash
bash scripts/package-npm.sh
cd packages/npm && npm pack
```

### 3. Python 生态

发布后：

```bash
pip install skillguard-cli
skillguard scan ./skills --fail-on high
```

当前仓库内验证：

```bash
python -m venv .venv
.venv/Scripts/python -m pip install setuptools wheel
.venv/Scripts/python -m pip install -e packages/python --no-build-isolation
.venv/Scripts/skillguard scan ./skills
```

本地打 wheel：

```bash
python -m pip install build
bash scripts/package-python.sh
```

---

## 命令

```text
skillguard scan <path> [options]
skillguard rules list
skillguard rules validate <file.json|dir>
skillguard init [dir] [--force]
skillguard version
```

### scan 选项

| 选项 | 说明 | 默认 |
|---|---|---|
| `--format text\|json\|sarif` | 输出格式 | `text` |
| `--fail-on none\|low\|medium\|high\|critical` | 达到该级别时退出码为 1 | `high` |
| `--min-severity low\|medium\|high\|critical` | 隐藏更低级别的结果 | `low` |
| `--ignore SG-XX-001[,SG-YY-*]` | 忽略规则，支持前缀通配 | 空 |
| `--allow-domain <domain>` | 允许某个域名的网络告警，可重复/逗号分隔 | 空 |
| `--rules <dir>` | 加载额外 JSON 规则包，同 ID 覆盖内置 | 空 |
| `--config <file>` | 指定配置文件 | `.skillguard.json` |
| `--max-file-size <kb>` | 超过该大小的文本文件跳过内容扫描 | `512` |
| `--no-color` | 关闭 ANSI 颜色 | 自动 |
| `--quiet` | 隐藏头部和汇总 | 关闭 |
| `--json-pretty` | JSON 美化输出 | 关闭 |

### 退出码

| 码 | 含义 |
|---|---|
| `0` | 没有达到 `--fail-on` 的问题 |
| `1` | 存在达到阈值的问题 |
| `2` | 参数、配置、规则或 IO 错误 |

### 示例

```bash
# 扫描一个 Skill 集合
skillguard scan ./skills

# CI：输出 SARIF 并上传 GitHub Code Scanning
skillguard scan ./skills --format sarif --fail-on high > skillguard.sarif

# 允许 GitHub 域名，忽略依赖未锁定提示
skillguard scan . --allow-domain github.com --ignore SG-SC-001

# 校验自定义规则包
skillguard rules validate ./my-rules/
```

---

## 它扫描什么

一个 Skill 通常是一个目录：

```text
my-skill/
├── SKILL.md          # frontmatter + 指令
├── scripts/          # sh / py / js / ps1 ...
├── config/           # json / yaml / env ...
└── assets/
```

SkillGuard 会：

1. 递归发现包含 `SKILL.md` 的目录（也支持扫描单个文件或 Skill 集合）
2. 解析 `SKILL.md` 的 frontmatter（name、description、version、allowed-tools 等）
3. 对每个文本文件按扩展名分类，执行对应规则
4. 对二进制文件只做路径规则，不读取内容
5. 在同一 Skill 内运行组合规则（例如“读敏感文件 + 外联”）
6. 应用忽略规则、文件级抑制和行级抑制
7. 按严重度、Skill、文件、行号排序输出

---

## 内置规则（29 条）

| ID | 严重度 | 类别 | 检测内容 |
|---|---|---|---|
| SG-PI-001 | high | 提示注入 | 忽略/绕过既有系统指令 |
| SG-PI-002 | high | 提示注入 | 套取系统提示词或内部指令 |
| SG-PI-003 | high | 提示注入 | 要求对用户/管理员隐瞒行为 |
| SG-PI-004 | medium | 提示注入 | 角色劫持、越狱模式 |
| SG-PI-005 | medium | 提示注入 | 绕过人工确认自动执行 |
| SG-PI-006 | high | 混淆 | 零宽字符、双向控制符、Unicode 标签 |
| SG-PI-007 | medium | 混淆 | 大段 Base64（解码含命令时升级为 critical） |
| SG-PERM-001 | medium | 权限 | allowed-tools 声明 shell/终端/进程执行 |
| SG-PERM-002 | high | 权限 | SSH、云凭证、Cookie、钱包等敏感路径 |
| SG-PERM-003 | medium | 权限 | 允许任意域名/全网访问 |
| SG-SEC-001 | critical | 密钥 | 私钥块 |
| SG-SEC-002 | high | 密钥 | 常见云厂商/代码托管平台密钥前缀 |
| SG-SEC-003 | medium | 密钥 | 高熵密钥赋值 |
| SG-SEC-004 | high | 密钥 | `.env`、凭证 JSON、pem/p12 证书文件 |
| SG-NET-001 | high | 网络 | Discord/Telegram/Slack Webhook、临时文件、隧道 |
| SG-NET-002 | medium | 网络 | 裸 IP 外联（私有/回环地址自动忽略） |
| SG-NET-003 | high | 网络 | `curl ... \| bash` 远程脚本直接执行 |
| SG-NET-004 | critical | 网络 | 下载后交解释器、PowerShell 编码命令、base64 解码执行 |
| SG-CMD-001 | high | 命令 | `rm -rf /`、全盘 chmod 777、mkfs、dd、fork bomb |
| SG-CMD-002 | critical | 命令 | nc/ncat/socat、`/dev/tcp` 反弹 Shell |
| SG-CMD-003 | high | 命令 | crontab、LaunchAgent、计划任务、注册表 Run |
| SG-SC-001 | low | 供应链 | pip/npm/pnpm/yarn 安装未锁定版本 |
| SG-SC-002 | medium | 供应链 | npm preinstall/postinstall 生命周期脚本 |
| SG-SC-003 | high | 供应链 | 下载后 `chmod +x` 立即执行 |
| SG-HYG-001 | low | 卫生 | frontmatter 缺少 name/description/version |
| SG-HYG-002 | high | 卫生 | 包含 exe/dll/so/msi 等二进制/安装包 |
| SG-HYG-003 | low | 卫生 | 包含 bat/cmd/ps1/vbs/hta/reg 脚本 |
| SG-COMBO-001 | critical | 组合 | 敏感信息访问 + 外部网络调用（数据外传） |
| SG-COMBO-002 | critical | 组合 | 提示注入 + 反弹 Shell/下载执行（远程控制） |

查看当前规则：

```bash
skillguard rules list
```

---

## 配置

在扫描根目录放置 `.skillguard.json`，或使用 `--config`：

```json
{
  "fail_on": "high",
  "min_severity": "low",
  "ignore_rules": [],
  "allow_domains": ["github.com", "pypi.org"],
  "max_file_size_kb": 512,
  "exclude": ["node_modules", ".git", "dist", "vendor", "*.min.js"],
  "rules_dir": "./extra-rules"
}
```

`exclude` 支持 `*` 和 `?` 通配。`rules_dir` 中的 JSON 规则包会与内置规则合并，同 ID 时外部规则覆盖内置规则。

生成默认配置和 GitHub Actions 工作流：

```bash
skillguard init
skillguard init ./my-skill --force
```

---

## 抑制误报

### 1. 项目级忽略

`.skillguardignore`：

```text
# 忽略单条规则
SG-SC-001

# 忽略一组规则
SG-NET-*

# 忽略全部
*
```

也可以用 `--ignore` 或配置里的 `ignore_rules`。

### 2. 文件级抑制

```markdown
<!-- skillguard-disable-file SG-HYG-003 -->
```

或忽略整个文件：

```markdown
<!-- skillguard-disable-file -->
```

### 3. 行级抑制

```bash
# skillguard-disable-next-line SG-CMD-001
rm -rf /tmp/old-build
```

```bash
curl https://example.com/install.sh | bash  # skillguard-disable-line SG-NET-003
```

只抑制指定规则；写 `*` 可抑制该行全部规则。

---

## 输出格式

### text

人类可读，带颜色（管道或 `--no-color` 时自动关闭）。

### JSON

```json
{
  "tool": "skillguard",
  "version": "0.1.0",
  "root": "/path/to/skills",
  "summary": {
    "skills": 1,
    "files": 2,
    "critical": 1,
    "high": 2,
    "medium": 1,
    "low": 0,
    "total": 4
  },
  "findings": [
    {
      "rule_id": "SG-COMBO-001",
      "title": "组合风险：读取敏感信息并外联",
      "severity": "critical",
      "category": "exfiltration",
      "skill": "sync-helper",
      "file": "SKILL.md",
      "line": 1,
      "column": 1,
      "excerpt": "signals: SG-PERM-002, SG-NET-001",
      "message": "combined signals: SG-PERM-002, SG-NET-001",
      "remediation": "...",
      "references": []
    }
  ]
}
```

### SARIF 2.1.0

```bash
skillguard scan ./skills --format sarif > skillguard.sarif
```

可直接上传 GitHub Code Scanning：

```yaml
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: skillguard.sarif
```

---

## CI 集成

### GitHub Actions（使用本仓库 Action）

```yaml
name: SkillGuard
on: [push, pull_request]
jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: Seramys/skillguard@main
        with:
          path: "."
          fail-on: high
          format: sarif
```

`action.yml` 当前通过 npm 包运行。发布到 npm 后即可使用；未发布时请用下面的本地构建方式。

### 本地构建 + CI

```yaml
- uses: dtolnay/rust-toolchain@stable
- run: cargo build --release
- run: ./target/release/skillguard scan ./skills --fail-on high --format sarif > skillguard.sarif
- uses: github/codeql-action/upload-sarif@v3
  if: always()
  with:
    sarif_file: skillguard.sarif
```

### 通用 CI（任意平台）

```bash
skillguard scan ./skills --fail-on high
```

退出码非 0 即失败。

---

## 自定义规则

规则包是 JSON：

```json
{
  "version": "1",
  "rules": [
    {
      "id": "ACME-001",
      "title": "禁止访问内部财务系统",
      "severity": "high",
      "category": "policy",
      "description": "Skill 不得连接财务内网地址。",
      "remediation": "改为走审批后的数据接口。",
      "applies_to": ["text"],
      "max_matches": 3,
      "match": {
        "type": "regex",
        "pattern": "(?i)https?://finance\\.internal\\.acme\\.com"
      }
    }
  ]
}
```

支持的匹配类型：

| type | 说明 |
|---|---|
| `regex` | 按行匹配正则；可选 `absent` 排除模式 |
| `path_regex` | 匹配相对路径 |
| `secret_entropy` | 高熵密钥赋值，可配置最小长度、熵阈值、前缀 |
| `hidden_unicode` | 隐藏/双向控制字符 |
| `base64_blob` | 大段 Base64，解码含命令时升级严重度 |
| `frontmatter_missing` | frontmatter 必填字段 |
| `combo` | 同 Skill 内多个规则信号组合 |

`applies_to` 可取：`skill_md`、`markdown`、`script`、`config`、`text`、`path`、`all`。

校验：

```bash
skillguard rules validate ./extra-rules
```

---

## 架构

```text
rules/core.json
      │
      ▼
规则编译器（regex 预编译 / 校验）
      │
      ▼
扫描引擎
  ├── Skill 发现（SKILL.md 目录）
  ├── frontmatter 解析
  ├── 文件分类（markdown/script/config/text/binary）
  ├── 文本规则 / 路径规则 / 熵检测 / Base64 / 隐藏字符
  ├── 组合规则（跨文件信号）
  └── 抑制（ignore / 文件级 / 行级）
      │
      ▼
text / JSON / SARIF
```

源码结构：

```text
src/
├── main.rs          CLI 参数、配置合并、子命令
├── engine.rs        Skill 发现、扫描、抑制、组合规则
├── rules.rs         规则模型、加载、合并、编译
├── frontmatter.rs   轻量 YAML frontmatter 解析
├── report.rs        text / JSON / SARIF
├── model.rs         Severity / Finding / Summary / Skill
└── config.rs        .skillguard.json
rules/core.json      内置规则包
packages/npm         npm 启动器
packages/python      Python 启动器
scripts/             构建与打包
```

### 安全设计

- **不执行**被扫描的代码，不解析动态行为
- **不联网**，不上传任何内容
- Rust `regex` 为线性时间，避免 ReDoS
- 限制目录深度（32）、Skill 数（1000）、文件数（5000）、单文件大小（默认 512KB）
- 密钥在 JSON/text 输出中自动脱敏（保留首尾各 4 字符）
- 跳过 `.git`、`node_modules`、`target`、`dist`、`.venv`、`vendor` 等目录
- 不跟随符号链接

---

## 构建与测试

```bash
cargo test                 # 19 单元 + 15 端到端
cargo build --release
bash scripts/build.sh      # 当前平台单文件
bash scripts/build-all.sh  # 交叉编译矩阵（缺工具链的目标会跳过）
```

测试夹具：

```text
tests/fixtures/
├── clean-skill/          无问题
├── prompt-injection/     提示注入
├── exfil-skill/          密钥读取 + Webhook 外传
├── dangerous-skill/      远程脚本执行
├── secret-skill/         密钥泄露与输出脱敏
├── suppressed-skill/     行级/文件级抑制
├── config-skill/         配置排除路径
└── collection/           多 Skill 发现
```

---

## 发布检查清单

### npm

1. 修改 `packages/npm/package.json` 中的包名、仓库地址和版本
2. `bash scripts/package-npm.sh`
3. `cd packages/npm && npm pack` 检查内容包含 `vendor/<platform>/skillguard[.exe]`
4. `npm publish --access public`

多平台发布建议：为每个 `os/arch` 生成一个平台包（`@skillguard/cli-win32-x64` 等），主包通过 `optionalDependencies` 引用；`lib/resolve.js` 已预留平台包解析逻辑。

### Python

1. 修改 `packages/python/pyproject.toml` 中的包名和仓库地址
2. `python -m pip install build`
3. `bash scripts/package-python.sh`
4. `twine upload packages/python/dist/*`

### 单文件二进制

1. `bash scripts/build-all.sh`
2. 将 `dist/<platform>-<arch>/skillguard[.exe]` 附加到 GitHub Release
3. 在 Release 说明中给出 SHA256

### 占位符

仓库地址已指向 `Seramys/skillguard`；`action.yml` 和 `init` 生成的工作流默认使用 npm 版本 `0.1.0`，发布新版本后同步更新。

---

## 已知限制

- 当前是**静态规则扫描**，不做 LLM 语义判定；复杂、改写过的提示注入可能漏报
- 不能检测只在运行时出现的行为；运行时防护属于下一阶段
- frontmatter 解析是轻量实现，复杂的嵌套 YAML 可能无法完整还原
- 规则是模式匹配，存在误报；请用 `--ignore`、`.skillguardignore` 或 `allow_domains` 收敛
- 不扫描超过 `--max-file-size` 的文本内容，也不读取二进制内容
- 扫描根目录若没有 `SKILL.md`，会把整个目录当作一个 Skill 处理；大型代码仓库建议显式指定 `./skills` 或使用 `exclude`

## 路线图

- **0.2**：规则基线/差异扫描、更多供应链规则、行业规则包、规则测试框架
- **0.3**：LLM 辅助的语义级提示注入分析（可选、离线）
- **1.0**：运行时 Agent 网关、工具调用审批、集中策略与审计控制台

## License

MIT