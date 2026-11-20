<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/brand/logo-dark.svg">
  <img src="docs/assets/brand/logo.svg" width="150" alt="SkillGuard logo">
</picture>

# SkillGuard

**AI Agent Skill 供应链安全扫描器**

在给 Agent 装 Skill 之前，先跑一遍 SkillGuard。

[![License: MIT](https://img.shields.io/badge/license-MIT-yellow?style=flat-square)](LICENSE) [![Rust stable](https://img.shields.io/badge/Rust-stable-orange?logo=rust&style=flat-square)](https://www.rust-lang.org/) [![Node 18+](https://img.shields.io/badge/Node-18%2B-339933?logo=node.js&logoColor=white&style=flat-square)](packages/npm) [![Python 3.9+](https://img.shields.io/badge/Python-3.9%2B-3776AB?logo=python&logoColor=white&style=flat-square)](packages/python) [![Tests](https://img.shields.io/badge/tests-61%20passing-17A673?style=flat-square)](#验证) [![Rules](https://img.shields.io/badge/rules-51-5B6CFF?style=flat-square)](rules/core.json) [![SARIF 2.1.0](https://img.shields.io/badge/SARIF-2.1.0-8B7CF6?style=flat-square)](#输出格式) [![CI](https://github.com/Seramys/skillguard/actions/workflows/ci.yml/badge.svg)](https://github.com/Seramys/skillguard/actions/workflows/ci.yml)

[简体中文](README.md) · [English](README_EN.md) · [日本語](README_JA.md) · [한국어](README_KO.md)

[30 秒开始](#30-秒开始) · [它扫描什么](#它扫描什么) · [内置规则](#内置规则) · [架构](#架构) · [验证](#验证)

</div>

<p align="center">
  <img src="docs/assets/screenshots/scan-exfil.png" width="920" alt="SkillGuard 扫描结果：critical 组合风险">
</p>
<p align="center"><sub>真实输出：一个读取 <code>~/.ssh/id_rsa</code> 并外传的 Skill，被组合规则判为 <b>critical</b>。</sub></p>

---

## 它解决什么问题

一个 Agent Skill 通常是一个目录：

```text
my-skill/
├── SKILL.md          # frontmatter + 给 Agent 的指令
├── scripts/          # sh / py / js / ps1 ...
├── config/           # json / yaml / env ...
└── assets/
```

安装一个 Skill，往往意味着同时把**指令、工具权限、脚本和网络访问**交给 Agent。恶意或粗心的 Skill 可以：

- 在 `SKILL.md` 里写“忽略之前的系统指令”，或者要求 Agent 不要告诉用户；
- 读取 `~/.ssh/id_rsa`、云凭证、Cookie、钱包文件；
- 通过 Discord/Telegram/Slack Webhook、裸 IP 或隧道把数据发出去；
- 用 `curl ... | bash`、`chmod +x` 后执行、PowerShell 编码命令等方式远程执行；
- 在安装脚本里加入 `preinstall` / `postinstall`，或引入未锁定版本的依赖；
- 用零宽字符、双向控制符或大段 Base64 隐藏真实意图。

SkillGuard 在**安装之前、进入 CI 之前**做一次静态安检：只读文件，不执行代码，不联网，把可疑指令、权限、密钥、外联和组合风险变成可定位、可抑制、可接入 Code Scanning 的结果。

它不是运行时沙箱，也不是杀毒软件。它是 Skill 供应链的第一道静态闸门。

---

## 30 秒开始

需要 Rust stable。克隆仓库后：

```bash
cargo build --release
./target/release/skillguard scan ./tests/fixtures/exfil-skill --no-color
```

Windows PowerShell：

```powershell
cargo build --release
.\target\release\skillguard.exe scan .\tests\fixtures\exfil-skill --no-color
```

你会看到一个 `critical` 组合风险、两个 `high` 和一个 `medium`，进程以退出码 `1` 结束：

```text
[CRITICAL] SG-COMBO-001  组合风险：读取敏感信息并外联
[HIGH]     SG-NET-001    可疑外联：Webhook/隧道
[HIGH]     SG-PERM-002   敏感路径访问
[MEDIUM]   SG-PERM-001   权限过大：声明 Shell 执行能力
Result: FAIL
```

不想看完整输出，只要 CI 结果：

```bash
./target/release/skillguard scan ./skills --fail-on high
echo $?   # 0 = 通过，1 = 达到阈值，2 = 参数/配置/规则/IO 错误
```

生成 SARIF 并交给 GitHub Code Scanning：

```bash
./target/release/skillguard scan ./skills --format sarif --fail-on high > skillguard.sarif
```

---

## P0 准入闭环

> P0 的核心不是“替用户安装”，而是回答“这个 Skill 能不能进，证据是什么”。

```bash
# 生成不可变清单：内容哈希 + 来源 + 扫描结论
skillguard lock ./my-skill --output skillguard.lock

# 离线校验漂移/缺失/篡改
skillguard verify ./my-skill --lock skillguard.lock

# 策略门禁：allow/warn/block、例外负责人与到期时间
skillguard scan ./my-skill --policy .skillguard-policy.json --audit .skillguard-audit.jsonl

# 已知风险基线：只关心新增问题
skillguard scan ./my-skill --write-baseline baseline.json
skillguard scan ./my-skill --baseline baseline.json
```

`.skillguard-policy.json` 示例：

```json
{
  "profile": "team",
  "fail_on": "high",
  "max_medium": 10,
  "deny": ["SG-COMBO-001"],
  "allow": [
    {"rule": "SG-PERM-001", "owner": "alice", "reason": "approved", "expires": "2026-12-31"}
  ]
}
```

`--audit` 追加 JSONL 审计记录，包含动作、主体、决策、计数、策略文件和工具版本。

---

## 安装

> **定位说明**：`install` 是 P1 实验轨道，不属于 P0 核心。P0 默认使用 `lock` / `verify` / `policy` / `scan`；`install` 保留用于受控环境，默认不宣传、不替用户改变环境。

> **当前状态（0.1.1）**：npm / PyPI 的 0.1.1 已发布，包含 **win32-x64、linux-x64、darwin-x64、darwin-arm64**；GitHub Release v0.1.1 提供 4 个平台二进制和 SHA256SUMS。单文件二进制、Node 启动器和 Python 启动器均已验证。

### 方式一：单文件二进制（推荐）

单文件、无运行时依赖，适合本机扫描和 CI。

**从 Release 下载**（v0.1.1）：下载对应平台的 `skillguard` / `skillguard.exe`，放到 `PATH` 或项目目录。

**从源码构建**：

```bash
git clone https://github.com/Seramys/skillguard.git
cd skillguard
cargo build --release

# POSIX
./target/release/skillguard version
./target/release/skillguard scan ./tests/fixtures/clean-skill
```

```powershell
# Windows
.\target\release\skillguard.exe version
.\target\release\skillguard.exe scan .\tests\fixtures\clean-skill
```

`scripts/build.sh` 会把当前平台的产物复制到 `dist/<platform>-<arch>/`：

```bash
bash scripts/build.sh
# dist/win32-x64/skillguard.exe
# dist/linux-x64/skillguard
# dist/macos-arm64/skillguard
```

<p align="center">
  <img src="docs/assets/screenshots/scan-clean.png" width="880" alt="扫描干净的 Skill：PASS">
</p>
<p align="center"><sub>扫描一个干净的 Skill：0 findings，退出码 0。</sub></p>

### 方式二：Node 生态

已发布（多平台），一行命令：

```bash
npx --yes seramys-skillguard@0.1.1 scan ./skills --fail-on high
```

全局安装：

```bash
npm install -g seramys-skillguard
skillguard scan ./skills --fail-on high
```

**仓库内验证**（不依赖 npm 发布）：

```bash
node packages/npm/bin/skillguard.js version
node packages/npm/bin/skillguard.js scan ./tests/fixtures/clean-skill --no-color
```

<p align="center">
  <img src="docs/assets/screenshots/node-launcher.png" width="880" alt="Node 启动器运行 SkillGuard">
</p>
<p align="center"><sub>Node 启动器只是薄壳，扫描仍由同一份 Rust 单文件二进制完成。</sub></p>

npm 包把 4 个平台二进制放在 `vendor/<platform>-<arch>/`。0.1.1 已包含 `win32-x64`、`linux-x64`、`darwin-x64`、`darwin-arm64`，`packages/npm/lib/resolve.js` 会按当前平台选择。

### 方式三：Python 生态

已发布（多平台）：

```bash
pip install seramys-skillguard
skillguard scan ./skills --fail-on high
```

> 0.1.1 为 Windows、Linux、macOS 提供独立的平台 wheel（win_amd64、manylinux_2_17_x86_64、macOS arm64/universal2）。

**仓库内验证**：

```bash
python -m venv .venv

# Windows
.venv\Scripts\python -m pip install --upgrade pip setuptools wheel
.venv\Scripts\python -m pip install -e packages/python --no-build-isolation
.venv\Scripts\skillguard version
.venv\Scripts\skillguard scan tests\fixtures\clean-skill --no-color

# macOS / Linux
.venv/bin/python -m pip install --upgrade pip setuptools wheel
.venv/bin/python -m pip install -e packages/python --no-build-isolation
.venv/bin/skillguard version
.venv/bin/skillguard scan tests/fixtures/clean-skill --no-color
```

<p align="center">
  <img src="docs/assets/screenshots/python-launcher.png" width="880" alt="Python 启动器运行 SkillGuard">
</p>
<p align="center"><sub>Python 启动器同样复用 Rust 引擎，规则与输出格式完全一致。</sub></p>

Python wheel 会把二进制放进 `skillguard/_bin/`，用 `scripts/package-python.sh` 生成。

### 方式四：源码开发

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
bash scripts/build-all.sh     # 交叉编译矩阵；缺少工具链的目标会跳过
```

三套入口共用同一份 `rules/core.json` 和同一个 `engine.rs`，不存在“Node 规则”和“Python 规则”分叉。

---

## 它扫描什么

| 类别 | 规则数 | 典型信号 |
|---|---:|---|
| 提示注入 | 6 | 忽略既有指令、套取系统提示词、对用户隐瞒、角色劫持、绕过人工确认、安全保证话术 |
| 混淆载荷 | 11 | 零宽字符、Base64、空白填充、pyc 篡改/危险字节码、容器内嵌脚本、OOXML 宏/OLE、PDF/SVG/HTML 主动内容 |
| 权限过大 | 3 | `allowed-tools` 声明 Shell、访问 SSH/云凭证/Cookie/钱包、任意域名 |
| 密钥与凭证 | 4 | 私钥块、云厂商 API Key 前缀、高熵赋值、`.env` / 凭证文件 |
| 可疑外联 | 4 | Discord/Telegram/Slack Webhook、裸 IP、隧道、`curl \| bash`、下载后执行 |
| 危险命令 | 4 | `rm -rf /`、反弹 Shell、crontab / LaunchAgent / 注册表持久化、`find -exec` |
| 供应链 | 4 | 未锁定版本、npm 生命周期脚本、下载后 `chmod +x` 执行、覆盖 registry |
| MCP 配置 | 5 | 未锁定 npx/uvx、Docker 高权限、自动批准、远程 URL、shell 包装远程执行 |
| 包卫生 | 3 | frontmatter 缺字段、exe/dll/so/msi、bat/cmd/ps1/vbs/hta/reg |
| 数据访问 | 2 | 读取环境变量、批量导出环境变量 |
| 组合风险 | 5 | 敏感读取+外联、提示注入+远程执行、环境变量+外联、话术+registry 劫持、容器脚本+批量执行 |
| **合计** | **51** | 规则均为 JSON 定义，可加载外部规则包 |

<p align="center">
  <img src="docs/assets/diagrams/rules-coverage.png" width="980" alt="51 条规则的类别与严重度分布">
</p>
<p align="center"><sub>51 条规则的类别与严重度分布；组合规则负责跨信号关联。</sub></p>

### 组合规则为什么重要

单条规则容易误报，也容易被拆开绕过。SkillGuard 会在同一个 Skill 内关联信号：

- `SG-COMBO-001`：读取 SSH 密钥/云凭证 + 外部网络调用 → **critical**，按数据外传处理；
- `SG-COMBO-002`：提示注入 + 反弹 Shell/下载执行 → **critical**，按远程控制处理；
- `SG-COMBO-003`：批量读取环境变量 + 外部网络调用 → **critical**，按凭证外传处理；
- `SG-COMBO-004`：安全保证话术 + registry 覆盖 → **critical**，按供应链劫持处理；
- `SG-COMBO-005`：容器内嵌脚本 + `find -exec` → **critical**，按隐藏载荷投递处理。

组合规则只在两个信号都未被抑制时触发；抑制其中任意一个，组合风险也会消失。

### 内置规则

<details>
<summary><b>展开：51 条内置规则完整列表</b></summary>

| ID | 严重度 | 类别 | 检测内容 |
|---|---|---|---|
| SG-PI-001 | high | 提示注入 | 忽略/绕过既有系统指令 |
| SG-PI-002 | high | 提示注入 | 套取系统提示词或内部指令 |
| SG-PI-003 | high | 提示注入 | 要求对用户/管理员隐瞒行为 |
| SG-PI-004 | medium | 提示注入 | 角色劫持、越狱模式 |
| SG-PI-005 | medium | 提示注入 | 绕过人工确认自动执行 |
| SG-PI-008 | medium | 提示注入 | 安全保证话术（“标准做法/已审计/不写凭证”） |
| SG-PI-006 | high | 混淆 | 零宽字符、双向控制符、Unicode 标签 |
| SG-PI-007 | medium | 混淆 | 大段 Base64（解码含命令时升级为 critical） |
| SG-OBF-003 | high | 混淆 | 空白填充隐藏尾部载荷 |
| SG-PYC-001 | high | 混淆 | Python 字节码与源码大小/哈希不一致 |
| SG-PYC-002 | high | 混淆 | pyc code object 中的环境变量外联/动态执行特征 |
| SG-CONT-001 | high | 混淆 | docx/xlsx/zip 容器内嵌脚本或可执行文件 |
| SG-CONT-002 | high | 混淆 | Office 容器含 vbaProject.bin/ActiveX/OLE 嵌入 |
| SG-CONT-003 | critical | 混淆 | 容器内嵌 OLE/CFB 复合文档 |
| SG-CONT-004 | high | 混淆 | Office 外部关系、附加模板、DDE、instrText、altChunk |
| SG-PDF-001 | high | 混淆 | PDF JavaScript/OpenAction/Launch/嵌入文件 |
| SG-WEB-001 | high | 混淆 | SVG/HTML 脚本、事件处理器、javascript:、自动刷新 |
| SG-MCP-001 | high | MCP | 未锁定版本的 npx/bunx/uvx/pipx 启动 |
| SG-MCP-002 | high | MCP | Docker privileged/docker.sock/SYS_ADMIN |
| SG-MCP-003 | high | MCP | autoApprove/alwaysAllow/通配批准 |
| SG-MCP-004 | medium | MCP | 远程 HTTP(S) MCP 端点 |
| SG-MCP-005 | high | MCP | bash -c / powershell 包装 curl/wget/IEX |
| SG-PERM-001 | medium | 权限 | `allowed-tools` 声明 shell/终端/进程执行 |
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
| SG-CMD-001 | high | 命令 | `rm -rf /`、全盘 `chmod 777`、mkfs、dd、fork bomb |
| SG-CMD-002 | critical | 命令 | nc/ncat/socat、`/dev/tcp` 反弹 Shell |
| SG-CMD-003 | high | 命令 | crontab、LaunchAgent、计划任务、注册表 Run |
| SG-CMD-004 | medium | 命令 | `find -exec` 批量执行 |
| SG-DATA-001 | medium | 数据访问 | 读取环境变量 |
| SG-DATA-002 | high | 数据访问 | 批量导出环境变量 |
| SG-SC-001 | low | 供应链 | pip/npm/pnpm/yarn 安装未锁定版本 |
| SG-SC-002 | medium | 供应链 | npm preinstall/postinstall 生命周期脚本 |
| SG-SC-003 | high | 供应链 | 下载后 `chmod +x` 立即执行 |
| SG-SUPPLY-004 | high | 供应链 | npm/yarn registry 指向非官方地址 |
| SG-HYG-001 | low | 卫生 | frontmatter 缺少 name/description/version |
| SG-HYG-002 | high | 卫生 | 包含 exe/dll/so/msi 等二进制/安装包 |
| SG-HYG-003 | low | 卫生 | 包含 bat/cmd/ps1/vbs/hta/reg 脚本 |
| SG-COMBO-001 | critical | 组合 | 敏感信息访问 + 外部网络调用（数据外传） |
| SG-COMBO-002 | critical | 组合 | 提示注入 + 反弹 Shell/下载执行（远程控制） |
| SG-COMBO-003 | critical | 组合 | 批量环境变量读取 + 外部网络调用（凭证外传） |
| SG-COMBO-004 | critical | 组合 | 安全话术 + registry 覆盖（供应链劫持） |
| SG-COMBO-005 | critical | 组合 | 容器内嵌脚本 + `find -exec`（隐藏载荷投递） |

</details>

查看当前规则：

```bash
skillguard rules list
```

---

## 命令参考

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
| `0` | 没有达到 `--fail-on` 阈值的问题 |
| `1` | 存在达到阈值的问题 |
| `2` | 参数、配置、规则或 IO 错误 |

### 常用命令

```bash
# 扫描一个 Skill
skillguard scan ./my-skill

# 扫描一个 Skill 集合
skillguard scan ./skills

# CI：SARIF + 阈值
skillguard scan ./skills --format sarif --fail-on high > skillguard.sarif

# 只保留 high 及以上
skillguard scan ./skills --min-severity high

# 允许 GitHub 域名，忽略依赖未锁定提示
skillguard scan . --allow-domain github.com --ignore SG-SC-001

# 校验自定义规则包
skillguard rules validate ./my-rules/

# 生成默认配置和 GitHub Actions 工作流
skillguard init
skillguard init ./my-skill --force
```

---

## 输出格式

### text

人类可读，带颜色；管道或 `--no-color` 时自动关闭。每条 finding 包含规则 ID、严重度、Skill、文件:行:列、脱敏后的片段、原因和修复建议。

### JSON

```bash
skillguard scan ./tests/fixtures/exfil-skill --format json --json-pretty
```

```json
{
  "tool": "skillguard",
  "version": "0.1.1",
  "root": "/path/to/exfil-skill",
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
      "remediation": "立即停止使用该 Skill，审查脚本与网络目标，轮换可能泄露的凭证。",
      "references": []
    }
  ]
}
```

实际输出字段按字母序序列化；以上为可读性排序后的示例。

### SARIF 2.1.0

```bash
skillguard scan ./skills --format sarif > skillguard.sarif
```

SARIF 包含 51 条规则的元数据、`ruleId`、`level`、`physicalLocation` 和 `partialFingerprints`，可直接上传 GitHub Code Scanning：

```yaml
- uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: skillguard.sarif
```

本地验证：SARIF `version` 为 `2.1.0`，`driver.name` 为 `SkillGuard`，规则 40 条，扫描 exfil fixture 产生 4 条结果。

---

## 架构

<p align="center">
  <img src="docs/assets/diagrams/architecture.png" width="980" alt="SkillGuard 架构：一个 Rust 引擎，三种入口">
</p>
<p align="center"><sub>一个 Rust 引擎，三种入口：单文件二进制、Node 启动器、Python 启动器。</sub></p>

### 一次扫描的七个步骤

<p align="center">
  <img src="docs/assets/diagrams/pipeline.png" width="820" alt="SkillGuard 扫描流水线：发现、解析、分类、规则、组合、抑制、报告">
</p>

1. **发现 Skill**：递归查找包含 `SKILL.md` 的目录，也支持单文件或 Skill 集合；遵循 `exclude`。
2. **解析 frontmatter**：读取 `name`、`description`、`version`、`allowed-tools` 等字段。
3. **分类文件**：markdown / script / config / text / binary；Office/ZIP 容器先解包，pyc 先做完整性校验。
4. **运行规则**：regex、路径、熵、隐藏字符、Base64、frontmatter、空白填充、容器脚本、字节码、组合十类匹配器。
5. **组合信号**：同一 Skill 内跨文件关联，生成 `SG-COMBO-*`。
6. **应用抑制**：项目 ignore、文件头、行级注释、CLI/config，精确到规则。
7. **输出报告**：text / JSON / SARIF，按严重度、Skill、文件、行号排序。

### 源码结构

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
tests/fixtures/      13 组端到端测试夹具
```

### 为什么是 Rust，为什么是单文件

- **一个引擎**：规则、组合逻辑、抑制、报告只实现一次；Node/Python 只负责找到并 `exec` 平台二进制。
- **无运行时**：单文件二进制不要求目标机器安装 Node、Python 或 JVM；适合 CI、隔离环境和临时审查。
- **线性时间正则**：Rust `regex` 不支持回溯，避免 ReDoS；扫描不可信输入时更可控。
- **只读、离线**：不执行被扫描代码，不发起网络请求，不上传内容；适合处理私有仓库和敏感 Skill。
- **可审计**：规则是 JSON，找到误报可以直接看到规则 ID、正则和修复建议。

### 安全边界

- 不执行被扫描的代码，不解析动态行为；
- 不联网，不上传被扫描内容；
- 不跟随符号链接；
- 目录深度上限 32，Skill 数上限 1000，文件数上限 5000；
- 单文件文本扫描上限默认 512 KB；Office/ZIP 容器上限 16 MB、单条目 4 MB；
- 容器只解一层，不执行宏或 OLE 对象；pyc 只比对头部元数据并扫描可打印字符串；
- 密钥在 text/JSON 输出中自动脱敏（保留首尾各 4 字符）；
- 跳过 `.git`、`node_modules`、`target`、`dist`、`.venv`、`vendor` 等目录（可通过配置覆盖）。

---

## 抑制误报

<p align="center">
  <img src="docs/assets/diagrams/suppression.png" width="980" alt="SkillGuard 四层抑制：项目、文件、行、运行级">
</p>

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

忽略整个文件：

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

### 4. 运行级忽略

```bash
skillguard scan . --ignore SG-COMBO-001
```

或在配置中：

```json
{ "ignore_rules": ["SG-NET-*"] }
```

> **建议**：组合规则通常落在 `SKILL.md` 的汇总位置；如果确实要抑制，优先用文件级或项目级，并在注释里写明原因。不要用 `*` 掩盖整类风险，除非你确认该规则在该项目中不适用。

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

| 字段 | 说明 | 默认 |
|---|---|---|
| `fail_on` | 达到该级别时退出码为 1；`none` 表示永不失败 | `high` |
| `min_severity` | 隐藏更低级别的结果 | `low` |
| `ignore_rules` | 忽略规则 ID，支持前缀通配 | `[]` |
| `allow_domains` | 允许的网络域名，抑制 `SG-NET-001` 等 | `[]` |
| `max_file_size_kb` | 文本文件内容扫描上限 | `512` |
| `exclude` | 排除路径，支持 `*` / `?` 通配 | `[]` |
| `rules_dir` | 额外规则包目录，同 ID 覆盖内置 | 空 |

优先级：**CLI 参数 > `.skillguard.json` > 内置默认值**。

生成默认配置和工作流：

```bash
skillguard init
skillguard init ./my-skill --force
```

实际输出：

```text
wrote ./my-skill/.skillguard.json
wrote ./my-skill/.github/workflows/skillguard.yml
done. Run: skillguard scan .
```

---

## 自定义规则

规则包是 JSON。下面这条规则禁止访问未批准的第三方域名：

```json
{
  "version": "1",
  "rules": [
    {
      "id": "ACME-NET-001",
      "title": "禁止访问未批准的第三方域名",
      "severity": "high",
      "category": "policy",
      "description": "示例企业策略：Skill 不得连接未批准的第三方 API。",
      "remediation": "改用公司网关，或在 allow_domains 中申请白名单。",
      "applies_to": ["text"],
      "max_matches": 3,
      "match": {
        "type": "regex",
        "pattern": "(?i)https?://[^\\s\"']*evil\\.example\\.com"
      }
    }
  ]
}
```

加载：

```bash
skillguard scan ./skills --rules ./examples/extra-rules
```

校验：

```bash
skillguard rules validate ./examples/extra-rules
# OK: 1 rule(s) loaded and compiled
```

### 匹配类型

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

### 贡献一条规则

1. 在 `examples/extra-rules/` 或 `rules/core.json` 里添加规则；
2. 在 `tests/fixtures/` 添加一个能触发它的最小 Skill；
3. 在 `tests/cli.rs` 添加断言；
4. 运行 `skillguard rules validate ...`、`cargo test`、`cargo clippy --all-targets -- -D warnings`；
5. 在 PR 里说明：检测什么、为什么不是误报、如何修复、参考链接。

---

## CI 集成

<p align="center">
  <img src="docs/assets/diagrams/ci-flow.png" width="980" alt="SkillGuard CI 流程：提交、矩阵测试、扫描、SARIF、Code Scanning">
</p>

### 在你的仓库里扫描（今天就能用）

不依赖 npm 发布，直接构建并上传 SARIF：

```yaml
name: SkillGuard
on: [push, pull_request]

permissions:
  contents: read
  security-events: write

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - name: Scan skills
        run: >
          ./target/release/skillguard scan . --format sarif --fail-on high
          > skillguard.sarif
      - name: Upload SARIF
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: skillguard.sarif
```

### 使用本仓库的 composite Action（npm 包已发布，含 4 平台二进制）

`action.yml` 已包含在仓库中，通过 `npx seramys-skillguard` 运行；npm 包 0.1.1 已内含 4 个平台二进制。建议固定到发布标签：

```yaml
- uses: Seramys/skillguard@v0.1.1
  with:
    path: "."
    fail-on: high
    format: text
```

| 输入 | 默认值 | 说明 |
|---|---|---|
| `path` | `.` | 要扫描的目录 |
| `fail-on` | `high` | `low` / `medium` / `high` / `critical` / `none` |
| `format` | `sarif` | `text` / `json` / `sarif`，输出到运行日志 |
| `version` | `0.1.1` | npm 包版本 |

> composite Action 只运行扫描并设置退出码；它不写 SARIF 文件。需要 Code Scanning 时，用 `npx --yes seramys-skillguard@0.1.1 scan . --format sarif > skillguard.sarif`，或使用上面的本地构建工作流。

> npm 包 0.1.1 已包含 win32-x64、linux-x64、darwin-x64、darwin-arm64；Action 自测在 ubuntu/windows/macos x64 上 3/3 通过：[运行记录](https://github.com/Seramys/skillguard/actions/runs/44661440348)。

### 通用 CI

```bash
skillguard scan ./skills --fail-on high
```

退出码非 0 即失败；如果 CI 不消费 SARIF，也可以只保留 text/JSON 输出。

### 本仓库自己的 CI

`.github/workflows/ci.yml` 在 Ubuntu / Windows / macOS 三平台运行：

- `cargo test`
- `cargo build --release`
- `python bench/run.py --exe target/release/skillguard`（检测基准门禁）
- Python wheel 打包 + editable install
- npm 打包 + 启动器版本检查

---

## 检测基准

> 目标：把“检出率”变成可复现的数字，而不是规则数量的宣称。

当前基线（2026-11-20，`holdout`，真实语料 + 规则消歧后）：

| 阈值 | Recall | FPR |
|---|---:|---:|
| low | 95.0%（192/202） | 45.9% |
| medium | 69.8%（141/202） | 29.7% |
| high | **64.9%（131/202）** | **3.5%** |
| critical | 30.7%（62/202） | **0.0%** |

holdout 含 202 恶意 / 340 良性。`high` 阈值 FPR 3.5%、`critical` 阈值 FPR 0%，误报门禁达标；recall 仍低于 90% 目标，缺口集中在 MaliciousSkillBench 的改写型恶意样本。

规则消歧（2026-11-20）：

- `SG-NET-004` 不再把 `curl ... | python -c "json.load(...)"` 当作下载执行，只有管道进 `exec/eval/compile/subprocess` 等执行原语才命中；
- `SG-COMBO-003` 的网络组不再包含 `SG-NET-004`，避免「API token + 普通 URL」直接升为 critical；
- `SG-PI-003` 的中文分支收紧为「不要/禁止 + 告诉/通知/告知/提醒 + 用户」紧邻句式。

语料：

- ClawHub 500 条良性（按平台状态 provisional 抓取，逐条人工复核后标 `reviewed`）；
- MaliciousSkillBench 300 条恶意（沿用原始 benchmark 标注，未使用本扫描器自标）+ Cisco 评测集 6 条 + Trail of Bits `overtly-malicious-skills` 4 条；
- 10 个中文恶意样本 + 10 个中文良性样本；
- 320 恶意 / 520 良性生成语料，按 `dev/test/holdout` 切分；良性含 hard negatives：`.env.example`、合法 `curl`、单变量 `os.getenv`、`find -exec` 测试、官方 registry、锁定版本 MCP、远程 MCP 端点、普通 Docker、无害 Base64。

语义层消融（仅 medium 候选队列，68 条：10 恶意 / 58 良性，Qwen2.5-7B-Instruct Q4_K_M 本地 CPU 推理，`response_format=json_schema`）：

| 方案 | Recall | FPR | parse-error |
|---|---:|---:|---:|
| 规则（候选队列内） | 0% | 0% | — |
| LLM 语义（risk ≥ 0.6，json_schema） | 40%（4/10） | 0% | 0 |
| 规则 ∪ 语义 | 40% | 0% | 0 |

JSON schema 约束把 `parse-error` 从 32/68 清零，但召回仍是 40%：剩余 6 条恶意模型的 `labels` 已能指出 exfiltration / RCE / persistence，但 `risk` 落在 0.2–0.5。说明瓶颈已从「输出格式」转为「7B 模型的风险标定偏保守」，下一步应按 `labels` 而非单一 `risk` 阈值做升级判定。

语义层只作为 **medium 候选的 rerank**（`python bench/rerank.py`），不进入 `skillguard scan` 主路径，避免把 CPU 推理成本带给所有用户。JSON schema 约束把输出固定为 `{"risk", "labels", "evidence"}`，消除历史 `max_tokens=96` 时的截断问题。

运行：

```bash
cargo build --release
python bench/fetch_samples.py
python bench/make_corpus.py
python bench/run.py --exe target/release/skillguard --split holdout
```

结果写入 `bench/results/latest.json` 和版本化报告；CI 以 `target_fpr=0.05`、`min_recall_at_fpr=0.90`、`max_fpr_upper=0.10` 作为门禁。当前是合成+公开小语料，不是安全认证；P1 会接入真实 Hub 样本和外部复核。

> 该基准跑的是工作区源码。已发布的 0.1.1 不包含本次新增的容器、pyc、空白填充、环境变量、MCP 与策略规则；正式发布时建议升到 0.2.0。

### P1/P2 机制

这些机制已经可执行，但默认不进入 P0 门禁：

```bash
# P1：导入本地已授权的真实 Hub 样本；不自动抓取
python bench/import_samples.py --source ./hub-samples --label benign --split test

# P1：季度对抗集
python bench/make_adversarial.py --limit 8 --force

# P1：pyc 跨版本矩阵（每个 Python 解释器各跑一次）
python bench/make_pyc_variants.py --python 3.13

# P2：离线 LLM judge，只做候选重排；不提供模型
python bench/llm_judge.py --endpoint http://127.0.0.1:11434/v1/chat/completions --dry-run

# P2：校准、消融、候选队列
python bench/calibrate.py
python bench/ablation.py
python bench/candidates.py
```

外部阻塞项：真实 Hub 样本需要授权与人工标注；离线 LLM 需要本地模型端点与校准数据。两者都不会改变“规则是确定性闸门”的默认路径。

---

## 验证

<p align="center">
  <img src="docs/assets/screenshots/verification.png" width="980" alt="SkillGuard 本地验证报告：61 个测试、0 警告、自扫描 PASS">
</p>
<p align="center"><sub>本地验证报告：61 个测试、0 条 clippy 警告、自扫描 PASS、SARIF 2.1.0 有效。</sub></p>

本地验证环境：Windows 11 x64，SkillGuard 0.1.1，2026-11-20；Action 自测在 GitHub Actions 三平台完成（2026-11-20）。

| 检查项 | 命令 | 结果 |
|---|---|---|
| 单元测试 | `cargo test` | 19 passed · 0 failed |
| 端到端测试 | `cargo test` | 61 passed · 0 failed |
| 静态检查 | `cargo clippy --all-targets -- -D warnings` | 0 warnings |
| 自扫描 | `dist\win32-x64\skillguard.exe scan .` | PASS · 0 high · 4 low / 5 medium |
| SARIF | `--format sarif` | valid 2.1.0 · 40 rules · 4 results |
| npm 包 | `npm pack --dry-run` | 8 files · 4 platform binaries · bundled |
| PyPI | `twine upload` | 4 platform wheels · 0.1.1 |
| Python 启动器 | `skillguard version` | skillguard 0.1.1 |
| GitHub Action | [Action self-test](https://github.com/Seramys/skillguard/actions/runs/44661440348) | 3/3 passed · linux/windows/macos x64 |

<p align="center">
  <img src="docs/assets/screenshots/self-scan.png" width="900" alt="SkillGuard 自扫描：PASS，4 条 low 与 5 条 medium 提示">
</p>
<p align="center"><sub>用 SkillGuard 扫描自己的仓库：PASS，0 high；当前为 4 条 low 与 4 条 medium（依赖未锁定与 env 读取提示）。</sub></p>

<details>
<summary><b>展开：原始测试输出</b></summary>

```text
running 28 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 21 tests
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

SARIF 校验：

```text
version 2.1.0
tool SkillGuard
rules 51
results 4
schema https://json.schemastore.org/sarif-2.1.0.json
```

</details>

### 测试夹具

```text
tests/fixtures/
├── clean-skill/          无问题
├── prompt-injection/     提示注入
├── exfil-skill/          密钥读取 + Webhook 外传
├── dangerous-skill/      远程脚本执行
├── secret-skill/         密钥泄露与输出脱敏
├── suppressed-skill/     行级/文件级抑制
├── config-skill/         配置排除路径
├── collection/           多 Skill 发现
├── padding-skill/        空白填充隐藏环境变量导出
└── exclude-root/         发现阶段嵌套排除
```

> 截图来自本地实际运行；终端文本未改动，仅为可读性做了颜色和排版。验证报告中的数字来自上述命令的实际输出。

---

## 已知限制

- 当前是**静态规则 + 轻量启发式**扫描，不做 LLM 语义判定；高度改写或依赖运行时生成代码的提示注入仍可能漏报。
- 容器解包只覆盖常见 Office/ZIP 一层结构，不解析加密容器、宏或 OLE 对象。
- pyc 检测基于源码大小/时间戳/哈希与可打印字符串，不反编译完整字节码。
- 不能检测只在运行时出现的行为；运行时防护属于下一阶段。
- frontmatter 解析是轻量实现，复杂的嵌套 YAML 可能无法完整还原。
- 规则是模式匹配，存在误报；请用 `--ignore`、`.skillguardignore` 或 `allow_domains` 收敛。
- 不扫描超过 `--max-file-size` 的普通文本；Office/ZIP 容器按独立的 16 MB 上限处理。
- 扫描根目录若没有 `SKILL.md`，会把整个目录当作一个 Skill 处理；大型代码仓库建议显式指定 `./skills` 或使用 `exclude`。

**不要把 SkillGuard 当成唯一防线。** 它适合放在“安装前 / 合并前”的静态检查层；生产环境仍应配合沙箱、最小权限、依赖锁定、代码审查和运行时审批。

---

## FAQ

<details>
<summary><b>它会执行被扫描的 Skill 吗？</b></summary>

不会。SkillGuard 只读取文本和路径，不执行脚本、不加载依赖、不调用被扫描代码。

</details>

<details>
<summary><b>会上传代码或密钥吗？</b></summary>

不会。扫描完全在本地进行，不发起网络请求。输出中的密钥会脱敏；但 JSON/SARIF 仍可能包含文件名、路径和上下文片段，分享前请自行确认。

</details>

<details>
<summary><b>为什么用 Rust？</b></summary>

单文件、无运行时、启动快；线性时间正则更适合扫描不可信输入；同一引擎可以被 Node/Python 启动器复用。

</details>

<details>
<summary><b>为什么不用 LLM 做语义扫描？</b></summary>

LLM 语义分析可以作为补充，但不适合作为唯一闸门：结果不确定、需要网络/模型、难以在 CI 中稳定复现。0.1.x 先把确定性、离线、可解释的静态规则做好；语义分析在路线图中，会以可选、离线的方式加入。

</details>

<details>
<summary><b>误报太多怎么办？</b></summary>

先确认是否真的是误报；如果是，优先用最小范围的抑制：行级 → 文件级 → 项目级。合法域名可以用 `allow_domains`；企业策略可以写成外部规则包，而不是关掉整类规则。

</details>

<details>
<summary><b>能扫描整个仓库吗？</b></summary>

可以，但如果没有 `SKILL.md`，根目录会被当作一个 Skill；大型仓库建议显式指定 Skill 目录，或用 `exclude` 排除构建产物、依赖和测试夹具。扫描上限是 1000 个 Skill / 5000 个文件。

</details>

<details>
<summary><b>支持哪些平台？</b></summary>

源码支持 Windows / Linux / macOS；CI 矩阵在三平台运行测试和 release 构建。预编译单文件、npm 平台包和 Python wheel 以实际 Release / 发布产物为准。当前 0.1.1 的本地验证在 Windows 11 x64 完成；Release 提供四个平台二进制。

</details>

<details>
<summary><b>npm / PyPI 已经发布了吗？</b></summary>

0.1.1 已发布到 npm / PyPI，包含 Windows、Linux、macOS x64/arm64。Windows/Linux/macOS 均可直接安装；其他架构请源码构建。

</details>

<details>
<summary><b>能接入 GitHub Code Scanning 吗？</b></summary>

可以。使用 `--format sarif` 生成 SARIF 2.1.0，再用 `github/codeql-action/upload-sarif@v3` 上传；需要 `security-events: write` 权限。

</details>

<details>
<summary><b>如何添加自己的规则？</b></summary>

写一个 JSON 规则包，用 `--rules` 加载，或放到配置的 `rules_dir`。同 ID 会覆盖内置规则并输出 warning。建议同时提交一个最小 fixture 和 CLI 测试，确保规则可回归。

</details>

---

## 路线图

> 以下是方向，不是承诺；优先级会根据真实使用反馈调整。

- **0.2**：规则基线/差异扫描、更多供应链规则、规则测试框架、更多运行时/Agent 集成。
- **0.3**：可选的离线 LLM 语义分析，用于识别改写过的提示注入。
- **1.0**：运行时 Agent 网关、工具调用审批、集中策略与审计控制台。

### 标识

<p align="center">
  <img src="docs/assets/brand/logo.svg" width="96" alt="SkillGuard mark">
</p>

- **浏览器卡片**：Skill 是可分发、可安装的软件包；
- **四角扫描框**：静态扫描的聚焦与边界；
- **盾牌 + 对勾**：供应链防护与通过检查；
- **青色扫描光束**：只读、离线、逐行检查；
- **四角星**：AI / Agent 语境。

浅色与深色两套矢量标识在 [`docs/assets/brand/`](docs/assets/brand/)。

---

## 贡献

欢迎规则、fixture、误报报告、文档和翻译。提交前请确保：

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

新增规则时，最好同时提供：

- 一个能触发规则的最小 Skill（positive fixture）；
- 一个不应该触发的近似样例（negative fixture）；
- 一条 CLI 断言；
- 规则说明、修复建议和参考链接。

安全漏洞请通过 [SECURITY.md](SECURITY.md) 私下报告，不要开公开 issue。

<details>
<summary><b>维护者：发布与打包</b></summary>

### npm

1. 更新 `packages/npm/package.json` 的版本和仓库地址；
2. `bash scripts/package-npm.sh`；
3. `cd packages/npm && npm pack` 检查包内包含 `vendor/<platform>/skillguard[.exe]`；
4. `npm publish --access public`。

当前多平台发布采用“一个 npm 包内含全部平台二进制 + 每个平台独立 wheel”的方式；如果后续包体积成为问题，再拆平台子包。

### Python

1. 更新 `packages/python/pyproject.toml` 的版本和仓库地址；
2. `python -m pip install build`；
3. `bash scripts/package-python.sh`；
4. `twine upload packages/python/dist/*`。

### 单文件二进制

1. `bash scripts/build-all.sh`；
2. 将 `dist/<platform>-<arch>/skillguard[.exe]` 附加到 GitHub Release；
3. 在 Release 说明中给出 SHA256。

发布新版本后，同步更新 `action.yml` 的默认 `version` 和 README 中的版本号。

</details>

---

## License

MIT。见 [LICENSE](LICENSE)。

---

<div align="center">

<img src="docs/assets/brand/lockup.svg" width="480" alt="SkillGuard">

<br>

**装 Skill 之前，先让 SkillGuard 看一眼。**

<sub>AI Agent Skill Supply-Chain Security Scanner · MIT License</sub>

</div>
