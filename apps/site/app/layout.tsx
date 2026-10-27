import type { Metadata, Viewport } from "next";
import { Archivo, Inter, JetBrains_Mono } from "next/font/google";
import { SmoothScroll } from "@/components/SmoothScroll";
import "lenis/dist/lenis.css";
import "./globals.css";

const inter = Inter({ subsets: ["latin"], variable: "--font-inter", display: "swap" });
const archivo = Archivo({ subsets: ["latin"], axes: ["wdth"], variable: "--font-archivo", display: "swap" });
const mono = JetBrains_Mono({ subsets: ["latin"], variable: "--font-mono", display: "swap" });

const title = "Nuvora: your own media server";
const description =
  "A free, open-source media server for your movies, shows and anime. Plex's polish, Jellyfin's freedom, and no account with anyone but you.";
// Made by `bun run og` from the Home screen on this page.
const image = { url: "/og.jpg", width: 1200, height: 630, alt: "Nuvora's Home screen next to the line: Plex's polish. Jellyfin's freedom." };

export const metadata: Metadata = {
  metadataBase: new URL("https://getnuvora.online"),
  title,
  description,
  openGraph: { title, description, url: "/", siteName: "Nuvora", type: "website", images: [image] },
  twitter: { card: "summary_large_image", title, description, images: [image] },
};

export const viewport: Viewport = {
  themeColor: "#000000",
  colorScheme: "dark",
};

/*
 * Motion renders entrances with their starting styles inline (opacity 0, shifted, blurred),
 * so without JavaScript the page would stay hidden. This undoes those styles when scripts are off.
 */
const NO_SCRIPT = `<style>
[style*="opacity:0"]{opacity:1!important;transform:none!important;filter:none!important}
.word{transform:none!important}
.cmp-cell path{stroke-dasharray:none}
</style>`;

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${inter.variable} ${archivo.variable} ${mono.variable}`} suppressHydrationWarning>
      <head>
        {/* Lets CSS tell a scripted page from one without scripts before anything paints. */}
        <script dangerouslySetInnerHTML={{ __html: "document.documentElement.classList.add('js')" }} />
        <noscript dangerouslySetInnerHTML={{ __html: NO_SCRIPT }} />
      </head>
      <body>
        <SmoothScroll>{children}</SmoothScroll>
      </body>
    </html>
  );
}
