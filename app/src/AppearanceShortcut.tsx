import type { SettingsGroup } from "./settingsData";

/** The header's one-click way to Appearance (0036): opens Settings at it, lit while it's open.
 *  Its own file, so the window's first chunk doesn't carry Settings. */
export function AppearanceShortcut({ active, onOpen }: { active: boolean; onOpen: (g: SettingsGroup) => void }) {
  return (
    <button type="button" className={active ? "tab on" : "tab"} onClick={() => onOpen("appearance")} title="Theme, accent, text size and cards: Settings at Appearance">
      Appearance
    </button>
  );
}
