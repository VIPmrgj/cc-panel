import { AlertCircle, CheckCircle2, Info, XCircle } from "lucide-react";
import clsx from "clsx";

interface Props {
  tone?: "info" | "success" | "warning" | "danger";
  children: React.ReactNode;
  role?: "status" | "alert";
}

export function Notice({ tone = "info", children, role }: Props) {
  const Icon =
    tone === "success"
      ? CheckCircle2
      : tone === "danger"
        ? XCircle
        : tone === "warning"
          ? AlertCircle
          : Info;
  return (
    <div className={clsx("notice", `notice--${tone}`)} role={role}>
      <Icon size={16} aria-hidden="true" />
      <div>{children}</div>
    </div>
  );
}
