import { useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { KNOWN_LANGUAGES } from "../../../app/knownLanguages";
import { t } from "../../../app/strings";
import type { LangSetting, LanguageEntry } from "../../../app/types";
import { Switch } from "../../../components/common/controls";
import { Card, Row, SectionHeader } from "../../../components/settings/layout";
import { LanguagePicker } from "../../../components/settings/LanguagePicker";
import { useS } from "./Basic";

const labelOf = (e: LanguageEntry) => (e.builtIn ? t(`languages.${e.code}`) : e.displayName);

/** Everything about languages in one place: replies, the languages you use, and how speech is recognized. */
export function LanguageSection() {
  const [s, set] = useS();
  const [adding, setAdding] = useState<string | null>(null);
  const entries = s.language.entries;
  const replyOptions: { value: string; label: string }[] = [{ value: "auto", label: t("settings.language.responseAuto") }, ...entries.map((e) => ({ value: e.code, label: labelOf(e) }))];

  const add = () => {
    const known = KNOWN_LANGUAGES.find((k) => k.code === adding);
    if (!known || entries.some((e) => e.code === known.code)) return;
    set((d) => void d.language.entries.push({ code: known.code, displayName: known.name, direction: known.direction, sttLanguage: known.code, ttsVoice: "auto", builtIn: false }));
    setAdding(null);
  };
  const remove = (code: string) => {
    if (entries.length <= 1) return;
    set((d) => void (d.language.entries = d.language.entries.filter((e) => e.code !== code)));
  };
  const speechOptions = (
    <>
      <option value="auto">{t("app.automatic")}</option>
      {entries.map((e) => (
        <option key={e.code} value={e.code}>
          {labelOf(e)}
        </option>
      ))}
    </>
  );

  return (
    <>
      <SectionHeader title={t("settings.sections.language")} intro={t("settings.language.detectHint")} />
      <Card>
        <Row label={t("settings.language.response")} stack>
          <div className="radio-list" role="radiogroup" aria-label={t("settings.language.response")}>
            {replyOptions.map((o) => (
              <label key={o.value}>
                <input type="radio" name="resp-lang" checked={s.language.responseLanguage === o.value} onChange={() => set((d) => void (d.language.responseLanguage = o.value))} />
                {o.label}
              </label>
            ))}
          </div>
        </Row>
      </Card>
      <Card
        title={t("settings.language.yourLanguages")}
        actions={
          <button className="btn btn-sm" onClick={() => setAdding("")}>
            <Plus size={12} /> {t("settings.language.add")}
          </button>
        }
      >
        <div className="lang-rows">
          {entries.map((e) => (
            <div className="lang-row" key={e.code}>
              <span className="badge lang-code">{e.code.toUpperCase()}</span>
              <span className="lang-name">{labelOf(e)}</span>
              {e.builtIn ? (
                <span className="lang-tag">{t("settings.language.builtIn")}</span>
              ) : (
                <button className="icon-btn danger" aria-label={`${t("settings.language.remove")}: ${labelOf(e)}`} title={t("settings.language.remove")} onClick={() => remove(e.code)}>
                  <Trash2 size={14} />
                </button>
              )}
            </div>
          ))}
        </div>
        {adding !== null && (
          <Row label={t("settings.language.newLanguage")}>
            <LanguagePicker
              options={KNOWN_LANGUAGES.filter((k) => !entries.some((e) => e.code === k.code))}
              value={adding}
              onChange={setAdding}
              label={t("settings.language.newLanguage")}
              placeholder={t("settings.language.searchLanguage")}
              emptyText={t("settings.language.noLanguageMatch")}
            />
            <button className="btn btn-sm btn-primary" disabled={!adding} onClick={add}>
              {t("app.add")}
            </button>
            <button className="btn btn-sm" onClick={() => setAdding(null)}>
              {t("app.cancel")}
            </button>
          </Row>
        )}
      </Card>
      <Card title={t("settings.language.speechCard")}>
        <Row label={t("settings.language.speechLanguage")} htmlFor="sel-stt-lang">
          <select id="sel-stt-lang" className="select" value={s.stt.language} onChange={(e) => set((d) => void (d.stt.language = e.target.value as LangSetting))}>
            {speechOptions}
          </select>
        </Row>
        <Row label={t("settings.language.dictationLanguage")} hint={t("settings.language.dictationLanguageHint")} htmlFor="sel-dict-lang">
          <select id="sel-dict-lang" className="select" value={s.dictation.language || s.stt.language} onChange={(e) => set((d) => void (d.dictation.language = e.target.value))}>
            {speechOptions}
          </select>
        </Row>
      </Card>
      <Card title={t("settings.language.arabicTashkeel")}>
        <Row label={t("settings.language.arabicTashkeelEnabled")} hint={t("settings.language.arabicTashkeelEnabledHint")} htmlFor="sw-tashkeel">
          <Switch id="sw-tashkeel" label={t("settings.language.arabicTashkeelEnabled")} checked={s.language.arabicTashkeelEnabled} onChange={(v) => set((d) => void (d.language.arabicTashkeelEnabled = v))} />
        </Row>
        {s.language.arabicTashkeelEnabled && (
          <Row label={t("settings.language.arabicTashkeelInstruction")} hint={t("settings.language.arabicTashkeelInstructionHint")} htmlFor="ta-tashkeel" stack>
            <textarea
              id="ta-tashkeel"
              className="textarea"
              rows={3}
              dir="rtl"
              placeholder={t("settings.language.arabicTashkeelPlaceholder")}
              defaultValue={s.language.arabicTashkeelInstruction}
              onBlur={(e) => set((d) => void (d.language.arabicTashkeelInstruction = e.target.value))}
            />
          </Row>
        )}
      </Card>
    </>
  );
}
