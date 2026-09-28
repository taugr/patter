import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { native } from "./storage";
import { nextZoom, normalizeZoom } from "./zoom";

export function useZoom(onError: (message: string) => void) {
  const [zoom, setZoom] = useState(() => {
    try {
      return normalizeZoom(Number(localStorage.getItem("patter.zoom")));
    } catch {
      return 100;
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem("patter.zoom", String(zoom));
    } catch {
      /* Session zoom still works. */
    }
    if (native)
      void invoke("set_zoom", { percent: zoom }).catch((e) =>
        onError(String(e)),
      );
    else document.documentElement.style.zoom = String(zoom / 100);
  }, [zoom, onError]);
  useEffect(() => {
    const change = (action: string) =>
      setZoom((value) =>
        action === "reset" ? 100 : nextZoom(value, action === "in" ? 1 : -1),
      );
    const key = (event: KeyboardEvent) => {
      // Native menu accelerators also work when an input or dialog has focus.
      if (!(event.metaKey || event.ctrlKey) || event.altKey) return;
      if (native && !["+", "="].includes(event.key)) return;
      if (!["+", "=", "-", "0"].includes(event.key)) return;
      event.preventDefault();
      change(event.key === "0" ? "reset" : event.key === "-" ? "out" : "in");
    };
    window.addEventListener("keydown", key, true);
    let disposed = false;
    let stop: (() => void) | undefined;
    if (native)
      void listen<string>("patter-zoom", ({ payload }) => change(payload))
        .then((unlisten) => {
          if (disposed) unlisten();
          else stop = unlisten;
        })
        .catch((e) => onError(String(e)));
    return () => {
      disposed = true;
      stop?.();
      window.removeEventListener("keydown", key, true);
    };
  }, [onError]);
  return { zoom, setZoom: (value: number) => setZoom(normalizeZoom(value)) };
}
