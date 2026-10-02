import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { t } from "../../../app/strings";
import type { DictationProfile } from "../../../app/types";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { useS } from "./Basic";

function newId(existing: DictationProfile[]): string {
  let n = existing.length + 1;
  let id = `p${n}`;
  const taken = new Set(existing.map((p) => p.id));
  while (taken.has(id)) id = `p${++n}`;
  return id;
}

/** Editor for the dictation profiles the overlay's profile dropdown offers.
 *  A profile's prompt is appended to the built-in correction prompt, so the
 *  only thing it has to say is what the dictated text should become. */
export function DictationProfilesSection() {
  const [s, set] = useS();
  const profiles = s.dictation.profiles;

  const patch = (id: string, change: Partial<DictationProfile>) =>
    set((d) => void d.dictation.profiles.forEach((p) => p.id === id && Object.assign(p, change)));

  const add = () => {
    const id = newId(profiles);
    set((d) => void d.dictation.profiles.push({ id, title: t("settings.dictation.newProfile"), prompt: "" }));
  };

  const remove = (id: string) =>
    set((d) => {
      d.dictation.profiles = d.dictation.profiles.filter((p) => p.id !== id);
      // The dropdown's selection has to point at a profile that still exists.
      if (d.dictation.activeProfile === id) d.dictation.activeProfile = "";
    });

  const move = (index: number, step: -1 | 1) =>
    set((d) => {
      const to = index + step;
      if (to < 0 || to >= d.dictation.profiles.length) return;
      const [row] = d.dictation.profiles.splice(index, 1);
      d.dictation.profiles.splice(to, 0, row);
    });

  return (
    <>
      <SectionHeader title={t("settings.dictation.profiles")} intro={t("settings.dictation.profilesHint")} />
      <button className="btn btn-sm" style={{ alignSelf: "flex-start", marginBottom: 10 }} onClick={() => (location.hash = "#/settings/dictation")}>
        ← {t("settings.sections.dictation")}
      </button>
      <Card
        title={t("settings.dictation.manageProfiles")}
        actions={
          <button className="btn btn-sm" onClick={add}>
            <Plus size={12} /> {t("settings.dictation.addProfile")}
          </button>
        }
      >
        {profiles.length === 0 ? <p className="settings-intro">{t("settings.dictation.noProfiles")}</p> : null}
        {profiles.map((p, i) => (
          <div key={p.id} className="profile-row">
            <div className="profile-row-head">
              <input
                className="input profile-title"
                value={p.title}
                maxLength={60}
                placeholder={t("settings.dictation.profileTitlePlaceholder")}
                aria-label={t("settings.dictation.profileTitle")}
                onChange={(e) => patch(p.id, { title: e.target.value })}
              />
              <button className="icon-btn" title={t("app.moveUp")} aria-label={t("app.moveUp")} disabled={i === 0} onClick={() => move(i, -1)}>
                <ArrowUp size={14} />
              </button>
              <button className="icon-btn" title={t("app.moveDown")} aria-label={t("app.moveDown")} disabled={i === profiles.length - 1} onClick={() => move(i, 1)}>
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
          <select id="sel-active-profile" className="select" value={s.dictation.activeProfile} onChange={(e) => set((d) => void (d.dictation.activeProfile = e.target.value))}>
            <option value="">{t("settings.dictation.activeProfileNone")}</option>
            {profiles.map((p) => (
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
            value={s.dictation.profileSearchThreshold}
            onChange={(e) => set((d) => void (d.dictation.profileSearchThreshold = Number(e.target.value) || 0))}
          />
        </Row>
      </Card>
    </>
  );
}
