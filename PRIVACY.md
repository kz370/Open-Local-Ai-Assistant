# Privacy policy

Open Local Assistant is a desktop app. It has no user accounts, no analytics and no telemetry, and the project runs no servers. The developer receives no data from the app.

## What stays on your computer

Everything the app stores is kept in `%APPDATA%\com.localassistant.app` on your PC:

- conversations and messages
- settings, including API keys for hosted AI providers
- downloaded voice and speech models
- log files (they contain no conversation content unless you turn that on in Settings)

Speech recognition (including dictation) and the assistant's voice always run on your PC. Audio from your microphone is never sent anywhere.

To delete everything, uninstall the app and remove that folder.

## When the app connects to other services

The app only connects to other systems for the features below. Each one is either chosen by you or started by an action you take.

| Feature | What is sent | Where |
| --- | --- | --- |
| **Chat with LM Studio** (default) | Your messages, attached files and images | Your own LM Studio server, normally on the same PC (`localhost`) |
| **Chat with a hosted provider** (only if you choose one) | Your messages, attached files and images, and your API key | OpenRouter, Google Gemini or Hugging Face, whichever you selected. The app shows a notice when a hosted provider is in use. |
| **Web search** (when the assistant uses the search tool) | The search query | A SearXNG instance (your own, or a public one picked from the list at searx.space), with DuckDuckGo as the fallback |
| **Model downloads** (only when you click **Download**) | A normal file download request | Hugging Face and GitHub (sherpa-onnx releases) |
| **NVIDIA GPU pack** (only when you install it) | A normal file download request | GitHub (sherpa-onnx releases) and NVIDIA's download server |
| **MCP tools** (only servers you add) | Whatever the tool needs for the request. Tools that write data or run commands ask you first. | The MCP servers you configured |
| **Dictation grammar cleanup** (optional) | The dictated text | The AI provider you chose, as for chat |

## Third-party services

When you use a hosted AI provider, web search, a download source or an MCP server, that service's own privacy policy applies to what it receives:

- OpenRouter: https://openrouter.ai/privacy
- Google Gemini API: https://policies.google.com/privacy
- Hugging Face: https://huggingface.co/privacy
- GitHub: https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement
- NVIDIA: https://www.nvidia.com/en-us/about-nvidia/privacy-policy/
- DuckDuckGo: https://duckduckgo.com/privacy
- Public SearXNG instances are run by independent volunteers; see the instance's own page.

## Changes

Changes to this policy are made in this file, so its git history shows every version.
