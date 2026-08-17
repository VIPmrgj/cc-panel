import { invoke } from "@tauri-apps/api/core";
import { normalizeInvokeError } from "./errors";
import type {
  AttachmentImportResult,
  AttachmentRecord,
  BootstrapResponse,
  CompositionRequest,
  CompositionResult,
  CopyResult,
  EnhancePromptResponse,
  ModelStatus,
  OllamaStatus,
  RootEntry,
  SkillInventory,
  SkillOverrideSelection,
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
  importDroppedAttachments: (paths: string[]) =>
    call<AttachmentImportResult>("import_dropped_attachments", { paths }),
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
};
