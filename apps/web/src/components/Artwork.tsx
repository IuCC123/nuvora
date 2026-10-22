import { useState } from "react";
import { placeholderDataUrl, type Image, type ImageSize } from "@nuvora/core";
import { client } from "../nuvora";
import "../styles/artwork.css";

/**
 * The ThumbHash placeholder paints in the first frame (it is a data URL from
 * the API response, no request), the real WebP loads on top and fades in over
 * `--nv-motion-image-fade`. The box reserves its aspect ratio, so nothing
 * shifts; `fill` instead covers a positioned parent (backdrops).
 */
export function Artwork({
  image,
  size,
  alt,
  fill = false,
  eager = false,
}: {
  image: Image;
  size: ImageSize;
  alt: string;
  fill?: boolean;
  eager?: boolean;
}) {
  const [loaded, setLoaded] = useState(false);
  return (
    <span
      className={fill ? "artwork artwork-fill" : "artwork"}
      style={{ aspectRatio: fill ? undefined : String(image.aspect_ratio), backgroundImage: `url(${placeholderDataUrl(image)})` }}
    >
      <img
        src={client.imageUrl(image, size)}
        alt={alt}
        loading={eager ? "eager" : "lazy"}
        fetchPriority={eager ? "high" : "auto"}
        decoding="async"
        data-loaded={loaded}
        onLoad={() => setLoaded(true)}
      />
    </span>
  );
}
