import { useContext, useState } from "react";
import { api } from "./api";
import { useT } from "./i18n";
import { askerShown, stamp } from "./lib";
import Markdown from "./Markdown";
import PickOne, { answeredText } from "./PickOne";
import { RoleNamesContext } from "./settingsData";
import { pressed } from "./shortcuts";
import type { Question, Task } from "./types";
import { Badge, Button } from "./ui";

/** One open question with a box to answer it (⌘↩ sends) and its options as chips (0049): in the
 *  pane, and in place under a block waiting on it in Waiting for you (0060). Its own file so that
 *  Waiting for you doesn't pull the lazy task pane into the startup chunk. `boxId` is the pane's
 *  only (`answerId`, what a block's Answer focuses), so two open boxes never share an id. */
export default function AnswerBox({ task, q, onChanged, keys, onLink, boxId }: { task: Task; q: Question; onChanged: () => void; keys?: Map<string, string>; onLink?: (href: string) => void; boxId?: string }) {
  const t = useT("common");
  const names = useContext(RoleNamesContext);
  const [text, setText] = useState("");
  const [state, setState] = useState<"idle" | "saving" | "done" | "error">("idle");
  const [msg, setMsg] = useState("");
  const send = async () => {
    if (!text.trim() || q.n === null) return;
    setState("saving");
    try {
      const asker = await api.answer(task.queue, task.id, q.n, text, stamp(new Date()));
      setState("done");
      setMsg(answeredText(asker));
      setText("");
      onChanged();
    } catch (e) {
      setState("error");
      setMsg(String(e));
    }
  };
  return (
    <div className={`answer${q.escalated ? " needs-you" : ""}`}>
      <p>
        {/* i18n-ignore: the question's id (Q2), as muster answer takes it */}
        <b>{`Q${q.n}`}</b> <span className="asker" dir="auto">{askerShown(q, names)}</span>
        {q.escalated && <Badge variant="warn">{t("answer.forYou")}</Badge>}
      </p>
      <Markdown text={q.choices ? q.choices.question : q.text} keys={keys} onLink={onLink} />
      {q.choices && <PickOne task={task} q={q} choices={q.choices} extra={text} onAnswered={() => setText("")} onChanged={onChanged} />}
      <textarea
        id={boxId}
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={q.choices ? t("answer.placeholderPick") : t("answer.placeholder")}
        rows={3}
        maxLength={4000}
        onKeyDown={(e) => {
          if (pressed(e, "send")) void send();
        }}
      />
      <div className="answer-bar">
        <Button disabled={!text.trim() || state === "saving"} onClick={() => void send()}>
          {state === "saving" ? t("answer.sending") : t("answer.send")}
        </Button>
        {msg && <span className={state === "error" ? "err" : "ok"}>{msg}</span>}
      </div>
    </div>
  );
}
