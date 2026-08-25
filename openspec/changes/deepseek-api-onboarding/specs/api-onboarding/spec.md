## Purpose

为没有模型 API 配置经验的新用户提供一条可完成、可跳过且不暴露密钥的 DeepSeek 接入路径，让用户理解申请、保存和验证 API 的完整过程。

## ADDED Requirements

### Requirement: The onboarding SHALL provide one DeepSeek example path

新手引导 SHALL 提供 DeepSeek 作为完整示例，并同时提供已有其他 API Key 和稍后配置的出口。引导不得暗示 CC Panel 只能使用 DeepSeek。

#### Scenario: User starts without an API Key

- **WHEN** 用户在默认模型步骤选择“跟着示例配置 DeepSeek”
- **THEN** 系统显示分步骤的 DeepSeek 申请说明、官方入口和当前步骤进度

#### Scenario: User already has another provider key

- **WHEN** 用户选择“我已有其他 API Key”
- **THEN** 系统打开现有通用模型配置，不要求用户完成 DeepSeek 教程

#### Scenario: User postpones model setup

- **WHEN** 用户选择“稍后配置”或关闭引导
- **THEN** 系统保留其它引导功能，并且不会在每次点击发送时重复弹出 API 教程

### Requirement: The onboarding SHALL use progressive disclosure

基础引导 SHALL 每一步只能要求一个主要操作；API 地址和模型 ID必须从 DeepSeek 预设中自动填写，用户可以进入高级配置后查看或修改。

#### Scenario: User follows the basic path

- **WHEN** 用户在基础模式进行 DeepSeek 配置
- **THEN** 用户只需要阅读当前说明、打开申请页面或继续、输入密钥并进行验证，不需要手动填写端点和模型 ID

#### Scenario: User opens advanced configuration

- **WHEN** 用户点击高级配置或选择其他 API
- **THEN** 系统显示现有的提供商、API 地址、模型 ID和备注字段，并保留自定义 Anthropic 兼容端点能力

### Requirement: The API Key SHALL remain protected

API Key SHALL 通过原生凭据输入流程保存到本地受保护存储；前端状态、普通 IPC 请求、引导文字和日志中不得出现明文密钥。

#### Scenario: User saves a new DeepSeek key

- **WHEN** 用户提交 DeepSeek API Key
- **THEN** 系统打开原生凭据输入或保存流程，保存成功后仅返回“密钥已保存”状态，不返回完整密钥

#### Scenario: User cancels credential entry

- **WHEN** 用户取消系统凭据输入
- **THEN** 系统保留引导位置和已填写的非敏感配置，不创建半成品模型配置

### Requirement: Connection testing SHALL be explicit and observable

系统 SHALL 不得在保存密钥后自动发起真实模型请求。用户主动点击连接测试后，系统必须显示加载状态，并返回成功或可理解的失败原因；实际测试前必须提示可能产生少量 API 费用。

#### Scenario: User starts a real connection test

- **WHEN** 用户主动点击“测试连接”并确认可能产生费用
- **THEN** 系统使用已保存的模型配置进行一次最小请求，显示进行中状态，并在结束后显示成功或失败结果

#### Scenario: User does not start a real test

- **WHEN** 用户只保存配置但没有点击测试
- **THEN** 系统不访问 DeepSeek 模型接口，只显示配置已保存和“等待测试”状态

### Requirement: Connection failures SHALL be actionable

连接测试 SHALL 将失败至少区分为密钥无效或无权限、余额或限流、模型或端点错误、网络超时/不可达和服务端错误，并保留重试及返回高级配置的入口。错误响应不得回显 API Key 或完整服务商响应体。

#### Scenario: Provider rejects credentials

- **WHEN** DeepSeek 返回未授权或禁止访问状态
- **THEN** 系统显示“API Key 无效或没有权限”，不展示密钥内容，并允许重新输入或重试

#### Scenario: Provider or network is unavailable

- **WHEN** 请求超时、无法连接或服务端返回 5xx
- **THEN** 系统显示对应的网络/服务暂不可用提示，并允许稍后重试

#### Scenario: Model configuration is rejected

- **WHEN** 服务商返回模型不存在、端点不支持或请求参数错误
- **THEN** 系统提示检查模型和 API 地址，并提供进入高级配置的操作

### Requirement: Successful setup SHALL complete the onboarding step

连接测试成功后，系统 SHALL 刷新模型配置、选中 DeepSeek 配置作为默认模型，并将新手引导中的模型步骤标记为完成；用户可以继续后续引导或关闭引导。

#### Scenario: DeepSeek connection succeeds

- **WHEN** 最小连接测试成功
- **THEN** 系统显示已连接的提供商和模型，默认选中该配置，并允许继续工作目录、Prompt 优化和演示步骤
