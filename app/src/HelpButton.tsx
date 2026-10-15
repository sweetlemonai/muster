import { createContext, useContext } from "react";
import { HELP_TOPICS, type HelpTopic } from "./helpTopics";
import { tl } from "./i18n";

/** Opens the manual at a topic, in the Help tab; the window provides it. Nothing without one. */
export const OpenHelp = createContext<(topic: HelpTopic) => void>(() => {});

/** The small "?" beside a pane's title or in a view's toolbar: opens that part of the manual in
 *  the Help tab, with the pane left where it is. */
export function HelpButton({ topic }: { topic: HelpTopic }) {
  const open = useContext(OpenHelp);
  const label = tl("help.on", { topic: HELP_TOPICS[topic].label });
  return (
    <button type="button" className="help-q" title={label} aria-label={label} onClick={() => open(topic)}>
      ?
    </button>
  );
}
