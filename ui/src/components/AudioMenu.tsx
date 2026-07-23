import { useT } from "../i18n";
import type { Track } from "../ipc/types";
import type { Transport } from "../lib/transport";
import { audioTracks, trackLabel } from "../lib/tracks";
import { Menu, MenuItem } from "./Menu";

/**
 * The audio-track menu: a popover list of the media's audio streams, with the current one
 * marked. Switching is instant — the engine changes `aid` and the transport snapshot reflects
 * it. Disabled when the media has fewer than two audio tracks (nothing to switch between).
 */
export function AudioMenu({
  tracks,
  audioId,
  transport,
}: {
  tracks: Track[];
  audioId: number | null;
  transport: Transport;
}) {
  const t = useT();
  const audio = audioTracks(tracks);

  return (
    <Menu
      icon={<AudioIcon />}
      title={t("audio-menu")}
      disabled={audio.length < 2}
      align="end"
      panelClassName="min-w-52"
    >
      {(close) => (
        <>
          <p className="px-2.5 pt-1 pb-0.5 text-[10px] font-semibold uppercase tracking-wide text-havoc-muted">
            {t("audio-tracks")}
          </p>
          {audio.map((track, i) => (
            <MenuItem
              key={track.id}
              selected={track.id === audioId}
              onSelect={() => {
                transport.setAudioTrack(track.id);
                close();
              }}
            >
              <span className="truncate">
                {trackLabel(track, t("audio-track-n", { n: i + 1 }))}
              </span>
            </MenuItem>
          ))}
        </>
      )}
    </Menu>
  );
}

function AudioIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      className="h-4 w-4"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      aria-hidden="true"
    >
      <path d="M2.5 6.5v3M5 4.5v7M8 2.5v11M11 5v6M13.5 7v2" strokeLinecap="round" />
    </svg>
  );
}
