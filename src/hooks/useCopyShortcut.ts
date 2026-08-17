import { useEffect } from "react";

interface Options {
  onCopy: () => void;
  disabled: boolean;
}

export function useCopyShortcut({ onCopy, disabled }: Options) {
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (
        disabled ||
        event.isComposing ||
        event.key !== "Enter" ||
        !event.ctrlKey
      ) {
        return;
      }
      event.preventDefault();
      onCopy();
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [disabled, onCopy]);
}
