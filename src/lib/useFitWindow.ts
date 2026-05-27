import { useEffect } from 'react';
import type { RefObject } from 'react';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';

// Resize the current window's height to fit the referenced content element,
// keeping a fixed width. Re-fits whenever the content's size changes (e.g. an
// onboarding "advanced" form expanding). Used by the decorated onboarding and
// settings windows so they're responsive to their content instead of a fixed box.
export function useFitWindowHeight(ref: RefObject<HTMLElement | null>, width: number): void {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    let frame = 0;
    const fit = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        const height = Math.ceil(el.getBoundingClientRect().height);
        if (height > 0) {
          getCurrentWindow()
            .setSize(new LogicalSize(width, height))
            .catch(() => {
              /* window-size permission missing or window closed — ignore */
            });
        }
      });
    };

    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(el);
    return () => {
      observer.disconnect();
      window.cancelAnimationFrame(frame);
    };
  }, [ref, width]);
}
