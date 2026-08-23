import { Check, KeyRound, Pencil, Plus, Trash2 } from "lucide-react";
import type { ModelProfile, ModelStatus } from "../../api/dto";
import { ModelControl } from "./ModelControl";

interface Props {
  profiles: ModelProfile[];
  loading?: boolean;
  busy?: boolean;
  model?: ModelStatus;
  modelSaving?: boolean;
  onAdd: () => void;
  onEdit: (profile: ModelProfile) => void;
  onSelect: (profileId: string | null) => void;
  onDelete: (profileId: string) => void;
  onSaveModel?: (model: string) => void;
  onClearModel?: () => void;
}

export function ModelManager({
  profiles,
  loading = false,
  busy = false,
  model,
  modelSaving = false,
  onAdd,
  onEdit,
  onSelect,
  onDelete,
  onSaveModel,
  onClearModel,
}: Props) {
  return (
    <section className="model-manager" aria-labelledby="model-manager-title">
      <div className="context-panel__header">
        <div>
          <p className="panel-eyebrow">PROVIDERS</p>
          <h2 id="model-manager-title">模型配置</h2>
        </div>
        <button
          type="button"
          className="panel-icon-button"
          aria-label="添加模型配置"
          onClick={onAdd}
        >
          <Plus size={16} aria-hidden="true" />
        </button>
      </div>
      <p className="model-manager__help">
        API 密钥由系统安全存储；界面不会重新显示完整密钥。
      </p>
      <div className="model-list" aria-busy={loading || undefined}>
        {loading ? (
          <p className="panel-muted">正在读取模型配置…</p>
        ) : profiles.length === 0 ? (
          <div className="panel-empty">
            <KeyRound size={20} aria-hidden="true" />
            <p>尚未配置模型提供商</p>
            <button type="button" onClick={onAdd}>
              添加配置
            </button>
          </div>
        ) : (
          profiles.map((profile) => (
            <article
              className="model-profile"
              data-selected={profile.selected || undefined}
              key={profile.id}
            >
              <button
                type="button"
                className="model-profile__select"
                aria-pressed={profile.selected}
                disabled={busy}
                onClick={() => onSelect(profile.selected ? null : profile.id)}
              >
                <span className="model-profile__check" aria-hidden="true">
                  {profile.selected && <Check size={12} />}
                </span>
                <span className="model-profile__identity">
                  <strong>{profile.providerName}</strong>
                  <code>{profile.modelId}</code>
                </span>
              </button>
              <div className="model-profile__meta">
                <span
                  className={
                    profile.hasApiKey ? "has-secret" : "missing-secret"
                  }
                >
                  {profile.hasApiKey ? "密钥已保存" : "未保存密钥"}
                </span>
                {profile.note && (
                  <span title={profile.note}>{profile.note}</span>
                )}
              </div>
              <div className="model-profile__actions">
                <button
                  type="button"
                  aria-label={`编辑 ${profile.providerName}`}
                  disabled={busy}
                  onClick={() => onEdit(profile)}
                >
                  <Pencil size={13} aria-hidden="true" />
                </button>
                <button
                  type="button"
                  className="danger-icon"
                  aria-label={`删除 ${profile.providerName}`}
                  disabled={busy}
                  onClick={() => onDelete(profile.id)}
                >
                  <Trash2 size={13} aria-hidden="true" />
                </button>
              </div>
            </article>
          ))
        )}
      </div>
      {model && onSaveModel && onClearModel && (
        <ModelControl
          model={model}
          saving={modelSaving}
          onSave={onSaveModel}
          onClear={onClearModel}
        />
      )}
    </section>
  );
}
