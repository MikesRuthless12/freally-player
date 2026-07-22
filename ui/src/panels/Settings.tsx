import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { settingsSet } from "../ipc/commands";
import type { AppInfo, Theme, UserSettings } from "../ipc/types";

/**
 * The Settings modal, in the same shape as Freally Capture's: a fixed-size two-pane shell
 * with a category sidebar on the left and the selected pane on the right.
 *
 * Rendered through a portal into `document.body` so no ancestor's `transform`/`filter`/
 * `backdrop-blur` can become the containing block for the fixed overlay and centre it inside
 * some inner box instead of the window. Escape closes it, focus is trapped while it is open
 * and restored on close.
 *
 * Every control here is a REAL setting that persists — nothing decorative.
 */
const CATEGORIES = ["general", "appearance", "about"] as const;
export type CategoryId = (typeof CATEGORIES)[number];

const CATEGORY_LABELS: Record<CategoryId, string> = {
  general: "General",
  appearance: "Appearance",
  about: "About",
};

export function SettingsModal({
  settings,
  info,
  initialCategory = "general",
  onChange,
  onClose,
}: {
  settings: UserSettings;
  info: AppInfo | null;
  initialCategory?: CategoryId;
  /** Applied immediately so the UI reflects the change while it persists. */
  onChange: (next: UserSettings) => void;
  onClose: () => void;
}) {
  const [category, setCategory] = useState<CategoryId>(initialCategory);
  const [error, setError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  // Escape closes; focus moves into the dialog and returns where it came from on close.
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    dialogRef.current?.focus();

    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) onClose();
      if (event.key !== "Tab" || !dialogRef.current) return;

      // Trap Tab inside the dialog: `aria-modal` tells assistive tech the rest of the app is
      // inert, so letting Tab walk out of it would contradict what we just announced.
      const focusable = dialogRef.current.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
      );
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      previouslyFocused?.focus?.();
    };
  }, [onClose]);

  const apply = (next: UserSettings) => {
    setError(null);
    onChange(next);
    settingsSet(next).catch((err) => setError(String(err)));
  };

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        tabIndex={-1}
        className="flex h-[30rem] w-full max-w-3xl overflow-hidden rounded-xl border border-havoc-border bg-havoc-panel outline-none"
      >
        {/* Sidebar */}
        <nav
          aria-label="Settings categories"
          className="flex w-44 shrink-0 flex-col gap-0.5 border-r border-havoc-border bg-havoc-surface p-2"
        >
          {CATEGORIES.map((id) => (
            <button
              key={id}
              type="button"
              onClick={() => setCategory(id)}
              aria-current={category === id ? "page" : undefined}
              className={`rounded-md px-3 py-2 text-left text-xs transition-colors ${
                category === id
                  ? "bg-havoc-accent/20 font-semibold text-havoc-text"
                  : "text-havoc-muted hover:bg-havoc-panel hover:text-havoc-text"
              }`}
            >
              {CATEGORY_LABELS[id]}
            </button>
          ))}
        </nav>

        {/* Pane */}
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex items-center justify-between border-b border-havoc-border px-4 py-2.5">
            <h2 className="m-0 text-sm font-bold text-havoc-text">{CATEGORY_LABELS[category]}</h2>
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-havoc-border px-3 py-1 text-xs text-havoc-muted hover:border-havoc-accent hover:text-havoc-text"
            >
              Close
            </button>
          </div>

          <div className="min-h-0 flex-1 overflow-auto px-4 py-3 text-xs text-havoc-text">
            {category === "general" && (
              <Section title="Window" hint="How Freally Player behaves when you put it away.">
                <Toggle
                  label="Minimize to system tray"
                  hint="Minimising hides the window and leaves a tray icon. Click the icon to bring it back."
                  checked={settings.minimizeToTray}
                  onChange={(minimizeToTray) => apply({ ...settings, minimizeToTray })}
                />
              </Section>
            )}

            {category === "appearance" && (
              <Section title="Theme" hint="Dark is the Havoc default.">
                <div className="flex gap-2">
                  {(["dark", "light"] as const).map((mode) => (
                    <button
                      key={mode}
                      type="button"
                      onClick={() => apply({ ...settings, theme: mode as Theme })}
                      aria-pressed={settings.theme === mode}
                      className={`rounded-md border px-3 py-1.5 text-xs capitalize ${
                        settings.theme === mode
                          ? "border-havoc-accent bg-havoc-accent/15 font-semibold text-havoc-text"
                          : "border-havoc-border text-havoc-muted hover:border-havoc-accent hover:text-havoc-text"
                      }`}
                    >
                      {mode}
                    </button>
                  ))}
                </div>
              </Section>
            )}

            {category === "about" && (
              <Section
                title="Freally Player"
                hint="Plays anything. Beautifully. No ads, no spyware."
              >
                <dl className="grid grid-cols-[8rem_1fr] gap-y-1.5 text-[11px]">
                  <dt className="text-havoc-muted">Version</dt>
                  <dd className="m-0 font-mono">{info ? info.version : "…"}</dd>
                  <dt className="text-havoc-muted">Licence</dt>
                  <dd className="m-0">© 2026 Mike Weaver — All Rights Reserved</dd>
                  <dt className="text-havoc-muted">Privacy</dt>
                  <dd className="m-0">No ads, no telemetry, no analytics, no account.</dd>
                </dl>
              </Section>
            )}

            {error && (
              <p role="alert" className="mt-3 mb-0 text-[11px] text-red-400">
                {error}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

function Section({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2">
      <div>
        <h3 className="m-0 text-xs font-semibold text-havoc-text">{title}</h3>
        {hint && <p className="m-0 text-[11px] text-havoc-muted">{hint}</p>}
      </div>
      {children}
    </section>
  );
}

function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-2.5 rounded-md border border-havoc-border bg-havoc-surface px-3 py-2.5">
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        className="mt-0.5"
      />
      <span className="flex flex-col gap-0.5">
        <span className="text-xs text-havoc-text">{label}</span>
        {hint && <span className="text-[11px] leading-snug text-havoc-muted">{hint}</span>}
      </span>
    </label>
  );
}
