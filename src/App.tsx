import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "./api/commands";
import type {
  AttachmentImportResult,
  BootstrapResponse,
  CompositionRequest,
  PendingSensitiveAttachment,
  SkillOverrideSelection,
  SkillRecord,
} from "./api/dto";
import { Button } from "./components/common/Button";
import { Drawer } from "./components/common/Drawer";
import { Notice } from "./components/common/Notice";
import { SensitiveImportDialog } from "./components/common/SensitiveImportDialog";
import { ComposerPane } from "./components/composer/ComposerPane";
import { InspectorPane } from "./components/shell/InspectorPane";
import { LeftSidebar } from "./components/shell/LeftSidebar";
import { StatusBar } from "./components/shell/StatusBar";
import { useCopyShortcut } from "./hooks/useCopyShortcut";
import { useDragDrop } from "./hooks/useDragDrop";
import { composerReducer, initialComposerState } from "./state/composerReducer";

export default function App() {
  const queryClient = useQueryClient();
  const [composer, dispatch] = useReducer(
    composerReducer,
    initialComposerState,
  );
  const [search, setSearch] = useState("");
  const [leftDrawerOpen, setLeftDrawerOpen] = useState(false);
  const [rightDrawerOpen, setRightDrawerOpen] = useState(false);
  const [previewedSkill, setPreviewedSkill] = useState<SkillRecord | null>(
    null,
  );
  const [sensitiveQueue, setSensitiveQueue] = useState<
    PendingSensitiveAttachment[]
  >([]);
  const [operationMessage, setOperationMessage] = useState("");
  const [liveMessage, setLiveMessage] = useState("");
  const composerRef = useRef(composer);
  composerRef.current = composer;

  const bootstrapQuery = useQuery({
    queryKey: ["bootstrap"],
    queryFn: commands.getBootstrap,
  });
  const bootstrap = bootstrapQuery.data;

  useEffect(() => {
    if (bootstrap?.attachments.length) {
      dispatch({ type: "addAttachments", attachments: bootstrap.attachments });
    }
  }, [bootstrap?.attachments]);

  const updateBootstrap = useCallback(
    (updater: (current: BootstrapResponse) => BootstrapResponse) => {
      queryClient.setQueryData<BootstrapResponse>(["bootstrap"], (current) => {
        if (!current) return current;
        const next = updater(current);
        if (next.skills !== current.skills) {
          dispatch({ type: "reconcileSkills", skills: next.skills.skills });
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
      setLiveMessage("Skill 状态已保存。可能需要重新加载 Claude Code。 ");
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
      dispatch({ type: "reconcileSkills", skills: next.skills.skills });
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
      dispatch({ type: "setEnhancedPrompt", value: result.text });
      setOperationMessage(
        `已由本地 ${result.model} 生成增强候选；原文保持不变。`,
      );
      setLiveMessage("Ollama 增强候选已生成。 ");
    },
    onError: reportError,
  });

  const absorbImportResult = useCallback((result: AttachmentImportResult) => {
    if (result.imported.length) {
      dispatch({ type: "addAttachments", attachments: result.imported });
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
    mutationFn: (paths?: string[]) =>
      paths
        ? commands.importDroppedAttachments(paths)
        : commands.pickAndImportAttachments(),
    onSuccess: absorbImportResult,
    onError: reportError,
  });

  const confirmSensitiveMutation = useMutation({
    mutationFn: commands.confirmSensitiveImport,
    onSuccess: (attachment, token) => {
      dispatch({ type: "addAttachments", attachments: [attachment] });
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
      dispatch({ type: "removeAttachment", handle });
      setLiveMessage("附件已移除。 ");
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
    () => ({
      originalPrompt: composer.originalPrompt,
      enhancedPrompt: composer.enhancedPrompt,
      useEnhanced: composer.useEnhanced,
      selectedSkills: composer.selectedSkills.map((skill) => ({
        instanceId: skill.instanceId,
        manifestHash: skill.manifestHash,
      })),
      attachmentHandles: composer.attachments.map((item) => item.handle),
    }),
    [composer],
  );
  const compositionSignature = useMemo(
    () => JSON.stringify(compositionRequest),
    [compositionRequest],
  );
  const canCompose = Boolean(
    (composer.useEnhanced
      ? composer.enhancedPrompt?.trim()
      : composer.originalPrompt.trim()) ||
    composer.selectedSkills.length ||
    composer.attachments.length,
  );

  const previewMutation = useMutation({
    mutationFn: ({
      request,
    }: {
      request: CompositionRequest;
      signature: string;
    }) => commands.composePreview(request),
    onSuccess: (preview, variables) => {
      dispatch({ type: "setPreview", preview });
      if (
        JSON.stringify(buildCompositionRequest(composerRef.current)) !==
        variables.signature
      ) {
        dispatch({ type: "markStale" });
        setOperationMessage(
          "预览已构建，但输入在构建期间发生变化，当前预览已标记为过期。",
        );
      } else {
        setOperationMessage("最终 Prompt 预览已构建。 ");
      }
      setRightDrawerOpen(true);
      setLiveMessage("最终 Prompt 预览已构建。 ");
    },
    onError: reportError,
  });

  const copyMutation = useMutation({
    mutationFn: ({
      request,
    }: {
      request: CompositionRequest;
      signature: string;
    }) => commands.composeAndCopy(request),
    onSuccess: (result, variables) => {
      dispatch({ type: "setPreview", preview: result });
      const inputsChanged =
        JSON.stringify(buildCompositionRequest(composerRef.current)) !==
        variables.signature;
      if (inputsChanged) dispatch({ type: "markStale" });
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
    previewMutation.mutate({
      request: compositionRequest,
      signature: compositionSignature,
    });
  }, [canCompose, compositionRequest, compositionSignature, previewMutation]);

  const handleCopy = useCallback(() => {
    if (!canCompose || copyMutation.isPending) return;
    copyMutation.mutate({
      request: compositionRequest,
      signature: compositionSignature,
    });
  }, [canCompose, compositionRequest, compositionSignature, copyMutation]);
  useCopyShortcut({
    onCopy: handleCopy,
    disabled:
      !canCompose || copyMutation.isPending || Boolean(sensitiveQueue[0]),
  });

  const handleDrop = useCallback(
    (paths: string[]) => {
      if (paths.length && !importMutation.isPending)
        importMutation.mutate(paths);
    },
    [importMutation],
  );
  useDragDrop(handleDrop);

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

  const selectedIds = new Set(
    composer.selectedSkills.map((skill) => skill.instanceId),
  );
  const sidebar = (
    <LeftSidebar
      preferences={bootstrap.preferences}
      model={bootstrap.model}
      inventory={bootstrap.skills}
      selectedIds={selectedIds}
      search={search}
      modelSaving={modelMutation.isPending}
      skillsRefreshing={refreshSkills.isPending}
      skillInventoryBusy={skillInventoryBusy}
      onSearch={setSearch}
      onSaveModel={(value) => modelMutation.mutate({ value, clear: false })}
      onClearModel={() => modelMutation.mutate({ value: "", clear: true })}
      onChooseProject={() => rootsMutation.mutate("project")}
      onAddRoot={() => rootsMutation.mutate("additional")}
      onRefreshSkills={() => {
        if (!skillInventoryBusy) refreshSkills.mutate();
      }}
      onToggleSkill={(skill) => dispatch({ type: "toggleSkill", skill })}
      onChangeSkillState={(skill, value) => {
        if (!skillInventoryBusy) skillMutation.mutate({ skill, value });
      }}
      onPreviewSkill={(skill) => {
        setLeftDrawerOpen(false);
        setPreviewedSkill(skill);
      }}
    />
  );
  const inspector = (
    <InspectorPane
      attachments={composer.attachments}
      preview={composer.preview}
      previewStale={composer.previewStale}
      ollama={bootstrap.ollama}
      operationMessage={operationMessage}
      nativeNotificationsEnabled={
        bootstrap.preferences.nativeNotificationsEnabled
      }
      notificationSaving={notificationMutation.isPending}
      onSetNativeNotifications={(enabled) =>
        notificationMutation.mutate(enabled)
      }
      onRemoveAttachment={(handle) => removeAttachmentMutation.mutate(handle)}
      onMoveAttachment={(handle, direction) =>
        dispatch({ type: "moveAttachment", handle, direction })
      }
    />
  );

  return (
    <div className="app-frame">
      <a className="skip-link" href="#main-content">
        跳到 Prompt 编辑器
      </a>
      <div className="app-grid">
        <div className="desktop-left">{sidebar}</div>
        <ComposerPane
          originalPrompt={composer.originalPrompt}
          enhancedPrompt={composer.enhancedPrompt}
          useEnhanced={composer.useEnhanced}
          selectedSkills={composer.selectedSkills}
          ollama={bootstrap.ollama}
          enhancing={enhancementMutation.isPending}
          composing={previewMutation.isPending}
          copying={copyMutation.isPending}
          canCompose={canCompose}
          onPromptChange={(value) =>
            dispatch({ type: "setOriginalPrompt", value })
          }
          onUseEnhanced={(value) => dispatch({ type: "setUseEnhanced", value })}
          onEnhance={() => {
            const model = bootstrap.ollama.selectedModel;
            if (model) {
              enhancementMutation.mutate({
                prompt: composer.originalPrompt,
                model,
              });
            }
          }}
          onAddFiles={() => importMutation.mutate(undefined)}
          onPreview={handlePreview}
          onCopy={handleCopy}
          onRemoveSkill={(instanceId) =>
            dispatch({ type: "removeSkill", instanceId })
          }
          onOpenLeft={() => setLeftDrawerOpen(true)}
          onOpenRight={() => setRightDrawerOpen(true)}
        />
        <div className="desktop-right">{inspector}</div>
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
