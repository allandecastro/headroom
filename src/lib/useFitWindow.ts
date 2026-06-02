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
        const contentH = Math.ceil(el.getBoundingClientRect().height);
        if (contentH <= 0) return;
        const win = getCurrentWindow();
        try {
          // Resolve the monitor first so we can clamp the window to the usable
          // work area BEFORE sizing — otherwise tall content (e.g. a grown
          // Settings page) makes the window overflow behind the taskbar.
          const monitor = await currentMonitor();
          if (!monitor) {
            await win.setSize(new LogicalSize(width, contentH));
            return;
          }
          const scale = monitor.scaleFactor;
          const monW = monitor.size.width / scale;
          const monH = monitor.size.height / scale;
          const monX = monitor.position.x / scale;
          const monY = monitor.position.y / scale;

          // Never taller than the work area (above the taskbar), with a margin
          // top and bottom. If the content exceeds this, the window scrolls
          // (see the panel's overflow container) rather than clipping the taskbar.
          const maxH = Math.max(0, monH - TASKBAR - TITLEBAR - 2 * MARGIN);
          const height = Math.min(contentH, maxH);
          await win.setSize(new LogicalSize(width, height));

          if (anchorBottomRight) {
            // Popover: pin to the bottom-right corner near the tray.
            const x = Math.round(monX + monW - width - MARGIN);
            const y = Math.round(monY + monH - height - TITLEBAR - TASKBAR - MARGIN);
            await win.setPosition(new LogicalPosition(x, y));
          } else {
            // Settings/onboarding: center within the usable area so the bottom
            // never slips behind the taskbar.
            const usableH = monH - TASKBAR - TITLEBAR;
            const x = Math.round(monX + (monW - width) / 2);
            const y = Math.round(monY + Math.max(MARGIN, (usableH - height) / 2));
            await win.setPosition(new LogicalPosition(x, y));
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
