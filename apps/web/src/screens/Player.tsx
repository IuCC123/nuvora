import { useQuery } from "@tanstack/react-query";
import { useNavigate, useParams, useSearch } from "@tanstack/react-router";
import { client, queries } from "../nuvora";

/**
 * Full-screen `<video>` with native controls (Step 1). The URL comes from the
 * cached item's playback decision; the server does Range, so seeking is native.
 * Escape / Back returns to the detail screen.
 */
export function Player() {
  const { id } = useParams({ from: "/movie/$id/play" });
  const { file } = useSearch({ from: "/movie/$id/play" });
  const navigate = useNavigate();
  const { data } = useQuery(queries.item(id));
  const version = data?.versions.find((v) => v.file_id === file);
  if (!version || version.playback.kind !== "direct_play") return null; // TODO: route back with a message
  return (
    <video
      src={client.resolve(version.playback.url)}
      autoPlay
      controls
      preload="auto"
      onKeyDown={(e) => e.key === "Escape" && navigate({ to: "/movie/$id", params: { id } })}
      style={{ position: "fixed", inset: 0, width: "100%", height: "100%", background: "black" }}
    />
  );
}
