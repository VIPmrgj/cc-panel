import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

interface NativeAttachmentDrop {
  grant: string;
}

export function useDragDrop(onDrop: (drop: NativeAttachmentDrop) => void) {
  const handlerRef = useRef(onDrop);
  handlerRef.current = onDrop;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<NativeAttachmentDrop>("cc-panel://attachment-drop", (event) => {
      const { grant } = event.payload;
      if (typeof grant === "string" && grant.trim()) {
        handlerRef.current({ grant });
      }
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
