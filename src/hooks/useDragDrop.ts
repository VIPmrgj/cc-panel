import { useEffect, useRef } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

export function useDragDrop(onPaths: (paths: string[]) => void) {
  const handlerRef = useRef(onPaths);
  handlerRef.current = onPaths;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          handlerRef.current(event.payload.paths);
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
