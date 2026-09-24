import { useEffect, useRef, useState } from "react";
import { Play, Pause, SpeakerHigh } from "@phosphor-icons/react";
import { audioSource } from "../lib/storage";
import { formatTime, type Recording } from "../lib/types";
export function AudioPlayer({
  recordings,
  seek,
  onError,
}: {
  recordings: Recording[];
  seek: { time: number; key: number; speaker?: string } | null;
  onError: (message: string) => void;
}) {
  const audio = useRef<HTMLAudioElement>(null);
  const pendingSeek = useRef<number | null>(null);
  const continuePlaying = useRef(false);
  const [index, setIndex] = useState(0);
  const [source, setSource] = useState("");
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [speed, setSpeed] = useState(1);
  useEffect(() => {
    setIndex(0);
  }, [recordings[0]?.id]);
  const recording = recordings[index] ?? recordings[0];
  useEffect(() => {
    let disposed = false;
    let url = "";
    setSource("");
    setPlaying(false);
    setTime(0);
    setDuration(0);
    if (recording)
      audioSource(recording)
        .then((s) => {
          url = s;
          if (!disposed) setSource(s);
          else if (s.startsWith("blob:")) URL.revokeObjectURL(s);
        })
        .catch((e) => onError(String(e)));
    return () => {
      disposed = true;
      if (url.startsWith("blob:")) URL.revokeObjectURL(url);
    };
  }, [recording?.id]);
  useEffect(() => {
    if (!seek || !audio.current) return;
    const candidates = recordings
      .map((r, i) => ({ r, i }))
      .filter(
        ({ r }) =>
          r.offset <= seek.time && (!seek.speaker || r.track === seek.speaker),
      );
    candidates.sort((a, b) => b.r.offset - a.r.offset);
    const target = candidates[0]?.i ?? 0;
    const position = Math.max(0, seek.time - (recordings[target]?.offset ?? 0));
    if (target !== index) {
      pendingSeek.current = position;
      continuePlaying.current = playing;
      setIndex(target);
    } else if (audio.current.readyState >= 1) {
      audio.current.currentTime = Math.min(
        position,
        audio.current.duration || position,
      );
    } else {
      pendingSeek.current = position;
    }
  }, [seek]);
  useEffect(() => {
    if (audio.current) audio.current.playbackRate = speed;
  }, [speed, source]);
  async function toggle() {
    const el = audio.current;
    if (!el) return;
    try {
      if (el.paused) await el.play();
      else el.pause();
    } catch {
      onError("The recording could not be played. Try selecting it again.");
    }
  }
  return (
    <div className="audio-area">
      {recordings.length > 1 && (
        <label className="recording-picker">
          Recording
          <select
            value={index}
            onChange={(e) => {
              pendingSeek.current = null;
              continuePlaying.current = false;
              setIndex(Number(e.target.value));
            }}
          >
            {recordings.map((r, i) => (
              <option key={r.id} value={i}>
                {r.name}
              </option>
            ))}
          </select>
        </label>
      )}
      <div className="audio-player">
        <button
          className="play-button"
          disabled={!source}
          onClick={toggle}
          aria-label={playing ? "Pause recording" : "Play recording"}
        >
          {playing ? (
            <Pause size={27} weight="fill" />
          ) : (
            <Play size={27} weight="fill" />
          )}
        </button>
        <div className="audio-progress">
          <span className="audio-caption">
            <SpeakerHigh size={16} />
            {recording?.track ?? "Recording"}
          </span>
          <input
            aria-label="Seek recording"
            type="range"
            min={0}
            max={duration || 1}
            step={0.1}
            value={Math.min(time, duration || 1)}
            disabled={!duration}
            onChange={(e) => {
              const next = Number(e.target.value);
              if (audio.current) audio.current.currentTime = next;
              setTime(next);
            }}
          />
        </div>
        <span className="audio-time">
          {formatTime(time)} <span>/ {formatTime(duration)}</span>
        </span>
        <button
          className="speed-button"
          aria-label={`Playback speed ${speed}x`}
          onClick={() => setSpeed((s) => (s === 2 ? 1 : s + 0.25))}
        >
          {speed}x
        </button>
        <audio
          ref={audio}
          src={source || undefined}
          preload="metadata"
          onLoadedMetadata={() => {
            const el = audio.current;
            if (!el) return;
            setDuration(el.duration);
            if (pendingSeek.current !== null) {
              el.currentTime = Math.min(pendingSeek.current, el.duration);
              pendingSeek.current = null;
            }
            if (continuePlaying.current) {
              continuePlaying.current = false;
              void el
                .play()
                .catch(() =>
                  onError("Press Play to continue the next audio chunk."),
                );
            }
          }}
          onTimeUpdate={() => setTime(audio.current?.currentTime ?? 0)}
          onPlay={() => setPlaying(true)}
          onPause={() => setPlaying(false)}
          onEnded={() => {
            setPlaying(false);
            const next = recordings
              .map((r, i) => ({ r, i }))
              .filter(
                ({ r }) =>
                  r.track === recording.track && r.offset > recording.offset,
              )
              .sort((a, b) => a.r.offset - b.r.offset)[0];
            if (next) {
              continuePlaying.current = true;
              setIndex(next.i);
            }
          }}
          onError={() =>
            source &&
            onError(
              "Audio could not be loaded. The saved conversation is still available.",
            )
          }
        />
      </div>
    </div>
  );
}
