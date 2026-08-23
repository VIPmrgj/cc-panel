import type {
  AttachmentRecord,
  CompositionResult,
  SkillRecord,
} from "../api/dto";

export interface ComposerState {
  originalPrompt: string;
  enhancedPrompt: string | null;
  useEnhanced: boolean;
  selectedSkills: SkillRecord[];
  attachments: AttachmentRecord[];
  preview: CompositionResult | null;
  previewStale: boolean;
}

export type ComposerAction =
  | { type: "setOriginalPrompt"; value: string }
  | { type: "setEnhancedPrompt"; value: string | null }
  | { type: "setUseEnhanced"; value: boolean }
  | { type: "toggleSkill"; skill: SkillRecord }
  | { type: "applyBasicDefaults"; skills: SkillRecord[] }
  | { type: "removeSkill"; instanceId: string }
  | { type: "reconcileSkills"; skills: SkillRecord[] }
  | { type: "addAttachments"; attachments: AttachmentRecord[] }
  | { type: "removeAttachment"; handle: string }
  | { type: "moveAttachment"; handle: string; direction: -1 | 1 }
  | { type: "setPreview"; preview: CompositionResult }
  | {
      type: "clearSentContext";
      sent: ComposerState;
      defaultSkills?: SkillRecord[];
    }
  | { type: "markStale" }
  | { type: "reset" };

export const initialComposerState: ComposerState = {
  originalPrompt: "",
  enhancedPrompt: null,
  useEnhanced: false,
  selectedSkills: [],
  attachments: [],
  preview: null,
  previewStale: true,
};

export function composerReducer(
  state: ComposerState,
  action: ComposerAction,
): ComposerState {
  switch (action.type) {
    case "setOriginalPrompt":
      return {
        ...state,
        originalPrompt: action.value,
        enhancedPrompt: null,
        useEnhanced: false,
        previewStale: true,
      };
    case "setEnhancedPrompt":
      return {
        ...state,
        enhancedPrompt: action.value,
        useEnhanced: Boolean(action.value),
        previewStale: true,
      };
    case "setUseEnhanced":
      return {
        ...state,
        useEnhanced: action.value && Boolean(state.enhancedPrompt),
        previewStale: true,
      };
    case "toggleSkill": {
      if (action.skill.overrideState === "off" || !action.skill.manifestHash) {
        return state;
      }
      const selected = state.selectedSkills.some(
        (skill) => skill.instanceId === action.skill.instanceId,
      );
      return {
        ...state,
        selectedSkills: selected
          ? state.selectedSkills.filter(
              (skill) => skill.instanceId !== action.skill.instanceId,
            )
          : [...state.selectedSkills, action.skill],
        previewStale: true,
      };
    }
    case "applyBasicDefaults": {
      const selectedIds = new Set(
        state.selectedSkills.map((skill) => skill.instanceId),
      );
      const additions = action.skills.filter(
        (skill) =>
          !selectedIds.has(skill.instanceId) &&
          skill.overrideState !== "off" &&
          Boolean(skill.manifestHash),
      );
      return additions.length
        ? {
            ...state,
            selectedSkills: [...state.selectedSkills, ...additions],
            previewStale: true,
          }
        : state;
    }
    case "removeSkill":
      return {
        ...state,
        selectedSkills: state.selectedSkills.filter(
          (skill) => skill.instanceId !== action.instanceId,
        ),
        previewStale: true,
      };
    case "reconcileSkills": {
      const current = new Map(
        action.skills.map((skill) => [skill.instanceId, skill] as const),
      );
      const selectedSkills = state.selectedSkills.flatMap((selected) => {
        const skill = current.get(selected.instanceId);
        return skill && skill.overrideState !== "off" && skill.manifestHash
          ? [skill]
          : [];
      });
      const changed =
        selectedSkills.length !== state.selectedSkills.length ||
        selectedSkills.some(
          (skill, index) =>
            skill.manifestHash !== state.selectedSkills[index]?.manifestHash ||
            skill.overrideState !== state.selectedSkills[index]?.overrideState,
        );
      return changed ? { ...state, selectedSkills, previewStale: true } : state;
    }
    case "addAttachments": {
      const handles = new Set(state.attachments.map((item) => item.handle));
      const added = action.attachments.filter(
        (item) => !handles.has(item.handle),
      );
      return {
        ...state,
        attachments: [...state.attachments, ...added],
        previewStale: true,
      };
    }
    case "removeAttachment":
      return {
        ...state,
        attachments: state.attachments.filter(
          (attachment) => attachment.handle !== action.handle,
        ),
        previewStale: true,
      };
    case "moveAttachment": {
      const index = state.attachments.findIndex(
        (attachment) => attachment.handle === action.handle,
      );
      const target = index + action.direction;
      if (index < 0 || target < 0 || target >= state.attachments.length) {
        return state;
      }
      const attachments = [...state.attachments];
      [attachments[index], attachments[target]] = [
        attachments[target],
        attachments[index],
      ];
      return { ...state, attachments, previewStale: true };
    }
    case "setPreview":
      return { ...state, preview: action.preview, previewStale: false };
    case "markStale":
      return { ...state, previewStale: true };
    case "clearSentContext": {
      const promptUnchanged =
        state.originalPrompt === action.sent.originalPrompt;
      const sentSkillIds = new Set(
        action.sent.selectedSkills.map((skill) => skill.instanceId),
      );
      const sentAttachmentHandles = new Set(
        action.sent.attachments.map((attachment) => attachment.handle),
      );
      const retainedSkills = state.selectedSkills.filter(
        (skill) => !sentSkillIds.has(skill.instanceId),
      );
      const retainedSkillIds = new Set(
        retainedSkills.map((skill) => skill.instanceId),
      );
      return {
        ...state,
        originalPrompt: promptUnchanged ? "" : state.originalPrompt,
        enhancedPrompt: promptUnchanged ? null : state.enhancedPrompt,
        useEnhanced: promptUnchanged ? false : state.useEnhanced,
        selectedSkills: [
          ...retainedSkills,
          ...(action.defaultSkills ?? []).filter(
            (skill) =>
              !retainedSkillIds.has(skill.instanceId) &&
              skill.overrideState !== "off" &&
              Boolean(skill.manifestHash),
          ),
        ],
        attachments: state.attachments.filter(
          (attachment) => !sentAttachmentHandles.has(attachment.handle),
        ),
        preview: null,
        previewStale: true,
      };
    }
    case "reset":
      return initialComposerState;
  }
}
