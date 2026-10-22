let interacted = false;
const mark = () => (interacted = true);
addEventListener("keydown", mark, { capture: true, once: true });
addEventListener("pointerdown", mark, { capture: true, once: true });

/** Whether the person has pressed a key or a pointer since the page loaded. */
export const hasInteracted = () => interacted;
