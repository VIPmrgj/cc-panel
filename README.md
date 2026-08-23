# CC Panel

CC Panel 是运行在 Windows 上的 Claude Code 本地图形控制面板。它包装本机已安装的官方 `claude` CLI，不重写 Claude 的 agent loop；React 负责交互展示，Rust 负责进程、文件、网络、环境变量、密钥、Skill、附件和 Prompt 组合。

> 安全边界：**React 无文件/Shell/网络访问、内容不写盘**。

## 下载体验

Windows 用户可以直接下载当前体验版：[CC Panel 0.1.10 安装包](./CC%20Panel_0.1.10_x64-setup.exe)。没有 API Key 时也可以直接使用“演示模式”；只有真实 Agent 对话才需要本机已安装并登录官方 claude CLI。

## 当前能力

### 演示模式（无需 API Key）

- 使用固定的安全沙盒流程，不调用真实模型，不产生 API 费用。
- 不读取用户项目、密钥或隐私文件，不执行任意 Shell 命令。
- 在 Windows 桌面真实创建 hello_用户ID.html，展示文件内容和安全预览。
- 界面明确标注为“演示模式”，不会伪造模型内部思考过程。

### Claude Code 会话

- 新建、恢复、继续、重试和分叉会话。
- 使用长驻 NDJSON stdin/stdout 与官方 CLI 通信。
- 增量显示 Claude 输出、工具调用、工具结果，以及 CLI 实际发出的 thinking/summary。
- 工具权限在界面中逐次显示，用户明确选择“允许”或“拒绝”；默认不使用 `bypassPermissions` 或 blanket `dontAsk`。
- 支持软中断、停止、30 分钟 active-turn 超时、输出上限和 Windows Job Object 进程树清理。
- 同一时间最多一个 Claude 子进程和一个 active turn。

所有受管会话启动路径都包含且只包含一次：

```text
--autocompact 272k
```

界面显示配置策略：`Auto-compact: 272k`。只有收到可信的 `PreCompact`/`compact_boundary` 记录后，才会显示实际压缩状态；仅设置启动参数不等于压缩已经发生。

### 对话历史和模型切换

- Claude 自己的 session JSONL 是 transcript 的唯一真相来源。
- CC Panel 的 `~/.cc-panel/conversations.json` 只保存有界元数据，不保存 Prompt、回复、thinking、工具结果或附件正文。
- 切换提供商/模型会停止当前子进程并使用 `--fork-session` 创建新会话，旧会话保持不变。
- v1 仅支持 Anthropic-compatible provider，不提供 OpenAI 协议翻译代理。

### Provider profiles

- Provider、Base URL、model ID 和备注由 CC Panel 管理。
- API key 使用 Windows DPAPI CurrentUser 保护后再写入 `~/.cc-panel/models.json`。
- token 不返回 React，不出现在日志、错误、诊断、会话元数据或 stream DTO 中。
- URL 默认要求 HTTPS；仅回环地址可使用 HTTP；拒绝 URL credential、query 和 fragment。

### Skills、附件和 Prompt

- 扫描用户、项目、附加根和已启用插件的 `SKILL.md`。
- 管理 Claude 原生 `skillOverrides`：`on`、`name-only`、`user-invocable-only`、`off` 和继承默认。
- 附件通过 Rust 导入；正文仅保存在 Rust 内存，React 只获得句柄和安全元数据。
- `.env`、私钥和凭据类附件需要二次确认。
- Preview、Copy 和 Send 共用同一个 Rust 确定性组合器，包括 Skill hash、附件句柄、排序、XML 转义和大小限制。
- Ollama 只接收当前原始草稿，不接收 Skill 正文、附件正文、设置或 token；增强结果不会覆盖原文。

### 桌面界面

```text
52px activity rail | clamp(240px, 20vw, 360px) context panel | flexible chat
```

活动栏包含聊天、Skills、模型、附件和设置。聊天区包含 Markdown transcript、可折叠工具卡、thinking 卡、权限卡和可纵向调整的 composer。

- `Ctrl+Enter`：发送
- `Enter`：换行
- Markdown：`react-markdown` + `rehype-highlight`
- 不启用 `rehype-raw`，不渲染不安全原始 HTML
- 弹窗提供焦点陷阱、Escape 关闭和焦点恢复
- 支持 reduced motion 和窄窗口侧栏覆盖模式

## 技术结构

```text
React 18 + TypeScript + Vite
              │ typed Tauri commands + Channel events
Tauri 2 / Rust
              │
official claude CLI · Claude JSONL · Ollama · local files
```

主要子系统：

- `src-tauri/src/sessions/`：安全启动、NDJSON 协议、生命周期、权限、历史和 Windows Job Object。
- `src-tauri/src/model_profiles/`：provider profile 验证、DPAPI 和 masked DTO。
- `src-tauri/src/conversations/`：metadata-only 索引。
- `src-tauri/src/prompt/`：Preview/Copy/Send 共用的确定性组合。
- `src/state/chatReducer.ts` 与 `composerReducer.ts`：对话状态和下一轮草稿状态相互独立。

## 开发与验证

要求 Node.js/npm、Rust 1.88+、Windows WebView2，以及可在 `PATH` 中运行的官方 `claude` CLI。

```bash
npm install
npm run tauri dev
```

前端检查：

```bash
npm run format:check
npm run lint
npm run typecheck
npm run test:run
npm run build:web
```

Rust 检查（在 `src-tauri`）：

```bash
cargo fmt --check
cargo check --all-targets
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

当前自动化基线：前端 11 个测试文件、33 个测试；Rust 62 个测试。Vite 生产构建成功，但主 JS chunk 仍会触发大于 500 kB 的非致命警告。

## 手工验收清单

发布前仍应在真实窗口和已认证 CLI 上检查：

1. 新建、Resume、Continue、Retry 和模型切换 Fork 的 session identity。
2. 增量文字、工具调用/结果，以及 thinking 仅在 CLI 发出时出现。
3. 权限 Allow/Deny、停止、超时、窗口关闭和整个进程树退出。
4. Claude JSONL 历史重建且 CC Panel 不创建消息正文副本。
5. UI 始终显示 `Auto-compact: 272k`，但不伪造压缩完成事件。
6. 1280、1024、839、720 px，以及 100–200% 缩放、键盘、Narrator/NVDA、reduced motion。
7. Provider token、附件正文和 Skill 正文不出现在 React DTO、日志、错误和元数据中。

## 已知后续工作

- 对真实已认证 Claude CLI 做完整 GUI round-trip 冒烟。
- 为 stream normalizer、fork identity、权限交互和应用退出增加更深的集成测试。
- 对主前端 bundle 做代码分割。
- PDF 解析目前仍在主进程中；后续可用受限 worker 隔离解析超时、内存和页数。
- 发布前完成代码签名、干净 Windows 安装/卸载测试和正式安装包验证。
