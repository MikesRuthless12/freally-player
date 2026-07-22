import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCallback, useEffect, useState } from "react";

import { useT } from "../i18n";

/**
 * The app's own title bar. The window is borderless (`decorations: false`), so minimise,
 * maximise/restore, close and dragging are ours to provide.
 *
 * Dragging works through `data-tauri-drag-region` on the bar itself; the buttons and the
 * right-hand actions opt out so a click on them never starts a drag. Resizing still comes
 * from the OS — a borderless Tauri window keeps its resize border as long as it stays
 * `resizable`, so all four edges and corners drag as usual.
 */
export function TitleBar({
  title,
  showActions = true,
  onOpenSettings,
  onOpenAbout,
}: {
  title: string;
  /** Settings and About are hidden behind the first-run gate, where nothing is usable yet. */
  showActions?: boolean;
  onOpenSettings: () => void;
  onOpenAbout: () => void;
}) {
  const t = useT();
  const [maximized, setMaximized] = useState(false);

  // Keep the maximise/restore glyph honest when the window changes by any other route —
  // a double-click on the bar, the OS shortcut, or snapping.
  useEffect(() => {
    const window = getCurrentWindow();
    let cancelled = false;

    const sync = () => {
      window
        .isMaximized()
        .then((value) => !cancelled && setMaximized(value))
        .catch(() => {});
    };
    sync();

    const unlisten = window.onResized(sync);
    return () => {
      cancelled = true;
      void unlisten.then((off) => off()).catch(() => {});
    };
  }, []);

  const minimize = useCallback(() => {
    void getCurrentWindow().minimize();
  }, []);

  const toggleMaximize = useCallback(() => {
    const window = getCurrentWindow();
    void (maximized ? window.unmaximize() : window.maximize());
  }, [maximized]);

  const close = useCallback(() => {
    void getCurrentWindow().close();
  }, []);

  const action =
    "grid h-8 w-8 place-items-center rounded text-havoc-muted transition-colors hover:bg-havoc-surface hover:text-havoc-text";
  const control =
    "grid h-8 w-11 place-items-center text-havoc-muted transition-colors hover:bg-havoc-surface hover:text-havoc-text";

  return (
    <header
      data-tauri-drag-region
      className="relative flex h-9 shrink-0 items-center justify-end border-b border-havoc-border bg-havoc-panel ps-3 select-none"
    >
      {/* Centred independently of the buttons, so it stays centred in the window rather
          than in whatever space the controls leave over. */}
      <span
        data-tauri-drag-region
        className="pointer-events-none absolute inset-x-0 text-center text-xs font-semibold tracking-wide text-havoc-text"
      >
        {title}
      </span>

      {showActions && (
        <div className="z-10 flex items-center gap-1 pe-1">
          <button
            type="button"
            onClick={onOpenSettings}
            aria-label={t("titlebar-settings")}
            title={t("titlebar-settings")}
            className={action}
          >
            <GearIcon />
          </button>
          <button
            type="button"
            onClick={onOpenAbout}
            aria-label={t("titlebar-about")}
            title={t("titlebar-about")}
            className={action}
          >
            <InfoIcon />
          </button>
        </div>
      )}

      <div className="z-10 flex items-center">
        <button
          type="button"
          onClick={minimize}
          aria-label={t("titlebar-minimize")}
          className={control}
        >
          <svg viewBox="0 0 12 12" className="h-3 w-3" aria-hidden="true">
            <path d="M2 6h8" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
        <button
          type="button"
          onClick={toggleMaximize}
          aria-label={maximized ? t("titlebar-restore") : t("titlebar-maximize")}
          className={control}
        >
          {maximized ? (
            <svg viewBox="0 0 12 12" className="h-3 w-3" aria-hidden="true" fill="none">
              <path d="M3.5 3.5V2.5h6v6h-1" stroke="currentColor" strokeWidth="1.2" />
              <rect x="2.5" y="3.5" width="6" height="6" stroke="currentColor" strokeWidth="1.2" />
            </svg>
          ) : (
            <svg viewBox="0 0 12 12" className="h-3 w-3" aria-hidden="true" fill="none">
              <rect x="2.5" y="2.5" width="7" height="7" stroke="currentColor" strokeWidth="1.2" />
            </svg>
          )}
        </button>
        <button
          type="button"
          onClick={close}
          aria-label={t("titlebar-close")}
          className="grid h-8 w-11 place-items-center text-havoc-muted transition-colors hover:bg-red-600 hover:text-white"
        >
          <svg viewBox="0 0 12 12" className="h-3 w-3" aria-hidden="true">
            <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
      </div>
    </header>
  );
}

function GearIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4 w-4" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="3.2" stroke="currentColor" strokeWidth="1.6" />
      <path
        d="M12 2.8v2.1M12 19.1v2.1M21.2 12h-2.1M4.9 12H2.8M18.5 5.5l-1.5 1.5M7 17l-1.5 1.5M18.5 18.5L17 17M7 7L5.5 5.5"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
      />
    </svg>
  );
}

function InfoIcon() {
  return (
    <svg viewBox="0 0 24 24" className="h-4 w-4" fill="none" aria-hidden="true">
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.6" />
      <path d="M12 11v5.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      <circle cx="12" cy="7.8" r="1.05" fill="currentColor" />
    </svg>
  );
}
