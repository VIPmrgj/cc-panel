import {
  AlertCircle,
  CheckCircle2,
  CircleAlert,
  Download,
  GitBranch,
  KeyRound,
  LoaderCircle,
  RefreshCw,
  Settings2,
} from "lucide-react";
import type { DomesticInstallProgress } from "../../api/dto";
import { Button } from "../common/Button";

const copy = {
  eyebrow: "\u56fd\u5185\u5feb\u901f\u51c6\u5907",
  installTitle: "\u4e00\u952e\u51c6\u5907 Claude Code",
  installDescription:
    "\u4e0d\u7528\u590d\u5236\u547d\u4ee4\u3002CC Panel \u4f1a\u81ea\u52a8\u51c6\u5907 Node.js\u3001Git\u3001npm \u56fd\u5185\u955c\u50cf\u548c Claude Code\u3002",
  configureTitle:
    "\u73af\u5883\u5df2\u51c6\u5907\uff0c\u8fd8\u9700\u8981\u914d\u7f6e\u6a21\u578b",
  configureDescription:
    "\u8bf7\u5728 CC Panel \u7684\u6a21\u578b\u680f\u6dfb\u52a0\u5e76\u9009\u62e9\u4e00\u4e2a\u7b2c\u4e09\u65b9\u6a21\u578b\u3002\u4e0d\u9700\u8981\u53e6\u5916\u5b89\u88c5\u6a21\u578b\u5207\u6362\u5de5\u5177\u3002",
  projectTitle:
    "\u8fd8\u5dee\u4e00\u6b65\uff0c\u5c31\u53ef\u4ee5\u5f00\u59cb\u771f\u5b9e Agent \u5bf9\u8bdd",
  projectDescription:
    "\u9009\u62e9\u9879\u76ee\u76ee\u5f55\u548c\u9ed8\u8ba4\u6a21\u578b\uff1b\u8fd9\u4e24\u9879\u4e5f\u53ef\u4ee5\u7a0d\u540e\u5728\u8bbe\u7f6e\u4e2d\u5b8c\u6210\u3002",
  installBadge: "\u672a\u5b8c\u6210",
  configureBadge: "\u5f85\u914d\u7f6e",
  projectBadge: "\u5f85\u914d\u7f6e",
  claude: "Claude Code",
  git: "Git for Windows",
  model: "\u6a21\u578b\u6765\u6e90",
  project: "\u9879\u76ee\u4e0e\u9ed8\u8ba4\u6a21\u578b",
  installed: "\u5df2\u5b89\u88c5",
  detected: "\u5df2\u68c0\u6d4b",
  configured: "\u5df2\u914d\u7f6e",
  needInstall: "\u9700\u8981\u5b89\u88c5",
  needConfig: "\u9700\u8981\u914d\u7f6e",
  needProject: "\u8fd8\u672a\u9009\u9879\u76ee",
  needModel: "\u8fd8\u672a\u9009\u6a21\u578b",
  installButton: "\u4e00\u952e\u51c6\u5907\u56fd\u5185\u73af\u5883",
  configButton: "\u6253\u5f00\u6a21\u578b\u914d\u7f6e",
  setupButton: "\u53bb\u5b8c\u6210\u8bbe\u7f6e",
  recheck: "\u91cd\u65b0\u68c0\u6d4b",
  note: "\u8f93\u5165\u5185\u5bb9\u4f1a\u4fdd\u7559\uff0c\u4e0d\u4f1a\u56e0\u4e3a\u672a\u914d\u7f6e\u5b8c\u6210\u800c\u4e22\u5931\u3002\u6ca1\u6709 API Key \u65f6\u4e5f\u53ef\u4ee5\u5148\u8fdb\u5165\u6f14\u793a\u6a21\u5f0f\u3002",
};

const installSteps = [
  { phase: "node", label: "Node.js" },
  { phase: "git", label: "Git" },
  { phase: "npm", label: "npm \u56fd\u5185\u955c\u50cf" },
  { phase: "claude", label: "Claude Code" },
  { phase: "onboarding", label: "\u9996\u6b21\u542f\u52a8\u914d\u7f6e" },
] as const;

interface Props {
  claudeInstalled: boolean;
  claudeAuthenticated: boolean;
  gitAvailable: boolean;
  nodeReady?: boolean;
  npmReady?: boolean;
  npmMirrorConfigured?: boolean;
  projectReady: boolean;
  modelReady: boolean;
  busy?: boolean;
  installProgress?: DomesticInstallProgress | null;
  onInstall: () => void;
  onOpenModels: () => void;
  onRecheck: () => void;
  onOpenSetup: () => void;
}

export function SetupCenter({
  claudeInstalled,
  claudeAuthenticated,
  gitAvailable,
  nodeReady = true,
  npmReady = true,
  npmMirrorConfigured = true,
  projectReady,
  modelReady,
  busy = false,
  installProgress = null,
  onInstall,
  onOpenModels,
  onRecheck,
  onOpenSetup,
}: Props) {
  const environmentReady =
    claudeInstalled &&
    gitAvailable &&
    nodeReady &&
    npmReady &&
    npmMirrorConfigured;
  const state = !environmentReady
    ? "install"
    : !claudeAuthenticated
      ? "configure"
      : !projectReady || !modelReady
        ? "project"
        : "ready";
  if (state === "ready") return null;
  const title =
    state === "install"
      ? copy.installTitle
      : state === "configure"
        ? copy.configureTitle
        : copy.projectTitle;
  const description =
    state === "install"
      ? copy.installDescription
      : state === "configure"
        ? copy.configureDescription
        : copy.projectDescription;
  const badge =
    state === "install"
      ? copy.installBadge
      : state === "configure"
        ? copy.configureBadge
        : copy.projectBadge;
  return (
    <section
      className="setup-center"
      data-state={state}
      aria-labelledby="setup-center-title"
      aria-busy={busy || undefined}
    >
      <div className="setup-center__heading">
        <span className="setup-center__icon" aria-hidden="true">
          <Download size={20} />
        </span>
        <div>
          <p className="panel-eyebrow">{copy.eyebrow}</p>
          <h2 id="setup-center-title">{title}</h2>
          <p>{description}</p>
        </div>
        <span className="setup-center__badge">{badge}</span>
      </div>
      {installProgress && (busy || installProgress.status === "failed") && (
        <InstallProgressView progress={installProgress} />
      )}
      <div className="setup-center__checks" aria-label="runtime checks">
        <SetupCheck
          label="Node.js"
          ready={nodeReady}
          detail={nodeReady ? copy.detected : copy.needInstall}
          icon={Download}
        />
        <SetupCheck
          label="npm"
          ready={npmReady}
          detail={npmReady ? copy.detected : copy.needInstall}
          icon={Download}
        />
        <SetupCheck
          label="npm \u56fd\u5185\u955c\u50cf"
          ready={npmMirrorConfigured}
          detail={npmMirrorConfigured ? copy.configured : copy.needConfig}
          icon={Settings2}
        />
        <SetupCheck
          label={copy.claude}
          ready={claudeInstalled}
          detail={claudeInstalled ? copy.installed : copy.needInstall}
          icon={Download}
        />
        <SetupCheck
          label={copy.git}
          ready={gitAvailable}
          detail={gitAvailable ? copy.detected : copy.needInstall}
          icon={GitBranch}
        />
        <SetupCheck
          label={copy.model}
          ready={claudeAuthenticated}
          detail={claudeAuthenticated ? copy.configured : copy.needConfig}
          icon={KeyRound}
        />
        <SetupCheck
          label={copy.project}
          ready={projectReady && modelReady}
          detail={
            projectReady && modelReady
              ? copy.configured
              : !projectReady
                ? copy.needProject
                : copy.needModel
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
            {copy.installButton}
          </Button>
        )}
        {state === "configure" && (
          <Button
            variant="primary"
            icon={<Settings2 size={16} />}
            onClick={onOpenModels}
          >
            {copy.configButton}
          </Button>
        )}
        {state === "project" && (
          <Button
            variant="primary"
            icon={<Settings2 size={16} />}
            onClick={onOpenSetup}
          >
            {copy.setupButton}
          </Button>
        )}
        <Button
          variant="ghost"
          busy={busy}
          icon={<RefreshCw size={15} />}
          onClick={onRecheck}
        >
          {copy.recheck}
        </Button>
      </div>
      <div className="setup-center__footnote">
        <CircleAlert size={15} aria-hidden="true" />
        <span>{copy.note}</span>
      </div>
    </section>
  );
}

export function InstallProgressView({
  progress,
}: {
  progress: DomesticInstallProgress;
}) {
  const complete =
    progress.phase === "complete" && progress.status === "completed";
  const failed = progress.status === "failed";
  const completedSteps = complete
    ? installSteps.length
    : progress.status === "completed"
      ? Math.min(progress.step, installSteps.length)
      : Math.max(progress.step - 1, 0);
  const currentStep = installSteps.find(
    (step) => step.phase === progress.phase,
  );
  const currentLabel = complete
    ? "\u56fd\u5185\u73af\u5883\u5df2\u51c6\u5907\u5b8c\u6210"
    : failed
      ? (currentStep?.label ?? "\u5f53\u524d\u6b65\u9aa4") + "\u5931\u8d25"
      : (currentStep?.label ?? "\u51c6\u5907\u73af\u5883");
  const detail = complete
    ? "\u73af\u5883\u5df2\u5904\u7406\u5b8c\u6bd5\uff0c\u4e0d\u9700\u8981\u91cd\u590d\u5b89\u88c5\u3002"
    : failed
      ? progress.message
        ? "\u539f\u56e0\uff1a" + progress.message
        : "\u8fd9\u4e00\u6b65\u6ca1\u6709\u5b8c\u6210\uff0c\u53ef\u4ee5\u4fee\u590d\u540e\u91cd\u8bd5\u3002"
      : "\u6b63\u5728\u5904\u7406" +
        currentLabel +
        "\uff0c\u8bf7\u4e0d\u8981\u5173\u95ed\u7a97\u53e3\u3002";
  const percent = Math.round((completedSteps / installSteps.length) * 100);
  return (
    <section
      className="setup-progress"
      data-status={failed ? "failed" : complete ? "completed" : "running"}
      role="status"
      aria-live="polite"
      aria-busy={!failed && !complete}
    >
      <div className="setup-progress__heading">
        <span className="setup-progress__icon" aria-hidden="true">
          {failed ? (
            <AlertCircle size={17} />
          ) : complete ? (
            <CheckCircle2 size={17} />
          ) : (
            <LoaderCircle size={17} className="spin" />
          )}
        </span>
        <div>
          <strong>{currentLabel}</strong>
          <small>{detail}</small>
        </div>
        <b>
          {Math.min(
            completedSteps + (failed || complete ? 0 : 1),
            installSteps.length,
          )}
          /{installSteps.length}
        </b>
      </div>
      <div className="setup-progress__track" aria-hidden="true">
        <span style={{ width: String(percent) + "%" }} />
      </div>
      <ol className="setup-progress__steps">
        {installSteps.map((step, index) => {
          const done = complete || completedSteps > index;
          const active = !complete && progress.phase === step.phase;
          return (
            <li
              key={step.phase}
              data-state={
                done
                  ? "done"
                  : active
                    ? failed
                      ? "failed"
                      : "active"
                    : "pending"
              }
            >
              <span aria-hidden="true">
                {done ? (
                  <CheckCircle2 size={13} />
                ) : active && !failed ? (
                  <LoaderCircle size={13} className="spin" />
                ) : (
                  <span className="setup-progress__dot" />
                )}
              </span>
              <span>{step.label}</span>
            </li>
          );
        })}
      </ol>
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
      {ready && <CheckCircle2 size={15} aria-label="completed" />}
    </div>
  );
}
