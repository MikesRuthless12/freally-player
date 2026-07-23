import { useEffect, useState } from "react";

import { useT, type Translate } from "../i18n";
import {
  openSubtitlesDownload,
  openSubtitlesLogin,
  openSubtitlesSearch,
  settingsGet,
} from "../ipc/commands";
import {
  SUB_POS_DEFAULT,
  SUB_SCALE_MAX,
  SUB_SCALE_MIN,
  type SubtitleCandidate,
  type SubtitleState,
  type Track,
} from "../ipc/types";
import type { Transport } from "../lib/transport";
import { subtitleTracks, trackLabel } from "../lib/tracks";
import { Menu, MenuItem } from "./Menu";

/**
 * The subtitle menu: choose the primary (and a secondary) track, show/hide, load an external
 * file, fine-tune sync/position/scale, and — when the user has opted in — fetch subtitles
 * online. Rendering stays the engine's job (libass into the surface); this only drives it, so
 * everything here is a control, never a subtitle drawn in the web layer.
 */
export function SubtitleMenu({
  tracks,
  subtitle,
  transport,
  mediaTitle,
}: {
  tracks: Track[];
  subtitle: SubtitleState;
  transport: Transport;
  mediaTitle?: string;
}) {
  const t = useT();
  const subs = subtitleTracks(tracks);
  const [note, setNote] = useState<string | null>(null);

  const loadFile = async () => {
    setNote(null);
    const loaded = await transport.addSubtitleFile();
    if (loaded) setNote(encodingNote(loaded.sourceEncoding, loaded.imageBased, t));
  };

  return (
    <Menu icon={<SubtitleIcon />} title={t("subtitle-menu")} align="end" panelClassName="min-w-72">
      {() => (
        <div className="flex flex-col gap-2 p-1 text-xs">
          {/* Primary track. */}
          <Group label={t("subtitle-tracks")}>
            <MenuItem
              selected={subtitle.id === null}
              onSelect={() => transport.setSubtitleTrack(null)}
            >
              {t("subtitle-off")}
            </MenuItem>
            {subs.map((track, i) => (
              <MenuItem
                key={track.id}
                selected={subtitle.id === track.id}
                onSelect={() => transport.setSubtitleTrack(track.id)}
              >
                <span className="truncate">
                  {trackLabel(track, t("subtitle-track-n", { n: i + 1 }))}
                </span>
                {track.external && <Badge>{t("subtitle-external")}</Badge>}
                {track.imageBased && <Badge>{t("subtitle-image-based")}</Badge>}
              </MenuItem>
            ))}
            <div className="mt-1 flex items-center gap-2 px-1">
              <button
                type="button"
                onClick={loadFile}
                className="rounded-md border border-havoc-border px-2.5 py-1 text-[11px] text-havoc-muted hover:border-havoc-accent hover:text-havoc-text"
              >
                {t("subtitle-load-file")}
              </button>
              <label className="flex cursor-pointer items-center gap-1.5 text-[11px] text-havoc-muted">
                <input
                  type="checkbox"
                  checked={subtitle.visible}
                  onChange={(e) => transport.setSubtitleVisible(e.target.checked)}
                  disabled={subtitle.id === null}
                />
                {t("subtitle-visible")}
              </label>
            </div>
            {note && <p className="m-0 px-1 text-[11px] text-havoc-accent-2">{note}</p>}
          </Group>

          {/* Secondary track — two subtitles at once (e.g. language learning). */}
          {subs.length > 0 && (
            <Group label={t("subtitle-secondary")}>
              <MenuItem
                selected={subtitle.secondaryId === null}
                onSelect={() => transport.setSecondarySubtitleTrack(null)}
              >
                {t("subtitle-off")}
              </MenuItem>
              {subs.map((track, i) => (
                <MenuItem
                  key={track.id}
                  selected={subtitle.secondaryId === track.id}
                  onSelect={() => transport.setSecondarySubtitleTrack(track.id)}
                >
                  <span className="truncate">
                    {trackLabel(track, t("subtitle-track-n", { n: i + 1 }))}
                  </span>
                </MenuItem>
              ))}
            </Group>
          )}

          {/* Sync, position, scale — only meaningful with a track showing. */}
          {subtitle.id !== null && (
            <Group label={t("subtitle-adjust")}>
              <Stepper
                label={t("subtitle-delay")}
                value={`${subtitle.delaySecs.toFixed(1)} ${t("unit-seconds-short")}`}
                onDown={() => transport.setSubtitleDelay(round1(subtitle.delaySecs - 0.1))}
                onUp={() => transport.setSubtitleDelay(round1(subtitle.delaySecs + 0.1))}
                onReset={() => transport.setSubtitleDelay(0)}
                resetLabel={t("subtitle-reset")}
              />
              <Slider
                label={t("subtitle-position")}
                min={0}
                max={150}
                step={5}
                value={subtitle.pos}
                onChange={(v) => transport.setSubtitlePos(v)}
                onReset={() => transport.setSubtitlePos(SUB_POS_DEFAULT)}
                resetLabel={t("subtitle-reset")}
              />
              <Slider
                label={t("subtitle-scale")}
                min={SUB_SCALE_MIN}
                max={SUB_SCALE_MAX}
                step={0.05}
                value={subtitle.scale}
                onChange={(v) => transport.setSubtitleScale(v)}
                onReset={() => transport.setSubtitleScale(1)}
                resetLabel={t("subtitle-reset")}
              />
            </Group>
          )}

          {/* Opt-in online fetch. */}
          <OnlineSubtitles mediaTitle={mediaTitle} />
        </div>
      )}
    </Menu>
  );
}

/** The opt-in OpenSubtitles panel. Self-contained: it reads the stored opt-in configuration and
 * shows a route to Settings when it is off, so nothing reaches the network unless the user has
 * turned it on and supplied their own key. */
function OnlineSubtitles({ mediaTitle }: { mediaTitle?: string }) {
  const t = useT();
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [username, setUsername] = useState("");
  const [query, setQuery] = useState(mediaTitle ?? "");
  const [languages, setLanguages] = useState("en");
  const [password, setPassword] = useState("");
  const [signedIn, setSignedIn] = useState(false);
  const [results, setResults] = useState<SubtitleCandidate[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Read the opt-in configuration once when the panel mounts.
  useEffect(() => {
    let cancelled = false;
    settingsGet().then(
      (s) => {
        if (cancelled) return;
        setEnabled(!!s.openSubtitles?.enabled && !!s.openSubtitles?.apiKey);
        setUsername(s.openSubtitles?.username ?? "");
      },
      () => !cancelled && setEnabled(false),
    );
    return () => {
      cancelled = true;
    };
  }, []);

  if (enabled === null) return null;
  if (!enabled) {
    return (
      <Group label={t("subtitle-online")}>
        <p className="m-0 px-1 text-[11px] text-havoc-muted">{t("subtitle-online-disabled")}</p>
      </Group>
    );
  }

  const langList = languages
    .split(",")
    .map((l) => l.trim())
    .filter(Boolean);

  // One busy/error scaffold for the three async actions — clears the error, marks busy, runs, and
  // surfaces any failure to the panel's own error line.
  const runBusy = async (action: () => Promise<void>) => {
    setError(null);
    setBusy(true);
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const search = () =>
    runBusy(async () => {
      setResults(await openSubtitlesSearch(query, langList));
    });

  const signIn = () =>
    runBusy(async () => {
      await openSubtitlesLogin(username, password);
      setSignedIn(true);
      setPassword("");
    });

  const download = (fileId: number) =>
    runBusy(async () => {
      await openSubtitlesDownload(fileId);
    });

  return (
    <Group label={t("subtitle-online")}>
      <div className="flex flex-col gap-1.5 px-1">
        <div className="flex gap-1.5">
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("subtitle-online-query")}
            aria-label={t("subtitle-online-query")}
            className="min-w-0 flex-1 rounded-md border border-havoc-border bg-havoc-surface px-2 py-1 text-[11px] text-havoc-text"
          />
          <input
            value={languages}
            onChange={(e) => setLanguages(e.target.value)}
            aria-label={t("subtitle-online-languages")}
            className="w-16 rounded-md border border-havoc-border bg-havoc-surface px-2 py-1 text-[11px] text-havoc-text"
            dir="ltr"
          />
          <button
            type="button"
            onClick={search}
            disabled={busy || query.trim().length === 0}
            className="rounded-md border border-havoc-accent bg-havoc-accent/15 px-2.5 py-1 text-[11px] font-semibold text-havoc-text disabled:opacity-40"
          >
            {t("subtitle-online-search")}
          </button>
        </div>

        {!signedIn && (
          <div className="flex gap-1.5">
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder={t("subtitle-online-username")}
              aria-label={t("subtitle-online-username")}
              className="min-w-0 flex-1 rounded-md border border-havoc-border bg-havoc-surface px-2 py-1 text-[11px] text-havoc-text"
              dir="ltr"
            />
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t("subtitle-online-password")}
              aria-label={t("subtitle-online-password")}
              className="min-w-0 flex-1 rounded-md border border-havoc-border bg-havoc-surface px-2 py-1 text-[11px] text-havoc-text"
              dir="ltr"
            />
            <button
              type="button"
              onClick={signIn}
              disabled={busy || username.trim().length === 0 || password.length === 0}
              className="rounded-md border border-havoc-border px-2.5 py-1 text-[11px] text-havoc-muted hover:text-havoc-text disabled:opacity-40"
            >
              {t("subtitle-online-signin")}
            </button>
          </div>
        )}

        {results.length > 0 && (
          <ul className="m-0 flex max-h-40 list-none flex-col gap-0.5 overflow-auto p-0">
            {results.map((r) => (
              <li key={r.fileId}>
                <button
                  type="button"
                  onClick={() => download(r.fileId)}
                  disabled={busy}
                  title={r.release ?? r.fileName}
                  className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-start text-[11px] text-havoc-muted hover:bg-havoc-surface hover:text-havoc-text disabled:opacity-40"
                >
                  {r.language && (
                    <span className="uppercase text-havoc-accent-2" dir="ltr">
                      {r.language}
                    </span>
                  )}
                  <span className="truncate">{r.fileName}</span>
                  <span className="ms-auto tabular-nums text-havoc-muted" dir="ltr">
                    ↓{r.downloadCount}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}

        {error && (
          <p role="alert" className="m-0 text-[11px] text-red-400">
            {error}
          </p>
        )}
      </div>
    </Group>
  );
}

/** The honest note shown after loading an external/online subtitle. */
function encodingNote(sourceEncoding: string | null, imageBased: boolean, t: Translate): string {
  if (imageBased) return t("subtitle-loaded-image");
  if (sourceEncoding) return t("subtitle-loaded-encoding", { encoding: sourceEncoding });
  return t("subtitle-loaded");
}

function round1(value: number): number {
  return Math.round(value * 10) / 10;
}

function Group({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-0.5 border-t border-havoc-border pt-1.5 first:border-0 first:pt-0">
      <p className="px-2.5 pb-0.5 text-[10px] font-semibold uppercase tracking-wide text-havoc-muted">
        {label}
      </p>
      {children}
    </section>
  );
}

function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span className="ms-auto rounded-sm bg-havoc-surface px-1 text-[9px] uppercase text-havoc-muted">
      {children}
    </span>
  );
}

function Stepper({
  label,
  value,
  onDown,
  onUp,
  onReset,
  resetLabel,
}: {
  label: string;
  value: string;
  onDown: () => void;
  onUp: () => void;
  onReset: () => void;
  resetLabel: string;
}) {
  const btn =
    "grid h-6 w-6 place-items-center rounded-md border border-havoc-border text-havoc-muted hover:border-havoc-accent hover:text-havoc-text";
  return (
    <div className="flex items-center gap-2 px-1 py-0.5">
      <span className="w-16 shrink-0 text-[11px] text-havoc-muted">{label}</span>
      <button type="button" onClick={onDown} aria-label={`${label} −`} className={btn}>
        −
      </button>
      <span className="w-16 text-center text-[11px] tabular-nums text-havoc-text" dir="ltr">
        {value}
      </span>
      <button type="button" onClick={onUp} aria-label={`${label} +`} className={btn}>
        +
      </button>
      <button
        type="button"
        onClick={onReset}
        className="ms-auto text-[10px] text-havoc-muted hover:text-havoc-text"
      >
        {resetLabel}
      </button>
    </div>
  );
}

function Slider({
  label,
  min,
  max,
  step,
  value,
  onChange,
  onReset,
  resetLabel,
}: {
  label: string;
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (value: number) => void;
  onReset: () => void;
  resetLabel: string;
}) {
  return (
    <div className="flex items-center gap-2 px-1 py-0.5">
      <span className="w-16 shrink-0 text-[11px] text-havoc-muted">{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        aria-label={label}
        className="h-1 flex-1 cursor-pointer accent-havoc-accent"
        dir="ltr"
      />
      <button
        type="button"
        onClick={onReset}
        className="text-[10px] text-havoc-muted hover:text-havoc-text"
      >
        {resetLabel}
      </button>
    </div>
  );
}

function SubtitleIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-4 w-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      aria-hidden="true"
    >
      <rect x="2" y="3.5" width="12" height="9" rx="1.5" />
      <path d="M4.5 9h3M9 9h2.5M4.5 6.5h2M8 6.5h3.5" strokeLinecap="round" />
    </svg>
  );
}
