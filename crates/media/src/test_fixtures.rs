//! Media files generated with ffmpeg for tests: 2 s of `testsrc2` plus a
//! `sine` tone, encoded per fixture. Generated into a caller-owned directory
//! so every test owns its files and nothing depends on `/tmp`.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy)]
pub enum Fixture {
    H264AacMp4,
    Hevc10Eac3Mkv,
    Vp9OpusWebm,
    Av1FlacMkv,
    Mpeg4Ac3Avi,
    SubsMkv,
    CoverMp4,
}

const VIDEO_SRC: &[&str] = &[
    "-f",
    "lavfi",
    "-i",
    "testsrc2=size=320x180:rate=24000/1001:duration=2",
];
const AUDIO_SRC: &[&str] = &[
    "-f",
    "lavfi",
    "-i",
    "sine=frequency=440:sample_rate=48000:duration=2",
];

impl Fixture {
    pub fn file_name(self) -> &'static str {
        match self {
            Fixture::H264AacMp4 => "h264_aac.mp4",
            Fixture::Hevc10Eac3Mkv => "hevc10_eac3.mkv",
            Fixture::Vp9OpusWebm => "vp9_opus.webm",
            Fixture::Av1FlacMkv => "av1_flac.mkv",
            Fixture::Mpeg4Ac3Avi => "mpeg4_ac3.avi",
            Fixture::SubsMkv => "subs.mkv",
            Fixture::CoverMp4 => "cover.mp4",
        }
    }

    /// Write the fixture into `dir` and return its path.
    pub fn generate(self, dir: &Path) -> PathBuf {
        let out = dir.join(self.file_name());
        let src: Vec<&str> = [VIDEO_SRC, AUDIO_SRC].concat();
        match self {
            Fixture::H264AacMp4 => {
                let args = [
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc2=size=1280x720:rate=24000/1001:duration=2",
                ];
                let args: Vec<&str> = [&args[..], AUDIO_SRC].concat();
                ffmpeg(
                    &[
                        &args[..],
                        &[
                            "-c:v",
                            "libx264",
                            "-preset",
                            "veryfast",
                            "-profile:v",
                            "high",
                            "-pix_fmt",
                            "yuv420p",
                            "-c:a",
                            "aac",
                            "-ac",
                            "2",
                        ],
                    ]
                    .concat(),
                    &out,
                );
            }
            Fixture::Hevc10Eac3Mkv => ffmpeg(
                &[
                    &src[..],
                    &[
                        "-c:v",
                        "libx265",
                        "-preset",
                        "ultrafast",
                        "-pix_fmt",
                        "yuv420p10le",
                        "-x265-params",
                        "colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:log-level=error",
                        "-c:a",
                        "eac3",
                        "-ac",
                        "6",
                    ],
                ]
                .concat(),
                &out,
            ),
            Fixture::Vp9OpusWebm => ffmpeg(
                &[
                    &src[..],
                    &[
                        "-c:v",
                        "libvpx-vp9",
                        "-deadline",
                        "realtime",
                        "-cpu-used",
                        "8",
                        "-c:a",
                        "libopus",
                    ],
                ]
                .concat(),
                &out,
            ),
            Fixture::Av1FlacMkv => ffmpeg(
                &[
                    &src[..],
                    &["-c:v", "libsvtav1", "-preset", "12", "-c:a", "flac"],
                ]
                .concat(),
                &out,
            ),
            Fixture::Mpeg4Ac3Avi => ffmpeg(
                &[&src[..], &["-c:v", "mpeg4", "-c:a", "ac3"]].concat(),
                &out,
            ),
            Fixture::SubsMkv => {
                let srt = dir.join("subs.srt");
                std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:01,000\nHello\n\n").unwrap();
                let srt = srt.to_str().unwrap().to_owned();
                let args: Vec<&str> = [
                    &src[..],
                    &[
                        "-f",
                        "lavfi",
                        "-i",
                        "sine=frequency=880:sample_rate=48000:duration=2",
                        "-i",
                        &srt,
                    ],
                    &[
                        "-map",
                        "0:v",
                        "-map",
                        "1:a",
                        "-map",
                        "2:a",
                        "-map",
                        "3:s",
                        "-c:v",
                        "libx264",
                        "-preset",
                        "ultrafast",
                        "-c:a",
                        "aac",
                        "-c:s",
                        "srt",
                        "-disposition:a:0",
                        "0",
                        "-disposition:a:1",
                        "default",
                        "-disposition:s:0",
                        "default+forced",
                        "-metadata:s:a:0",
                        "language=eng",
                        "-metadata:s:a:1",
                        "language=fra",
                        "-metadata:s:s:0",
                        "language=eng",
                    ],
                ]
                .concat();
                ffmpeg(&args, &out);
            }
            Fixture::CoverMp4 => {
                let plain = Fixture::H264AacMp4.generate(dir);
                let cover = dir.join("cover.jpg");
                ffmpeg(
                    &["-f", "lavfi", "-i", "testsrc2=size=64x64", "-frames:v", "1"],
                    &cover,
                );
                let (plain, cover) = (plain.to_str().unwrap(), cover.to_str().unwrap());
                ffmpeg(
                    &[
                        "-i",
                        plain,
                        "-i",
                        cover,
                        "-map",
                        "0",
                        "-map",
                        "1",
                        "-c",
                        "copy",
                        "-disposition:v:1",
                        "attached_pic",
                    ],
                    &out,
                );
            }
        }
        out
    }
}

/// Run `ffmpeg -y -nostdin -v error <args> <out>`; panics with ffmpeg's stderr on failure.
pub fn ffmpeg(args: &[&str], out: &Path) {
    let output = Command::new("ffmpeg")
        .args(["-y", "-nostdin", "-v", "error"])
        .args(args)
        .arg(out)
        .output()
        .expect("ffmpeg on PATH");
    assert!(
        output.status.success(),
        "ffmpeg {args:?} -> {}: {}",
        out.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}
