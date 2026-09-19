import type { ProviderId } from "./types";

/** Selectable providers; `url` is the OpenAI-compatible base URL preset (mirrors `PROVIDERS` in settings/mod.rs). */
export const PROVIDERS: { id: ProviderId; label: string; url: string }[] = [
  { id: "lmstudio", label: "LM Studio", url: "http://localhost:1234/v1" },
  { id: "openrouter", label: "OpenRouter", url: "https://openrouter.ai/api/v1" },
  { id: "groq", label: "Groq", url: "https://api.groq.com/openai/v1" },
  { id: "gemini", label: "Google Gemini API", url: "https://generativelanguage.googleapis.com/v1beta/openai" },
  { id: "huggingface", label: "Hugging Face", url: "https://router.huggingface.co/v1" },
  { id: "cerebras", label: "Cerebras", url: "https://api.cerebras.ai/v1" },
];

export const providerLabel = (id: string | undefined) => (PROVIDERS.find((p) => p.id === id) ?? PROVIDERS[0]).label;
