import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { useT } from "../i18n";
import { LOCALES } from "../i18n/locales";
import { setSubtitleStyleOverride, settingsSet } from "../ipc/commands";
import type { AppInfo, SubStyleOverride, Theme, UserSettings } from "../ipc/types";

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
const CATEGORIES = ["general", "appearance", "subtitles", "language", "about"] as const;
export type CategoryId = (typeof CATEGORIES)[number];

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
  const t = useT();
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

  // The subtitle style override goes through its own command so it also applies to the engine
  // live (not only on the next file open), while still persisting.
  const applyStyle = (style: SubStyleOverride) => {
    setError(null);
    onChange({ ...settings, subtitleStyle: style });
    setSubtitleStyleOverride(style).catch((err) => setError(String(err)));
  };

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={t("settings-title")}
        tabIndex={-1}
        className="flex h-[30rem] w-full max-w-3xl overflow-hidden rounded-xl border border-havoc-border bg-havoc-panel outline-none"
      >
        {/* Sidebar */}
        <nav
          aria-label={t("settings-categories")}
          className="flex w-44 shrink-0 flex-col gap-0.5 border-e border-havoc-border bg-havoc-surface p-2"
        >
          {CATEGORIES.map((id) => (
            <button
              key={id}
              type="button"
              onClick={() => setCategory(id)}
              aria-current={category === id ? "page" : undefined}
              className={`rounded-md px-3 py-2 text-start text-xs transition-colors ${
                category === id
                  ? "bg-havoc-accent/20 font-semibold text-havoc-text"
                  : "text-havoc-muted hover:bg-havoc-panel hover:text-havoc-text"
              }`}
            >
              {t(`settings-${id}`)}
            </button>
          ))}
        </nav>

        {/* Pane */}
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex items-center justify-between border-b border-havoc-border px-4 py-2.5">
            <h2 className="m-0 text-sm font-bold text-havoc-text">{t(`settings-${category}`)}</h2>
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-havoc-border px-3 py-1 text-xs text-havoc-muted hover:border-havoc-accent hover:text-havoc-text"
            >
              {t("settings-close")}
            </button>
          </div>

          <div className="min-h-0 flex-1 overflow-auto px-4 py-3 text-xs text-havoc-text">
            {category === "general" && (
              <Section title={t("settings-window-title")} hint={t("settings-window-hint")}>
                <Toggle
                  label={t("settings-tray-label")}
                  hint={t("settings-tray-hint")}
                  checked={settings.minimizeToTray}
                  onChange={(minimizeToTray) => apply({ ...settings, minimizeToTray })}
                />
              </Section>
            )}

            {category === "appearance" && (
              <Section title={t("settings-theme-title")} hint={t("settings-theme-hint")}>
                <div className="flex gap-2">
                  {(["dark", "light"] as const).map((mode) => (
                    <button
                      key={mode}
                      type="button"
                      onClick={() => apply({ ...settings, theme: mode as Theme })}
                      aria-pressed={settings.theme === mode}
                      className={`rounded-md border px-3 py-1.5 text-xs ${
                        settings.theme === mode
                          ? "border-havoc-accent bg-havoc-accent/15 font-semibold text-havoc-text"
                          : "border-havoc-border text-havoc-muted hover:border-havoc-accent hover:text-havoc-text"
                      }`}
                    >
                      {t(`settings-theme-${mode}`)}
                    </button>
                  ))}
                </div>
              </Section>
            )}

            {category === "subtitles" && (
              <div className="flex flex-col gap-5">
                <Section title={t("settings-sub-style-title")} hint={t("settings-sub-style-hint")}>
                  <Toggle
                    label={t("settings-sub-style-enable")}
                    hint={t("settings-sub-style-enable-hint")}
                    checked={settings.subtitleStyle.enabled}
                    onChange={(enabled) => applyStyle({ ...settings.subtitleStyle, enabled })}
                  />
                  {settings.subtitleStyle.enabled && (
                    <div className="flex flex-col gap-2">
                      <Field label={t("settings-sub-style-font")}>
                        <input
                          type="text"
                          value={settings.subtitleStyle.font ?? ""}
                          placeholder="sans-serif"
                          onChange={(e) =>
                            applyStyle({
                              ...settings.subtitleStyle,
                              font: e.target.value.trim() || null,
                            })
                          }
                          className="w-48 rounded-md border border-havoc-border bg-havoc-surface px-2 py-1 text-xs text-havoc-text"
                        />
                      </Field>
                      <Field label={t("settings-sub-style-size")}>
                        <input
                          type="number"
                          min={10}
                          max={200}
                          value={settings.subtitleStyle.fontSize ?? ""}
                          placeholder="55"
                          onChange={(e) =>
                            applyStyle({
                              ...settings.subtitleStyle,
                              fontSize: e.target.value === "" ? null : Number(e.target.value),
                            })
                          }
                          className="w-24 rounded-md border border-havoc-border bg-havoc-surface px-2 py-1 text-xs text-havoc-text"
                          dir="ltr"
                        />
                      </Field>
                      <Field label={t("settings-sub-style-color")}>
                        <input
                          type="color"
                          value={settings.subtitleStyle.color ?? "#ffffff"}
                          onChange={(e) =>
                            applyStyle({ ...settings.subtitleStyle, color: e.target.value })
                          }
                          className="h-7 w-12 cursor-pointer rounded-md border border-havoc-border bg-havoc-surface"
                        />
                      </Field>
                    </div>
                  )}
                </Section>

                <Section title={t("settings-online-title")} hint={t("settings-online-hint")}>
                  <Toggle
                    label={t("settings-online-enable")}
                    hint={t("settings-online-enable-hint")}
                    checked={settings.openSubtitles.enabled}
                    onChange={(enabled) =>
                      apply({
                        ...settings,
                        openSubtitles: { ...settings.openSubtitles, enabled },
                      })
                    }
                  />
                  {settings.openSubtitles.enabled && (
                    <div className="flex flex-col gap-2">
                      <Field label={t("settings-online-key")}>
                        <input
                          type="password"
                          value={settings.openSubtitles.apiKey ?? ""}
                          onChange={(e) =>
                            apply({
                              ...settings,
                              openSubtitles: {
                                ...settings.openSubtitles,
                                apiKey: e.target.value || null,
                              },
                            })
                          }
                          className="w-56 rounded-md border border-havoc-border bg-havoc-surface px-2 py-1 text-xs text-havoc-text"
                          dir="ltr"
                        />
                      </Field>
                      <Field label={t("settings-online-username")}>
                        <input
                          type="text"
                          value={settings.openSubtitles.username ?? ""}
                          onChange={(e) =>
                            apply({
                              ...settings,
                              openSubtitles: {
                                ...settings.openSubtitles,
                                username: e.target.value || null,
                              },
                            })
                          }
                          className="w-56 rounded-md border border-havoc-border bg-havoc-surface px-2 py-1 text-xs text-havoc-text"
                          dir="ltr"
                        />
                      </Field>
                      <p className="m-0 text-[11px] text-havoc-muted">
                        {t("settings-online-privacy")}
                      </p>
                    </div>
                  )}
                </Section>
              </div>
            )}

            {category === "language" && (
              <Section title={t("settings-language-title")} hint={t("settings-language-hint")}>
                <div className="grid grid-cols-3 gap-1.5">
                  {LOCALES.map(({ code, autonym }) => (
                    <button
                      key={code}
                      type="button"
                      onClick={() => apply({ ...settings, language: code })}
                      aria-pressed={settings.language === code}
                      // Each name is written in its OWN language, so it needs its own `lang`:
                      // `styles/fonts.css` keys the per-script font stack off `:lang()`, and
                      // without this the whole list would draw in the current UI language's
                      // stack — rendering 日本語 in Simplified Chinese letterforms next to
                      // 简体中文. `dir="auto"` puts العربية the right way round in an LTR list.
                      lang={code}
                      dir="auto"
                      className={`truncate rounded-md border px-2.5 py-1.5 text-xs ${
                        settings.language === code
                          ? "border-havoc-accent bg-havoc-accent/15 font-semibold text-havoc-text"
                          : "border-havoc-border text-havoc-muted hover:border-havoc-accent hover:text-havoc-text"
                      }`}
                    >
                      {autonym}
                    </button>
                  ))}
                </div>
              </Section>
            )}

            {category === "about" && (
              <Section title="Freally Player" hint={t("settings-about-hint")}>
                <dl className="grid grid-cols-[8rem_1fr] gap-y-1.5 text-[11px]">
                  <dt className="text-havoc-muted">{t("settings-about-version")}</dt>
                  <dd className="m-0 font-mono">{info ? info.version : "…"}</dd>
                  <dt className="text-havoc-muted">{t("settings-about-licence")}</dt>
                  <dd className="m-0">{t("settings-about-rights")}</dd>
                  <dt className="text-havoc-muted">{t("settings-about-privacy")}</dt>
                  <dd className="m-0">{t("settings-about-privacy-value")}</dd>
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

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex items-center gap-3">
      <span className="w-28 shrink-0 text-xs text-havoc-muted">{label}</span>
      {children}
    </label>
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
