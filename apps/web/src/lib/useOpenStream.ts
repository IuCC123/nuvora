import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { client } from "../nuvora";

/** Starts playing a release while it downloads, then opens its player where the viewer stopped last time. */
export function useOpenStream() {
  const navigate = useNavigate();
  return useMutation({
    mutationFn: client.admin.openStream,
    onSuccess: (stream) => {
      const t = stream.resume_ms ? Math.floor(stream.resume_ms / 1000) : undefined;
      void navigate({ to: "/watch/$id", params: { id: stream.id }, search: t ? { t } : {} });
    },
  });
}
