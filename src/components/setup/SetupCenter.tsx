import {
  CheckCircle2,
  CircleAlert,
  Download,
  GitBranch,
  KeyRound,
  LogIn,
  RefreshCw,
  Settings2,
} from "lucide-react";
import { Button } from "../common/Button";

interface Props {
  claudeInstalled: boolean;
  claudeAuthenticated: boolean;
  gitAvailable: boolean;
  projectReady: boolean;
  modelReady: boolean;
  busy?: boolean;
  onInstall: () => void;
  onLogin: () => void;
  onRecheck: () => void;
  onOpenSetup: () => void;
}

export function SetupCenter({
  claudeInstalled,
  claudeAuthenticated,
  gitAvailable,
  projectReady,
  modelReady,
  busy = false,
  onInstall,
  onLogin,
  onRecheck,
  onOpenSetup,
}: Props) {
  const state = !claudeInstalled
    ? "install"
    : !claudeAuthenticated
      ? "login"
      : !projectReady || !modelReady
        ? "configure"
        : "ready";

  if (state === "ready") return null;

  const title =
    state === "install"
      ? "还没有安装 Claude Code"
      : state === "login"
        ? "Claude Code 已安装，还需要登录"
        : "还差一步，就可以开始真实 Agent 对话";
  const description =
    state === "install"
      ? "不用复制命令。点击下面的按钮，CC Panel 会调用 Claude 官方安装流程。"
      : state === "login"
        ? "点击登录后会打开一个专用窗口，并在浏览器中完成登录。"
        : "你可以选择项目目录和默认模型；这两项也可以稍后在设置中完成。";

  return (
    <section
      className="setup-center"
      data-state={state}
      aria-labelledby="setup-center-title"
    >
      <div className="setup-center__heading">
        <span className="setup-center__icon" aria-hidden="true">
          {state === "install" ? (
            <Download size={20} />
          ) : state === "login" ? (
            <LogIn size={20} />
          ) : (
            <Settings2 size={20} />
          )}
        </span>
        <div>
          <p className="panel-eyebrow">连接真实 AGENT</p>
          <h2 id="setup-center-title">{title}</h2>
          <p>{description}</p>
        </div>
        <span className="setup-center__badge">
          {state === "install"
            ? "未安装"
            : state === "login"
              ? "未登录"
              : "待配置"}
        </span>
      </div>

      <div className="setup-center__checks" aria-label="运行条件">
        <SetupCheck
          label="Claude Code"
          ready={claudeInstalled}
          detail={claudeInstalled ? "已安装" : "需要安装"}
          icon={Download}
        />
        <SetupCheck
          label="登录状态"
          ready={claudeAuthenticated}
          detail={claudeAuthenticated ? "已登录" : "需要登录"}
          icon={KeyRound}
        />
        <SetupCheck
          label="Git for Windows"
          ready={gitAvailable}
          detail={gitAvailable ? "已检测" : "建议安装"}
          icon={GitBranch}
        />
        <SetupCheck
          label="项目与模型"
          ready={projectReady && modelReady}
          detail={
            projectReady && modelReady
              ? "已准备"
              : !projectReady
                ? "还未选项目"
                : "还未选模型"
          }
          icon={Settings2}
        />
      </div>

      <div className="setup-center__actions">
        {state === "install" && (
          <Button
            variant="primary"
            busy={busy}
            icon={<Download size={16} />}
            onClick={onInstall}
          >
            一键安装 Claude Code
          </Button>
        )}
        {state === "login" && (
          <Button
            variant="primary"
            busy={busy}
            icon={<LogIn size={16} />}
            onClick={onLogin}
          >
            开始登录
          </Button>
        )}
        {state === "configure" && (
          <Button
            variant="primary"
            icon={<Settings2 size={16} />}
            onClick={onOpenSetup}
          >
            去完成设置
          </Button>
        )}
        <Button
          variant="ghost"
          busy={busy}
          icon={<RefreshCw size={15} />}
          onClick={onRecheck}
        >
          重新检测
        </Button>
      </div>

      <div className="setup-center__footnote">
        <CircleAlert size={15} aria-hidden="true" />
        <span>
          当前输入内容会保留，不会因为未配置完成而丢失。也可以先进入无 API
          的演示模式。
        </span>
      </div>
    </section>
  );
}

function SetupCheck({
  label,
  ready,
  detail,
  icon: Icon,
}: {
  label: string;
  ready: boolean;
  detail: string;
  icon: typeof Download;
}) {
  return (
    <div className="setup-center__check" data-ready={ready || undefined}>
      <Icon size={15} aria-hidden="true" />
      <span>
        <strong>{label}</strong>
        <small>{detail}</small>
      </span>
      {ready && <CheckCircle2 size={15} aria-label="已完成" />}
    </div>
  );
}
