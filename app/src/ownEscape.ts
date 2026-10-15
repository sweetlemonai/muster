// Esc owned by an in-place confirm (mu-184, review R4). An unsaved write-up and a typed comment
// listen for Esc on the whole window, as Release's confirm does, and every listener on the window
// runs whatever another one stops, so marking the key handled isn't enough: the confirm's group
// carries `data-own-esc`, and those listeners leave an Esc pressed inside it alone. One Esc then
// cancels the confirm and never also arms a discard of unsaved text.

/** The attribute that marks an element as owning the Esc pressed inside it. */
export const OWN_ESC = "data-own-esc";

/** Was this key pressed inside an element marked `OWN_ESC`? False for no target, or one that
 *  isn't an element (the window, the document). */
export function escOwned(target: EventTarget | null): boolean {
  const el = target as { closest?: (selector: string) => unknown } | null;
  return typeof el?.closest === "function" && el.closest(`[${OWN_ESC}]`) != null;
}
