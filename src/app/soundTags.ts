// Sound cues (<laugh>, <sigh>, <breath>) that Supertonic voices perform in
// spoken replies. They are for the voice only: hidden wherever text is shown
// or copied.

const SOUND_TAG = /[ \t]*<\s*(?:laugh|sigh|breath)\s*>[ \t]*/gi;

export function stripSoundTags(text: string): string {
  return text.replace(SOUND_TAG, (m) => (m.trim() === m ? "" : " ")).replace(/ +([.,!?;:،؟])/g, "$1");
}
