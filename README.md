# Pirately

Fork of [Pluely](https://github.com/iamsrikanthnani/pluely).

I tried Pluely and liked the idea — a floating AI overlay for meetings and conversations — but the audio transcription pipeline didn't work the way I needed it to. Everything was gated behind VAD segments, short utterances like "ok" and "yeah" were getting sent to the LLM as standalone messages, and the whole thing felt choppy rather than continuous.

So I forked it and started making changes.

## What's different

- **Licensing stripped** — all license checks, payment integration, and feature gating removed. Everything is unlocked.
- **Pluely API disabled** — `shouldUsePluelyAPI()` always returns false. You configure your own AI and STT providers.
- **In-process STT** — local transcription via `transcribe-rs` (Whisper, Parakeet, Moonshine, SenseVoice, etc.). Models are downloaded and run in-process. No external server needed.
- **Silero VAD v4** — neural voice activity detection replaces the old RMS/peak energy thresholds.
- **Live system audio transcription** — continuous capture of all system audio (Zoom, YouTube, Spotify, whatever). Audio is chunked every ~3 seconds, transcribed on a background thread, and displayed as a running live transcript. Short utterances stay in the transcript for context but don't trigger the LLM on their own. The AI only fires at natural conversation breaks (3s silence) after enough speech has accumulated (5s minimum).
- **Removed** — `GetLicense`, `Promote`, `Contribute` components. Speechmatics and Rev.ai STT providers (they returned async job IDs, not transcriptions).

## Build

```bash
npm install
npm run tauri dev      # development
npm run tauri build    # production (.dmg / .msi / .deb)
```

Requires Node.js 18+, Rust stable, and the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

## License

GPL v3 — same as the upstream Pluely project.
