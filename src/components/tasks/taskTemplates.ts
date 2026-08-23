import {
  FileSearch,
  FlaskConical,
  FolderKanban,
  Palette,
  Wrench,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

export interface TaskTemplate {
  id: string;
  title: string;
  description: string;
  prompt: string;
  icon: LucideIcon;
}

export const TASK_TEMPLATES: TaskTemplate[] = [
  {
    id: "analyze-project",
    title: "分析项目",
    description: "了解项目技术栈、目录结构和启动方式，不修改文件。",
    prompt:
      "请分析当前项目的技术栈、主要目录、启动方式和关键配置。先不要修改任何文件，用中文给出清晰的项目概览。",
    icon: FolderKanban,
  },
  {
    id: "fix-error",
    title: "修复报错",
    description: "定位一个报错，说明原因并提出尽量小的修复。",
    prompt:
      "请检查当前项目中的报错或明显异常，先定位根因并用中文解释。确认原因后再进行最小范围修复，并说明改动了哪些文件。",
    icon: Wrench,
  },
  {
    id: "write-tests",
    title: "编写测试",
    description: "根据现有代码补充测试，并保持项目原有测试风格。",
    prompt:
      "请先了解当前项目的测试框架和现有测试风格，然后为最需要覆盖的功能补充测试。运行相关测试并用中文总结结果。",
    icon: FlaskConical,
  },
  {
    id: "modify-ui",
    title: "修改界面",
    description: "先理解现有界面，再按描述调整布局或样式。",
    prompt:
      "请先检查当前项目的界面结构、样式组织和运行方式。根据现有设计提出一个小范围界面改进方案，确认后再修改，并说明改动。",
    icon: Palette,
  },
  {
    id: "organize-docs",
    title: "整理文档",
    description: "梳理 README、注释和使用说明，让项目更容易理解。",
    prompt:
      "请检查当前项目的 README、注释和主要使用说明，找出过时或缺失的内容并整理文档。保持技术事实准确，用中文总结修改内容。",
    icon: FileSearch,
  },
];
