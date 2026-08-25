export type SkillOverrideValue =
  "on" | "name-only" | "user-invocable-only" | "off";

export type SkillOverrideSelection = "default" | SkillOverrideValue | "unknown";
export type SkillSource = "user" | "project" | "additional" | "plugin";

export interface ApiError {
  code: string;
  message: string;
  retryable: boolean;
  field?: string | null;
  details?: Record<string, string | number | boolean> | null;
}

export interface RootEntry {
  id: string;
  path: string;
  label: string;
  kind: "project" | "additional";
}

export interface ModelCandidate {
  source: "process-env" | "user-env" | "project" | "project-local" | "managed";
  label: string;
  value: string;
  enforced: boolean;
}

export interface ModelStatus {
  desiredUserModel: string | null;
  settingsRevision: string;
  candidates: ModelCandidate[];
  activeSessionObservable: false;
  warnings: string[];
}

export interface SkillRecord {
  instanceId: string;
  canonicalId: string;
  displayName: string;
  description: string;
  source: SkillSource;
  sourceLabel: string;
  manifestPath: string;
  manifestHash: string;
  manifestPreview: string;
  overrideState: SkillOverrideSelection;
  rawOverrideValue?: string | null;
  explicitOverride: boolean;
  userInvocable: boolean;
  modelInvocable: boolean;
  collisionInstanceIds: string[];
  warnings: string[];
}

export interface SkillInventory {
  skills: SkillRecord[];
  settingsRevision: string;
  claudeCliAvailable: boolean;
  pluginWarning?: string | null;
  scannedAtRevision: string;
}

export interface OllamaModel {
  name: string;
  size?: number | null;
  modifiedAt?: string | null;
}

export interface OllamaStatus {
  online: boolean;
  baseUrl: string;
  selectedModel: string | null;
  models: OllamaModel[];
  autoSelected: boolean;
  message: string;
}

export type AttachmentKind = "text" | "pdf" | "image";

export interface AttachmentRecord {
  handle: string;
  name: string;
  kind: AttachmentKind;
  mime: string;
  rawBytes: number;
  extractedBytes: number;
  sha256: string;
  width?: number | null;
  height?: number | null;
  warnings: string[];
}

export interface AttachmentPreview {
  attachment: AttachmentRecord;
  content: string;
  truncated: boolean;
  dataUrl?: string | null;
}
export interface PendingSensitiveAttachment {
  confirmationToken: string;
  name: string;
  reason: string;
  rawBytes: number;
}

export interface AttachmentImportResult {
  imported: AttachmentRecord[];
  pendingConfirmation: PendingSensitiveAttachment[];
  rejected: Array<{ name: string; code: string; message: string }>;
}

export interface AppPreferences {
  selectedProjectRoot: RootEntry | null;
  additionalRoots: RootEntry[];
  ollamaBaseUrl: string;
  ollamaModel: string | null;
  nativeNotificationsEnabled: boolean;
}

export interface BootstrapResponse {
  preferences: AppPreferences;
  model: ModelStatus;
  skills: SkillInventory;
  ollama: OllamaStatus;
  attachments: AttachmentRecord[];
  claudeCodeVersion: string | null;
  claudeCodeAuthenticated: boolean;
  claudeCodeConfigured: boolean;
  nodeVersion: string | null;
  npmVersion: string | null;
  npmMirrorConfigured: boolean;
  gitAvailable: boolean;
  powershellAvailable: boolean;
}

export type DomesticInstallPhase =
  "node" | "git" | "npm" | "claude" | "onboarding" | "complete";

export type DomesticInstallStatus = "running" | "completed" | "failed";

export interface DomesticInstallProgress {
  step: number;
  totalSteps: number;
  phase: DomesticInstallPhase;
  status: DomesticInstallStatus;
  message?: string | null;
}

export interface EnhancePromptResponse {
  text: string;
  model: string;
}

export interface CompositionRequest {
  originalPrompt: string;
  enhancedPrompt: string | null;
  useEnhanced: boolean;
  selectedSkills: Array<{ instanceId: string; manifestHash: string }>;
  attachmentHandles: string[];
}

export interface CompositionResult {
  text: string;
  compositionId: string;
  utf8Bytes: number;
  characters: number;
  lines: number;
  skillCount: number;
  attachmentCount: number;
  promptVariant: "original" | "ollama-enhanced";
  warnings: string[];
}

export interface CopyResult extends CompositionResult {
  copied: true;
  notificationSent: boolean;
  notificationWarning?: string | null;
}

/** Chat/session DTOs mirror the serializable boundary exposed by Tauri. */
export interface ModelProfile {
  id: string;
  providerName: string;
  note?: string | null;
  websiteUrl?: string | null;
  baseUrl: string;
  modelId: string;
  selected: boolean;
  hasApiKey: boolean;
}

export interface ModelProfileInput {
  id?: string | null;
  providerName: string;
  note?: string | null;
  websiteUrl?: string | null;
  baseUrl: string;

  modelId: string;

  selected: boolean;
}

export interface ModelConnectionTestResult {
  ok: boolean;
  code: string;
  message: string;
  providerName: string;
  modelId: string;
}

export interface ModelProfileList {
  schemaVersion: number;
  revision: number;
  profiles: ModelProfile[];
}

export interface ConversationSummary {
  sessionId: string;
  title: string;
  projectPath: string | null;
  profileId: string | null;
  providerName: string | null;
  modelId: string | null;
  parentSessionId: string | null;
  createdAtMs: number;
  updatedAtMs: number;
  status: string;
  favorite: boolean;
  archived: boolean;
}

export type ChatRole =
  | "user"
  | "assistant"
  | "system"
  | "tool"
  | "thinking"
  | "permission"
  | "error";

export type AssistantBlock =
  | { type: "text"; text: string }
  | { type: "thinking"; thinking: string }
  | { type: "tool-use"; toolUseId: string; toolName: string; input: unknown }
  | {
      type: "tool-result";
      toolUseId: string;
      content: string;
      isError: boolean;
    };

export interface PermissionRule {
  id: string;
  toolName: string;
  command?: string | null;
  cwd?: string | null;
}

export interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  turnId?: string | null;
  blocks?: AssistantBlock[];
  createdAt?: string | null;
  toolName?: string | null;
  toolInput?: unknown;
  toolOutput?: unknown;
  requestId?: string | null;
  permissionDescription?: string | null;
  permissionOptions?: string[];
  permissionExpiresAt?: number | null;
  status?: "running" | "complete" | "error" | "pending";
}

export interface ConversationHistory {
  sessionId: string;
  messages: ChatMessage[];
  [key: string]: unknown;
}

export type StartClaudeSessionMode =
  "new" | "resume" | "continue" | "fork" | "retry";

export interface StartClaudeSessionRequest {
  mode: StartClaudeSessionMode;
  sessionId?: string | null;
  parentSessionId?: string | null;
  profileId?: string | null;
  title?: string | null;
}

export interface ClaudeSession {
  sessionId: string;
  runId: string;
  status: string;
  favorite: boolean;
  archived: boolean;
  autoCompactTokens: number;
  compactionObservable: boolean;
}

export interface SendClaudeMessageRequest {
  sessionId: string;
  runId: string;
  composition: CompositionRequest;
}

export interface PermissionResponseRequest {
  sessionId: string;
  runId: string;
  requestId: string;
  behavior: "allow" | "deny" | "deny-interrupt";
  message?: string | null;
}

export interface PermissionRuleRequest {
  toolName: string;
  command?: string | null;
  cwd?: string | null;
}

export type ClaudeLifecycleStatus =
  | "starting"
  | "running"
  | "idle"
  | "awaiting-permission"
  | "stopping"
  | "exited"
  | "failed"
  | "timed-out"
  | "interrupted";

export type ClaudeRunEvent =
  | {
      type: "lifecycle";
      status: ClaudeLifecycleStatus;
      message?: string | null;
    }
  | {
      type: "init";
      model?: string | null;
      claudeCodeVersion?: string | null;
      permissionMode?: string | null;
      slashCommands?: string[];
    }
  | {
      type: "assistant";
      messageId: string;
      blocks: AssistantBlock[];
    }
  | {
      type: "stream";
      messageId?: string | null;
      blockIndex?: number | null;
      deltaType: "text" | "thinking" | "input-json";
      delta: string;
    }
  | {
      type: "tool-use";
      toolUseId: string;
      toolName: string;
      input: unknown;
    }
  | {
      type: "tool-result";
      toolUseId: string;
      content: string;
      isError: boolean;
    }
  | {
      type: "tool-progress";
      toolUseId: string;
      toolName: string;
      state: string;
      subtype?: string | null;
      text?: string | null;
    }
  | {
      type: "permission";
      requestId: string;
      toolUseId?: string | null;
      toolName?: string | null;
      input?: unknown;
      expiresAt?: number;
    }
  | {
      type: "compaction";
      phase: "starting" | "completed";
      trigger?: string | null;
      preTokens?: number | null;
      postTokens?: number | null;
      durationMs?: number | null;
    }
  | {
      type: "result";
      success: boolean;
      isError: boolean;
      stopReason?: string | null;
      durationMs?: number | null;
      numTurns?: number | null;
    }
  | { type: "error"; code: string; message: string; retryable: boolean }
  | { type: "unknown"; rawType: string };

export interface ClaudeRunEnvelope {
  sessionId: string;
  runId: string;
  sequence: number;
  event: ClaudeRunEvent;
}

export type StreamChannel =
  import("@tauri-apps/api/core").Channel<ClaudeRunEnvelope>;

export interface ProjectMemory {
  projectPath: string;
  enabled: boolean;
  purpose: string;
  techStack: string;
  rules: string;
  avoid: string;
  testCommand: string;
  preferredLanguage: string;
  updatedAtMs: number;
}

export interface ProjectMemoryInput {
  enabled: boolean;
  purpose: string;
  techStack: string;
  rules: string;
  avoid: string;
  testCommand: string;
  preferredLanguage: string;
}

export type EnvironmentCheckStatus = "ok" | "warning" | "error";

export interface EnvironmentCheck {
  id: string;
  label: string;
  status: EnvironmentCheckStatus;
  summary: string;
  detail: string;
  fixAvailable: boolean;
}

export interface EnvironmentReport {
  checkedAtMs: number;
  checks: EnvironmentCheck[];
}

export interface DiagnosticResult {
  path: string;
  createdAtMs: number;
  includedSections: string[];
}

export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string | null;
  updateAvailable: boolean;
  releaseUrl: string | null;
  installerUrl: string | null;
  message: string;
}

export interface DownloadedUpdate {
  path: string;
  bytes: number;
}
export interface DemoRunResult {
  userId: string;
  fileName: string;
  displayPath: string;
  content: string;
  createdAtMs: number;
}
