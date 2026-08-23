export interface ProviderPreset {
  value: string;
  label: string;
  baseUrl: string;
  modelId: string;
  note?: string;
}

export interface ProviderGroup {
  group: string;
  items: ProviderPreset[];
}

/**
 * 供应商预设目录（端点取自 cc-switch 的 Claude provider 目录，2026-08 核对）。
 * 第三方中转商的模型 ID 多不固定，默认填 claude-sonnet-5，可在对话框修改。
 */
export const PROVIDER_GROUPS: ProviderGroup[] = [
  {
    group: "官方",
    items: [
      {
        value: "Claude Official",
        label: "ClaudeClaude Official",
        baseUrl: "https://api.anthropic.com",
        modelId: "claude-opus-5",
      },
      {
        value: "Gemini Native",
        label: "Gemini Native",
        baseUrl: "https://generativelanguage.googleapis.com",
        modelId: "gemini-3.6-flash",
      },
      {
        value: "DeepSeek",
        label: "DeepSeek",
        baseUrl: "https://api.deepseek.com/anthropic",
        modelId: "deepseek-v4-pro",
      },
      {
        value: "Zhipu GLM",
        label: "Zhipu GLM",
        baseUrl: "https://open.bigmodel.cn/api/anthropic",
        modelId: "glm-5.1",
      },
      {
        value: "Zhipu GLM en",
        label: "Zhipu GLM en",
        baseUrl: "https://api.z.ai/api/anthropic",
        modelId: "glm-5.1",
      },
      {
        value: "Kimi",
        label: "Kimi",
        baseUrl: "https://api.moonshot.cn/anthropic",
        modelId: "kimi-k2.7-code",
      },
      {
        value: "Kimi For Coding",
        label: "Kimi For Coding",
        baseUrl: "https://api.kimi.com/coding/",
        modelId: "kimi-for-coding",
      },
      {
        value: "MiniMax",
        label: "MiniMax",
        baseUrl: "https://api.minimaxi.com/anthropic",
        modelId: "MiniMax-M2.7",
      },
      {
        value: "MiniMax en",
        label: "MiniMax en",
        baseUrl: "https://api.minimax.io/anthropic",
        modelId: "MiniMax-M2.7",
      },
      {
        value: "OpenRouter",
        label: "OpenRouter",
        baseUrl: "https://openrouter.ai/api",
        modelId: "anthropic/claude-sonnet-5",
      },
      {
        value: "TheRouter",
        label: "TheRouter",
        baseUrl: "https://api.therouter.ai",
        modelId: "anthropic/claude-sonnet-5",
      },
      {
        value: "Novita AI",
        label: "Novita AI",
        baseUrl: "https://api.novita.ai/anthropic",
        modelId: "zai-org/glm-5.1",
      },
      {
        value: "GitHub Copilot",
        label: "GitHub Copilot",
        baseUrl: "https://api.githubcopilot.com",
        modelId: "claude-sonnet-5",
      },
      {
        value: "Codex",
        label: "Codex",
        baseUrl: "https://chatgpt.com/backend-api/codex",
        modelId: "gpt-5.6-sol",
      },
      {
        value: "Nvidia",
        label: "Nvidia",
        baseUrl: "https://integrate.api.nvidia.com",
        modelId: "moonshotai/kimi-k2.5",
      },
      {
        value: "PIPELLM",
        label: "PIPELLM",
        baseUrl: "https://cc-api.pipellm.ai",
        modelId: "claude-opus-5",
      },
      {
        value: "Xiaomi MiMo",
        label: "Xiaomi MiMo",
        baseUrl: "https://api.xiaomimimo.com/anthropic",
        modelId: "mimo-v2.5-pro",
      },
      {
        value: "AWS Bedrock (AKSK)",
        label: "AWS Bedrock（AKSK）",
        baseUrl: "https://bedrock-runtime.us-east-1.amazonaws.com",
        modelId: "global.anthropic.claude-opus-5",
        note: "AWS Bedrock 需要地区与特殊鉴权，密钥方式可能无法直接用。",
      },
      {
        value: "AWS Bedrock (API Key)",
        label: "AWS Bedrock（API Key）",
        baseUrl: "https://bedrock-runtime.us-east-1.amazonaws.com",
        modelId: "global.anthropic.claude-opus-5",
        note: "AWS Bedrock 需要地区与特殊鉴权，密钥方式可能无法直接用。",
      },
    ],
  },
  {
    group: "中转",
    items: [
      {
        value: "Shengsuanyun",
        label: "胜算云",
        baseUrl: "https://router.shengsuanyun.com/api",
        modelId: "anthropic/claude-sonnet-5",
      },
      {
        value: "PatewayAI",
        label: "PatewayAI",
        baseUrl: "https://api.pateway.ai",
        modelId: "claude-sonnet-5",
      },
      {
        value: "AiHubMix",
        label: "AiHubMix",
        baseUrl: "https://aihubmix.com",
        modelId: "claude-sonnet-5",
      },
      {
        value: "DMXAPI",
        label: "DMXAPI",
        baseUrl: "https://www.dmxapi.cn",
        modelId: "claude-sonnet-5",
      },
      {
        value: "PackyCode",
        label: "PackyCode",
        baseUrl: "https://www.packyapi.ai",
        modelId: "claude-sonnet-5",
      },
      {
        value: "ClaudeAPI",
        label: "ClaudeAPI",
        baseUrl: "https://gw.apito.ai",
        modelId: "claude-sonnet-5",
      },
      {
        value: "ClaudeCN",
        label: "ClaudeCN",
        baseUrl: "https://claudecn.top",
        modelId: "claude-sonnet-5",
      },
      {
        value: "RunAPI",
        label: "RunAPI",
        baseUrl: "https://runapi.host",
        modelId: "claude-sonnet-5",
      },
      {
        value: "RelaxyCode",
        label: "RelaxyCode",
        baseUrl: "https://www.relaxycode.com",
        modelId: "claude-sonnet-5",
      },
      {
        value: "Cubence",
        label: "Cubence",
        baseUrl: "https://api.cubence.com",
        modelId: "claude-sonnet-5",
      },
      {
        value: "AIGoCode",
        label: "AIGoCode",
        baseUrl: "https://api.aigocode.app",
        modelId: "claude-sonnet-5",
      },
      {
        value: "RightCode",
        label: "RightCode",
        baseUrl: "https://www.rightapi.ai/claude",
        modelId: "claude-sonnet-5",
      },
      {
        value: "AICodeMirror",
        label: "AICodeMirror",
        baseUrl: "https://api.aicodemirror.ai/api/claudecode",
        modelId: "claude-sonnet-5",
      },
      {
        value: "AICoding",
        label: "AICoding",
        baseUrl: "https://api.aicoding.inc",
        modelId: "claude-sonnet-5",
      },
      {
        value: "CrazyRouter",
        label: "CrazyRouter",
        baseUrl: "https://cn.crazyrouter.com",
        modelId: "claude-sonnet-5",
      },
      {
        value: "SSSAiCode",
        label: "SSSAiCode",
        baseUrl: "https://node-hk.sssaicodeapi.com/api",
        modelId: "claude-sonnet-5",
      },
      {
        value: "Micu",
        label: "Micu",
        baseUrl: "https://www.micuapi.ai",
        modelId: "claude-sonnet-5",
      },
      {
        value: "ETok.ai",
        label: "CTok.ai（ETok）",
        baseUrl: "https://api.etok.ai",
        modelId: "claude-sonnet-5",
      },
      {
        value: "E-FlowCode",
        label: "E-FlowCode",
        baseUrl: "https://e-flowcode.cc",
        modelId: "claude-sonnet-5",
      },
      {
        value: "LionCCAPI",
        label: "LionCCAPI",
        baseUrl: "",
        modelId: "claude-sonnet-5",
        note: "未收录到现成目录，请填写该供应商的 API 地址。",
      },
      {
        value: "LemonData",
        label: "LemonData",
        baseUrl: "https://api.lemondata.cc",
        modelId: "claude-sonnet-5",
      },
    ],
  },
  {
    group: "国内云",
    items: [
      {
        value: "火山 Agent Plan",
        label: "火山 Agent Plan",
        baseUrl: "https://ark.cn-beijing.volces.com/api/plan",
        modelId: "ark-code-latest",
      },
      {
        value: "BytePlus",
        label: "BytePlus",
        baseUrl: "https://ark.ap-southeast.bytepluses.com/api/coding",
        modelId: "ark-code-latest",
      },
      {
        value: "DouBaoSeed",
        label: "DouBaoSeed",
        baseUrl: "https://ark.cn-beijing.volces.com/api/compatible",
        modelId: "doubao-seed-2-1-pro-260628",
      },
      {
        value: "Baidu Qianfan Coding Plan",
        label: "Baidu Qianfan Coding Plan",
        baseUrl: "https://qianfan.baidubce.com/anthropic/coding",
        modelId: "qianfan-code-latest",
      },
      {
        value: "Bailian",
        label: "Bailian",
        baseUrl: "https://dashscope.aliyuncs.com/apps/anthropic",
        modelId: "claude-sonnet-5",
      },
      {
        value: "Bailian For Coding",
        label: "Bailian For Coding",
        baseUrl: "https://coding.dashscope.aliyuncs.com/apps/anthropic",
        modelId: "claude-sonnet-5",
      },
      {
        value: "StepFun",
        label: "StepFun",
        baseUrl: "https://api.stepfun.com/step_plan",
        modelId: "step-3.5-flash-2603",
      },
      {
        value: "StepFun en",
        label: "StepFun en",
        baseUrl: "https://api.stepfun.ai/step_plan",
        modelId: "step-3.5-flash-2603",
      },
      {
        value: "ModelScope",
        label: "ModelScope",
        baseUrl: "https://api-inference.modelscope.cn",
        modelId: "ZhipuAI/GLM-5.2",
      },
      {
        value: "KAT-Coder",
        label: "KAT-Coder",
        baseUrl:
          "https://vanchin.streamlake.ai/api/gateway/v1/endpoints/ENDPOINT_ID/claude-code-proxy",
        modelId: "",
        note: "需替换 ENDPOINT_ID 并填写模型 ID。",
      },
      {
        value: "Longcat",
        label: "Longcat",
        baseUrl: "https://api.longcat.chat/anthropic",
        modelId: "LongCat-2.0",
      },
      {
        value: "BaiLing",
        label: "BaiLing",
        baseUrl: "https://api.tbox.cn/api/anthropic",
        modelId: "Ling-2.5-1T",
      },
      {
        value: "SiliconFlow",
        label: "SiliconFlow",
        baseUrl: "https://api.siliconflow.cn",
        modelId: "Pro/MiniMaxAI/MiniMax-M2.5",
      },
      {
        value: "SiliconFlow en",
        label: "SiliconFlow en",
        baseUrl: "https://api.siliconflow.com",
        modelId: "MiniMaxAI/MiniMax-M3",
      },
      {
        value: "优云智算",
        label: "优云智算",
        baseUrl: "https://api.modelverse.cn",
        modelId: "deepseek-ai/DeepSeek-V3.2-Exp",
      },
      {
        value: "优云智算 Coding Plan",
        label: "优云智算 Coding Plan",
        baseUrl: "https://cp.compshare.cn",
        modelId: "claude-sonnet-5",
      },
    ],
  },
];

export function presetFor(value: string): ProviderPreset | null {
  for (const group of PROVIDER_GROUPS) {
    const found = group.items.find((item) => item.value === value);
    if (found) return found;
  }
  return null;
}

/** OpenAI 风格（或裸域名）路径 → 该厂商的 Anthropic 兼容路径。 */
const OPENAI_TO_ANTHROPIC_PATH: Record<
  string,
  { anthropic: string; openAiPaths: string[] }
> = {
  "api.deepseek.com": {
    anthropic: "https://api.deepseek.com/anthropic",
    openAiPaths: ["", "/", "/v1"],
  },
  "api.z.ai": {
    anthropic: "https://api.z.ai/api/anthropic",
    openAiPaths: ["", "/api/paas/v4"],
  },
  "open.bigmodel.cn": {
    anthropic: "https://open.bigmodel.cn/api/anthropic",
    openAiPaths: ["", "/api/paas/v4"],
  },
  "api.moonshot.ai": {
    anthropic: "https://api.moonshot.ai/anthropic",
    openAiPaths: ["", "/", "/v1"],
  },
  "api.moonshot.cn": {
    anthropic: "https://api.moonshot.cn/anthropic",
    openAiPaths: ["", "/", "/v1"],
  },
};

/**
 * Auto-converts a provider's bare/OpenAI-style API origin to the
 * Anthropic-compatible endpoint used by CC Panel (e.g. `api.deepseek.com` →
 * `api.deepseek.com/anthropic`). cc-switch-style tools accept the bare origin
 * and translate under the hood, so we do the same on save. Unknown hosts are
 * returned untouched.
 */
export function normalizeAnthropicBaseUrl(value: string): {
  url: string;
  normalized: boolean;
} {
  const trimmed = value.trim();
  try {
    const url = new URL(trimmed);
    const host = url.host.toLowerCase();
    const rule = OPENAI_TO_ANTHROPIC_PATH[host];
    if (rule) {
      const path = url.pathname.replace(/\/+$/, "");
      if (rule.openAiPaths.includes(path)) {
        return { url: rule.anthropic, normalized: true };
      }
    }
  } catch {
    // Leave malformed values to the existing form validation.
  }
  return { url: trimmed, normalized: false };
}
