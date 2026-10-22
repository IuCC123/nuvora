import { useState } from "react";
import { placeholderDataUrl, type Image, type ImageSize } from "@nuvora/core";
import { client } from "../nuvora";

/**
 * The ThumbHash placeholder paints in the first frame (it is a data URL from
 * the API response, no request), the real WebP loads lazily on top and fades in
 * over `--nv-motion-image-fade`. The box reserves its aspect ratio, so nothing
 * shifts. `decoding="async"` keeps decode off the main thread.
 */
export function Artwork({ image, size, alt }: { image: Image; size: ImageSize; alt: string }) {
  const [loaded, setLoaded] = useState(false);
  return (
    <span
      style={{
        display: "block",
        aspectRatio: String(image.aspect_ratio),
        backgroundImage: `url(${placeholderDataUrl(image)})`,
        backgroundSize: "cover",
      }}
    >
      <img
        src={client.imageUrl(image, size)}
        alt={alt}
        loading="lazy"
        decoding="async"
        onLoad={() => setLoaded(true)}
        style={{
          width: "100%",
          height: "100%",
          opacity: loaded ? 1 : 0,
          transition: "opacity var(--nv-motion-image-fade) var(--nv-motion-easing-standard)",
        }}
      />
    </span>
  );
}
