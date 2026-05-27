import { useEffect } from 'react';
import type { RefObject } from 'react';
import {
  getCurrentWindow,
  currentMonitor,
  LogicalSize,
  LogicalPosition,
} from '@tauri-apps/api/window';

// Rough Windows taskbar + title-bar heights (logical px). We anchor by the
// content height, so add the title bar to keep the whole decorated window —
// footer included — above the taskbar.
const TASKBAR = 48;
const TITLEBAR = 36;
const MARGIN = 12;

// Resize the current window's height to fit the referenced content element,
// keeping a fixed width. Re-fits whenever the content's size changes (e.g. an
// onboarding "advanced" form expanding). When `anchorBottomRight` is set (the
// popover), it's also repositioned to the bottom-right corner near the tray so
// it stays anchored as its height changes.
export function useFitWindowHeight(
  ref: RefObject<HTMLElement | null>,
  width: number,
  anchorBottomRight = false,
): void {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    let frame = 0;
    const fit = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(async () => {
        const height = Math.ceil(el.getBoundingClientRect().height);
        if (height <= 0) return;
        const win = getCurrentWindow();
        try {
          await win.setSize(new LogicalSize(width, height));
          if (anchorBottomRight) {
            const monitor = await currentMonitor();
            if (monitor) {
              const scale = monitor.scaleFactor;
              const monW = monitor.size.width / scale;
              const monH = monitor.size.height / scale;
              const monX = monitor.position.x / scale;
              const monY = monitor.position.y / scale;
              const x = Math.round(monX + monW - width - MARGIN);
              const y = Math.round(monY + monH - height - TITLEBAR - TASKBAR - MARGIN);
              await win.setPosition(new LogicalPosition(x, y));
            }
          }
        } catch {
          /* window-size/position permission missing or window closed — ignore */
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
  }, [ref, width, anchorBottomRight]);
}
