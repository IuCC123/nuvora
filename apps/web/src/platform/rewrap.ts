import {
  EncodedAudioPacketSource,
  EncodedPacketSink,
  EncodedVideoPacketSource,
  Input,
  MATROSKA,
  Mp4OutputFormat,
  NullTarget,
  Output,
  UrlSource,
  WEBM,
  type EncodedPacket,
  type InputAudioTrack,
} from "mediabunny";
import { mediaSourceClass } from "./deviceProfile";

/** Seconds fed past the playhead, and kept behind it for a quick look back. */
const AHEAD = 60;
const BEHIND = 30;

/**
 * Plays a Matroska or WebM file in a browser that decodes its streams but can't
 * open the container (Safari, and Firefox with H.264). Mediabunny reads the file
 * from the server with range requests and rewraps its packets as fragmented MP4
 * for Media Source Extensions. Nothing is converted, on the server or here: the
 * picture and sound are the file's own.
 *
 * Only what's near the playhead is fetched, a minute ahead and half a minute
 * behind. A seek outside that restarts the rewrap at the keyframe before it.
 * Plays the default audio track, the one the server judged playable.
 *
 * Resolves to a function that stops playback and releases the file. Rejects when
 * the file can't be rewrapped here, and calls `onFailed` if playback breaks later.
 */
export async function rewrap(video: HTMLVideoElement, url: string, startAt: number, onFailed: (error: unknown) => void): Promise<() => void> {
  const MediaSourceType = mediaSourceClass();
  if (!MediaSourceType) throw new Error("No Media Source Extensions");

  const input = new Input({
    // Two quick retries ride out a blip; after that, give up so the player can fall back to the server's stream.
    // A read-ahead that fails with no read waiting is dropped: the read that needs those bytes retries and reports.
    source: new UrlSource(url, { getRetryDelay: (attempts) => (attempts < 3 ? attempts : null), handleUnhandledError: () => {} }),
    formats: [MATROSKA, WEBM],
  });
  const videoTrack = await input.getPrimaryVideoTrack();
  const audioTrack = defaultTrack(await input.getAudioTracks());
  const videoConfig = await videoTrack?.getDecoderConfig();
  const audioConfig = audioTrack ? await audioTrack.getDecoderConfig() : null;
  if (!videoTrack?.codec || !videoConfig || (audioTrack && (!audioTrack.codec || !audioConfig))) {
    input.dispose();
    throw new Error("Unreadable tracks");
  }
  const mime = `video/mp4; codecs="${[videoConfig.codec, audioConfig?.codec].filter(Boolean).join(",")}"`;
  if (!MediaSourceType.isTypeSupported(mime)) {
    input.dispose();
    throw new Error(`Can't play ${mime}`);
  }
  const duration = await input.computeDuration();

  const source = new MediaSourceType();
  // Safari's ManagedMediaSource only plays when AirPlay is off or another source is offered.
  video.disableRemotePlayback = true;
  video.src = URL.createObjectURL(source);
  await new Promise((resolve) => source.addEventListener("sourceopen", resolve, { once: true }));
  URL.revokeObjectURL(video.src);
  source.duration = duration;
  const buffer = source.addSourceBuffer(mime);

  let generation = 0;
  let output: Output | null = null;
  let target = startAt;
  /** The current rewrap is still feeding, so a seek to its starting point is already on its way. */
  let pumping = false;
  let appending = Promise.resolve();
  let stopped = false;

  const settled = () =>
    buffer.updating ? new Promise<void>((resolve) => buffer.addEventListener("updateend", () => resolve(), { once: true })) : Promise.resolve();

  async function append(bytes: Uint8Array) {
    await settled();
    if (stopped) return;
    const before = video.currentTime - BEHIND;
    if (buffer.buffered.length > 0 && buffer.buffered.start(0) < before) {
      buffer.remove(0, before);
      await settled();
    }
    try {
      buffer.appendBuffer(bytes as BufferSource);
    } catch (error) {
      if (!(error instanceof DOMException && error.name === "QuotaExceededError")) throw error;
      // The browser's buffer is full: drop everything behind the playhead and try once more.
      buffer.remove(0, Math.max(0, video.currentTime - 2));
      await settled();
      buffer.appendBuffer(bytes as BufferSource);
    }
    await settled();
  }

  /** Rewraps from the keyframe at or before `from` onwards, replacing whatever is buffered. */
  async function pump(from: number) {
    const run = ++generation;
    target = from;
    pumping = true;
    const previous = output;
    output = null;
    await cancel(previous);
    await appending.catch(() => {});
    await settled();
    if (run !== generation) return;
    if (buffer.buffered.length > 0) {
      buffer.remove(0, Infinity);
      await settled();
    }

    const videoSink = new EncodedPacketSink(videoTrack!);
    const audioSink = audioTrack ? new EncodedPacketSink(audioTrack) : null;
    const key = (await videoSink.getKeyPacket(from)) ?? (await videoSink.getFirstPacket());
    if (!key || run !== generation) return;
    const audioStart = audioSink ? ((await audioSink.getPacket(key.timestamp)) ?? (await audioSink.getFirstPacket())) : null;

    // The init segment (ftyp + moov) goes in once per rewrap, then each fragment as moof + mdat.
    let init: Uint8Array[] = [];
    let moof: Uint8Array | null = null;
    const queue = (bytes: Uint8Array) => {
      appending = appending.then(() => (run === generation ? append(bytes) : undefined));
    };
    const out = new Output({
      format: new Mp4OutputFormat({
        fastStart: "fragmented",
        minimumFragmentDuration: 2,
        onFtyp: (data) => void init.push(data.slice()),
        onMoov: (data) => {
          queue(concat([...init, data]));
          init = [];
        },
        onMoof: (data) => void (moof = data.slice()),
        onMdat: (data) => {
          if (moof) queue(concat([moof, data]));
          moof = null;
        },
      }),
      target: new NullTarget(),
    });
    const videoSource = new EncodedVideoPacketSource(videoTrack!.codec!);
    out.addVideoTrack(videoSource);
    const audioSource = audioTrack ? new EncodedAudioPacketSource(audioTrack.codec!) : null;
    if (audioSource) out.addAudioTrack(audioSource);
    await out.start();
    output = out;

    const videoPackets = videoSink.packets(key);
    const audioPackets = audioSink && audioStart ? audioSink.packets(audioStart) : null;
    let nextVideo = await videoPackets.next();
    let nextAudio: IteratorResult<EncodedPacket, void> = audioPackets ? await audioPackets.next() : { done: true, value: undefined };
    let firstVideo = true;
    let firstAudio = true;
    let fed = key.timestamp;

    while (!nextVideo.done || !nextAudio.done) {
      if (run !== generation) return;
      // Stay a minute ahead of the playhead, counting what's fed rather than what the
      // browser has taken in yet, so a long film never piles up in memory.
      while (run === generation && fed - Math.max(video.currentTime, from) > AHEAD) {
        await appending;
        await sleep(500);
      }
      if (run !== generation) return;
      const takeVideo = !nextVideo.done && (nextAudio.done || nextVideo.value.timestamp <= nextAudio.value.timestamp);
      if (takeVideo && !nextVideo.done) {
        await videoSource.add(nextVideo.value, firstVideo ? { decoderConfig: videoConfig! } : undefined);
        fed = Math.max(fed, nextVideo.value.timestamp);
        firstVideo = false;
        nextVideo = await videoPackets.next();
      } else if (!nextAudio.done && audioSource) {
        await audioSource.add(nextAudio.value, firstAudio ? { decoderConfig: audioConfig! } : undefined);
        firstAudio = false;
        nextAudio = await audioPackets!.next();
      }
    }
    await out.finalize();
    await appending;
    if (run !== generation) return;
    pumping = false;
    if (source.readyState === "open" && !buffer.updating) source.endOfStream();
  }

  const fail = (error: unknown) => {
    if (stopped) return;
    console.warn("Rewrapped playback stopped:", error);
    onFailed(error);
  };
  const run = (from: number) => void pump(from).catch(fail);

  /** A seek into what's buffered plays from there; anywhere else rewraps from that point. */
  const onSeeking = () => {
    // Safari resets a start position set before any media arrives and reports that as a seek to 0.
    if (video.readyState === HTMLMediaElement.HAVE_NOTHING) return;
    const now = video.currentTime;
    if (pumping && Math.abs(now - target) < 0.5) return;
    for (let i = 0; i < buffer.buffered.length; i++) {
      if (buffer.buffered.start(i) <= now && now < buffer.buffered.end(i) + 3) return;
    }
    run(now);
  };

  // Jump to the start point once the keyframe before it is in; earlier, Safari drops it.
  if (startAt > 0) video.addEventListener("loadedmetadata", () => (video.currentTime = startAt), { once: true });
  video.addEventListener("seeking", onSeeking);
  run(startAt);

  return () => {
    stopped = true;
    generation++;
    video.removeEventListener("seeking", onSeeking);
    void cancel(output);
    input.dispose();
    video.removeAttribute("src");
    video.load();
  };
}

/** Stops a rewrap that's still running; a finished one has nothing to stop. */
async function cancel(output: Output | null) {
  if (output?.state === "started") await output.cancel().catch(() => {});
}

/** The track the server judged: the one flagged default, else the first. */
function defaultTrack(tracks: InputAudioTrack[]): InputAudioTrack | null {
  return tracks.find((t) => t.disposition.default) ?? tracks[0] ?? null;
}

function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.byteLength, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.byteLength;
  }
  return out;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
