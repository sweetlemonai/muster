// Settings › Agents › Roles (0112): each role's profile from roles.toml, a row per role with its
// name and slot count; the open one's fields inline below it (no overlay). Saved to roles.toml by
// the core, which refuses a bad value with why.
import { useEffect, useState } from "react";
import { api } from "./api";
import { Trans, useT } from "./i18n";
import { ROLE_FIELDS, roleLine, slotWords } from "./rolesData";
import type { Role, RoleField } from "./types";
import { Button } from "./ui";

/** Roles with its data, asked of the core when Agents opens. `onChanged` reads the queues again,
 *  so a rename shows on the board now, not at the next poll. */
export default function Roles({ onChanged }: { onChanged: () => void }) {
  const [roles, setRoles] = useState<Role[] | null>(null);
  const [error, setError] = useState("");
  const [open, setOpen] = useState("");
  useEffect(() => {
    let live = true;
    api.rolesInfo().then(
      (r) => live && setRoles(r),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, []);
  return (
    <RolesView
      roles={roles}
      error={error}
      open={open}
      onOpen={(id) => setOpen((o) => (o === id ? "" : id))}
      onSave={async (id, field, value) => {
        try {
          setRoles(await api.setRole(id, field, value));
          setError("");
          onChanged();
          return null;
        } catch (e) {
          return String(e);
        }
      }}
    />
  );
}

/** Roles as drawn: `roles` is null while they load; `error` is the core's refusal of the file. */
export function RolesView({ roles, error = "", open, onOpen, onSave }: {
  roles: Role[] | null;
  error?: string;
  /** The role whose fields show, "" for none. */
  open: string;
  onOpen: (id: string) => void;
  /** Resolves to null when saved, else the core's refusal to show. */
  onSave: (id: string, field: RoleField, value: string) => Promise<string | null>;
}) {
  const t = useT("shell");
  return (
    <section aria-labelledby="roles-head">
      <h2 id="roles-head">{t("roles.head")}</h2>
      <p className="meta">
        <Trans ns="shell" k="roles.meta" components={{ code: <code /> }} />
      </p>
      {error && <p className="err" role="alert">{error}</p>}
      {roles === null && !error && <p className="none">{t("common:loading")}</p>}
      {roles && (
        <ul className="role-list">
          {roles.map((r) => (
            <li key={r.id}>
              <button type="button" className="role-row" aria-expanded={open === r.id} aria-controls={open === r.id ? `role-${r.id}` : undefined} onClick={() => onOpen(r.id)}>
                <b dir="auto">{r.name}</b> <span className="meta">{r.id} · {slotWords(r.slots)}</span>
              </button>
              {open === r.id && (
                <div className="role-fields" id={`role-${r.id}`} role="group" aria-label={t("roles.profile", { name: r.name })}>
                  <p className="meta" dir="auto">{roleLine(r)}</p>
                  {ROLE_FIELDS.map((f) => (
                    <RoleFieldEdit key={`${f.field}-${f.value(r)}`} label={f.label} what={f.what} value={f.value(r)} size={f.size} rtl={f.field === "name"} onSave={(v) => onSave(r.id, f.field, v)} />
                  ))}
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** One field of a role: its label, what it changes, and a box to change it; the core's refusal
 *  under it. */
function RoleFieldEdit({ label, what, value, size, rtl, onSave }: {
  label: string;
  what: string;
  value: string;
  size: number;
  /** The name may be Persian: the box takes its direction from what's typed. */
  rtl: boolean;
  onSave: (value: string) => Promise<string | null>;
}) {
  const [text, setText] = useState(value);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const t = useT("shell");
  return (
    <form
      className="setting-edit"
      onSubmit={(e) => {
        e.preventDefault();
        setBusy(true);
        void onSave(text).then((refused) => {
          setBusy(false);
          setError(refused ?? "");
        });
      }}
    >
      <label>
        <b>{label}</b> <input value={text} onChange={(e) => setText(e.target.value)} size={size} maxLength={400} dir={rtl ? "auto" : undefined} disabled={busy} />
      </label>
      <Button type="submit" disabled={busy || text.trim() === value}>{t("common:save")}</Button>
      <span className="meta">{what}</span>
      {error && <span className="err" role="alert">{error}</span>}
    </form>
  );
}
