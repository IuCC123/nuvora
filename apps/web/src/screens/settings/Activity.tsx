import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import {
  containerLabel,
  formatBitrate,
  formatBytes,
  formatClock,
  formatDuration,
  formatLastSeen,
  isScanRunning,
  queryKeys,
  type Activity,
  type ActiveStream,
  type PreviewQueue,
  type ScanStatus,
  type StreamState,
} from "@nuvora/core";
import { client, queries } from "../../nuvora";
import { LoadError, SettingsPage } from "./SettingsPage";
import "../../styles/activity.css";

/** How often the page asks the server what it is doing. */
const POLL_MS = 1000;

export function SettingsActivity() {
  const activity = useQuery({ ...queries.adminActivity(), refetchInterval: POLL_MS });
  return (
    <SettingsPage title="Activity" lede="What the server is working on right now: conversions, scans and seek previews. Updates every second.">
      {activity.isPending ? (
        <div className="server-facts-skeleton" aria-busy="true" aria-label="Loading activity" />
      ) : activity.isError && !activity.data ? (
        <LoadError error={activity.error} retry={() => void activity.refetch()} />
      ) : (
        <ActivityView activity={activity.data} />
      )}
    </SettingsPage>
  );
}

function ActivityView({ activity }: { activity: Activity }) {
  const converting = activity.streams.filter((s) => s.video_output).length;
  return (
    <>
      <section className="settings-section" aria-labelledby="activity-streams">
        <div>
          <h2 id="activity-streams">Streams</h2>
          <p className="settings-section-note">
            {activity.streams.length === 0
              ? "None open."
              : `${activity.streams.length} open, ${converting} converting video.`}
          </p>
        </div>
        {activity.streams.length === 0 ? (
          <p className="activity-empty">
            Nothing is playing through the server. Videos a device plays as they are don't appear here: they need no work.
          </p>
        ) : (
          <ul role="list" className="stream-list">
            {activity.streams.map((stream) => (
              <StreamCard key={stream.id} stream={stream} />
            ))}
          </ul>
        )}
      </section>
      <ScanSection scan={activity.scan} />
      <PreviewSection previews={activity.previews} />
    </>
  );
}

const STATE_LABEL: Record<StreamState, string> = {
  waiting: "Waiting",
  converting: "Working",
  paused: "Paused, far enough ahead",
  finished: "Done to the end",
  failed: "Failed",
};

function StreamCard({ stream }: { stream: ActiveStream }) {
  const ahead = aheadOfPlayer(stream);
  const madeSeconds = stream.made.reduce((sum, { start, end }) => sum + (end - start), 0);
  const percent = stream.duration_s > 0 ? Math.min(100, (madeSeconds / stream.duration_s) * 100) : 0;
  const verb = stream.video_output ? "Converting" : "Repackaging";
  return (
    <li className="stream-card">
      <header className="stream-head">
        <div className="stream-title">
          <h3>{stream.title}</h3>
          <p>
            {stream.user} · {verb} · opened {formatLastSeen(stream.opened_at, Date.now()).toLowerCase()}
          </p>
        </div>
        <span className={`stream-state is-${stream.state}`}>
          {stream.state === "converting" && <span className="stream-pulse" aria-hidden />}
          {STATE_LABEL[stream.state]}
        </span>
      </header>

      <dl className="stream-route">
        <div>
          <dt>Video</dt>
          <dd>
            <span>{describeVideo(stream)}</span>
            <Becomes converted={!!stream.video_output}>
              {stream.video_output
                ? `${stream.video_output.codec} · up to ${stream.video_output.max_height}p · ${stream.video_output.encoder}`
                : "Copied as it is"}
            </Becomes>
          </dd>
        </div>
        {stream.source_audio && (
          <div>
            <dt>Audio</dt>
            <dd>
              <span>
                {stream.source_audio.codec} · {stream.source_audio.layout ?? `${stream.source_audio.channels} ch`}
              </span>
              <Becomes converted={stream.audio_output?.kind === "aac"}>
                {stream.audio_output?.kind === "aac"
                  ? `AAC · ${channelName(stream.audio_output.channels)} · ${stream.audio_output.kbps} kb/s`
                  : "Copied as it is"}
              </Becomes>
            </dd>
          </div>
        )}
        <div>
          <dt>From</dt>
          <dd>
            <span>{containerLabel(stream.container)}</span>
            <Becomes converted={false}>HLS, fMP4 segments</Becomes>
          </dd>
        </div>
      </dl>

      <Timeline stream={stream} percent={percent} />
      <div className="timeline-legend">
        <span>
          Player at <strong>{formatClock(stream.position_s * 1000)}</strong> of {formatClock(stream.duration_s * 1000)}
        </span>
        <span>{ahead === null ? "Nothing made at the player yet" : `${formatClock(ahead * 1000)} ready ahead`}</span>
      </div>

      <dl className="stream-stats">
        <Stat term="Speed" value={stream.speed != null ? `${stream.speed.toFixed(2)}×` : "–"} detail={speedWord(stream)} />
        <Stat term="Frames per second" value={stream.fps ? stream.fps.toFixed(0) : "–"} />
        <Stat term="Bitrate" {...bitrate(stream, madeSeconds)} />
        <Stat term="Made" value={`${percent.toFixed(0)}%`} detail={`${stream.segments} segments in all`} />
        <Stat term="On disk" value={formatBytes(stream.disk_bytes)} />
        <Stat term="This run" value={stream.run_for_s != null ? formatDuration(stream.run_for_s * 1000) : "–"} detail={stream.frames != null ? `${stream.frames.toLocaleString()} frames` : undefined} />
        <Stat term="Last request" value={stream.idle_s < 2 ? "Now" : `${formatDuration(stream.idle_s * 1000)} ago`} />
      </dl>

      {stream.error && (
        <pre className="stream-error" aria-label="FFmpeg's last words">
          {stream.error}
        </pre>
      )}
    </li>
  );
}

/** The whole video, with what is made filled in, the current run's edge, and the player. */
function Timeline({ stream, percent }: { stream: ActiveStream; percent: number }) {
  const at = (seconds: number) => `${stream.duration_s > 0 ? Math.min(100, (seconds / stream.duration_s) * 100) : 0}%`;
  const run = stream.run;
  return (
    <div
      className="timeline"
      role="progressbar"
      aria-label={`${stream.title}: how much is made`}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(percent)}
      aria-valuetext={`${percent.toFixed(0)}% made, player at ${formatClock(stream.position_s * 1000)}`}
    >
      {stream.made.map(({ start, end }) => (
        <span key={start} className="timeline-made" style={{ left: at(start), width: `calc(${at(end)} - ${at(start)})` }} />
      ))}
      {run && stream.state === "converting" && <span className="timeline-edge" style={{ left: at(run.end) }} />}
      <span className="timeline-player" style={{ left: at(stream.position_s) }} />
    </div>
  );
}

/** What a stream turns into: the arrow stays with it when the line wraps. */
function Becomes({ converted, children }: { converted: boolean; children: string }) {
  return (
    <span className={`stream-output ${converted ? "is-converted" : ""}`}>
      <span className="stream-arrow" aria-label="becomes">
        →
      </span>
      {children}
    </span>
  );
}

/** FFmpeg's own figure when it gives one (it doesn't for HLS), else the average of what is on disk. */
function bitrate(stream: ActiveStream, madeSeconds: number): { value: string; detail?: string } {
  if (stream.bitrate_kbps) return { value: formatBitrate(stream.bitrate_kbps * 1000) };
  if (madeSeconds < 1) return { value: "–" };
  return { value: formatBitrate((stream.disk_bytes * 8) / madeSeconds), detail: "average on disk" };
}

function Stat({ term, value, detail }: { term: string; value: string; detail?: string | undefined }) {
  return (
    <div>
      <dt>{term}</dt>
      <dd>
        {value}
        {detail && <span>{detail}</span>}
      </dd>
    </div>
  );
}

function ScanSection({ scan }: { scan: ScanStatus }) {
  const qc = useQueryClient();
  const start = useMutation({
    mutationFn: client.startScan,
    onSuccess: (status) => {
      qc.setQueryData(queryKeys.scan, status);
      void qc.invalidateQueries({ queryKey: queryKeys.adminActivity });
    },
  });
  const running = isScanRunning(scan);
  const last = scan.last;
  return (
    <section className="settings-section" aria-labelledby="activity-scan">
      <div>
        <h2 id="activity-scan">Library scan</h2>
        <p className="settings-section-note">Finds new, changed and removed files.</p>
      </div>
      <div className="activity-task">
        {scan.state === "running" ? (
          <>
            <p>
              <strong>{PHASE[scan.phase]}</strong>
              {scan.total > 0 && ` · ${scan.done.toLocaleString()} of ${scan.total.toLocaleString()}`}
              {scan.queued && " · another scan is queued"}
            </p>
            <Bar value={scan.total > 0 ? scan.done / scan.total : null} label="Scan progress" />
          </>
        ) : last ? (
          <p>
            Last scan {formatLastSeen(last.finished_at, Date.now()).toLowerCase()}, in {formatDuration(last.duration_ms)}: {last.added} added,{" "}
            {last.changed} changed, {last.removed} removed, {last.unchanged.toLocaleString()} unchanged
            {last.failed > 0 && `, ${last.failed} unreadable`}.
          </p>
        ) : (
          <p>No scan has finished yet.</p>
        )}
        <div className="form-actions">
          <button type="button" className="btn btn-secondary btn-small" disabled={running || start.isPending} onClick={() => start.mutate()}>
            {running ? "Scanning…" : "Scan now"}
          </button>
        </div>
      </div>
    </section>
  );
}

const PHASE: Record<Extract<ScanStatus, { state: "running" }>["phase"], string> = {
  discovering: "Looking for files",
  reading: "Reading files",
  artwork: "Making artwork",
  cleaning_up: "Tidying up",
};

function PreviewSection({ previews }: { previews: PreviewQueue }) {
  return (
    <section className="settings-section" aria-labelledby="activity-previews">
      <div>
        <h2 id="activity-previews">Seek previews</h2>
        <p className="settings-section-note">Pictures above the scrubber, made one video at a time.</p>
      </div>
      <div className="activity-task">
        {!previews.enabled ? (
          <p>
            Off. Turn them on in <Link to="/settings/transcoding">Transcoding</Link>.
          </p>
        ) : previews.current ? (
          <>
            <p>
              Working on <strong className="activity-file">{previews.current}</strong>
              {previews.queued > 0 && ` · ${previews.queued.toLocaleString()} waiting`}
            </p>
            <Bar value={null} label="Making seek previews" />
          </>
        ) : (
          <p>{previews.queued > 0 ? `${previews.queued.toLocaleString()} waiting.` : "Every video has them."}</p>
        )}
        <p className="settings-note">
          Since the server started: {previews.made.toLocaleString()} made{previews.failed > 0 && `, ${previews.failed} couldn't be read`}.
        </p>
      </div>
    </section>
  );
}

/** A bar for a task; `null` when how far along can't be known. */
function Bar({ value, label }: { value: number | null; label: string }) {
  return (
    <div
      className={`activity-bar ${value === null ? "is-indeterminate" : ""}`}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value === null ? undefined : Math.round(value * 100)}
    >
      <span style={value === null ? undefined : { width: `${value * 100}%` }} />
    </div>
  );
}

/** Seconds made past the player without a gap, or `null` when the player's segment isn't made. */
function aheadOfPlayer(stream: ActiveStream): number | null {
  const span = stream.made.find(({ start, end }) => start <= stream.position_s && stream.position_s < end);
  return span ? span.end - stream.position_s : null;
}

function describeVideo(stream: ActiveStream): string {
  const v = stream.source_video;
  if (!v) return "No picture";
  return [
    v.codec,
    v.bit_depth && v.bit_depth > 8 ? `${v.bit_depth}-bit` : null,
    `${v.width}×${v.height}`,
    v.frame_rate ? `${Number(v.frame_rate.toFixed(3))} fps` : null,
    v.hdr,
  ]
    .filter(Boolean)
    .join(" · ");
}

function channelName(channels: number): string {
  return channels === 1 ? "mono" : channels === 2 ? "stereo" : channels === 6 ? "5.1" : channels === 8 ? "7.1" : `${channels} channels`;
}

function speedWord(stream: ActiveStream): string | undefined {
  if (stream.speed == null || stream.state !== "converting") return undefined;
  return stream.speed >= 1 ? "keeping ahead" : "slower than playback";
}
