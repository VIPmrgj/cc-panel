import { FolderPlus, TerminalSquare } from "lucide-react";
import type {
  AppPreferences,
  ModelStatus,
  SkillInventory,
  SkillOverrideSelection,
  SkillRecord,
} from "../../api/dto";
import { Button } from "../common/Button";
import { ModelControl } from "../models/ModelControl";
import { SkillList } from "../skills/SkillList";

interface Props {
  preferences: AppPreferences;
  model: ModelStatus;
  inventory: SkillInventory;
  selectedIds: Set<string>;
  search: string;
  modelSaving: boolean;
  skillsRefreshing: boolean;
  skillInventoryBusy: boolean;
  onSearch: (value: string) => void;
  onSaveModel: (model: string) => void;
  onClearModel: () => void;
  onChooseProject: () => void;
  onAddRoot: () => void;
  onRefreshSkills: () => void;
  onToggleSkill: (skill: SkillRecord) => void;
  onChangeSkillState: (
    skill: SkillRecord,
    value: Exclude<SkillOverrideSelection, "unknown">,
  ) => void;
  onPreviewSkill: (skill: SkillRecord) => void;
}

export function LeftSidebar({
  preferences,
  model,
  inventory,
  selectedIds,
  search,
  modelSaving,
  skillsRefreshing,
  skillInventoryBusy,
  onSearch,
  onSaveModel,
  onClearModel,
  onChooseProject,
  onAddRoot,
  onRefreshSkills,
  onToggleSkill,
  onChangeSkillState,
  onPreviewSkill,
}: Props) {
  return (
    <aside className="left-sidebar" aria-label="模型与 Skills">
      <header className="brand-header">
        <div className="brand-mark" aria-hidden="true">
          <TerminalSquare size={18} />
        </div>
        <div>
          <strong>CC Panel</strong>
          <span>LOCAL CONTROL</span>
        </div>
      </header>
      <div className="roots-row">
        <Button
          variant="ghost"
          icon={<FolderPlus size={14} />}
          onClick={onChooseProject}
        >
          {preferences.selectedProjectRoot?.label ?? "选择项目"}
        </Button>
        <Button
          variant="ghost"
          className="icon-button"
          icon={<FolderPlus size={14} />}
          aria-label="登记附加目录"
          title="登记附加目录"
          onClick={onAddRoot}
        >
          <span className="sr-only">附加目录</span>
        </Button>
      </div>
      <ModelControl
        model={model}
        saving={modelSaving}
        onSave={onSaveModel}
        onClear={onClearModel}
      />
      <SkillList
        skills={inventory.skills}
        selectedIds={selectedIds}
        search={search}
        refreshing={skillsRefreshing}
        pending={skillInventoryBusy}
        pluginWarning={inventory.pluginWarning}
        onSearch={onSearch}
        onRefresh={onRefreshSkills}
        onToggleSelected={onToggleSkill}
        onChangeState={onChangeSkillState}
        onPreview={onPreviewSkill}
      />
    </aside>
  );
}
