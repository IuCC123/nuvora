import { decodingInfo } from '@amazon-devices/react-native-w3cmedia';
import {
  createClient,
  type AudioSupport,
  type DeviceProfile,
  type FetchLike,
  type Image,
  type NuvoraClient,
  type VideoSupport,
} from '@nuvora/core';

/** A Nuvora server this TV connects to. `id` recognizes it at a new address. */
export interface Server {
  url: string;
  name: string;
  id: string | null;
}

const fetchLike: FetchLike = (url, init) => fetch(url, init as RequestInit);

/**
 * What this TV decodes, asked of the platform once at startup. No containers:
 * every file comes as HLS, which the player streams with the session token on
 * each request. The server only repackages what the TV decodes and converts
 * the rest.
 */
export async function probeDevice(): Promise<DeviceProfile> {
  const decodes = async (contentType: string, width: number, height: number, transferFunction?: 'pq') => {
    try {
      const info = await decodingInfo({
        type: 'media-source',
        video: { contentType, width, height, bitrate: 20_000_000, framerate: 30, hasAlphaChannel: false, ...(transferFunction && { transferFunction }) },
      } as Parameters<typeof decodingInfo>[0]);
      return info.supported;
    } catch {
      return false;
    }
  };
  const hears = async (contentType: string, channels: number) => {
    try {
      const info = await decodingInfo({
        type: 'media-source',
        audio: { contentType, channels: String(channels), bitrate: 640_000, samplerate: 48_000, spatialRendering: false },
      } as Parameters<typeof decodingInfo>[0]);
      return info.supported;
    } catch {
      return false;
    }
  };

  const video: VideoSupport[] = [];
  const h264 = (await decodes('video/mp4; codecs="avc1.640033"', 3840, 2160))
    ? { max_width: 3840, max_height: 2160 }
    : { max_width: 1920, max_height: 1080 };
  video.push({ codec: 'h264', ...h264, max_bit_depth: 8, hdr: [] });
  if (await decodes('video/mp4; codecs="hvc1.2.4.L153.B0"', 3840, 2160)) {
    const hdr10 = await decodes('video/mp4; codecs="hvc1.2.4.L153.B0"', 3840, 2160, 'pq');
    video.push({ codec: 'hevc', max_width: 3840, max_height: 2160, max_bit_depth: 10, hdr: hdr10 ? ['hdr10'] : [] });
  } else if (await decodes('video/mp4; codecs="hvc1.1.6.L120.B0"', 1920, 1080)) {
    video.push({ codec: 'hevc', max_width: 1920, max_height: 1080, max_bit_depth: 8, hdr: [] });
  }

  const audio: AudioSupport[] = [{ codec: 'aac', max_channels: (await hears('audio/mp4; codecs="mp4a.40.2"', 6)) ? 6 : 2 }];
  for (const [codec, type] of [['ac3', 'ac-3'], ['eac3', 'ec-3']] as const) {
    if (await hears(`audio/mp4; codecs="${type}"`, 6)) audio.push({ codec, max_channels: 6 });
  }
  return { containers: [], video, audio };
}

export function clientFor(baseUrl: string, device: DeviceProfile, token?: string, onUnauthorized?: () => void): NuvoraClient {
  return createClient({ baseUrl, fetch: fetchLike, device, ...(token && { token }), ...(onUnauthorized && { onUnauthorized }) });
}

/**
 * The addresses to try for what someone typed with a remote: `192.168.1.20`,
 * `192.168.1.20:8640`, `nuvora.example.com` or a full URL. A bare host is tried
 * on Nuvora's port first, then as a website. (The Fire TV app's rule.)
 */
export function serverCandidates(typed: string): string[] {
  const input = typed.trim().replace(/\/+$/, '').replace(/\s/g, '');
  if (!input) return [];
  if (/^https?:\/\//i.test(input)) return [input];
  const host = input.split('/')[0]!;
  const hasPort = /:\d+$/.test(host);
  const candidates = hasPort ? [`http://${input}`, `https://${input}`] : [`http://${host}:8640`, `https://${input}`, `http://${input}`];
  return [...new Set(candidates)];
}

/** The first candidate that answers like a Nuvora server, set up and ready to sign in to. */
export async function findServer(typed: string, device: DeviceProfile): Promise<Server> {
  const candidates = serverCandidates(typed);
  if (!candidates.length) throw new Error("Type your server's address, like 192.168.1.20:8640.");
  let last: unknown = null;
  for (const url of candidates) {
    let state;
    try {
      state = await clientFor(url, device).authState();
    } catch (e) {
      last = e;
      continue;
    }
    if (state.setup_required) throw new Error(`This server isn't set up yet. Open ${url} in a browser to create the first account.`);
    return { url, name: state.server_name, id: state.server_id || null };
  }
  // fetch fails with a TypeError when nothing answers; anything else means something did.
  throw new Error(
    last instanceof TypeError
      ? `Couldn't reach a Nuvora server at ${typed.trim()}. Check the address and that the server is running.`
      : `Something answered at ${typed.trim()}, but it isn't a Nuvora server.`,
  );
}

/** An artwork as an image source; artwork needs the session, like everything else. */
export function imageSource(server: string, token: string, image: Image, size: 'small' | 'large' = 'small') {
  return { uri: `${server}/api/images/${image.id}/${size}`, headers: { Authorization: `Bearer ${token}` } };
}

/** The TV's language as a BCP 47 tag (`"en-US"`), or English when it won't say. */
export function deviceLanguage(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().locale || 'en';
  } catch {
    return 'en';
  }
}
