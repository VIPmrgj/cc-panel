import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import { Channel } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "./api/commands";
import type {
  AttachmentImportResult,
  AttachmentPreview,
  BootstrapResponse,
  ClaudeRunEnvelope,
  ChatMessage,
  CompositionRequest,
  CompositionResult,
  DiagnosticResult,
  DomesticInstallProgress,
  DemoRunResult,
  DownloadedUpdate,
  EnvironmentReport,
  ConversationSummary,
  ModelProfile,
  ModelProfileInput,
  PendingSensitiveAttachment,
  PermissionRule,
  PermissionRuleRequest,
  SkillOverrideSelection,
  SkillRecord,
  UpdateInfo,
} from "./api/dto";
import { ActivityRail, type ActivityId } from "./components/shell/ActivityRail";
import { InspectorPane } from "./components/shell/InspectorPane";
import { LeftSidebar } from "./components/shell/LeftSidebar";
import {
  getBasicDefaultSkills,
  persistSkillPanelMode,
  readSkillPanelMode,
  type SkillPanelMode,
} from "./components/skills/skillMode";
import { StatusBar } from "./components/shell/StatusBar";
import { SettingsPanel } from "./components/shell/SettingsPanel";
import { SetupCenter } from "./components/setup/SetupCenter";
import { AddModelDialog } from "./components/models/AddModelDialog";
import { ModelManager } from "./components/models/ModelManager";
import { ChatComposer } from "./components/chat/ChatComposer";
import { ChatHeader } from "./components/chat/ChatHeader";
import { ChatTranscript } from "./components/chat/ChatTranscript";
import type { PermissionDecision } from "./components/chat/ChatCards";
import { ConversationPanel } from "./components/chat/ConversationPanel";
import { Button } from "./components/common/Button";
import { Drawer } from "./components/common/Drawer";
import { Notice } from "./components/common/Notice";
import { SensitiveImportDialog } from "./components/common/SensitiveImportDialog";
import { OnboardingDialog } from "./components/onboarding/OnboardingDialog";
import { TaskPanel } from "./components/tasks/TaskPanel";
import { RunCenter } from "./components/runtime/RunCenter";
import type { TaskTemplate } from "./components/tasks/taskTemplates";
import {
  persistExperienceMode,
  persistOnboardingComplete,
  readExperienceMode,
  readOnboardingComplete,
  type ExperienceMode,
} from "./state/experienceMode";
import { classifyPermissionRisk } from "./state/permissionRisk";
import { persistTheme, readTheme, type AppTheme } from "./state/theme";
import { useDragDrop } from "./hooks/useDragDrop";
import {
  chatReducer,
  getChatRunState,
  initialChatState,
} from "./state/chatReducer";
import { composerReducer, initialComposerState } from "./state/composerReducer";
import {
  beginTransitionState,
  finishTransitionState,
  transitionGenerationMatches,
  transitionIsCurrent as isCurrentTransition,
  type TransitionFenceState,
} from "./state/transitionFence";

function apiErrorCode(error: unknown) {
  if (!error || typeof error !== "object") return null;
  const code = (error as { code?: unknown }).code;
  return typeof code === "string" ? code : null;
}

function permissionInputFields(input: unknown) {
  if (!input || typeof input !== "object") {
    return { command: null as string | null, cwd: null as string | null };
  }
  const record = input as Record<string, unknown>;
  const pick = (...keys: string[]) =>
    keys
      .map((key) => record[key])
      .find(
        (value): value is string =>
          typeof value === "string" && value.trim().length > 0,
      )
      ?.trim() ?? null;
  return {
    command: pick("command", "cmd", "script"),
    cwd: pick("cwd", "working_directory", "workingDirectory"),
  };
}

function permissionRuleFor(message: ChatMessage): PermissionRuleRequest {
  const fields = permissionInputFields(message.toolInput);
  return {
    toolName: message.toolName?.trim() || "unknown",
    command: fields.command,
    cwd: fields.cwd,
  };
}

function permissionRuleMatches(
  rule: PermissionRule,
  message: ChatMessage,
): boolean {
  const candidate = permissionRuleFor(message);
  return (
    rule.toolName === candidate.toolName &&
    (rule.command == null || rule.command === candidate.command) &&
    (rule.cwd == null || rule.cwd === candidate.cwd)
  );
}

interface QueuedPrompt {
  id: string;
  composition: CompositionRequest;
}

export default function App() {
  const queryClient = useQueryClient();
  const [composer, dispatchComposer] = useReducer(
    composerReducer,
    initialComposerState,
  );
  const [chat, dispatchChat] = useReducer(chatReducer, initialChatState);
  const [activeActivity, setActiveActivity] = useState<ActivityId>("chat");
  const [experienceMode, setExperienceMode] =
    useState<ExperienceMode>(readExperienceMode);
  const [theme, setTheme] = useState<AppTheme>(readTheme);
  const [panelOpen, setPanelOpen] = useState(true);
  const [search, setSearch] = useState("");
  const [skillMode, setSkillMode] =
    useState<SkillPanelMode>(readSkillPanelMode);
  const [leftDrawerOpen, setLeftDrawerOpen] = useState(false);
  const [rightDrawerOpen, setRightDrawerOpen] = useState(false);
  const [previewedSkill, setPreviewedSkill] = useState<SkillRecord | null>(
    null,
  );
  const [attachmentPreview, setAttachmentPreview] =
    useState<AttachmentPreview | null>(null);
  const [promptPreviewRequest, setPromptPreviewRequest] = useState(0);
  const [sensitiveQueue, setSensitiveQueue] = useState<
    PendingSensitiveAttachment[]
  >([]);
  const [operationMessage, setOperationMessage] = useState("");
  const [liveMessage, setLiveMessage] = useState("");
  const [sending, setSending] = useState(false);
  const [queuedPrompts, setQueuedPrompts] = useState<QueuedPrompt[]>([]);
  const [showFinalPrompt, setShowFinalPrompt] = useState(false);
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  const [claudeSetupBusy, setClaudeSetupBusy] = useState(false);
  const [installProgress, setInstallProgress] =
    useState<DomesticInstallProgress | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<DomesticInstallProgress>(
      "cc-panel://domestic-install-progress",
      (event) => {
        if (!disposed) setInstallProgress(event.payload);
      },
    )
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
  const autoPromptedRef = useRef(false);
  const [transitionBusy, setTransitionBusy] = useState(false);
  const [permissionBusy, setPermissionBusy] = useState(false);
  const [modelDialog, setModelDialog] = useState<ModelProfile | null | false>(
    false,
  );
  const [modelDialogBusy, setModelDialogBusy] = useState(false);
  const [environmentReport, setEnvironmentReport] =
    useState<EnvironmentReport>();
  const [downloadedUpdate, setDownloadedUpdate] = useState<DownloadedUpdate>();
  const [diagnosticResult, setDiagnosticResult] = useState<DiagnosticResult>();
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo>();
  const [environmentRepairId, setEnvironmentRepairId] = useState<string | null>(
    null,
  );

  const composerRef = useRef(composer);
  const chatRef = useRef(chat);
  const activeChannelRef = useRef<Channel<ClaudeRunEnvelope> | null>(null);
  const transitionFenceRef = useRef<TransitionFenceState>({
    generation: 0,
    activeGeneration: null,
  });
  const transitionBusyRef = useRef(false);
  const sendInFlightRef = useRef(false);
  const queuedPromptsRef = useRef<QueuedPrompt[]>([]);
  const lastCompositionRef = useRef<CompositionRequest | null>(null);
  const pendingPermissionRef = useRef<string | null>(null);
  const sessionPermissionRulesRef = useRef<Record<string, PermissionRule[]>>(
    {},
  );
  const userMessageCounter = useRef(0);
  const basicDefaultsAppliedRef = useRef(false);
  composerRef.current = composer;
  chatRef.current = chat;
  queuedPromptsRef.current = queuedPrompts;

  const bootstrapQuery = useQuery({
    queryKey: ["bootstrap"],
    queryFn: commands.getBootstrap,
  });
  const bootstrap = bootstrapQuery.data;

  const basicDefaultSkills = useMemo(
    () => getBasicDefaultSkills(bootstrap?.skills.skills ?? []),
    [bootstrap?.skills.skills],
  );

  const permissionRulesQuery = useQuery({
    queryKey: ["permission-rules"],
    queryFn: commands.listPermissionRules,
  });
  const profilesQuery = useQuery({
    queryKey: ["model-profiles"],
    queryFn: commands.listModelProfiles,
  });
  const conversationsQuery = useQuery({
    queryKey: ["conversations"],
    queryFn: commands.listConversations,
  });
  const projectMemoryQuery = useQuery({
    queryKey: [
      "project-memory",
      bootstrap?.preferences.selectedProjectRoot?.path ?? null,
    ],
    queryFn: commands.getProjectMemory,
    enabled: Boolean(bootstrap?.preferences.selectedProjectRoot),
  });
  const profiles = profilesQuery.data?.profiles ?? [];
  const selectedProfile = profiles.find((profile) => profile.selected) ?? null;

  const claudeOk = Boolean(bootstrap?.claudeCodeVersion);
  const projectOk = Boolean(bootstrap?.preferences.selectedProjectRoot);
  const modelOk = profiles.some(
    (profile) => profile.selected && profile.hasApiKey,
  );
  const runtimeReady = Boolean(
    claudeOk &&
    bootstrap?.nodeVersion &&
    bootstrap?.npmVersion &&
    bootstrap?.npmMirrorConfigured &&
    bootstrap?.gitAvailable,
  );
  const claudeReady =
    runtimeReady && (Boolean(bootstrap?.claudeCodeConfigured) || modelOk);
  const missingPrerequisites = !claudeReady || !projectOk || !modelOk;
  const conversations = useMemo(
    () => conversationsQuery.data ?? [],
    [conversationsQuery.data],
  );

  useEffect(() => {
    if (!bootstrapQuery.isSuccess || basicDefaultsAppliedRef.current) return;
    basicDefaultsAppliedRef.current = true;
    if (skillMode === "basic") {
      dispatchComposer({
        type: "applyBasicDefaults",
        skills: basicDefaultSkills,
      });
    }
  }, [basicDefaultSkills, bootstrapQuery.isSuccess, skillMode]);
  useEffect(() => {
    if (bootstrap?.attachments.length) {
      dispatchComposer({
        type: "addAttachments",
        attachments: bootstrap.attachments,
      });
    }
  }, [bootstrap?.attachments]);

  // 首次启动引导：所有步骤可跳过，完成后可从设置再次打开。
  useEffect(() => {
    if (!bootstrapQuery.isSuccess || !profilesQuery.isSuccess) return;
    if (!readOnboardingComplete() && !autoPromptedRef.current) {
      autoPromptedRef.current = true;
      setOnboardingOpen(true);
    }
  }, [bootstrapQuery.isSuccess, profilesQuery.isSuccess]);

  const updateBootstrap = useCallback(
    (updater: (current: BootstrapResponse) => BootstrapResponse) => {
      queryClient.setQueryData<BootstrapResponse>(["bootstrap"], (current) => {
        if (!current) return current;
        const next = updater(current);
        if (next.skills !== current.skills) {
          dispatchComposer({
            type: "reconcileSkills",
            skills: next.skills.skills,
          });
          setPreviewedSkill((previewed) =>
            previewed
              ? (next.skills.skills.find(
                  (skill) => skill.instanceId === previewed.instanceId,
                ) ?? null)
              : null,
          );
        }
        return next;
      });
    },
    [queryClient],
  );

  const reportError = useCallback((error: unknown) => {
    const message =
      error instanceof Error && error.message
        ? error.message
        : "操作失败，请重试。";
    setOperationMessage(message);
    setLiveMessage(message);
  }, []);

  const runDemoSandbox = useCallback(
    async (userId: string): Promise<DemoRunResult> => {
      try {
        const result = await commands.runDemoSandbox(userId);
        setOperationMessage("演示文件已真实写入桌面。");
        setLiveMessage("演示已完成，可以查看文件内容和预览。");
        return result;
      } catch (error) {
        reportError(error);
        throw error;
      }
    },
    [reportError],
  );

  const installClaudeCode = useCallback(async () => {
    setClaudeSetupBusy(true);
    setInstallProgress({
      step: 1,
      totalSteps: 5,
      phase: "node",
      status: "running",
    });
    setOperationMessage(
      "\u6b63\u5728\u51c6\u5907\u56fd\u5185\u73af\u5883\u2026",
    );
    setLiveMessage(
      "\u5c06\u6309\u7167 Node.js\u3001Git\u3001npm \u955c\u50cf\u3001Claude Code \u548c\u9996\u6b21\u914d\u7f6e\u4f9d\u6b21\u5904\u7406\u3002",
    );
    try {
      await commands.installDomesticEnvironment();
      await bootstrapQuery.refetch();
      setOperationMessage(
        "\u56fd\u5185\u73af\u5883\u5df2\u51c6\u5907\u5b8c\u6210\u3002",
      );
      setLiveMessage(
        "Claude Code \u5df2\u66f4\u65b0\u5e76\u5b8c\u6210\u9996\u6b21\u914d\u7f6e\uff1b\u73b0\u5728\u53ef\u5728 CC Panel \u6a21\u578b\u680f\u914d\u7f6e\u6a21\u578b\u3002",
      );
    } catch (error) {
      reportError(error);
      await bootstrapQuery.refetch().catch(() => undefined);
      setInstallProgress((current) => ({
        step: current?.step ?? 1,
        totalSteps: current?.totalSteps ?? 5,
        phase: current?.phase ?? "node",
        status: "failed",
        message:
          error instanceof Error && error.message
            ? error.message
            : "\u56fd\u5185\u73af\u5883\u51c6\u5907\u5931\u8d25",
      }));
      setLiveMessage(
        "\u56fd\u5185\u73af\u5883\u6ca1\u6709\u5b8c\u6210\uff0c\u5df2\u4fdd\u7559\u5df2\u5b8c\u6210\u7684\u6b65\u9aa4\uff0c\u53ef\u4fee\u590d\u540e\u91cd\u8bd5\u3002",
      );
    } finally {
      setClaudeSetupBusy(false);
    }
  }, [bootstrapQuery, reportError]);

  const openModelConfig = useCallback(() => {
    setActiveActivity("models");
    setPanelOpen(true);
  }, []);
  const recheckClaudeSetup = useCallback(async () => {
    setClaudeSetupBusy(true);
    try {
      await bootstrapQuery.refetch();
      setOperationMessage("已重新检测本机环境。");
      setLiveMessage("环境状态已更新。");
    } catch (error) {
      reportError(error);
    } finally {
      setClaudeSetupBusy(false);
    }
  }, [bootstrapQuery, reportError]);

  const projectMemoryMutation = useMutation({
    mutationFn: commands.saveProjectMemory,
    onSuccess: (memory) => {
      queryClient.setQueryData(["project-memory", memory.projectPath], memory);
      setOperationMessage("项目记忆已保存。");
      setLiveMessage("项目记忆已保存，之后发送的消息会带上项目上下文。");
    },
    onError: reportError,
  });

  const environmentMutation = useMutation({
    mutationFn: commands.runEnvironmentCheck,
    onSuccess: (report) => {
      setEnvironmentReport(report);
      setOperationMessage("环境自检完成。");
    },
    onError: reportError,
  });

  const environmentRepairMutation = useMutation({
    mutationFn: commands.repairEnvironmentCheck,
    onMutate: (id) => setEnvironmentRepairId(id),
    onSuccess: (report) => {
      setEnvironmentReport(report);
      setOperationMessage("环境修复操作已完成，请查看检查结果。");
    },
    onError: reportError,
    onSettled: () => setEnvironmentRepairId(null),
  });

  const updateMutation = useMutation({
    mutationFn: commands.checkForUpdates,
    onSuccess: (info) => {
      setUpdateInfo(info);
      setOperationMessage(info.message);
    },
    onError: reportError,
  });

  const downloadUpdateMutation = useMutation({
    mutationFn: ({ url, version }: { url: string; version: string }) =>
      commands.downloadUpdate(url, version),
    onSuccess: (downloaded) => {
      setDownloadedUpdate(downloaded);
      setOperationMessage("更新安装包已下载到临时目录。");
    },
    onError: reportError,
  });

  const diagnosticMutation = useMutation({
    mutationFn: commands.collectDiagnostics,
    onSuccess: (result) => {
      setDiagnosticResult(result);
      setOperationMessage("诊断包已生成。");
    },
    onError: reportError,
  });
  const deleteConversationMutation = useMutation({
    mutationFn: async (conversation: ConversationSummary) => {
      if (
        chatRef.current.sessionId === conversation.sessionId &&
        chatRef.current.lifecycle !== "disconnected" &&
        chatRef.current.lifecycle !== "exited"
      ) {
        throw new Error("当前正在使用的对话不能删除，请先新建对话。");
      }
      return commands.deleteConversation(conversation.sessionId);
    },
    onSuccess: (next) => {
      queryClient.setQueryData(["conversations"], next);
      setOperationMessage("对话记录已删除。");
      setLiveMessage("对话记录已删除。 ");
    },
    onError: reportError,
  });

  const refreshSkills = useMutation({
    mutationFn: commands.refreshSkills,
    onSuccess: (skills) => {
      updateBootstrap((current) => ({ ...current, skills }));
      setOperationMessage(`已刷新 ${skills.skills.length} 个 Skill。`);
    },
    onError: reportError,
  });

  const skillMutation = useMutation({
    mutationFn: async ({
      skill,
      value,
    }: {
      skill: SkillRecord;
      value: Exclude<SkillOverrideSelection, "unknown">;
    }) => {
      if (!bootstrap) throw new Error("Bootstrap unavailable");
      return commands.setSkillOverride(
        skill.canonicalId,
        value,
        bootstrap.skills.settingsRevision,
      );
    },
    onSuccess: (skills) => {
      updateBootstrap((current) => ({ ...current, skills }));
      setOperationMessage(
        "Skill 状态已保存。现有会话请使用 /reload-plugins 或重启；已载入的上下文不会被移除。",
      );
      setLiveMessage(
        "\u6b63\u5728\u51c6\u5907\u56fd\u5185\u73af\u5883\uff0c\u8bf7\u7a0d\u5019\u3002",
      );
    },
    onError: reportError,
  });
  const skillInventoryBusy = skillMutation.isPending || refreshSkills.isPending;

  const modelMutation = useMutation({
    mutationFn: async ({ value, clear }: { value: string; clear: boolean }) => {
      if (!bootstrap) throw new Error("Bootstrap unavailable");
      return clear
        ? commands.clearUserModel(bootstrap.model.settingsRevision)
        : commands.setUserModel(value, bootstrap.model.settingsRevision);
    },
    onSuccess: (model) => {
      updateBootstrap((current) => ({ ...current, model }));
      setOperationMessage(
        "用户默认模型已安全写入。当前会话的实际模型不受此状态保证。",
      );
      setLiveMessage("用户默认模型已保存。 ");
    },
    onError: reportError,
  });

  const rootsMutation = useMutation({
    mutationFn: async (kind: "project" | "additional") => {
      const selected =
        kind === "project"
          ? await commands.chooseProjectRoot()
          : await commands.chooseAdditionalRoot();
      return selected ? commands.getBootstrap() : null;
    },
    onSuccess: (next) => {
      if (!next) {
        setOperationMessage("已取消目录选择。");
        return;
      }
      queryClient.setQueryData(["bootstrap"], next);
      dispatchComposer({ type: "reconcileSkills", skills: next.skills.skills });
      setPreviewedSkill(null);
      setOperationMessage(
        "目录已登记并重新扫描。附加目录不代表另一个 Claude 进程实际使用了 --add-dir。",
      );
    },
    onError: reportError,
  });

  const enhancementMutation = useMutation({
    mutationFn: async ({ prompt, model }: { prompt: string; model: string }) =>
      commands.enhancePrompt(prompt, model),
    onSuccess: (result, source) => {
      if (composerRef.current.originalPrompt !== source.prompt) {
        setOperationMessage(
          "原始 Prompt 在增强期间已变化，已丢弃过期的 Ollama 结果。",
        );
        return;
      }
      dispatchComposer({ type: "setEnhancedPrompt", value: result.text });
      setOperationMessage(
        `已由本地 ${result.model} 生成增强候选；原文保持不变。`,
      );
      setLiveMessage("Ollama 增强候选已生成。 ");
    },
    onError: reportError,
  });

  const ollamaMutation = useMutation({
    mutationFn: (model: string | null) =>
      commands.saveOllamaPreferences(
        bootstrap?.preferences.ollamaBaseUrl ?? "http://localhost:11434",
        model,
      ),
    onSuccess: (ollama, model) => {
      updateBootstrap((current) => ({
        ...current,
        ollama,
        preferences: { ...current.preferences, ollamaModel: model },
      }));
      setOperationMessage(
        model
          ? "本地 Prompt 优化已选择：" + model + "。"
          : "本地 Prompt 优化已关闭。",
      );
    },
    onError: reportError,
  });

  const absorbImportResult = useCallback((result: AttachmentImportResult) => {
    if (result.imported.length) {
      dispatchComposer({
        type: "addAttachments",
        attachments: result.imported,
      });
    }
    if (result.pendingConfirmation.length) {
      setSensitiveQueue((current) => [
        ...current,
        ...result.pendingConfirmation.filter(
          (item) =>
            !current.some(
              (queued) => queued.confirmationToken === item.confirmationToken,
            ),
        ),
      ]);
    }
    const parts = [];
    if (result.imported.length)
      parts.push(`已导入 ${result.imported.length} 个附件`);
    if (result.pendingConfirmation.length)
      parts.push(`${result.pendingConfirmation.length} 个敏感文件等待确认`);
    if (result.rejected.length)
      parts.push(`${result.rejected.length} 个文件被拒绝`);
    const message = parts.join("，") || "没有导入文件。";
    setOperationMessage(message);
    setLiveMessage(message);
  }, []);

  const importMutation = useMutation({
    mutationFn: (drop?: { grant: string }) =>
      drop
        ? commands.importDroppedAttachments(drop.grant)
        : commands.pickAndImportAttachments(),
    onSuccess: absorbImportResult,
    onError: reportError,
  });

  const confirmSensitiveMutation = useMutation({
    mutationFn: commands.confirmSensitiveImport,
    onSuccess: (attachment, token) => {
      dispatchComposer({ type: "addAttachments", attachments: [attachment] });
      setSensitiveQueue((queue) =>
        queue.filter((item) => item.confirmationToken !== token),
      );
      setLiveMessage(`已确认并导入敏感附件 ${attachment.name}。`);
    },
    onError: reportError,
  });

  const removeAttachmentMutation = useMutation({
    mutationFn: commands.removeAttachment,
    onSuccess: (_, handle) => {
      dispatchComposer({ type: "removeAttachment", handle });
      setAttachmentPreview((current) =>
        current?.attachment.handle === handle ? null : current,
      );
      setLiveMessage("附件已移除。 ");
    },
    onError: reportError,
  });

  const attachmentPreviewMutation = useMutation({
    mutationFn: commands.previewAttachment,
    onSuccess: (preview) => {
      setAttachmentPreview(preview);
      setLiveMessage("附件预览已打开。 ");
    },
    onError: reportError,
  });
  const notificationMutation = useMutation({
    mutationFn: async (enabled: boolean) => {
      if (enabled) {
        let granted = await isPermissionGranted();
        if (!granted) granted = (await requestPermission()) === "granted";
        if (!granted) {
          throw new Error("系统通知权限未获授权，通知开关保持关闭。");
        }
      }
      return commands.setNativeNotificationsEnabled(enabled);
    },
    onSuccess: (enabled) => {
      updateBootstrap((current) => ({
        ...current,
        preferences: {
          ...current.preferences,
          nativeNotificationsEnabled: enabled,
        },
      }));
      const message = enabled ? "系统通知已开启。" : "系统通知已关闭。";
      setOperationMessage(message);
      setLiveMessage(message);
    },
    onError: reportError,
  });

  const compositionRequest = useMemo<CompositionRequest>(
    () => buildCompositionRequest(composer),
    [composer],
  );
  const canCompose = Boolean(
    (composer.useEnhanced
      ? composer.enhancedPrompt?.trim()
      : composer.originalPrompt.trim()) ||
    composer.selectedSkills.length ||
    composer.attachments.length,
  );

  const previewMutation = useMutation({
    mutationFn: ({ request }: { request: CompositionRequest }) =>
      commands.composePreview(request),
    onSuccess: (preview, variables) => {
      dispatchComposer({ type: "setPreview", preview });
      if (
        JSON.stringify(buildCompositionRequest(composerRef.current)) !==
        JSON.stringify(variables.request)
      ) {
        dispatchComposer({ type: "markStale" });
        setOperationMessage(
          "预览已构建，但输入在构建期间发生变化，当前预览已标记为过期。",
        );
      } else {
        setOperationMessage("最终 Prompt 预览已构建。 ");
      }
      setRightDrawerOpen(true);
      setPromptPreviewRequest((value) => value + 1);
      setLiveMessage("最终 Prompt 预览已构建。 ");
    },
    onError: reportError,
  });

  const copyMutation = useMutation({
    mutationFn: ({ request }: { request: CompositionRequest }) =>
      commands.composeAndCopy(request),
    onSuccess: (result, variables) => {
      dispatchComposer({ type: "setPreview", preview: result });
      const inputsChanged =
        JSON.stringify(buildCompositionRequest(composerRef.current)) !==
        JSON.stringify(variables.request);
      if (inputsChanged) dispatchComposer({ type: "markStale" });
      const baseMessage = result.notificationWarning
        ? `最终 Prompt 已复制；${result.notificationWarning}`
        : "最终 Prompt 已复制到剪贴板。";
      const message = inputsChanged
        ? `${baseMessage} 输入在复制期间已变化，预览已标记为过期。`
        : baseMessage;
      setOperationMessage(message);
      setLiveMessage(message);
    },
    onError: reportError,
  });

  const handlePreview = useCallback(() => {
    if (!canCompose || previewMutation.isPending) return;
    previewMutation.mutate({ request: compositionRequest });
  }, [canCompose, compositionRequest, previewMutation]);

  // 「显示最终格式」开关：切到最终格式时若预览缺失或过期，自动重新构建。
  const handleToggleFinalPrompt = useCallback(() => {
    setShowFinalPrompt((current) => {
      const next = !current;
      if (next && (!composer.preview || composer.previewStale)) {
        if (canCompose && !previewMutation.isPending) {
          previewMutation.mutate({ request: compositionRequest });
        }
      }
      return next;
    });
  }, [
    canCompose,
    composer.preview,
    composer.previewStale,
    compositionRequest,
    previewMutation,
  ]);

  useEffect(() => {
    if (
      showFinalPrompt &&
      canCompose &&
      (composer.previewStale || !composer.preview) &&
      !previewMutation.isPending
    ) {
      previewMutation.mutate({ request: compositionRequest });
    }
  }, [
    showFinalPrompt,
    canCompose,
    composer.preview,
    composer.previewStale,
    compositionRequest,
    previewMutation,
  ]);

  const handleCopy = useCallback(() => {
    if (!canCompose || copyMutation.isPending) return;
    copyMutation.mutate({ request: compositionRequest });
  }, [canCompose, compositionRequest, copyMutation]);

  const handleDrop = useCallback(
    (drop: { grant: string }) => {
      if (!importMutation.isPending) importMutation.mutate(drop);
    },
    [importMutation],
  );
  useDragDrop(handleDrop);

  const profileMutation = useMutation({
    mutationFn: ({
      profile,
      promptForApiKey,
    }: {
      profile: ModelProfileInput;
      promptForApiKey: boolean;
    }) =>
      promptForApiKey
        ? commands.promptAndSaveModelProfile(
            profile,
            profilesQuery.data?.revision ?? 0,
          )
        : commands.saveModelProfile(profile, profilesQuery.data?.revision ?? 0),
    onMutate: () => setModelDialogBusy(true),
    onSuccess: (next) => {
      if (!next) {
        setOperationMessage("已取消凭据输入，模型配置未保存。");
        return;
      }
      queryClient.setQueryData(["model-profiles"], next);
      setModelDialog(false);
      setOperationMessage("模型配置已保存。");
      setLiveMessage("模型配置已保存。 ");
    },
    onError: reportError,
    onSettled: () => setModelDialogBusy(false),
  });

  const deleteProfileMutation = useMutation({
    mutationFn: (profileId: string) =>
      commands.deleteModelProfile(profileId, profilesQuery.data?.revision ?? 0),
    onSuccess: (next) => {
      queryClient.setQueryData(["model-profiles"], next);
      setOperationMessage("模型配置已删除。");
      setLiveMessage("模型配置已删除。 ");
    },
    onError: reportError,
  });

  const selectProfileMutation = useMutation({
    mutationFn: async (profileId: string | null) => {
      if (sendInFlightRef.current)
        throw new Error("消息发送中，请稍后切换模型。");
      const generation = beginTransition();
      const current = chatRef.current;
      const shouldFork =
        Boolean(current.sessionId && current.runId) &&
        !["disconnected", "exited", "failed", "timed-out"].includes(
          current.lifecycle,
        );
      const previousProfileId =
        profilesQuery.data?.profiles.find((profile) => profile.selected)?.id ??
        null;
      if (shouldFork) {
        dispatchChat({
          type: "session-leaving",
          generation,
          sessionId: current.sessionId!,
          runId: current.runId!,
        });
        await waitForSessionRelease();
        if (!transitionIsCurrent(generation)) {
          throw new Error("模型切换已被更新的操作取代。");
        }
      }
      let selectedRevision: number | null = null;
      try {
        const next = await commands.selectModelProfile(
          profileId,
          profilesQuery.data?.revision ?? 0,
        );
        selectedRevision = next.revision;
        if (shouldFork) {
          if (!transitionIsCurrent(generation)) {
            throw new Error("模型切换已被更新的操作取代。");
          }
          activeChannelRef.current = null;
          await startSession(
            {
              mode: "fork",
              parentSessionId: current.sessionId!,
              profileId,
              title: "模型分支",
            },
            generation,
          );
        }
        finishTransition(generation);
        return next;
      } catch (error) {
        if (
          shouldFork &&
          selectedRevision !== null &&
          transitionIsCurrent(generation)
        ) {
          try {
            const restored = await commands.restoreModelProfileSelection(
              previousProfileId,
              selectedRevision,
            );
            queryClient.setQueryData(["model-profiles"], restored);
          } catch {
            // Keep the original error; a conflicting rollback is reported by
            // the next profile refresh instead of masking the fork failure.
          }
        }
        finishTransition(generation);
        throw error;
      }
    },
    onSuccess: (next) => {
      queryClient.setQueryData(["model-profiles"], next);
      setOperationMessage("默认模型配置已更新。");
    },
    onError: reportError,
  });

  const modelMutationBusy =
    profilesQuery.isFetching ||
    profileMutation.isPending ||
    deleteProfileMutation.isPending ||
    selectProfileMutation.isPending;

  const beginTransition = useCallback(() => {
    const begun = beginTransitionState(transitionFenceRef.current);
    transitionFenceRef.current = begun.state;
    transitionBusyRef.current = true;
    setTransitionBusy(true);
    return begun.generation;
  }, []);

  const transitionIsCurrent = useCallback(
    (generation: number) =>
      isCurrentTransition(transitionFenceRef.current, generation),
    [],
  );

  const finishTransition = useCallback((generation: number) => {
    const current = transitionFenceRef.current;
    const next = finishTransitionState(current, generation);
    if (next === current) return;
    transitionFenceRef.current = next;
    transitionBusyRef.current = false;
    setTransitionBusy(false);
  }, []);

  const createChannel = useCallback(() => {
    const channel = new Channel<ClaudeRunEnvelope>();
    const pending: ClaudeRunEnvelope[] = [];
    let ready = false;
    channel.onmessage = (envelope) => {
      if (activeChannelRef.current !== channel) return;
      if (!ready) {
        pending.push(envelope);
        return;
      }
      dispatchChat({ type: "envelope", envelope });
    };
    activeChannelRef.current = channel;
    return {
      channel,
      activate() {
        ready = true;
        pending.splice(0).forEach((envelope) => {
          dispatchChat({ type: "envelope", envelope });
        });
      },
    };
  }, []);

  const startSession = useCallback(
    async (
      request: {
        mode: "new" | "resume" | "continue" | "fork" | "retry";
        sessionId?: string | null;
        parentSessionId?: string | null;
        profileId?: string | null;
        title?: string | null;
      },
      generation = beginTransition(),
    ) => {
      if (!transitionIsCurrent(generation)) {
        throw new Error("会话切换已被更新的操作取代。");
      }
      if (request.mode === "new") {
        dispatchChat({ type: "reset", generation });
      } else {
        const current = chatRef.current;
        if (current.sessionId && current.runId) {
          dispatchChat({
            type: "session-leaving",
            generation,
            sessionId: current.sessionId,
            runId: current.runId,
          });
        } else if (current.sessionId && request.mode === "resume") {
          // Lazily-loaded conversation (history shown, no process running yet).
          // Keep the loaded messages visible while the resume spawns instead of
          // wiping them with a reset; session-started attaches the new run.
          dispatchChat({
            type: "session-loading",
            generation,
            sessionId: current.sessionId,
          });
        } else {
          dispatchChat({
            type: "reset",
            generation,
            sessionId: request.sessionId ?? null,
          });
        }
      }
      const stream = createChannel();
      try {
        const session = await commands.startClaudeSession(
          {
            mode: request.mode,
            sessionId: request.sessionId ?? null,
            parentSessionId: request.parentSessionId ?? null,
            profileId: request.profileId ?? null,
            title: request.title ?? null,
          },
          stream.channel,
        );
        if (!transitionIsCurrent(generation)) {
          try {
            await commands.stopClaudeSession(session.sessionId, session.runId);
          } catch {
            // A stale successful start may already have exited. It must never be
            // activated or allowed to occupy the singleton manager.
          }
          throw new Error("会话切换已被更新的操作取代。");
        }
        dispatchChat({
          type: "session-started",
          generation,
          sessionId: session.sessionId,
          runId: session.runId,
        });
        stream.activate();
        finishTransition(generation);
        return session;
      } catch (error) {
        if (activeChannelRef.current === stream.channel) {
          activeChannelRef.current = null;
        }
        finishTransition(generation);
        throw error;
      }
    },
    [beginTransition, createChannel, finishTransition, transitionIsCurrent],
  );

  const stopSession = useCallback(async () => {
    const current = chatRef.current;
    const sessionId = current.sessionId;
    const runId = current.runId;
    if (!sessionId || !runId) return;
    try {
      await commands.stopClaudeSession(sessionId, runId);
      queuedPromptsRef.current = [];
      setQueuedPrompts([]);
      setOperationMessage("已停止当前回合，待发送消息已清除。");
    } catch (error) {
      reportError(error);
    }
  }, [reportError]);

  const removeQueuedPrompt = useCallback((id: string) => {
    const next = queuedPromptsRef.current.filter((item) => item.id !== id);
    queuedPromptsRef.current = next;
    setQueuedPrompts(next);
    setOperationMessage("已取消一条排队消息。");
  }, []);

  const runState = getChatRunState(chat);
  const lifecycleBusy =
    sending ||
    transitionBusy ||
    [
      "starting",
      "thinking",
      "tool-running",
      "awaiting-permission",
      "stopping",
      "stalled",
      "recovering",
    ].includes(runState);

  // A silent turn is not automatically killed: a long-running build or test
  // may legitimately produce no text. We mark it as stalled and offer a safe
  // recovery action instead of replaying a possibly destructive request.
  useEffect(() => {
    const timer = window.setInterval(() => {
      const current = chatRef.current;
      const active =
        Boolean(current.activeTurnId) &&
        (current.turnStatus === "running" ||
          current.turnStatus === "awaiting-permission");
      if (
        !active ||
        current.pendingPermission ||
        current.recoveryStatus === "recovering"
      ) {
        if (current.recoveryStatus !== "none" && !current.pendingPermission) {
          dispatchChat({ type: "recovery-cleared" });
        }
        return;
      }
      const lastEventAt = current.lastEventAt ?? Date.now();
      if (
        Date.now() - lastEventAt >= 45_000 &&
        current.recoveryStatus === "none"
      ) {
        dispatchChat({ type: "recovery-suspected" });
        setOperationMessage(
          "超过 45 秒没有收到新的进度；请在运行中心检查或恢复会话。",
        );
        setLiveMessage("当前回合可能卡住，运行中心提供恢复操作。");
      }
    }, 5_000);
    return () => window.clearInterval(timer);
  }, []);

  const sendMessage = useCallback(
    async (queued?: QueuedPrompt) => {
      const current = chatRef.current;
      const activeTurn =
        current.turnStatus === "running" ||
        current.turnStatus === "awaiting-permission" ||
        Boolean(current.pendingPermission);
      const sentComposer = queued ? null : composerRef.current;

      if (!queued && !sentComposer?.originalPrompt.trim()) return;
      if (missingPrerequisites) {
        setOperationMessage(
          "当前还不能发送，已保留你输入的内容。请先完成环境配置。",
        );
        setLiveMessage(
          "未配置完成的消息不会丢失，请使用连接中心完成安装、登录或配置。",
        );
        return;
      }

      // Claude Code keeps accepting input while it is working. We mirror that
      // behavior in the panel by holding the new composition until the current
      // turn produces its result, rather than writing it into the active turn.
      const canQueue =
        activeTurn ||
        sendInFlightRef.current ||
        transitionBusyRef.current ||
        sending;
      if (!queued && canQueue) {
        const composition = buildCompositionRequest(sentComposer!);
        const item: QueuedPrompt = {
          id: "turn-" + Date.now() + "-" + (userMessageCounter.current + 1),
          composition,
        };
        userMessageCounter.current += 1;
        const next = [...queuedPromptsRef.current, item];
        queuedPromptsRef.current = next;
        setQueuedPrompts(next);
        dispatchComposer({
          type: "clearSentContext",
          sent: sentComposer!,
          defaultSkills: skillMode === "basic" ? basicDefaultSkills : [],
        });
        setOperationMessage("已排队；当前回合完成后自动发送。");
        setLiveMessage("消息已排队，当前回合完成后自动发送。");
        return;
      }

      if (sendInFlightRef.current || transitionBusyRef.current || sending) {
        return;
      }

      const composition =
        queued?.composition ?? buildCompositionRequest(sentComposer!);
      lastCompositionRef.current = composition;
      const turnId =
        queued?.id ??
        "turn-" + Date.now() + "-" + (userMessageCounter.current + 1);
      if (!queued) userMessageCounter.current += 1;

      let expectedGeneration = transitionFenceRef.current.generation;
      sendInFlightRef.current = true;
      setSending(true);
      try {
        let sessionId = chatRef.current.sessionId;
        let runId = chatRef.current.runId;
        const terminal = [
          "disconnected",
          "exited",
          "failed",
          "timed-out",
        ].includes(chatRef.current.lifecycle);
        if (
          !sessionId ||
          !runId ||
          terminal ||
          chatRef.current.processReleased
        ) {
          const generation = beginTransition();
          expectedGeneration = generation;
          const hasLoadedConversation = Boolean(sessionId);
          const loadedProfile =
            conversations.find((c) => c.sessionId === sessionId)?.profileId ??
            selectedProfile?.id ??
            null;
          const session = await startSession(
            hasLoadedConversation
              ? {
                  mode: "resume",
                  sessionId,
                  profileId: loadedProfile,
                  title: null,
                }
              : {
                  mode: "new",
                  profileId: selectedProfile?.id ?? null,
                  title: "新对话",
                },
            generation,
          );
          sessionId = session.sessionId;
          runId = session.runId;
        }
        if (
          !sessionId ||
          !runId ||
          !transitionGenerationMatches(
            transitionFenceRef.current,
            expectedGeneration,
          ) ||
          transitionBusyRef.current
        ) {
          throw new Error("会话已切换，消息未发送。");
        }
        const userMessage: ChatMessage = {
          id: "user-" + turnId,
          role: "user",
          content: "",
          turnId,
          status: "running",
        };
        dispatchChat({
          type: "turn-started",
          sessionId,
          runId,
          turnId,
          message: userMessage,
        });
        let result: CompositionResult;
        try {
          result = await commands.sendClaudeMessage({
            sessionId,
            runId,
            composition,
          });
        } catch (error) {
          const code = apiErrorCode(error);
          const sameSession =
            chatRef.current.sessionId === sessionId &&
            chatRef.current.runId === runId;
          const recoverableDisconnect =
            sameSession &&
            code === "SESSION_NOT_ACTIVE" &&
            !chatRef.current.pendingPermission;
          if (!recoverableDisconnect) throw error;
          dispatchChat({ type: "recovery-started" });
          setOperationMessage("连接已断开，正在恢复会话并重新发送这条消息…");
          setLiveMessage("连接已断开，正在恢复会话。");
          const recoveryGeneration = beginTransition();
          expectedGeneration = recoveryGeneration;
          try {
            const loadedProfile =
              conversations.find((item) => item.sessionId === sessionId)
                ?.profileId ??
              selectedProfile?.id ??
              null;
            const recovered = await startSession(
              {
                mode: "resume",
                sessionId,
                profileId: loadedProfile,
                title: null,
              },
              recoveryGeneration,
            );
            sessionId = recovered.sessionId;
            runId = recovered.runId;
            result = await commands.sendClaudeMessage({
              sessionId,
              runId,
              composition,
            });
            dispatchChat({ type: "recovery-cleared" });
            setOperationMessage("会话已自动恢复，消息已重新发送。");
          } catch (recoveryError) {
            dispatchChat({ type: "recovery-cleared" });
            throw recoveryError;
          }
        }
        const latest = chatRef.current;
        if (
          !transitionGenerationMatches(
            transitionFenceRef.current,
            expectedGeneration,
          ) ||
          transitionBusyRef.current ||
          latest.sessionId !== sessionId ||
          latest.runId !== runId
        ) {
          return;
        }
        const text = compositionText(result);
        dispatchChat({
          type: "turn-message-committed",
          sessionId,
          runId,
          turnId,
          message: { ...userMessage, content: text, status: "complete" },
        });
        if (sentComposer) {
          dispatchComposer({
            type: "clearSentContext",
            sent: sentComposer!,
            defaultSkills: skillMode === "basic" ? basicDefaultSkills : [],
          });
        }
        setOperationMessage(
          queued
            ? "排队消息已发送给 Claude Code。"
            : "消息已发送给 Claude Code。",
        );
        setLiveMessage(
          queued
            ? "排队消息已发送给 Claude Code。 "
            : "消息已发送给 Claude Code。 ",
        );
        void conversationsQuery.refetch();
      } catch (error) {
        const latest = chatRef.current;
        if (
          latest.activeTurnId === turnId &&
          latest.sessionId &&
          latest.runId
        ) {
          dispatchChat({
            type: "turn-failed",
            sessionId: latest.sessionId,
            runId: latest.runId,
            turnId,
            message: error instanceof Error ? error.message : "消息发送失败。",
          });
        }
        reportError(error);
      } finally {
        sendInFlightRef.current = false;
        setSending(false);
      }
    },
    [
      basicDefaultSkills,
      skillMode,
      beginTransition,
      conversations,
      conversationsQuery,
      missingPrerequisites,
      reportError,
      selectedProfile?.id,
      sending,
      startSession,
    ],
  );

  const retryLastTurn = useCallback(() => {
    const composition = lastCompositionRef.current;
    if (!composition || sendInFlightRef.current || transitionBusyRef.current)
      return;
    void sendMessage({
      id: "retry-" + Date.now(),
      composition,
    });
  }, [sendMessage]);
  const runTaskTemplate = useCallback(
    (template: TaskTemplate, details?: string) => {
      const prompt = details?.trim()
        ? template.prompt + "\n\n用户补充信息：\n" + details.trim()
        : template.prompt;
      setActiveActivity("chat");
      setPanelOpen(true);
      const current = chatRef.current;
      const activeTurn =
        current.turnStatus === "running" ||
        current.turnStatus === "awaiting-permission" ||
        Boolean(current.pendingPermission);
      if (activeTurn || sendInFlightRef.current || transitionBusyRef.current) {
        dispatchComposer({ type: "setOriginalPrompt", value: prompt });
        setOperationMessage("当前回合完成后，请发送已放入输入框的任务。");
        return;
      }
      const base = buildCompositionRequest(composerRef.current);
      const queued: QueuedPrompt = {
        id: "task-" + Date.now() + "-" + template.id,
        composition: {
          ...base,
          originalPrompt: prompt,
          enhancedPrompt: null,
          useEnhanced: false,
        },
      };
      void sendMessage(queued);
    },
    [sendMessage],
  );

  useEffect(() => {
    if (
      queuedPrompts.length === 0 ||
      sendInFlightRef.current ||
      transitionBusyRef.current ||
      sending
    ) {
      return;
    }
    const current = chatRef.current;
    if (
      current.turnStatus === "running" ||
      current.turnStatus === "awaiting-permission" ||
      current.pendingPermission ||
      current.lifecycle === "starting" ||
      current.lifecycle === "stopping"
    ) {
      return;
    }
    const queued = queuedPromptsRef.current[0];
    if (!queued) return;
    const rest = queuedPromptsRef.current.slice(1);
    queuedPromptsRef.current = rest;
    setQueuedPrompts(rest);
    void sendMessage(queued);
  }, [
    chat.lifecycle,
    chat.turnStatus,
    queuedPrompts.length,
    sendMessage,
    sending,
    transitionBusy,
  ]);
  const renameConversationMutation = useMutation({
    mutationFn: ({ sessionId, title }: { sessionId: string; title: string }) =>
      commands.renameConversation(sessionId, title),
    onSuccess: (next) => {
      queryClient.setQueryData(["conversations"], next);
      setOperationMessage("对话名称已更新。");
    },
    onError: reportError,
  });
  const favoriteConversationMutation = useMutation({
    mutationFn: (conversation: ConversationSummary) =>
      commands.setConversationFavorite(
        conversation.sessionId,
        !conversation.favorite,
      ),
    onSuccess: (next) => queryClient.setQueryData(["conversations"], next),
    onError: reportError,
  });
  const archiveConversationMutation = useMutation({
    mutationFn: (conversation: ConversationSummary) =>
      commands.setConversationArchived(
        conversation.sessionId,
        !conversation.archived,
      ),
    onSuccess: (next) => {
      queryClient.setQueryData(["conversations"], next);
      setOperationMessage("对话归档状态已更新。");
    },
    onError: reportError,
  });
  const exportConversation = useCallback(
    async (conversation: ConversationSummary) => {
      try {
        const history = await commands.loadConversationHistory(
          conversation.sessionId,
        );
        const markdown = [
          "# " + (conversation.title || "未命名对话"),
          "",
          ...history.messages.map((message) => {
            const role =
              message.role === "user"
                ? "用户"
                : message.role === "assistant"
                  ? "Claude"
                  : "系统";
            return "## " + role + "\n\n" + message.content;
          }),
          "",
        ].join("\n");
        await writeText(markdown);
        setOperationMessage("对话已导出到剪贴板。");
      } catch (error) {
        reportError(error);
      }
    },
    [reportError],
  );
  const handleNewChat = useCallback(() => {
    if (sendInFlightRef.current) return;
    queuedPromptsRef.current = [];
    setQueuedPrompts([]);
    const generation = beginTransition();
    const current = chatRef.current;
    if (
      current.sessionId &&
      !["disconnected", "exited", "failed", "timed-out"].includes(
        current.lifecycle,
      )
    ) {
      const oldSessionId = current.sessionId;
      if (current.runId) {
        dispatchChat({
          type: "session-leaving",
          sessionId: oldSessionId,
          runId: current.runId,
        });
      }
      void commands
        .stopClaudeSession(oldSessionId, current.runId ?? "")
        .catch(reportError)
        .finally(() => {
          if (!transitionIsCurrent(generation)) return;
          activeChannelRef.current = null;
          dispatchChat({ type: "reset", generation });
          finishTransition(generation);
        });
      return;
    }
    activeChannelRef.current = null;
    dispatchChat({ type: "reset", generation });
    finishTransition(generation);
  }, [beginTransition, finishTransition, reportError, transitionIsCurrent]);

  const waitForSessionRelease = useCallback(async () => {
    const current = chatRef.current;
    if (
      !current.sessionId ||
      ["disconnected", "exited", "failed", "timed-out"].includes(
        current.lifecycle,
      )
    ) {
      return;
    }
    if (!current.runId) return;
    await commands.stopClaudeSession(current.sessionId, current.runId);
  }, []);

  const recoverCurrentSession = useCallback(async () => {
    const current = chatRef.current;
    if (
      !current.sessionId ||
      !current.runId ||
      current.recoveryStatus === "recovering"
    ) {
      return;
    }
    dispatchChat({ type: "recovery-started" });
    const generation = beginTransition();
    try {
      if (!current.processReleased) {
        await commands.stopClaudeSession(current.sessionId, current.runId);
      }
      if (!transitionIsCurrent(generation)) return;
      const loadedProfile =
        conversations.find((item) => item.sessionId === current.sessionId)
          ?.profileId ??
        selectedProfile?.id ??
        null;
      await startSession(
        {
          mode: "resume",
          sessionId: current.sessionId,
          profileId: loadedProfile,
          title: null,
        },
        generation,
      );
      dispatchChat({ type: "recovery-cleared" });
      setOperationMessage("会话已恢复，可以继续对话。");
      setLiveMessage("会话已恢复，可以继续发送消息。");
    } catch (error) {
      dispatchChat({ type: "recovery-cleared" });
      reportError(error);
    }
  }, [
    beginTransition,
    conversations,
    reportError,
    selectedProfile?.id,
    startSession,
    transitionIsCurrent,
  ]);

  const handleConversationSelect = useCallback(
    async (conversation: ConversationSummary) => {
      if (sendInFlightRef.current) return;
      queuedPromptsRef.current = [];
      setQueuedPrompts([]);
      const generation = beginTransition();
      try {
        const current = chatRef.current;
        if (current.sessionId && current.runId) {
          dispatchChat({
            type: "session-leaving",
            sessionId: current.sessionId,
            runId: current.runId,
          });
        }
        await waitForSessionRelease();
        if (!transitionIsCurrent(generation)) return;
        activeChannelRef.current = null;
        const history = await commands.loadConversationHistory(
          conversation.sessionId,
        );
        if (!transitionIsCurrent(generation)) return;
        dispatchChat({
          type: "reset",
          generation,
          sessionId: history.sessionId,
        });
        dispatchChat({
          type: "history",
          generation,
          sessionId: history.sessionId,
          messages: history.messages,
        });
        // Lazy resume: only the transcript is loaded here. The CLI process is
        // not spawned until the user actually sends a message, so switching
        // between conversations no longer pays the ~seconds of process teardown
        // + spawn. sendMessage() picks up mode "resume" when it sees a loaded
        // conversation without a live run.
        setOperationMessage("已加载对话历史。");
        finishTransition(generation);
      } catch (error) {
        if (transitionIsCurrent(generation)) {
          finishTransition(generation);
          reportError(error);
        }
      }
    },
    [
      beginTransition,
      finishTransition,
      reportError,
      transitionIsCurrent,
      waitForSessionRelease,
    ],
  );

  const respondToPermission = useCallback(
    async (requestId: string, behavior: PermissionDecision) => {
      const current = chatRef.current;
      const sessionId = current.sessionId;
      const runId = current.runId;
      const pending = current.pendingPermission;
      if (
        !sessionId ||
        !runId ||
        !pending ||
        pending.requestId !== requestId ||
        pendingPermissionRef.current === requestId
      )
        return;

      pendingPermissionRef.current = requestId;
      setPermissionBusy(true);
      const rule = permissionRuleFor(pending);
      const transportBehavior =
        behavior === "deny-interrupt"
          ? "deny-interrupt"
          : behavior === "deny"
            ? "deny"
            : "allow";

      try {
        if (behavior === "session") {
          const rules = sessionPermissionRulesRef.current[sessionId] ?? [];
          sessionPermissionRulesRef.current[sessionId] = [
            ...rules.filter(
              (item) =>
                item.toolName !== rule.toolName ||
                item.command !== rule.command ||
                item.cwd !== rule.cwd,
            ),
            { id: "session-" + requestId, ...rule },
          ];
        } else if (behavior === "always") {
          const next = await commands.savePermissionRule(rule);
          queryClient.setQueryData(["permission-rules"], next);
        }

        dispatchChat({
          type: "permission-response",
          sessionId,
          runId,
          requestId,
          behavior: transportBehavior === "allow" ? "allow" : "deny",
          interrupted: transportBehavior === "deny-interrupt",
        });
        await commands.respondToPermission({
          sessionId,
          runId,
          requestId,
          behavior: transportBehavior,
        });
      } catch (error) {
        const code =
          error && typeof error === "object" && "code" in error
            ? String((error as { code?: unknown }).code ?? "")
            : undefined;
        dispatchChat({
          type: "permission-response-failed",
          sessionId,
          runId,
          requestId,
          code,
        });
        reportError(error);
      } finally {
        if (pendingPermissionRef.current === requestId) {
          pendingPermissionRef.current = null;
        }
        setPermissionBusy(false);
      }
    },
    [queryClient, reportError],
  );

  const retryPermission = useCallback(
    async (requestId: string) => {
      const current = chatRef.current;
      if (
        !current.sessionId ||
        !current.runId ||
        current.pendingPermission?.requestId !== requestId ||
        pendingPermissionRef.current
      ) {
        return;
      }
      pendingPermissionRef.current = requestId;
      setPermissionBusy(true);
      try {
        await commands.retryPermission(
          current.sessionId,
          current.runId,
          requestId,
        );
        dispatchChat({
          type: "permission-retried",
          sessionId: current.sessionId,
          runId: current.runId,
          requestId,
          expiresAt: Date.now() + 120_000,
        });
        setOperationMessage("权限请求已重试，请在倒计时结束前确认。");
      } catch (error) {
        reportError(error);
      } finally {
        if (pendingPermissionRef.current === requestId) {
          pendingPermissionRef.current = null;
        }
        setPermissionBusy(false);
      }
    },
    [reportError],
  );

  useEffect(() => {
    const pending = chat.pendingPermission;
    const sessionId = chat.sessionId;
    if (
      !pending ||
      !sessionId ||
      !chat.runId ||
      pending.permissionExpiresAt === 0 ||
      (pending.permissionExpiresAt != null &&
        pending.permissionExpiresAt <= Date.now())
    )
      return;
    const risk = classifyPermissionRisk(pending);
    const persistent = permissionRulesQuery.data ?? [];
    const sessionRules = sessionPermissionRulesRef.current[sessionId] ?? [];
    const hasSavedRule = [...persistent, ...sessionRules].some((rule) =>
      permissionRuleMatches(rule, pending),
    );
    if (risk.level === "low" || hasSavedRule) {
      void respondToPermission(pending.requestId!, "allow");
    }
  }, [
    chat.pendingPermission,
    chat.sessionId,
    chat.runId,
    permissionRulesQuery.data,
    respondToPermission,
  ]);

  const handleExperienceModeChange = useCallback(
    (nextMode: ExperienceMode) => {
      setExperienceMode(nextMode);
      persistExperienceMode(nextMode);
      const nextSkillMode: SkillPanelMode =
        nextMode === "guided" ? "basic" : "advanced";
      setSkillMode(nextSkillMode);
      persistSkillPanelMode(nextSkillMode);
      if (nextSkillMode === "basic") {
        dispatchComposer({
          type: "applyBasicDefaults",
          skills: basicDefaultSkills,
        });
      }
    },
    [basicDefaultSkills],
  );

  const handleThemeChange = useCallback((nextTheme: AppTheme) => {
    setTheme(nextTheme);
    persistTheme(nextTheme);
  }, []);

  const selectedIds = new Set(
    composer.selectedSkills.map((skill) => skill.instanceId),
  );
  const handleSkillModeChange = useCallback(
    (nextMode: SkillPanelMode) => {
      setSkillMode(nextMode);
      persistSkillPanelMode(nextMode);
      if (nextMode === "basic") {
        dispatchComposer({
          type: "applyBasicDefaults",
          skills: basicDefaultSkills,
        });
      }
    },
    [basicDefaultSkills],
  );
  const sidebar = bootstrap ? (
    <LeftSidebar
      preferences={bootstrap.preferences}
      inventory={bootstrap.skills}
      selectedIds={selectedIds}
      search={search}
      skillsRefreshing={refreshSkills.isPending}
      skillInventoryBusy={skillInventoryBusy}
      skillMode={skillMode}
      onSkillModeChange={handleSkillModeChange}
      onSearch={setSearch}
      onChooseProject={() => rootsMutation.mutate("project")}
      onAddRoot={() => rootsMutation.mutate("additional")}
      onRefreshSkills={() => {
        if (!skillInventoryBusy) refreshSkills.mutate();
      }}
      onToggleSkill={(skill) =>
        dispatchComposer({ type: "toggleSkill", skill })
      }
      onChangeSkillState={(skill, value) => {
        if (!skillInventoryBusy) skillMutation.mutate({ skill, value });
      }}
      onPreviewSkill={(skill) => {
        setLeftDrawerOpen(false);
        setPreviewedSkill(skill);
      }}
    />
  ) : null;

  const inspector = bootstrap ? (
    <InspectorPane
      attachments={composer.attachments}
      attachmentPreview={attachmentPreview}
      attachmentPreviewLoading={attachmentPreviewMutation.isPending}
      promptPreviewRequest={promptPreviewRequest}
      preview={composer.preview}
      previewStale={composer.previewStale}
      ollama={bootstrap.ollama}
      operationMessage={operationMessage}
      nativeNotificationsEnabled={
        bootstrap.preferences.nativeNotificationsEnabled
      }
      notificationSaving={notificationMutation.isPending}
      onPreviewAttachment={(handle) => attachmentPreviewMutation.mutate(handle)}
      onSetNativeNotifications={(enabled) =>
        notificationMutation.mutate(enabled)
      }
      onRemoveAttachment={(handle) => removeAttachmentMutation.mutate(handle)}
      onMoveAttachment={(handle, direction) =>
        dispatchComposer({ type: "moveAttachment", handle, direction })
      }
    />
  ) : null;

  const contextPanel = (() => {
    if (activeActivity === "chat") {
      return (
        <ConversationPanel
          conversations={conversations}
          activeSessionId={chat.sessionId}
          loading={conversationsQuery.isPending}
          onSelect={handleConversationSelect}
          onDelete={(conversation) =>
            deleteConversationMutation.mutate(conversation)
          }
          onNew={handleNewChat}
          onRename={(conversation, title) =>
            renameConversationMutation.mutate({
              sessionId: conversation.sessionId,
              title,
            })
          }
          onFavorite={(conversation) =>
            favoriteConversationMutation.mutate(conversation)
          }
          onArchive={(conversation) =>
            archiveConversationMutation.mutate(conversation)
          }
          onExport={(conversation) => void exportConversation(conversation)}
        />
      );
    }
    if (activeActivity === "tasks") {
      return <TaskPanel busy={lifecycleBusy} onRun={runTaskTemplate} />;
    }
    if (activeActivity === "runtime") {
      return (
        <RunCenter
          chat={chat}
          queuedCount={queuedPrompts.length}
          onStop={() => void stopSession()}
          onRetry={retryLastTurn}
          onRecover={() => void recoverCurrentSession()}
          onOpenPermission={() => {
            setActiveActivity("chat");
            setPanelOpen(false);
          }}
        />
      );
    }
    if (activeActivity === "skills") return sidebar;
    if (activeActivity === "models") {
      return (
        <ModelManager
          profiles={profiles}
          loading={profilesQuery.isPending}
          busy={modelMutationBusy}
          model={bootstrap?.model}
          modelSaving={modelMutation.isPending}
          onSaveModel={(value) => modelMutation.mutate({ value, clear: false })}
          onClearModel={() => modelMutation.mutate({ value: "", clear: true })}
          onAdd={() => setModelDialog(null)}
          onEdit={(profile) => setModelDialog(profile)}
          onSelect={(profileId) => selectProfileMutation.mutate(profileId)}
          onDelete={(profileId) => deleteProfileMutation.mutate(profileId)}
        />
      );
    }
    if (activeActivity === "settings" && bootstrap) {
      return (
        <SettingsPanel
          bootstrap={bootstrap}
          experienceMode={experienceMode}
          theme={theme}
          notificationSaving={notificationMutation.isPending}
          ollamaSaving={ollamaMutation.isPending}
          onExperienceModeChange={handleExperienceModeChange}
          onThemeChange={handleThemeChange}
          onOpenOnboarding={() => setOnboardingOpen(true)}
          onSetNativeNotifications={(enabled) =>
            notificationMutation.mutate(enabled)
          }
          onSelectOllamaModel={(model) => ollamaMutation.mutate(model)}
          projectMemory={projectMemoryQuery.data}
          projectMemorySaving={projectMemoryMutation.isPending}
          onSaveProjectMemory={(input) => projectMemoryMutation.mutate(input)}
          environmentReport={environmentReport}
          environmentLoading={environmentMutation.isPending}
          environmentRepairId={environmentRepairId}
          onRunEnvironmentCheck={() => environmentMutation.mutate()}
          onRepairEnvironmentCheck={(check) =>
            environmentRepairMutation.mutate(check.id)
          }
          onChooseProject={() => rootsMutation.mutate("project")}
          onOpenModels={() => setActiveActivity("models")}
          updateInfo={updateInfo}
          updateLoading={updateMutation.isPending}
          downloadedUpdate={downloadedUpdate}
          downloadLoading={downloadUpdateMutation.isPending}
          diagnosticResult={diagnosticResult}
          diagnosticLoading={diagnosticMutation.isPending}
          onCheckUpdate={() => updateMutation.mutate()}
          onDownloadUpdate={() => {
            if (updateInfo?.installerUrl && updateInfo.latestVersion) {
              downloadUpdateMutation.mutate({
                url: updateInfo.installerUrl,
                version: updateInfo.latestVersion,
              });
            }
          }}
          onLaunchUpdate={() => {
            if (downloadedUpdate)
              void commands
                .launchUpdate(downloadedUpdate.path)
                .catch(reportError);
          }}
          onCollectDiagnostics={() => diagnosticMutation.mutate()}
        />
      );
    }
    return inspector;
  })();

  if (bootstrapQuery.isPending) {
    return (
      <main className="bootstrap-shell">
        <section className="bootstrap-card" aria-labelledby="app-title">
          <p className="eyebrow">CLAUDE CODE LOCAL CONTROL</p>
          <h1 id="app-title">CC Panel</h1>
          <p>正在读取本地配置与 Skill 清单…</p>
        </section>
      </main>
    );
  }

  if (bootstrapQuery.isError || !bootstrap) {
    return (
      <main className="bootstrap-shell">
        <section className="bootstrap-card" aria-labelledby="error-title">
          <p className="eyebrow">INITIALIZATION FAILED</p>
          <h1 id="error-title">CC Panel 无法启动</h1>
          <Notice tone="danger" role="alert">
            {bootstrapQuery.error instanceof Error
              ? bootstrapQuery.error.message
              : "无法读取本地状态。"}
          </Notice>
          <Button variant="primary" onClick={() => bootstrapQuery.refetch()}>
            重试
          </Button>
        </section>
      </main>
    );
  }

  return (
    <div className="app-frame" data-theme={theme}>
      <a className="skip-link" href="#main-content">
        跳到对话编辑器
      </a>
      <div
        className="chat-shell"
        style={
          panelOpen
            ? undefined
            : { gridTemplateColumns: "var(--rail-width) minmax(0, 1fr)" }
        }
      >
        <ActivityRail
          active={activeActivity}
          onChange={(activity) => {
            setActiveActivity(activity);
            setPanelOpen(true);
          }}
          attachmentCount={composer.attachments.length}
          skillCount={bootstrap.skills.skills.length}
        />
        {panelOpen && <aside className="context-panel">{contextPanel}</aside>}
        <main className="chat-workspace" id="main-content">
          <ChatHeader
            title={
              conversations.find(
                (conversation) => conversation.sessionId === chat.sessionId,
              )?.title ?? "新对话"
            }
            profile={selectedProfile}
            status={runState}
            panelOpen={panelOpen}
            onTogglePanel={() => setPanelOpen((open) => !open)}
            onSelectModel={() => {
              setActiveActivity("models");
              setPanelOpen(true);
            }}
            onNewChat={handleNewChat}
          />
          <ChatTranscript
            messages={chat.messages}
            activePermission={chat.pendingPermission}
            permissionBusy={permissionBusy}
            activeTool={chat.activeTool}
            onPermission={respondToPermission}
            onRetryPermission={retryPermission}
          />
          {missingPrerequisites && !onboardingOpen && (
            <SetupCenter
              claudeInstalled={claudeOk}
              claudeAuthenticated={bootstrap.claudeCodeConfigured || modelOk}
              gitAvailable={bootstrap.gitAvailable}

              nodeReady={Boolean(bootstrap.nodeVersion)}
              npmReady={Boolean(bootstrap.npmVersion)}
              npmMirrorConfigured={bootstrap.npmMirrorConfigured}
              projectReady={projectOk}
              modelReady={modelOk}
              busy={claudeSetupBusy}
              installProgress={installProgress}
              onOpenModels={openModelConfig}
              onInstall={() => void installClaudeCode()}

              onRecheck={() => void recheckClaudeSetup()}
              onOpenSetup={() => setOnboardingOpen(true)}
            />
          )}
          <ChatComposer
            value={composer.originalPrompt}
            busy={lifecycleBusy}
            queuedCount={queuedPrompts.length}
            queuedItems={queuedPrompts.map((item) => ({
              id: item.id,
              preview:
                item.composition.originalPrompt
                  .trim()
                  .replace(/\\s+/g, " ")
                  .slice(0, 120) || "空消息",
            }))}
            onRemoveQueued={removeQueuedPrompt}
            sessionActive={Boolean(
              chat.sessionId &&
              !["disconnected", "exited", "failed", "timed-out"].includes(
                chat.lifecycle,
              ),
            )}
            attachments={composer.attachments}
            selectedSkills={composer.selectedSkills}
            ollamaAvailable={bootstrap.ollama.online}
            ollamaSelectedModel={bootstrap.ollama.selectedModel}
            enhancedPrompt={composer.enhancedPrompt}
            useEnhanced={composer.useEnhanced}
            showFinal={showFinalPrompt}
            finalText={composer.preview?.text ?? null}
            enhancing={enhancementMutation.isPending}
            onChange={(value) =>
              dispatchComposer({ type: "setOriginalPrompt", value })
            }
            onUseEnhanced={(value) =>
              dispatchComposer({ type: "setUseEnhanced", value })
            }
            onToggleFinal={handleToggleFinalPrompt}
            onSend={() => void sendMessage()}
            onStop={() => void stopSession()}
            onAddFiles={() => importMutation.mutate(undefined)}
            onEnhance={() => {
              const model = bootstrap.ollama.selectedModel;
              if (model) {
                enhancementMutation.mutate({
                  prompt: composer.originalPrompt,
                  model,
                });
              } else {
                const message =
                  "Ollama 在线但未选择本地模型，无法增强。请在模型配置里选择一个模型。";
                setOperationMessage(message);
                setLiveMessage(message);
              }
            }}
          />
          <div className="chat-composer-extra-actions" aria-label="Prompt 操作">
            <Button
              variant="ghost"
              disabled={!canCompose || previewMutation.isPending}
              onClick={handlePreview}
            >
              {previewMutation.isPending ? "预览中…" : "预览最终 Prompt"}
            </Button>
            <Button
              variant="ghost"
              disabled={!canCompose || copyMutation.isPending}
              onClick={handleCopy}
            >
              {copyMutation.isPending ? "复制中…" : "复制 Prompt"}
            </Button>
          </div>
        </main>
      </div>
      <StatusBar
        project={bootstrap.preferences.selectedProjectRoot?.path ?? null}
        skillCount={bootstrap.skills.skills.length}
        attachmentCount={composer.attachments.length}
        ollamaOnline={bootstrap.ollama.online}
        message={operationMessage}
      />
      <div className="sr-only" aria-live="polite" aria-atomic="true">
        {liveMessage}
      </div>

      <Drawer
        open={leftDrawerOpen}
        title="模型与 Skills"
        className="drawer--left"
        onClose={() => setLeftDrawerOpen(false)}
      >
        {sidebar}
      </Drawer>
      <Drawer
        open={rightDrawerOpen}
        title="附件与预览"
        className="drawer--right"
        onClose={() => setRightDrawerOpen(false)}
      >
        {inspector}
      </Drawer>
      <Drawer
        open={Boolean(previewedSkill)}
        title={
          previewedSkill
            ? `${previewedSkill.displayName} · SKILL.md`
            : "Skill 预览"
        }
        className="drawer--preview"
        onClose={() => setPreviewedSkill(null)}
      >
        {previewedSkill && (
          <div className="skill-preview">
            {previewedSkill.warnings.map((warning) => (
              <Notice tone="warning" key={warning}>
                {warning}
              </Notice>
            ))}
            <p>{previewedSkill.manifestPath}</p>
            <pre>{previewedSkill.manifestPreview}</pre>
          </div>
        )}
      </Drawer>

      <OnboardingDialog
        open={onboardingOpen}
        claudeCliAvailable={claudeOk}
        claudeAuthenticated={bootstrap.claudeCodeConfigured || modelOk}
        gitAvailable={bootstrap.gitAvailable}
        environmentReady={runtimeReady}

        projectLabel={bootstrap.preferences.selectedProjectRoot?.label ?? null}
        modelReady={modelOk}
        experienceMode={experienceMode}
        ollama={bootstrap.ollama}
        busy={rootsMutation.isPending || claudeSetupBusy}
        installProgress={installProgress}
        onRunDemo={runDemoSandbox}
        onOpenDemoFile={commands.openDemoFile}
        ollamaSaving={ollamaMutation.isPending}
        onExperienceModeChange={handleExperienceModeChange}
        onInstallClaude={() => void installClaudeCode()}
        onOpenModelConfig={openModelConfig}
        onRecheckClaude={() => void recheckClaudeSetup()}
        onSelectProject={() => rootsMutation.mutate("project")}
        onAddModel={() => setModelDialog(null)}
        onSelectOllamaModel={(model) => ollamaMutation.mutate(model)}

        onClose={() => {
          persistOnboardingComplete();
          setOnboardingOpen(false);
        }}
      />
      {sensitiveQueue[0] && (
        <SensitiveImportDialog
          attachment={sensitiveQueue[0]}
          busy={confirmSensitiveMutation.isPending}
          onCancel={() => setSensitiveQueue((queue) => queue.slice(1))}
          onConfirm={() =>
            confirmSensitiveMutation.mutate(sensitiveQueue[0].confirmationToken)
          }
        />
      )}
      {modelDialog !== false && (
        <AddModelDialog
          profile={modelDialog}
          busy={modelDialogBusy}
          onClose={() => setModelDialog(false)}
          onSave={(profile, promptForApiKey) =>
            profileMutation.mutate({ profile, promptForApiKey })
          }
        />
      )}
    </div>
  );
}

function buildCompositionRequest(
  composer: typeof initialComposerState,
): CompositionRequest {
  return {
    originalPrompt: composer.originalPrompt,
    enhancedPrompt: composer.enhancedPrompt,
    useEnhanced: composer.useEnhanced,
    selectedSkills: composer.selectedSkills.map((skill) => ({
      instanceId: skill.instanceId,
      manifestHash: skill.manifestHash,
    })),
    attachmentHandles: composer.attachments.map((item) => item.handle),
  };
}

function compositionText(result: CompositionResult) {
  return result.text;
}
