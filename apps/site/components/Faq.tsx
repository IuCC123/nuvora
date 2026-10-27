import { GITHUB } from "./Nav";
import { Reveal, SplitWords } from "./Reveal";

const QUESTIONS: { q: string; a: string; link?: { href: string; text: string } }[] = [
  {
    q: "Should I switch from Plex or Jellyfin?",
    a: "You don't have to decide up front. Nuvora only reads your media folder (Docker mounts it read-only), so it can run next to Plex or Jellyfin on the same files, on its own port. Try it on your library and keep whichever you prefer.",
  },
  {
    q: "Can I bring my watch history from Plex or Jellyfin?",
    a: "Not yet. Your library comes across as it is, because Nuvora reads the same files and finds posters and descriptions itself. Watched status and progress don't transfer, so everyone starts with a fresh Continue Watching.",
  },
  {
    q: "What hardware do I need?",
    a: "Anything that runs Docker: a PC, a Mac, a NAS or a Raspberry Pi 4 or 5, on x86-64 or ARM64. Most of the time your device plays the file as it is, which takes almost no processing. Converting video takes more. A graphics chip does it cheaply (VideoToolbox on a Mac, NVENC on NVIDIA, VA-API for Intel and AMD on Linux), while converting 4K on a Raspberry Pi or a small NAS may stutter.",
  },
  {
    q: "How do I watch away from home?",
    a: "It's free, with no pass and nothing routed through us. The easiest way is Tailscale on the server and on each device. You can also put Nuvora behind a reverse proxy with HTTPS, such as Caddy, nginx or Traefik. Don't forward port 8640 straight to the internet.",
    link: { href: `${GITHUB}#remote-access`, text: "Remote access in the guide" },
  },
  {
    q: "Does anything leave my server?",
    a: "Your accounts, progress and settings stay on it. To fill in descriptions and artwork, Nuvora looks titles up on Wikipedia, Wikidata, AniList and TVmaze, and on TMDB if you add your own key. There's no Nuvora account and no analytics.",
  },
  {
    q: "Is it really free?",
    a: "Yes, every feature. Nuvora is free software under the AGPL-3.0: no subscription, no paid tier, and the source is on GitHub.",
  },
  {
    q: "Is it good for anime?",
    a: "It's built with anime in mind. Episode names like \"Show - 01 [1080p].mkv\" are understood, AniList fills in details and voice actors, and styled .ass subtitles keep their own fonts and placement.",
  },
];

const STRUCTURED = {
  "@context": "https://schema.org",
  "@type": "FAQPage",
  mainEntity: QUESTIONS.map(({ q, a }) => ({ "@type": "Question", name: q, acceptedAnswer: { "@type": "Answer", text: a } })),
};

export function Faq() {
  return (
    <section className="section container faq" id="faq" aria-labelledby="faq-title">
      <div className="faq-head">
        <h2 className="section-title" id="faq-title">
          <SplitWords text="Questions," />
          <SplitWords text="answered." className="dim" delay={0.12} />
        </h2>
        <Reveal as="p" className="section-lede" delay={0.15}>
          Something else? Ask on <a href={`${GITHUB}/issues`}>GitHub</a>.
        </Reveal>
      </div>
      <Reveal className="faq-list" delay={0.1}>
        {QUESTIONS.map(({ q, a, link }, i) => (
          <details key={q} name="faq" open={i === 0}>
            <summary>
              {q}
              <span className="faq-icon" aria-hidden />
            </summary>
            <div className="faq-answer">
              <p>{a}</p>
              {link && (
                <a className="faq-link" href={link.href}>
                  {link.text} <span aria-hidden>→</span>
                </a>
              )}
            </div>
          </details>
        ))}
      </Reveal>
      <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(STRUCTURED) }} />
    </section>
  );
}
