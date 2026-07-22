/**
 * The `player://…` event channel. The UI mirrors playback state from these snapshots rather
 * than polling, so a change made anywhere in the core reaches the UI by one path.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { MediaInfo, PlaybackState } from "./types";

export const PLAYER_STATE_EVENT = "player://state";
export const MEDIA_OPENED_EVENT = "player://media-opened";

export const onPlayerState = (handler: (state: PlaybackState) => void): Promise<UnlistenFn> =>
  listen<PlaybackState>(PLAYER_STATE_EVENT, (event) => handler(event.payload));

export const onMediaOpened = (handler: (media: MediaInfo) => void): Promise<UnlistenFn> =>
  listen<MediaInfo>(MEDIA_OPENED_EVENT, (event) => handler(event.payload));
