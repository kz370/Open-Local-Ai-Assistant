"""SILMA TTS helper for Open Local Assistant.

Started and stopped by the app; never run by hand. It speaks one JSON object
per line on stdin/stdout:

    -> {"id": 1, "cmd": "say", "text": "...", "speed": 1.0}
    <- {"id": 1, "ok": true, "sampleRate": 24000, "pcm": "<base64 float32 LE>"}

At start-up it loads the model and reports {"event": "ready", "device": ...}.
`--prepare` converts the downloaded checkpoint to a small fp16 file, fetches
the vocoder and tashkeel models and synthesizes one test sentence.

When the app exits (or crashes) stdin closes and this process ends with it.
"""

import base64
import json
import os
import sys
import threading
import types

HERE = os.path.dirname(os.path.abspath(__file__))
WEIGHTS = os.path.join(HERE, "weights")
CKPT_RAW = os.path.join(WEIGHTS, "model.pt")
CKPT = os.path.join(WEIGHTS, "silma-v1.fp16.safetensors")
VOCAB = os.path.join(WEIGHTS, "vocab.txt")

# The protocol owns the real stdout; everything SILMA prints goes to stderr
# (which the app writes to its log).
_proto = os.fdopen(os.dup(1), "w", encoding="utf-8", buffering=1)
os.dup2(2, 1)
sys.stdout = sys.stderr
_proto_lock = threading.Lock()


def send(obj):
    with _proto_lock:
        _proto.write(json.dumps(obj, ensure_ascii=False) + "\n")
        _proto.flush()


def install_nemo_shim():
    """SILMA imports NeMo's text normalizer, which needs pynini, which has no
    Windows build. `spell_numbers` below covers what it was used for, so a
    pass-through stands in for it."""
    if "nemo_text_processing" in sys.modules:
        return

    class Normalizer:
        def __init__(self, *args, **kwargs):
            pass

        def normalize(self, text, *args, **kwargs):
            return text

    names = [
        "nemo_text_processing",
        "nemo_text_processing.text_normalization",
        "nemo_text_processing.text_normalization.normalize",
    ]
    for name in names:
        sys.modules[name] = types.ModuleType(name)
    sys.modules[names[-1]].Normalizer = Normalizer


install_nemo_shim()


_ONES = ["صفر", "واحد", "اثنان", "ثلاثة", "أربعة", "خمسة", "ستة", "سبعة", "ثمانية", "تسعة", "عشرة",
         "أحد عشر", "اثنا عشر", "ثلاثة عشر", "أربعة عشر", "خمسة عشر", "ستة عشر", "سبعة عشر", "ثمانية عشر", "تسعة عشر"]
_TENS = ["", "", "عشرون", "ثلاثون", "أربعون", "خمسون", "ستون", "سبعون", "ثمانون", "تسعون"]
_HUNDREDS = ["", "مائة", "مائتان", "ثلاثمائة", "أربعمائة", "خمسمائة", "ستمائة", "سبعمائة", "ثمانمائة", "تسعمائة"]
# (value, one, two, plural 3-10, singular 11+)
_SCALES = [(10**9, "مليار", "ملياران", "مليارات", "مليار"), (10**6, "مليون", "مليونان", "ملايين", "مليون"), (1000, "ألف", "ألفان", "آلاف", "ألف")]


def _below_thousand(n):
    parts = []
    if n >= 100:
        parts.append(_HUNDREDS[n // 100])
        n %= 100
    if n >= 20:
        unit = n % 10
        parts.append(f"{_ONES[unit]} و{_TENS[n // 10]}" if unit else _TENS[n // 10])
    elif n > 0:
        parts.append(_ONES[n])
    return " و".join(parts)


def arabic_number(n):
    """Integer -> Arabic words (Modern Standard Arabic, pausal form)."""
    if n == 0:
        return _ONES[0]
    parts = []
    for value, one, two, plural, single in _SCALES:
        count, n = divmod(n, value)
        if count == 1:
            parts.append(one)
        elif count == 2:
            parts.append(two)
        elif 3 <= count <= 10:
            parts.append(f"{_below_thousand(count)} {plural}")
        elif count > 10:
            parts.append(f"{arabic_number(count)} {single}")
    if n:
        parts.append(_below_thousand(n))
    return " و".join(parts)


_DIGITS = str.maketrans("٠١٢٣٤٥٦٧٨٩۰۱۲۳۴۵۶۷۸۹", "01234567890123456789")


def spell_numbers(text):
    """Spells out numbers in Arabic sentences; the model mumbles digits."""
    import re

    if not re.search(r"[؀-ۿ]", text):
        return text
    text = text.translate(_DIGITS)

    def one(m):
        whole = int(m.group(1).replace(",", ""))
        if whole >= 10**12:
            return m.group(0)
        words = arabic_number(whole)
        if m.group(2):
            words += " فاصلة " + " ".join(_ONES[int(d)] for d in m.group(2))
        if m.group(3):
            words += " بالمائة"
        return f" {words} "

    text = re.sub(r"(\d{1,3}(?:,\d{3})+|\d+)(?:[.٫](\d+))?\s*(%|٪)?", one, text)
    return " ".join(text.split())


def pick_device():
    import torch

    return "cuda" if torch.cuda.is_available() else "cpu"


def prepare():
    """Slims the 2.6 GB training checkpoint (weights + optimizer state) down to
    the fp16 inference weights, then loads everything once so the vocoder and
    tashkeel models are downloaded now rather than on the first sentence."""
    import torch
    from safetensors.torch import save_file

    if not os.path.exists(CKPT):
        if not os.path.exists(CKPT_RAW):
            raise SystemExit("model.pt is missing")
        print("Converting checkpoint...", flush=True)
        ckpt = torch.load(CKPT_RAW, map_location="cpu", weights_only=True)
        state = ckpt.get("model_state_dict") or ckpt
        state = {k: v.half().contiguous() if v.is_floating_point() else v.contiguous() for k, v in state.items() if hasattr(v, "is_floating_point")}
        save_file(state, CKPT + ".tmp")
        os.replace(CKPT + ".tmp", CKPT)
        del ckpt, state
    engine = Engine()
    rate, pcm = engine.say("مرحبا، هذا اختبار للصوت.", 1.0)
    if len(pcm) < rate // 4:
        raise SystemExit("test synthesis produced no audio")
    send({"event": "prepared", "device": engine.device})


class Engine:
    def __init__(self):
        from importlib.resources import files

        from hydra.utils import get_class
        from omegaconf import OmegaConf
        from silma_tts.infer import utils_infer as ui

        self.ui = ui
        self.device = pick_device()
        ui.device = self.device
        cfg = OmegaConf.load(str(files("silma_tts").joinpath("config.yaml")))
        model_cls = get_class(f"silma_tts.model.{cfg.model.backbone}")
        self.mel_spec_type = cfg.model.mel_spec.mel_spec_type
        self.rate = int(cfg.model.mel_spec.target_sample_rate)
        ui.load_tashkeel_model()
        self.vocoder = ui.load_vocoder(self.mel_spec_type, False, None, self.device, None)
        self.model = ui.load_model(model_cls, cfg.model.arch, CKPT, self.mel_spec_type, VOCAB, "euler", False, self.device)
        self.ref_audio = str(files("silma_tts").joinpath("infer/ref_audio_samples/ar.ref.24k.wav"))
        # Transcript of the bundled reference clip (saves a Whisper download).
        self.ref_text = "ويدقق النظر في القرآن الكريم وسائر الكتب السماوية ويتبع مسالك الرسل العظام عليهم الصلاة والسلام."
        self.ref_audio, self.ref_text = ui.preprocess_ref_audio_text(self.ref_audio, self.ref_text, show_info=lambda *a: None)

    def say(self, text, speed):
        import numpy as np

        text = spell_numbers(" ".join(text.split()))
        wav, rate, _ = self.ui.infer_process(
            self.ref_audio,
            self.ref_text,
            text,
            self.model,
            self.vocoder,
            self.mel_spec_type,
            show_info=lambda *a: None,
            progress=None,
            nfe_step=16,
            speed=float(speed),
            device=self.device,
            force_tashkeel=True,
        )
        return int(rate), np.asarray(wav, dtype="<f4")


def serve():
    try:
        engine = Engine()
    except Exception as e:  # noqa: BLE001 - reported to the app
        send({"event": "failed", "error": f"{type(e).__name__}: {e}"})
        return
    send({"event": "ready", "device": engine.device})
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except ValueError:
            continue
        rid = req.get("id")
        if req.get("cmd") == "quit":
            break
        try:
            rate, pcm = engine.say(req.get("text", ""), req.get("speed", 1.0))
            send({"id": rid, "ok": True, "sampleRate": rate, "pcm": base64.b64encode(pcm.tobytes()).decode("ascii")})
        except Exception as e:  # noqa: BLE001 - reported to the app
            send({"id": rid, "ok": False, "error": f"{type(e).__name__}: {e}"})


if __name__ == "__main__":
    if "--prepare" in sys.argv:
        prepare()
    else:
        serve()
