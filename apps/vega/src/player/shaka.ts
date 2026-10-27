import type { VideoPlayer } from '@amazon-devices/react-native-w3cmedia';
// Built by scripts/setup-shaka.sh from Amazon's Vega patches (not committed).
import Document from '../w3cmedia/polyfills/DocumentPolyfill';
import DOMParserPolyfill from '../w3cmedia/polyfills/DOMParserPolyfill';
import Element from '../w3cmedia/polyfills/ElementPolyfill';
import MiscPolyfill from '../w3cmedia/polyfills/MiscPolyfill';
import TextDecoderPolyfill from '../w3cmedia/polyfills/TextDecoderPolyfill';
import W3CMediaPolyfill from '../w3cmedia/polyfills/W3CMediaPolyfill';

/** The part of Shaka Player this app uses. */
interface ShakaRequest {
  headers: Record<string, string>;
}
interface ShakaPlayer {
  getNetworkingEngine(): { registerRequestFilter(filter: (type: number, request: ShakaRequest) => void): void };
  configure(config: object): void;
  load(uri: string, startTime?: number): Promise<void>;
  unload(): Promise<void>;
  destroy(): Promise<void>;
}
interface Shaka {
  polyfill: { installAll(): void };
  Player: new (media: VideoPlayer) => ShakaPlayer;
  net: {
    HttpFetchPlugin: { parse: unknown };
    NetworkingEngine: {
      registerScheme(scheme: string, plugin: unknown, priority: number, progress: boolean): void;
      unregisterScheme(scheme: string): void;
      PluginPriority: { APPLICATION: number };
    };
  };
}
// eslint-disable-next-line @typescript-eslint/no-var-requires
const shaka: Shaka = require('../w3cmedia/shakaplayer/dist/shaka-player.compiled');

let installed = false;

function install() {
  if (installed) return;
  Document.install();
  Element.install();
  TextDecoderPolyfill.install();
  W3CMediaPolyfill.install();
  MiscPolyfill.install();
  DOMParserPolyfill.install();
  shaka.polyfill.installAll();
  // Amazon's setup: fetch, not XHR, for every request.
  const { NetworkingEngine, HttpFetchPlugin } = shaka.net;
  for (const scheme of ['http', 'https']) {
    NetworkingEngine.unregisterScheme(scheme);
    NetworkingEngine.registerScheme(scheme, HttpFetchPlugin.parse, NetworkingEngine.PluginPriority.APPLICATION, true);
  }
  installed = true;
}

export interface Stream {
  stop(): Promise<void>;
}

/**
 * Streams a Nuvora HLS playlist into `video` from `startSeconds`, with the
 * session token on every request (playlist and segments alike).
 */
export async function streamHls(video: VideoPlayer, url: string, token: string, startSeconds: number): Promise<Stream> {
  install();
  const player = new shaka.Player(video);
  player.getNetworkingEngine().registerRequestFilter((_type, request) => {
    request.headers.Authorization = `Bearer ${token}`;
  });
  player.configure({
    // A segment after a long seek can take a few seconds while the server restarts its conversion.
    streaming: { bufferingGoal: 30, rebufferingGoal: 2, retryParameters: { maxAttempts: 4, baseDelay: 1000, timeout: 60_000 } },
    manifest: { hls: { sequenceMode: false } },
    // One rendition per playlist: the server already chose it for this TV.
    abr: { enabled: false },
  });
  await player.load(url, startSeconds);
  return {
    stop: async () => {
      await player.unload().catch(() => undefined);
      await player.destroy().catch(() => undefined);
    },
  };
}
