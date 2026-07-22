import { useEffect, useMemo, useState } from "react";

import { useT } from "../i18n";
import { bugReportClearCrash, bugReportContext, bugReportSubmit } from "../ipc/commands";
import type { BugReportContext, BugReportTarget } from "../ipc/types";

/**
 * Report a bug — opt-in and anonymous (charter: no telemetry, nothing auto-sends). Shows the
 * user the EXACT anonymous report (app/OS + a scrubbed crash from the last run, if any), then
 * lets them submit it via a pre-filled GitHub issue, Gmail compose, or their mail client. The
 * subject is `[Freally Player] <what went wrong>` so a report is instantly attributable. No
 * server, no shipped credentials — the user still clicks send.
 */
export function BugReportDialog({ onClose }: { onClose: () => void }) {
  const t = useT();
  const [ctx, setCtx] = useState<BugReportContext | null>(null);
  const [description, setDescription] = useState("");
  const [includeCrash, setIncludeCrash] = useState(true);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    bugReportContext()
      .then(setCtx)
      .catch((err) => setError(String(err)));
  };

  useEffect(load, []);

  // Mirrors `compose_body(.., BodyStyle::Plain)` in `bugreport.rs`. The GitHub target sends
  // the same content as Markdown (`###` headings, fenced diagnostics); only the syntax
  // differs, never the information.
  //
  // DELIBERATELY NOT TRANSLATED. This block is headed "exactly what will be sent", and what is
  // actually sent is built in English by Rust — for the maintainer who has to read it. A
  // localized preview of an English report would make that heading a lie, which the honesty
  // invariant forbids. It is shown `dir="ltr"` below for the same reason.
  const preview = useMemo(() => {
    if (!ctx) return "";
    const parts = [
      "WHAT HAPPENED",
      description.trim() || "(no description provided)",
      "",
      "ANONYMOUS DIAGNOSTICS (no personal data)",
      "From: Freally Player",
      ctx.diagnostics.trimEnd(),
    ];
    if (includeCrash && ctx.pendingCrash) {
      parts.push("", "--- crash excerpt ---", ctx.pendingCrash.trimEnd());
    }
    return parts.join("\n");
  }, [ctx, description, includeCrash]);

  const submit = (target: BugReportTarget) => {
    setError(null);
    bugReportSubmit(target, description, includeCrash && !!ctx?.pendingCrash).catch((err) =>
      setError(String(err)),
    );
  };

  const copy = () => {
    navigator.clipboard
      .writeText(preview)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => setError(t("bug-copy-failed")));
  };

  const dismissCrash = () => {
    bugReportClearCrash()
      .then(load)
      .catch((err) => setError(String(err)));
  };

  const action =
    "rounded-md border border-havoc-accent bg-havoc-accent/15 px-3 py-1.5 text-xs font-semibold text-havoc-text hover:bg-havoc-accent/25";
  const secondary =
    "rounded-md border border-havoc-border px-3 py-1.5 text-xs text-havoc-muted hover:border-havoc-accent hover:text-havoc-text";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t("bug-title")}
      className="absolute inset-0 z-10 flex items-center justify-center bg-black/60 p-4"
    >
      <div className="flex max-h-full w-full max-w-2xl flex-col gap-3 overflow-auto rounded-xl border border-havoc-border bg-havoc-panel p-5 text-xs text-havoc-text">
        <div className="flex items-baseline justify-between gap-2">
          <h2 className="m-0 text-sm font-bold text-havoc-text">{t("bug-title")}</h2>
          <button type="button" onClick={onClose} className={secondary}>
            {t("bug-close")}
          </button>
        </div>

        <p className="m-0 text-[11px] leading-snug text-havoc-muted">{t("bug-intro")}</p>

        {ctx?.pendingCrash && (
          <div className="rounded-lg border border-amber-500/50 bg-amber-500/10 px-2.5 py-2">
            <p className="m-0 text-[11px] text-amber-500">{t("bug-pending-crash")}</p>
          </div>
        )}

        <label className="flex flex-col gap-1 text-[11px] text-havoc-muted">
          {t("bug-what-happened")}
          <textarea
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            rows={3}
            placeholder={t("bug-placeholder")}
            className="rounded-md border border-havoc-border bg-havoc-surface px-2 py-1.5 text-xs text-havoc-text outline-none focus:border-havoc-accent"
          />
        </label>

        {ctx?.pendingCrash && (
          <label className="flex items-center gap-2 text-[11px] text-havoc-muted">
            <input
              type="checkbox"
              checked={includeCrash}
              onChange={(event) => setIncludeCrash(event.target.checked)}
            />
            {t("bug-include-crash")}
          </label>
        )}

        <div className="flex flex-col gap-1">
          <span className="text-[10px] tracking-wide text-havoc-muted uppercase">
            {t("bug-preview-heading")}
          </span>
          <pre
            lang="en"
            dir="ltr"
            className="m-0 max-h-48 overflow-auto rounded-md border border-havoc-border bg-havoc-surface px-2 py-1.5 font-mono text-[10px] leading-snug break-words whitespace-pre-wrap text-havoc-muted"
          >
            {preview}
          </pre>
        </div>

        <div className="flex flex-wrap gap-2">
          <button type="button" onClick={() => submit("github")} className={action}>
            {t("bug-submit-github")}
          </button>
          <button type="button" onClick={() => submit("gmail")} className={action}>
            {t("bug-submit-gmail")}
          </button>
          <button type="button" onClick={() => submit("email")} className={action}>
            {t("bug-submit-email")}
          </button>
          <button type="button" onClick={copy} className={secondary}>
            {copied ? t("bug-copied") : t("bug-copy")}
          </button>
          {ctx?.pendingCrash && (
            <button
              type="button"
              onClick={dismissCrash}
              className="rounded-md border border-havoc-border px-3 py-1.5 text-xs text-havoc-muted hover:border-red-400/60 hover:text-red-400"
            >
              {t("bug-dismiss-crash")}
            </button>
          )}
        </div>

        {error && (
          <p role="alert" className="m-0 text-[11px] text-red-400">
            {error}
          </p>
        )}
      </div>
    </div>
  );
}
