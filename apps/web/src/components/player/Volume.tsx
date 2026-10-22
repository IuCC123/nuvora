import type { CSSProperties } from "react";
import { VolumeIcon } from "../Icon";

interface VolumeProps {
  volume: number;
  muted: boolean;
  onVolume: (volume: number) => void;
  onToggleMute: () => void;
}

/** Mute button, and a slider that opens beside it on hover or focus. */
export function Volume({ volume, muted, onVolume, onToggleMute }: VolumeProps) {
  const silent = muted || volume === 0;
  const level = silent ? "muted" : volume < 0.5 ? "low" : "high";
  return (
    <div className="volume">
      <button type="button" className="player-btn" aria-label={silent ? "Unmute" : "Mute"} title={silent ? "Unmute (M)" : "Mute (M)"} onMouseDown={(e) => e.preventDefault()} onClick={onToggleMute}>
        <VolumeIcon level={level} />
      </button>
      <input
        className="volume-slider"
        type="range"
        min={0}
        max={100}
        step={1}
        value={silent ? 0 : Math.round(volume * 100)}
        aria-label="Volume"
        aria-valuetext={silent ? "Muted" : `${Math.round(volume * 100)}%`}
        style={{ "--level": silent ? 0 : volume } as CSSProperties}
        onChange={(e) => onVolume(Number(e.currentTarget.value) / 100)}
        // A mouse drag leaves focus here, where Space and the arrows would act on the slider instead of the player.
        onPointerUp={(e) => e.pointerType === "mouse" && e.currentTarget.blur()}
      />
    </div>
  );
}
