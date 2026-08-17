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
  path: string;
  kind: AttachmentKind;
  mime: string;
  rawBytes: number;
  extractedBytes: number;
  sha256: string;
  width?: number | null;
  height?: number | null;
  warnings: string[];
}

export interface PendingSensitiveAttachment {
  confirmationToken: string;
  name: string;
  path: string;
  reason: string;
  rawBytes: number;
}

export interface AttachmentImportResult {
  imported: AttachmentRecord[];
  pendingConfirmation: PendingSensitiveAttachment[];
  rejected: Array<{ path: string; code: string; message: string }>;
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
