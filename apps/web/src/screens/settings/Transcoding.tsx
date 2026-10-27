import { useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { queryKeys, type HardwareEncoding, type Tuning, type TuningResponse } from "@nuvora/core";
import { client, queries } from "../../nuvora";
import { FormError, RangeField, SelectField, SwitchField, TextField } from "../../components/Field";
import { AlertIcon } from "../../components/Icon";
import { serverError } from "../../lib/serverError";
import { LoadError, Saved, SettingsPage } from "./SettingsPage";

export function SettingsTranscoding() {
  const tuning = useQuery(queries.adminTuning());
  return (
    <SettingsPage
      title="Transcoding"
      lede="How the server converts video a device can't play as it is, how far ahead it works, and what runs in the background. New settings apply to the next video someone starts."
    >
      {tuning.isPending ? (
        <div className="server-facts-skeleton" aria-busy="true" aria-label="Loading transcoding settings" />
      ) : tuning.isError ? (
        <LoadError error={tuning.error} retry={() => void tuning.refetch()} />
      ) : (
        <TuningForm current={tuning.data} />
      )}
    </SettingsPage>
  );
}

const FIELDS = [
  "vaapi_device",
  "x264_crf",
  "nvenc_preset",
  "nvenc_cq",
  "target_mbps",
  "max_mbps",
  "threads",
  "stereo_kbps",
  "surround_kbps",
  "segment_seconds",
  "buffer_ahead_s",
  "keep_behind_s",
  "catch_up_s",
  "idle_timeout_s",
  "streams_per_user",
  "max_conversions",
  "scan_every_hours",
  "max_height",
] as const satisfies readonly (keyof Tuning)[];

function TuningForm({ current }: { current: TuningResponse }) {
  const qc = useQueryClient();
  const [draft, setDraft] = useState<Tuning>(current.settings);
  const save = useMutation({
    mutationFn: client.admin.updateTuning,
    onSuccess: (saved) => {
      qc.setQueryData(queryKeys.adminTuning, saved);
      setDraft(saved.settings);
    },
  });
  const set = <K extends keyof Tuning>(key: K, value: Tuning[K]) => {
    save.reset();
    setDraft((d) => ({ ...d, [key]: value }));
  };
  const failed = serverError(save.error, FIELDS);
  const dirty = !same(draft, current.settings);
  const atDefaults = same(draft, current.defaults);
  const { encoder } = current;

  return (
    <form
      className="tuning-form"
      noValidate
      onSubmit={(e) => {
        e.preventDefault();
        save.mutate(draft);
      }}
    >
      <Section id="tuning-encoder" title="Encoder" note="What turns a picture a device can't decode into H.264.">
        <EncoderStatus encoder={encoder} />
        <SelectField
          label="Convert video with"
          value={encoder.pinned ?? draft.hardware_encoding}
          disabled={encoder.pinned !== null && encoder.pinned !== undefined}
          hint={
            encoder.pinned
              ? "Set by hardware_encoding in nuvora.toml or --hardware-encoding, which win over this page."
              : "Automatic tries this machine's graphics chip with a test clip, and uses the processor if none works."
          }
          onChange={(e) => set("hardware_encoding", e.currentTarget.value as HardwareEncoding)}
        >
          <option value="auto">Automatic</option>
          <option value="off">The processor (x264)</option>
          <option value="videotoolbox">VideoToolbox (Apple graphics)</option>
          <option value="nvenc">NVENC (NVIDIA graphics)</option>
          <option value="vaapi">VA-API (Intel or AMD graphics, Linux)</option>
        </SelectField>
        {(draft.hardware_encoding === "vaapi" || encoder.active.startsWith("VA-API")) && (
          <TextField
            label="VA-API device"
            spellCheck={false}
            autoComplete="off"
            value={draft.vaapi_device}
            onChange={(e) => set("vaapi_device", e.currentTarget.value)}
            hint="The render node to open. Most machines have one, /dev/dri/renderD128."
            error={failed.field.vaapi_device}
          />
        )}
        <SwitchField
          label="Decode on the graphics chip too"
          hint="VideoToolbox only. Off, the processor reads the source and the chip only encodes."
          checked={draft.hardware_decoding}
          onChange={(v) => set("hardware_decoding", v)}
        />
        <RangeField
          label="Encoder threads"
          min={0}
          max={64}
          value={draft.threads}
          format={(n) => (n === 0 ? "Automatic" : String(n))}
          hint="Automatic lets FFmpeg choose. Fewer leaves more of the processor for everything else."
          error={failed.field.threads}
          onChange={(v) => set("threads", v)}
        />
      </Section>

      <Section id="tuning-picture" title="Picture" note="Size and quality of converted video. Copied video is never touched.">
        <SelectField
          label="Largest converted picture"
          value={String(draft.max_height)}
          hint="Smaller converts faster and needs less of the network. Sources smaller than this keep their own size."
          error={failed.field.max_height}
          onChange={(e) => set("max_height", Number(e.currentTarget.value))}
        >
          <option value="2160">Original, up to 4K</option>
          <option value="1440">1440p</option>
          <option value="1080">1080p</option>
          <option value="720">720p</option>
          <option value="480">480p</option>
        </SelectField>
        <SwitchField
          label="Deinterlace"
          hint="Smooths the combing in old TV recordings and DVDs. Only frames marked interlaced are changed."
          checked={draft.deinterlace}
          onChange={(v) => set("deinterlace", v)}
        />

        <h3 className="tuning-subhead">Processor (x264)</h3>
        <SelectField
          label="Preset"
          value={draft.x264_preset}
          hint="Slower presets look better at the same size, and need much more of the processor."
          onChange={(e) => set("x264_preset", e.currentTarget.value as Tuning["x264_preset"])}
        >
          {X264_PRESETS.map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </SelectField>
        <RangeField
          label="Quality (CRF)"
          min={0}
          max={51}
          value={draft.x264_crf}
          format={(n) => `${n} · ${crfWord(n)}`}
          hint="Lower is better and bigger. 18 to 23 suits most video."
          error={failed.field.x264_crf}
          onChange={(v) => set("x264_crf", v)}
        />
        <SelectField
          label="Tune for"
          value={draft.x264_tune}
          hint="Tells x264 what the video looks like, so it spends bits where they show."
          onChange={(e) => set("x264_tune", e.currentTarget.value as Tuning["x264_tune"])}
        >
          <option value="none">Anything</option>
          <option value="film">Film</option>
          <option value="animation">Animation and anime</option>
          <option value="grain">Grainy film</option>
          <option value="stillimage">Slideshows</option>
          <option value="fastdecode">Weak players (easier to decode)</option>
        </SelectField>

        <h3 className="tuning-subhead">NVIDIA (NVENC)</h3>
        <RangeField
          label="Preset"
          min={1}
          max={7}
          value={draft.nvenc_preset}
          format={(n) => `P${n} · ${n <= 2 ? "fastest" : n <= 4 ? "balanced" : n <= 6 ? "better" : "best"}`}
          error={failed.field.nvenc_preset}
          onChange={(v) => set("nvenc_preset", v)}
        />
        <RangeField
          label="Quality (CQ)"
          min={1}
          max={51}
          value={draft.nvenc_cq}
          format={(n) => `${n} · ${crfWord(n)}`}
          hint="Lower is better and bigger."
          error={failed.field.nvenc_cq}
          onChange={(v) => set("nvenc_cq", v)}
        />

        <h3 className="tuning-subhead">Bitrate</h3>
        <div className="form-row">
          <MbpsField
            label="Target"
            value={draft.target_mbps}
            hint="VideoToolbox and VA-API aim for this."
            error={failed.field.target_mbps}
            onChange={(v) => set("target_mbps", v)}
          />
          <MbpsField
            label="Ceiling"
            value={draft.max_mbps}
            hint="No encoder goes above this, even in busy scenes."
            error={failed.field.max_mbps}
            onChange={(v) => set("max_mbps", v)}
          />
        </div>
      </Section>

      <Section id="tuning-audio" title="Audio" note="Only for audio that gets converted. Audio a device plays as it is passes through untouched.">
        <div className="form-row">
          <SelectField
            label="Stereo bitrate"
            value={String(draft.stereo_kbps)}
            error={failed.field.stereo_kbps}
            onChange={(e) => set("stereo_kbps", Number(e.currentTarget.value))}
          >
            {withValue(STEREO_KBPS, draft.stereo_kbps).map((k) => (
              <option key={k} value={k}>
                {k} kb/s
              </option>
            ))}
          </SelectField>
          <SelectField
            label="Surround, per channel"
            value={String(draft.surround_kbps)}
            hint={`${draft.surround_kbps * 6} kb/s for 5.1`}
            error={failed.field.surround_kbps}
            onChange={(e) => set("surround_kbps", Number(e.currentTarget.value))}
          >
            {withValue(SURROUND_KBPS, draft.surround_kbps).map((k) => (
              <option key={k} value={k}>
                {k} kb/s
              </option>
            ))}
          </SelectField>
        </div>
        <SwitchField
          label="Mix surround down to stereo"
          hint="For TVs and headphones with two speakers."
          checked={draft.downmix_stereo}
          onChange={(v) => set("downmix_stereo", v)}
        />
        <SwitchField
          label="Night mode"
          hint="Brings loud scenes down and quiet speech up, so you don't reach for the remote."
          checked={draft.night_mode}
          onChange={(v) => set("night_mode", v)}
        />
      </Section>

      <Section id="tuning-streaming" title="Streaming" note="How each stream is cut, how far ahead the server works, and when it stops.">
        <RangeField
          label="Segment length"
          min={2}
          max={10}
          value={draft.segment_seconds}
          format={(n) => `${n} s`}
          hint="Converted video only. Shorter starts and jumps sooner; longer is a little more efficient."
          error={failed.field.segment_seconds}
          onChange={(v) => set("segment_seconds", v)}
        />
        <SwitchField
          label="Pause when far enough ahead"
          hint="Off, the server converts to the end as fast as it can, whatever the player needs."
          checked={draft.throttle}
          onChange={(v) => set("throttle", v)}
        />
        <RangeField
          label="Work ahead of the player"
          min={30}
          max={3600}
          step={30}
          value={draft.buffer_ahead_s}
          format={span}
          hint={draft.throttle ? `Pauses this far ahead and starts again ${span(draft.buffer_ahead_s / 2)} ahead.` : "Only used while pausing is on."}
          error={failed.field.buffer_ahead_s}
          onChange={(v) => set("buffer_ahead_s", v)}
        />
        <RangeField
          label="Keep behind the player"
          min={60}
          max={7200}
          step={60}
          value={draft.keep_behind_s}
          format={span}
          hint="Older segments are deleted. More keeps jumping back instant, and uses more disk."
          error={failed.field.keep_behind_s}
          onChange={(v) => set("keep_behind_s", v)}
        />
        <RangeField
          label="Wait instead of restarting"
          min={0}
          max={120}
          step={5}
          value={draft.catch_up_s}
          format={(n) => (n === 0 ? "Never wait" : span(n))}
          hint="A jump this far past what is made waits for it. Further jumps restart FFmpeg there."
          error={failed.field.catch_up_s}
          onChange={(v) => set("catch_up_s", v)}
        />
        <RangeField
          label="Stop an unwatched stream after"
          min={30}
          max={1800}
          step={30}
          value={draft.idle_timeout_s}
          format={span}
          error={failed.field.idle_timeout_s}
          onChange={(v) => set("idle_timeout_s", v)}
        />
        <div className="form-row">
          <RangeField
            label="Streams per person"
            min={1}
            max={10}
            value={draft.streams_per_user}
            hint="One more stops their oldest."
            error={failed.field.streams_per_user}
            onChange={(v) => set("streams_per_user", v)}
          />
          <RangeField
            label="Conversions at once"
            min={0}
            max={32}
            value={draft.max_conversions}
            format={(n) => (n === 0 ? "No limit" : String(n))}
            hint="Across everyone. Copied streams don't count."
            error={failed.field.max_conversions}
            onChange={(v) => set("max_conversions", v)}
          />
        </div>
        <SelectField
          label="FFmpeg log"
          value={draft.ffmpeg_log_level}
          hint="Written to ffmpeg.log in each stream's folder, under transcode in the data folder."
          onChange={(e) => set("ffmpeg_log_level", e.currentTarget.value as Tuning["ffmpeg_log_level"])}
        >
          <option value="error">Errors only</option>
          <option value="warning">Warnings</option>
          <option value="info">Everything</option>
          <option value="verbose">Everything, in detail</option>
        </SelectField>
      </Section>

      <Section id="tuning-library" title="Library tasks" note="Work the server does in the background.">
        <SelectField
          label="Scan libraries"
          value={String(draft.scan_every_hours)}
          hint="Counted from the end of the last scan. Libraries are always scanned when the server starts."
          error={failed.field.scan_every_hours}
          onChange={(e) => set("scan_every_hours", Number(e.currentTarget.value))}
        >
          {withValue(SCAN_HOURS, draft.scan_every_hours).map((h) => (
            <option key={h} value={h}>
              {h === 0 ? "Only at startup and when asked" : h === 1 ? "Every hour" : h === 168 ? "Every week" : `Every ${h} hours`}
            </option>
          ))}
        </SelectField>
        <SwitchField
          label="Make seek previews"
          hint="The pictures above the scrubber. Off, nothing new is made; videos keep the ones they have."
          checked={draft.seek_previews}
          onChange={(v) => set("seek_previews", v)}
        />
      </Section>

      <div className="tuning-actions">
        <FormError message={failed.form} />
        <div className="form-actions">
          <button type="submit" className="btn btn-primary" disabled={!dirty || save.isPending} aria-busy={save.isPending}>
            {save.isPending ? "Saving…" : "Save changes"}
          </button>
          <button
            type="button"
            className="btn btn-secondary"
            disabled={!dirty || save.isPending}
            onClick={() => {
              save.reset();
              setDraft(current.settings);
            }}
          >
            Discard
          </button>
          <Saved show={save.isSuccess && !dirty} />
          <span className="form-actions-spacer" />
          <button
            type="button"
            className="btn btn-secondary"
            disabled={atDefaults || save.isPending}
            onClick={() => {
              save.reset();
              setDraft(current.defaults);
            }}
          >
            Use the defaults
          </button>
        </div>
      </div>
    </form>
  );
}

function Section({ id, title, note, children }: { id: string; title: string; note: string; children: ReactNode }) {
  return (
    <section className="settings-section" aria-labelledby={id}>
      <div>
        <h2 id={id}>{title}</h2>
        <p className="settings-section-note">{note}</p>
      </div>
      <div className="form settings-form">{children}</div>
    </section>
  );
}

function EncoderStatus({ encoder }: { encoder: TuningResponse["encoder"] }) {
  return (
    <div className="encoder-status" role="status">
      <span className={`encoder-chip ${encoder.hardware ? "is-hardware" : ""}`}>{encoder.hardware ? "Graphics chip" : "Processor"}</span>
      <p>
        New conversions use <strong>{encoder.active}</strong>.
      </p>
      {encoder.problem && (
        <p className="encoder-problem">
          <AlertIcon size={16} />
          {encoder.problem}
        </p>
      )}
    </div>
  );
}

/** Mbit/s, or empty for automatic. */
function MbpsField({
  label,
  value,
  hint,
  error,
  onChange,
}: {
  label: string;
  value: number | null | undefined;
  hint: string;
  error: string | undefined;
  onChange: (value: number | null) => void;
}) {
  return (
    <TextField
      label={`${label} (Mb/s)`}
      type="number"
      inputMode="numeric"
      min={1}
      max={200}
      placeholder="Automatic"
      value={value ?? ""}
      hint={hint}
      error={error}
      onChange={(e) => {
        const raw = e.currentTarget.value.trim();
        onChange(raw === "" ? null : Math.round(Number(raw)));
      }}
    />
  );
}

const X264_PRESETS: readonly (readonly [Tuning["x264_preset"], string])[] = [
  ["ultrafast", "Ultra fast"],
  ["superfast", "Super fast"],
  ["veryfast", "Very fast (default)"],
  ["faster", "Faster"],
  ["fast", "Fast"],
  ["medium", "Medium"],
  ["slow", "Slow"],
  ["slower", "Slower"],
  ["veryslow", "Very slow"],
];

const STEREO_KBPS = [64, 96, 128, 160, 192, 256, 320, 384, 448, 512];
const SURROUND_KBPS = [32, 48, 64, 80, 96, 128, 160, 192];
const SCAN_HOURS = [0, 1, 3, 6, 12, 24, 48, 168];

/** The choices, plus `value` if it isn't one of them (saved by hand, or by a newer version). */
function withValue(choices: readonly number[], value: number): number[] {
  return choices.includes(value) ? [...choices] : [...choices, value].sort((a, b) => a - b);
}

function crfWord(n: number): string {
  if (n <= 17) return "near lossless";
  if (n <= 23) return "high";
  if (n <= 28) return "medium";
  if (n <= 35) return "low";
  return "very low";
}

/** `"45 s"`, `"3 min"`, `"2 min 30 s"`, `"1 h 30 min"`. */
function span(seconds: number): string {
  const s = Math.round(seconds);
  if (s < 60) return `${s} s`;
  if (s < 3600) return s % 60 === 0 ? `${s / 60} min` : `${Math.floor(s / 60)} min ${s % 60} s`;
  const m = Math.round((s % 3600) / 60);
  return m === 0 ? `${Math.floor(s / 3600)} h` : `${Math.floor(s / 3600)} h ${m} min`;
}

function same(a: Tuning, b: Tuning): boolean {
  return (Object.keys(b) as (keyof Tuning)[]).every((k) => (a[k] ?? null) === (b[k] ?? null));
}
