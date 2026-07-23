/** Helpers for presenting audio/subtitle tracks in the transport menus. */
import type { Track } from "../ipc/types";

/** The audio tracks among a media's tracks. */
export const audioTracks = (tracks: Track[]): Track[] => tracks.filter((t) => t.kind === "audio");

/** The subtitle tracks among a media's tracks. */
export const subtitleTracks = (tracks: Track[]): Track[] => tracks.filter((t) => t.kind === "sub");

/**
 * A human label for a track: its own title, else its language spelled out in the current UI
 * language, else a numbered fallback the caller provides (already translated).
 */
export function trackLabel(track: Track, fallback: string): string {
  if (track.title && track.title.trim().length > 0) return track.title;
  if (track.lang) return languageName(track.lang) ?? track.lang;
  return fallback;
}

/**
 * A language code spelled out in the active UI language (e.g. `de` → "German"), or `null` when
 * the platform cannot name it. Uses the document's active locale, which `applyLocale` stamps on
 * the root, so the names follow the chosen interface language.
 */
function languageName(code: string): string | null {
  const locale = (typeof document !== "undefined" && document.documentElement.lang) || "en";
  try {
    return displayNamesFor(locale).of(code) ?? null;
  } catch {
    return null;
  }
}

/**
 * A cached `Intl.DisplayNames` per locale — its construction is comparatively heavy, and the menu
 * re-renders on every `player://state` event while open, so one per (locale, track) would be
 * wasteful.
 */
const displayNamesCache = new Map<string, Intl.DisplayNames>();
function displayNamesFor(locale: string): Intl.DisplayNames {
  let instance = displayNamesCache.get(locale);
  if (!instance) {
    instance = new Intl.DisplayNames([locale], { type: "language" });
    displayNamesCache.set(locale, instance);
  }
  return instance;
}
