import { useEffect, useId, useRef } from "react";
import { AlertTriangle, FileWarning } from "lucide-react";
import type { PendingSensitiveAttachment } from "../../api/dto";
import { Button } from "./Button";
import { Notice } from "./Notice";

interface Props {
  attachment: PendingSensitiveAttachment;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

const focusableSelector =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function SensitiveImportDialog({
  attachment,
  busy,
  onCancel,
  onConfirm,
}: Props) {
  const panelRef = useRef<HTMLElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const cancelHandlerRef = useRef(onCancel);
  const busyRef = useRef(busy);
  const titleId = useId();
  const descriptionId = useId();
  cancelHandlerRef.current = onCancel;
  busyRef.current = busy;

  useEffect(() => {
    previousFocus.current = document.activeElement as HTMLElement;
    const frame = document.querySelector<HTMLElement>(".app-frame");
    const dialogBackdrop = panelRef.current?.parentElement;
    const background = Array.from(frame?.children ?? []).filter(
      (element): element is HTMLElement =>
        element instanceof HTMLElement && element !== dialogBackdrop,
    );
    background.forEach((element) => {
      element.inert = true;
    });
    cancelRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      const panel = panelRef.current;
      if (!panel) return;
      if (event.key === "Escape" && !busyRef.current) {
        event.preventDefault();
        cancelHandlerRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = Array.from(
        panel.querySelectorAll<HTMLElement>(focusableSelector),
      );
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && (active === first || !panel.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (
        !event.shiftKey &&
        (active === last || !panel.contains(active))
      ) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      background.forEach((element) => {
        element.inert = false;
      });
      previousFocus.current?.focus();
    };
  }, []);

  return (
    <div className="modal-backdrop">
      <section
        ref={panelRef}
        className="confirmation-modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      >
        <FileWarning size={24} aria-hidden="true" />
        <h2 id={titleId}>确认导入敏感文件</h2>
        <p id={descriptionId}>{attachment.reason}</p>
        <code>{attachment.path}</code>
        <Notice tone="warning">
          <AlertTriangle size={14} aria-hidden="true" />
          文件内容会进入最终 Prompt 和系统剪贴板，但不会持久化。
        </Notice>
        <div className="modal-actions">
          <Button ref={cancelRef} disabled={busy} onClick={onCancel}>
            取消
          </Button>
          <Button variant="danger" busy={busy} onClick={onConfirm}>
            仍然导入
          </Button>
        </div>
      </section>
    </div>
  );
}
