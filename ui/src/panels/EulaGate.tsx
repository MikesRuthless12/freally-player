import { exit } from "@tauri-apps/plugin-process";
import { Fragment, type ReactElement, type ReactNode, useEffect, useRef, useState } from "react";

import { useT } from "../i18n";
import { eulaAccept } from "../ipc/commands";
import type { EulaStatus } from "../ipc/types";

/** Decode the HTML entities the source doc may carry (belt + suspenders). */
function decodeEntities(s: string): string {
  return s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
}

/** Inline formatting: **bold** + `code`, entities decoded. */
function inline(text: string): ReactNode[] {
  return decodeEntities(text)
    .split(/(\*\*[^*]+\*\*|`[^`]+`)/g)
    .map((part, i) => {
      if (part.startsWith("**") && part.endsWith("**")) {
        return (
          <strong key={i} className="font-semibold text-havoc-text">
            {part.slice(2, -2)}
          </strong>
        );
      }
      if (part.startsWith("`") && part.endsWith("`")) {
        return (
          <code key={i} className="rounded bg-havoc-surface px-1 font-mono text-[10px]">
            {part.slice(1, -1)}
          </code>
        );
      }
      return <Fragment key={i}>{part}</Fragment>;
    });
}

/**
 * A tiny, safe markdown → JSX for the embedded EULA (headings, bold, lists, blockquotes,
 * paragraphs). No external parser; the text is build-time-embedded and trusted, so there is
 * no injection surface.
 */
function renderMarkdown(text: string): ReactElement[] {
  const blocks: ReactElement[] = [];
  let list: string[] = [];
  const flushList = () => {
    if (list.length === 0) return;
    const items = list;
    list = [];
    blocks.push(
      <ul key={`ul-${blocks.length}`} className="my-1 ml-4 list-disc space-y-0.5">
        {items.map((li, i) => (
          <li key={i}>{inline(li)}</li>
        ))}
      </ul>,
    );
  };
  for (const line of text.split("\n")) {
    if (/^#{2,}\s/.test(line)) {
      flushList();
      blocks.push(
        <h3 key={`b-${blocks.length}`} className="mt-3 mb-1 text-xs font-semibold text-havoc-text">
          {inline(line.replace(/^#{2,}\s/, ""))}
        </h3>,
      );
    } else if (/^#\s/.test(line)) {
      flushList();
      blocks.push(
        <h2 key={`b-${blocks.length}`} className="mt-2 mb-1.5 text-sm font-bold text-havoc-text">
          {inline(line.replace(/^#\s/, ""))}
        </h2>,
      );
    } else if (/^>\s/.test(line)) {
      flushList();
      blocks.push(
        <p key={`b-${blocks.length}`} className="my-1 border-l-2 border-havoc-border pl-2 italic">
          {inline(line.replace(/^>\s/, ""))}
        </p>,
      );
    } else if (/^[-*]\s/.test(line)) {
      list.push(line.replace(/^[-*]\s/, ""));
    } else if (line.trim() === "") {
      flushList();
    } else {
      flushList();
      blocks.push(
        <p key={`b-${blocks.length}`} className="my-1">
          {inline(line)}
        </p>,
      );
    }
  }
  flushList();
  return blocks;
}

/**
 * First-run EULA acceptance gate. Rendered by `App` instead of the player until the current
 * EULA version is accepted. The user must scroll to the end and click **I Agree** before the
 * app is usable; **Decline & Quit** exits. Acceptance is persisted, so it appears once (and
 * again only if the EULA version changes).
 */
export function EulaGate({ status, onAccepted }: { status: EulaStatus; onAccepted: () => void }) {
  const t = useT();
  const [scrolledToEnd, setScrolledToEnd] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  // If the agreement is short enough not to scroll, enable Agree immediately. Measured after
  // paint (rAF) so we never setState synchronously in the effect.
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const id = requestAnimationFrame(() => {
      if (el.scrollHeight <= el.clientHeight + 4) setScrolledToEnd(true);
    });
    return () => cancelAnimationFrame(id);
  }, []);

  const onScroll = () => {
    const el = scrollRef.current;
    if (el && el.scrollTop + el.clientHeight >= el.scrollHeight - 24) setScrolledToEnd(true);
  };

  const agree = () => {
    setBusy(true);
    setError(null);
    eulaAccept()
      .then(onAccepted)
      .catch((err) => {
        setBusy(false);
        setError(String(err));
      });
  };

  return (
    <div className="flex h-full w-full items-center justify-center bg-havoc-bg p-4 text-havoc-text">
      <div className="flex max-h-full w-full max-w-3xl flex-col gap-3 rounded-xl border border-havoc-border bg-havoc-panel p-5">
        <div className="flex items-baseline justify-between gap-2">
          <h1 className="m-0 bg-gradient-to-r from-havoc-accent to-havoc-accent-2 bg-clip-text text-lg font-bold tracking-wide text-transparent">
            {t("eula-heading")}
          </h1>
          <span className="shrink-0 font-mono text-[10px] text-havoc-muted">
            {t("eula-version", { version: status.version })}
          </span>
        </div>
        <p className="m-0 text-xs leading-relaxed text-havoc-muted">{t("eula-intro")}</p>
        {/* The agreement itself is always English — it is the document that legally binds, so
            it is never translated. Direction follows the CONTENT, not the UI: without this the
            Arabic shell would lay English legal text out right-to-left. */}
        <div
          ref={scrollRef}
          onScroll={onScroll}
          lang="en"
          dir="ltr"
          className="min-h-0 flex-1 overflow-auto rounded-lg border border-havoc-border bg-havoc-surface p-3 text-[11px] leading-relaxed text-havoc-muted"
        >
          {renderMarkdown(status.text)}
        </div>
        {error && (
          <p role="alert" className="m-0 text-[11px] text-red-400">
            {error}
          </p>
        )}
        <div className="flex flex-wrap items-center justify-between gap-2">
          <span className="text-[10px] text-havoc-muted">
            {scrolledToEnd ? t("eula-scrolled") : t("eula-scroll-prompt")}
          </span>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => void exit(0)}
              className="rounded-md border border-havoc-border px-3 py-1.5 text-xs text-havoc-muted hover:border-red-400/60 hover:text-red-400"
            >
              {t("eula-decline")}
            </button>
            <button
              type="button"
              onClick={agree}
              disabled={!scrolledToEnd || busy}
              className="rounded-md border border-havoc-accent bg-havoc-accent/15 px-3 py-1.5 text-xs font-semibold text-havoc-text hover:bg-havoc-accent/25 disabled:opacity-40"
            >
              {t("eula-agree")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
