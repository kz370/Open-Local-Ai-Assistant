import { useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Check, Plus, Trash2 } from "lucide-react";
import { useSettings } from "../../../app/settingsStore";
import { t } from "../../../app/strings";
import type { DictationProfile } from "../../../app/types";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";

/** The whole page is a draft: nothing reaches the store until Save is pressed,
 *  so a half-typed title or prompt is never persisted — and an emptied title
 *  never becomes a row the store would drop as invalid. */
interface Draft {
  profiles: DictationProfile[];
  activeProfile: string;
  searchAt: number;
}

type Status = "idle" | "saving" | "saved";

function newId(existing: DictationProfile[]): string {
  let n = existing.length + 1;
  let id = `p${n}`;
  const taken = new Set(existing.map((p) => p.id));
  while (taken.has(id)) id = `p${++n}`;
  return id;
}

const same = (a: DictationProfile[], b: DictationProfile[]) =>
  a.length === b.length && a.every((p, i) => p.id === b[i].id && p.title === b[i].title && p.prompt === b[i].prompt);

/** Editor for the dictation profiles the overlay's profile dropdown offers.
 *  A profile's prompt is appended to the built-in correction prompt, so the
 *  only thing it has to say is what the dictated text should become. */
export function DictationProfilesSection() {
  const settings = useSettings((s) => s.settings)!;
  const update = useSettings((s) => s.update);
  const error = useSettings((s) => s.error);
  const saved: Draft = {
    profiles: settings.dictation.profiles,
    activeProfile: settings.dictation.activeProfile,
    searchAt: settings.dictation.profileSearchThreshold,
  };
  const [draft, setDraft] = useState<Draft | null>(null);
  const [status, setStatus] = useState<Status>("idle");
  const model = draft ?? saved;
  const dirty =
    draft !== null &&
    (!same(draft.profiles, saved.profiles) || draft.activeProfile !== saved.activeProfile || draft.searchAt !== saved.searchAt);
  const incomplete = model.profiles.some((p) => !p.title.trim());

  const edit = (change: Partial<Draft>) => setDraft({ ...model, ...change });
  const patch = (id: string, change: Partial<DictationProfile>) =>
    setDraft({ ...model, profiles: model.profiles.map((p) => (p.id === id ? { ...p, ...change } : p)) });

  const add = () => setDraft({ ...model, profiles: [...model.profiles, { id: newId(model.profiles), title: t("settings.dictation.newProfile"), prompt: "" }] });
  const remove = (id: string) =>
    setDraft({ ...model, profiles: model.profiles.filter((p) => p.id !== id), activeProfile: model.activeProfile === id ? "" : model.activeProfile });
  const move = (index: number, step: -1 | 1) => {
    const to = index + step;
    if (to < 0 || to >= model.profiles.length) return;
    const next = [...model.profiles];
    [next[index], next[to]] = [next[to], next[index]];
    setDraft({ ...model, profiles: next });
  };

  // Only report "Saved" once the store really holds the draft, so a failed save
  // leaves the changes in the editor instead of claiming they persisted.
  useEffect(() => {
    if (status !== "saving" || !draft || error) return;
    if (draft.activeProfile !== saved.activeProfile || draft.searchAt !== saved.searchAt || !same(draft.profiles, saved.profiles)) return;
    setDraft(null);
    setStatus("saved");
    const id = window.setTimeout(() => setStatus("idle"), 2000);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [saved.profiles, saved.activeProfile, saved.searchAt, status, error]);

  const save = () => {
    if (!dirty || incomplete) return;
    setStatus("saving");
    void update((d) => {
      d.dictation.profiles = model.profiles;
      d.dictation.activeProfile = model.activeProfile;
      d.dictation.profileSearchThreshold = model.searchAt;
    });
  };

  return (
    <>
      <SectionHeader title={t("settings.dictation.profiles")} intro={t("settings.dictation.profilesHint")} />
      <div className="profile-bar">
        <button className="btn btn-sm" style={{ alignSelf: "flex-start" }} onClick={() => (location.hash = "#/settings/dictation")}>
          ← {t("settings.sections.dictation")}
        </button>
        <span className="profile-bar-status" role="status">
          {incomplete ? (
            <span className="badge err">{t("settings.dictation.profileTitleRequired")}</span>
          ) : status === "saving" ? (
            t("settings.dictation.saving")
          ) : status === "saved" ? (
            <span className="profile-saved">
              <Check size={12} aria-hidden /> {t("settings.dictation.saved")}
            </span>
          ) : dirty ? (
            t("settings.dictation.unsaved")
          ) : null}
        </span>
        <button className="btn btn-sm btn-primary" onClick={save} disabled={!dirty || incomplete}>
          {t("app.save")}
        </button>
      </div>
      <Card
        title={t("settings.dictation.manageProfiles")}
        actions={
          <button className="btn btn-sm" onClick={add}>
            <Plus size={12} /> {t("settings.dictation.addProfile")}
          </button>
        }
      >
        {model.profiles.length === 0 ? <p className="settings-intro">{t("settings.dictation.noProfiles")}</p> : null}
        {model.profiles.map((p, i) => (
          <div key={p.id} className="profile-row">
            <div className="profile-row-head">
              <input
                className="input profile-title"
                value={p.title}
                maxLength={60}
                placeholder={t("settings.dictation.profileTitlePlaceholder")}
                aria-label={t("settings.dictation.profileTitle")}
                aria-invalid={!p.title.trim()}
                onChange={(e) => patch(p.id, { title: e.target.value })}
              />
              <button className="icon-btn" title={t("app.moveUp")} aria-label={t("app.moveUp")} disabled={i === 0} onClick={() => move(i, -1)}>
                <ArrowUp size={14} />
              </button>
              <button className="icon-btn" title={t("app.moveDown")} aria-label={t("app.moveDown")} disabled={i === model.profiles.length - 1} onClick={() => move(i, 1)}>
                <ArrowDown size={14} />
              </button>
              <button className="icon-btn danger" title={t("settings.dictation.deleteProfile")} aria-label={`${t("settings.dictation.deleteProfile")}: ${p.title}`} onClick={() => remove(p.id)}>
                <Trash2 size={14} />
              </button>
            </div>
            <textarea
              className="textarea"
              rows={3}
              value={p.prompt}
              maxLength={4000}
              placeholder={t("settings.dictation.profilePromptPlaceholder")}
              aria-label={t("settings.dictation.profilePrompt")}
              onChange={(e) => patch(p.id, { prompt: e.target.value })}
            />
            <span className="row-hint">{t("settings.dictation.profilePromptHint")}</span>
          </div>
        ))}
      </Card>
      <Card title={t("settings.dictation.activeProfile")}>
        <Row label={t("settings.dictation.activeProfile")} hint={t("settings.dictation.profileNeedsModel")} htmlFor="sel-active-profile">
          <select id="sel-active-profile" className="select" value={model.activeProfile} onChange={(e) => edit({ activeProfile: e.target.value })}>
            <option value="">{t("settings.dictation.activeProfileNone")}</option>
            {model.profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {p.title}
              </option>
            ))}
          </select>
        </Row>
        <Row label={t("settings.dictation.searchAt")} hint={t("settings.dictation.searchAtHint")} htmlFor="num-profile-search">
          <input
            id="num-profile-search"
            className="input"
            type="number"
            min={2}
            max={100}
            value={model.searchAt}
            onChange={(e) => edit({ searchAt: Number(e.target.value) || 0 })}
          />
        </Row>
      </Card>
    </>
  );
}