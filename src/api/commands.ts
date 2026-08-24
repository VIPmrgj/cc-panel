import { Channel, invoke } from "@tauri-apps/api/core";
import { normalizeInvokeError } from "./errors";
import type {
  AttachmentImportResult,
  AttachmentPreview,
  AttachmentRecord,
  BootstrapResponse,
  ClaudeSession,
  ClaudeRunEnvelope,
  CompositionRequest,
  CompositionResult,
  ConversationHistory,
  DiagnosticResult,
  DemoRunResult,
  DownloadedUpdate,
  EnvironmentReport,
  ProjectMemory,
  ProjectMemoryInput,
  UpdateInfo,
  ConversationSummary,
  CopyResult,
  EnhancePromptResponse,
  ModelProfileInput,
  ModelProfileList,
  ModelStatus,
  OllamaStatus,
  PermissionResponseRequest,
  PermissionRule,
  PermissionRuleRequest,
  RootEntry,
  SendClaudeMessageRequest,
  SkillInventory,
  SkillOverrideSelection,
  StartClaudeSessionRequest,
} from "./dto";

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeInvokeError(error);
  }
}
export const commands = {
  getBootstrap: () => call<BootstrapResponse>("get_bootstrap"),
  installClaudeCode: () => call<void>("install_claude_code"),
  startClaudeLogin: () => call<void>("start_claude_login"),
  installDomesticEnvironment: () => call<void>("install_domestic_environment"),
  startCcSwitch: () => call<void>("start_cc_switch"),
  chooseProjectRoot: () => call<RootEntry | null>("choose_project_root"),
  clearProjectRoot: () => call<void>("clear_project_root"),
  chooseAdditionalRoot: () => call<RootEntry | null>("choose_additional_root"),
  removeAdditionalRoot: (rootId: string) =>
    call<void>("remove_additional_root", { rootId }),
  refreshSkills: () => call<SkillInventory>("refresh_skills"),
  setSkillOverride: (
    canonicalId: string,
    value: Exclude<SkillOverrideSelection, "unknown">,
    settingsRevision: string,
  ) =>
    call<SkillInventory>("set_skill_override", {
      canonicalId,
      value,
      settingsRevision,
    }),
  getModelStatus: () => call<ModelStatus>("get_model_status"),
  setUserModel: (model: string, settingsRevision: string) =>
    call<ModelStatus>("set_user_model", { model, settingsRevision }),
  clearUserModel: (settingsRevision: string) =>
    call<ModelStatus>("clear_user_model", { settingsRevision }),
  getOllamaStatus: () => call<OllamaStatus>("get_ollama_status"),
  saveOllamaPreferences: (baseUrl: string, model: string | null) =>
    call<OllamaStatus>("save_ollama_preferences", { baseUrl, model }),
  enhancePrompt: (prompt: string, model: string) =>
    call<EnhancePromptResponse>("enhance_prompt", { prompt, model }),
  pickAndImportAttachments: () =>
    call<AttachmentImportResult>("pick_and_import_attachments"),
  previewAttachment: (handle: string) =>
    call<AttachmentPreview>("preview_attachment", { handle }),
  importDroppedAttachments: (grant: string) =>
    call<AttachmentImportResult>("import_dropped_attachments", { grant }),
  confirmSensitiveImport: (confirmationToken: string) =>
    call<AttachmentRecord>("confirm_sensitive_import", { confirmationToken }),
  removeAttachment: (handle: string) =>
    call<void>("remove_attachment", { handle }),
  clearAttachments: () => call<void>("clear_attachments"),
  composePreview: (request: CompositionRequest) =>
    call<CompositionResult>("compose_preview", { request }),
  composeAndCopy: (request: CompositionRequest) =>
    call<CopyResult>("compose_and_copy", { request }),
  setNativeNotificationsEnabled: (enabled: boolean) =>
    call<boolean>("set_native_notifications_enabled", { enabled }),

  // Claude session API. Keep the command boundary in one place so the UI
  // never constructs untyped invoke payloads or handles raw IPC errors.
  listModelProfiles: () => call<ModelProfileList>("list_model_profiles"),
  saveModelProfile: (profile: ModelProfileInput, expectedRevision: number) =>
    call<ModelProfileList>("save_model_profile", { profile, expectedRevision }),
  promptAndSaveModelProfile: (
    profile: ModelProfileInput,
    expectedRevision: number,
  ) =>
    call<ModelProfileList | null>("prompt_and_save_model_profile", {
      profile,
      expectedRevision,
    }),
  deleteModelProfile: (profileId: string, expectedRevision: number) =>
    call<ModelProfileList>("delete_model_profile", {
      profileId,
      expectedRevision,
    }),
  selectModelProfile: (profileId: string | null, expectedRevision: number) =>
    call<ModelProfileList>("select_model_profile", {
      profileId,
      expectedRevision,
    }),
  restoreModelProfileSelection: (
    profileId: string | null,
    expectedRevision: number,
  ) =>
    call<ModelProfileList>("restore_model_profile_selection", {
      profileId,
      expectedRevision,
    }),
  listConversations: () => call<ConversationSummary[]>("list_conversations"),
  runDemoSandbox: (userId: string) =>
    call<DemoRunResult>("run_demo_sandbox", { userId }),
  openDemoFile: (fileName: string) =>
    call<void>("open_demo_file", { fileName }),
  deleteConversation: (sessionId: string) =>
    call<ConversationSummary[]>("delete_conversation", { sessionId }),
  startClaudeSession: (
    request: StartClaudeSessionRequest,
    channel: Channel<ClaudeRunEnvelope>,
  ) =>
    call<ClaudeSession>("start_claude_session", {
      request,
      channel,
    }),
  sendClaudeMessage: (request: SendClaudeMessageRequest) =>
    call<CompositionResult>("send_claude_message", { request }),
  stopClaudeSession: (sessionId: string, runId: string) =>
    call<void>("stop_claude_session", { sessionId, runId }),
  respondToPermission: (request: PermissionResponseRequest) =>
    call<void>("respond_to_permission", { request }),
  retryPermission: (sessionId: string, runId: string, requestId: string) =>
    call<void>("retry_permission", { sessionId, runId, requestId }),
  listPermissionRules: () => call<PermissionRule[]>("list_permission_rules"),
  savePermissionRule: (request: PermissionRuleRequest) =>
    call<PermissionRule[]>("save_permission_rule", { request }),
  deletePermissionRule: (ruleId: string) =>
    call<PermissionRule[]>("delete_permission_rule", { ruleId }),
  loadConversationHistory: (conversationId: string) =>
    call<ConversationHistory>("load_conversation_history", {
      request: { conversationId },
    }),
  renameConversation: (sessionId: string, title: string) =>
    call<ConversationSummary[]>("rename_conversation", { sessionId, title }),
  setConversationFavorite: (sessionId: string, favorite: boolean) =>
    call<ConversationSummary[]>("set_conversation_favorite", {
      sessionId,
      favorite,
    }),
  setConversationArchived: (sessionId: string, archived: boolean) =>
    call<ConversationSummary[]>("set_conversation_archived", {
      sessionId,
      archived,
    }),
  getProjectMemory: () => call<ProjectMemory | null>("get_project_memory"),
  saveProjectMemory: (input: ProjectMemoryInput) =>
    call<ProjectMemory>("save_project_memory", { input }),
  runEnvironmentCheck: () => call<EnvironmentReport>("run_environment_check"),
  repairEnvironmentCheck: (checkId: string) =>
    call<EnvironmentReport>("repair_environment_check", { checkId }),
  collectDiagnostics: () => call<DiagnosticResult>("collect_diagnostics"),
  checkForUpdates: () => call<UpdateInfo>("check_for_updates"),
  downloadUpdate: (installerUrl: string, version: string) =>
    call<DownloadedUpdate>("download_update", { installerUrl, version }),
  launchUpdate: (path: string) => call<void>("launch_update", { path }),
};
