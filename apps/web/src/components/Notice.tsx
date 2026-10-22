import { AlertIcon } from "./Icon";

/** Why something can't happen, then what will change that. */
export interface NoticeText {
  problem: readonly string[];
  hint: string;
}

export function Notice({ text, id }: { text: NoticeText; id?: string }) {
  return (
    <div className="notice" id={id} role="status">
      <AlertIcon />
      <div>
        {text.problem.map((line) => (
          <p key={line}>{line}</p>
        ))}
        <p className="notice-hint">{text.hint}</p>
      </div>
    </div>
  );
}
