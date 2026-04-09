# AGENTS.md — Pirately

Fork of [Pluely](https://github.com/iamsrikanthnani/pluely), an open-source AI overlay assistant (alternative to Cluely). Privacy-first desktop app that sits as a translucent floating toolbar on top of all windows, providing real-time AI assistance during meetings, interviews, and conversations.

## Fork Changes

- **Licensing stripped**: All license checks, payment integration, and feature gating removed. All features are unlocked by default.
- **Pluely API disabled**: The hosted Pluely API (`shouldUsePluelyAPI()` always returns `false`). Users must configure their own AI/STT providers in Dev Space.
- **In-process STT added**: Local transcription via `transcribe-rs` crate (Whisper, Parakeet V3, Moonshine, SenseVoice, etc.). No external server needed. Models downloaded from Handy's blob storage and run in-process via ONNX Runtime / whisper.cpp.
- **Silero VAD v4**: Neural voice activity detection replaces the old RMS/peak energy thresholds. Bundled as Tauri resource (~2MB ONNX model).
- **Audio resampler**: System audio (44.1/48kHz) resampled to 16kHz via `rubato` FFT resampler before VAD/transcription.
- **Local Whisper Server retained**: Fallback `local-whisper` provider pointing to `http://localhost:8080/v1/audio/transcriptions` (OpenAI-compatible API).
- **Removed components**: `GetLicense`, `Promote`, `Contribute` deleted entirely. Speechmatics and Rev.ai STT providers removed (they returned async job IDs, not transcriptions).
- **Dashboard simplified**: No longer shows license activation or GetLicense button.

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop runtime | Tauri v2 (Rust backend + system webview) |
| Frontend | React 19 + TypeScript |
| Styling | Tailwind CSS v4 + shadcn/ui (new-york style, Radix UI primitives) |
| State | React Context (`AppProvider`, `ThemeProvider`) + hooks |
| Persistence | localStorage (settings) + SQLite via `tauri-plugin-sql` (conversations, prompts) |
| Routing | React Router v7 (`BrowserRouter`) |
| Charts | Recharts |
| Markdown | Streamdown + remark-gfm + remark-math + rehype-katex + shiki |
| Audio (macOS) | CoreAudio via `cidre` crate |
| Audio (Windows) | WASAPI via `wasapi` crate |
| Audio (Linux) | PulseAudio via `libpulse-binding` |
| Audio resampling | `rubato` (FFT-based, native rate → 16kHz) |
| Voice Activity Detection | Silero VAD v4 via `vad-rs` (ONNX, ~2MB) |
| Speech-to-Text | `transcribe-rs` (Whisper via whisper.cpp, Parakeet/Moonshine/SenseVoice via ONNX Runtime) |
| Screen capture | `xcap` + `image` crates |
| Analytics | PostHog (session recording disabled) |

## Commands

```bash
npm run dev            # Vite dev server only (frontend hot reload)
npm run build          # TypeScript check + Vite build
npm run tauri dev      # Full Tauri dev (Rust + frontend, port 1420)
npm run tauri build    # Production build (creates .dmg/.msi/.deb/.rpm/.AppImage)
```

No test runner or linter is configured.

## Directory Structure

```
pirately/
├── src/                          # Frontend (React + TypeScript)
│   ├── main.tsx                  # App entry point
│   ├── routes/index.tsx          # Route definitions
│   ├── contexts/                 # React contexts
│   │   ├── app.context.tsx       # AppProvider — central app state (providers, license, config)
│   │   └── theme.context.tsx     # ThemeProvider — theme + transparency
│   ├── hooks/                    # All React hooks
│   │   ├── useCompletion.ts      # Overlay AI chat (single-turn, manages own state)
│   │   ├── useChatCompletion.ts  # Chat page AI (multi-turn, receives state from parent)
│   │   ├── useSystemAudio.ts     # System audio capture + STT + AI pipeline
│   │   ├── useApp.ts             # App initialization orchestrator
│   │   ├── useHistory.ts         # Conversation history CRUD
│   │   ├── useSystemPrompts.ts   # System prompt CRUD
│   │   ├── useSettings.ts        # Settings page state
│   │   ├── useCustomProvider.ts  # Custom AI provider form + CRUD
│   │   ├── useCustomSttProviders.ts  # Custom STT provider form + CRUD
│   │   ├── useWindow.ts          # Window resize + focus hooks
│   │   ├── useGlobalShortcuts.ts # Global shortcut registration (singleton pattern)
│   │   ├── useShortcuts.ts       # Shortcut helper (auto-registers callbacks)
│   │   ├── useVersion.ts         # App version fetcher
│   │   ├── useCopyToClipboard.ts # Clipboard utility
│   │   ├── useMenuItems.tsx      # Sidebar menu definition
│   │   └── useTitles.ts          # Disables HTML title attributes globally
│   ├── pages/                    # Route pages
│   │   ├── app/                  # "/" — floating overlay toolbar
│   │   ├── dashboard/            # "/dashboard" — license + usage charts
│   │   ├── chats/                # "/chats" — conversation list + viewer
│   │   ├── system-prompts/       # "/system-prompts" — prompt CRUD
│   │   ├── shortcuts/            # "/shortcuts" — keyboard shortcut config
│   │   ├── screenshot/           # "/screenshot" — capture mode settings
│   │   ├── settings/             # "/settings" — theme, autostart, icon, on-top
│   │   ├── audio/                # "/audio" — input/output device selection
│   │   ├── responses/            # "/responses" — response length, language, auto-scroll
│   │   └── dev/                  # "/dev-space" — AI/STT provider configuration
│   ├── components/               # Reusable components
│   │   ├── ui/                   # shadcn/ui components (Button, Card, Dialog, etc.)
│   │   ├── Sidebar.tsx           # Dashboard navigation sidebar
│   │   ├── Header.tsx            # Page header with title + back button
│   │   ├── Markdown.tsx          # Streamdown markdown renderer
│   │   ├── Overlay.tsx           # Screenshot selection overlay (separate Tauri window)
│   │   ├── Updater.tsx           # Auto-update checker + installer
│   │   ├── DragButton.tsx        # Window drag handle
│   │   ├── GetLicense.tsx        # License purchase button
│   │   ├── CustomCursor.tsx      # Invisible cursor overlay
│   │   └── ...                   # Selection, TextInput, CopyButton, Icons, etc.
│   ├── lib/                      # Utilities and business logic
│   │   ├── functions/
│   │   │   ├── ai-response.function.ts  # fetchAIResponse() — async generator for streaming AI
│   │   │   ├── stt.function.ts          # fetchSTT() — speech-to-text transcription
│   │   │   ├── common.function.ts       # getByPath, deepVariableReplacer, blobToBase64, etc.
│   │   │   └── pluely.api.ts            # shouldUsePluelyAPI() — built-in vs custom provider
│   │   ├── storage/              # localStorage CRUD helpers
│   │   │   ├── ai-providers.ts          # Custom AI provider storage
│   │   │   ├── stt-providers.ts         # Custom STT provider storage
│   │   │   ├── shortcuts.storage.ts     # Shortcuts config with conflict detection
│   │   │   ├── response-settings.storage.ts  # Response length/language/auto-scroll
│   │   │   └── customizable.storage.ts  # App icon, on-top, autostart, cursor
│   │   ├── database/             # SQLite operations
│   │   │   ├── config.ts                # Database singleton
│   │   │   ├── chat-history.action.ts   # Conversation + message CRUD + migration
│   │   │   └── system-prompt.action.ts  # System prompt CRUD
│   │   ├── utils.ts              # cn() (tailwind-merge), floatArrayToWav()
│   │   ├── platform.ts           # getPlatform(), isMacOS(), isWindows(), isLinux()
│   │   ├── analytics.ts          # PostHog captureEvent() + trackAppStart()
│   │   ├── curl-validator.ts     # cURL command validation
│   │   └── ...                   # version.ts, chat-constants.ts, etc.
│   ├── types/                    # TypeScript type definitions
│   ├── config/                   # Constants and built-in provider definitions
│   │   ├── constants.ts          # STORAGE_KEYS, DEFAULT_SYSTEM_PROMPT, MAX_FILES, etc.
│   │   ├── ai-providers.constants.ts  # 10 built-in AI providers (OpenAI, Claude, etc.)
│   │   ├── stt.constants.ts     # 8 built-in STT providers (In-Process, Whisper, ElevenLabs, etc.)
│   │   └── shortcuts.ts         # DEFAULT_SHORTCUT_ACTIONS with platform-specific keys
│   ├── layouts/                  # Page layouts
│   │   ├── DashboardLayout.tsx   # Sidebar + main content (all dashboard pages)
│   │   ├── PageLayout.tsx        # Header + ScrollArea wrapper
│   │   └── ErrorLayout.tsx       # ErrorBoundary fallback
│   └── global.css                # Tailwind base styles + CSS variables
│
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml                # Rust dependencies
│   ├── tauri.conf.json           # Tauri app config (windows, plugins, security, bundle)
│   ├── build.rs                  # Embeds env vars at compile time
│   ├── info.plist                # macOS entitlements (mic, screen capture, audio)
│   ├── capabilities/             # Permission configs (macOS vs cross-platform)
│   ├── src/
│   │   ├── main.rs               # Entry point (suppresses Windows console)
│   │   ├── lib.rs                # Core setup: plugins, commands, state, NSPanel init
│   │   ├── api.rs                # HTTP API layer: streaming chat, STT, license checks (1167 lines)
│   │   ├── capture.rs            # Multi-monitor screen capture with selection (391 lines)
│   │   ├── activate.rs           # License activation + secure storage (437 lines)
│   │   ├── window.rs             # Window management: overlay + dashboard (224 lines)
│   │   ├── shortcuts.rs          # Global shortcuts + app icon + always-on-top (659 lines)
│   │   ├── audio/                # Audio processing modules
│   │   │   ├── mod.rs            # Module declarations
│   │   │   ├── resampler.rs      # FrameResampler (rubato FFT, native → 16kHz)
│   │   │   └── vad.rs            # Silero VAD v4 + SmoothedVad (onset/prefill/hangover)
│   │   ├── transcription/        # In-process STT engine
│   │   │   ├── mod.rs            # Module declarations
│   │   │   ├── model_manager.rs  # Model download/cache/verify (10+ models)
│   │   │   └── engine.rs         # TranscriptionManager (load/infer/unload)
│   │   ├── db/                   # SQLite migrations (system_prompts + conversations/messages)
│   │   └── speaker/              # System audio capture
│   │       ├── mod.rs            # Platform abstraction
│   │       ├── commands.rs       # Silero VAD + local/remote STT + device listing
│   │       ├── macos.rs          # CoreAudio via cidre (386 lines)
│   │       ├── windows.rs        # WASAPI loopback (380 lines)
│   │       └── linux.rs          # PulseAudio monitor (473 lines)
│   ├── resources/
│   │   └── models/
│   │       └── silero_vad_v4.onnx  # Bundled Silero VAD model (~2MB)
│   └── icons/                    # App icons
│
├── components.json               # shadcn/ui config
├── vite.config.ts                # Vite config: React, Tailwind, path alias @/
├── tsconfig.json                 # TypeScript config (strict, @/ path alias)
└── package.json                  # npm scripts + dependencies
```

## Architecture

### Two-Window System

1. **Main window (overlay)** — A 600x54px floating toolbar (`/` route). On macOS, converted to an NSPanel (non-activating, visible on all spaces/fullscreen). Expands to 600px height when popovers are open.
2. **Dashboard window** — A separate 1200x800 window for settings, chats, providers, etc. (`/chats` route as entry). Created on app startup, hidden by default, toggled via keyboard shortcut.

### Frontend ↔ Backend Communication

**Tauri `invoke()` commands** (31 total, called from frontend):

| Category | Commands | Key Files |
|---|---|---|
| License | `activate_license_api`, `deactivate_license_api`, `validate_license_api`, `mask_license_key_cmd`, `get_checkout_url`, `secure_storage_save/get/remove`, `check_license_status` | `activate.rs`, `api.rs` |
| Window | `set_window_height`, `open_dashboard`, `toggle_dashboard`, `move_window` | `window.rs` |
| Screen capture | `capture_to_base64`, `start_screen_capture`, `capture_selected_area`, `close_overlay_window` | `capture.rs` |
| Shortcuts | `check_shortcuts_registered`, `get_registered_shortcuts`, `update_shortcuts`, `validate_shortcut_key`, `set_license_status`, `set_app_icon_visibility`, `set_always_on_top`, `exit_app` | `shortcuts.rs` |
| Audio | `start_system_audio_capture`, `stop_system_audio_capture`, `manual_stop_continuous`, `check_system_audio_access`, `request_system_audio_access`, `get_vad_config`, `update_vad_config`, `get_capture_status`, `get_audio_sample_rate`, `get_input_devices`, `get_output_devices`, `set_stt_mode`, `set_local_model` | `speaker/commands.rs` |
| AI/Chat | `transcribe_audio`, `chat_stream_response`, `fetch_models`, `fetch_prompts`, `create_system_prompt`, `get_activity` | `api.rs` |
| Transcription | `list_available_models`, `download_model`, `delete_model`, `cancel_model_download` | `transcription/model_manager.rs` |
| Other | `get_app_version` | `lib.rs` |

**Tauri events** (backend → frontend, listened via `@tauri-apps/api/event`):

| Event | Purpose |
|---|---|
| `chat_stream_chunk` | Individual AI response token (streaming) |
| `chat_stream_complete` | Full AI response complete |
| `captured-selection` | Screenshot area selected (PNG base64) |
| `capture-closed` | Screenshot overlay closed |
| `speech-detected` | Audio segment with speech (WAV base64) — remote STT path |
| `speech-transcribed` | In-process transcription result `{ text: string }` — local STT path |
| `speech-start` | Speech detected in audio stream |
| `speech-discarded` | Audio segment discarded (too short, etc.) |
| `model-download-progress` | Model download progress `{ model_id, downloaded_bytes, total_bytes, progress_pct }` |
| `capture-started/stopped` | System audio capture state changes |
| `continuous-recording-start/stopped` | Continuous recording mode state |
| `recording-progress` | Recording seconds elapsed |
| `focus-text-input` | Shortcut: focus the input box |
| `trigger-screenshot` | Shortcut: take screenshot |
| `start-audio-recording` | Shortcut: start voice recording |
| `toggle-system-audio` | Shortcut: toggle system audio |
| `toggle-window-visibility` | Window show/hide (Windows workaround) |
| `custom-shortcut-triggered` | User-defined shortcut action |

### Provider System (cURL-as-configuration)

AI and STT providers are defined as cURL command templates stored in localStorage. Template variables like `{{API_KEY}}`, `{{TEXT}}`, `{{IMAGE}}`, `{{MODEL}}`, `{{SYSTEM_PROMPT}}`, `{{AUDIO}}` are replaced at request time.

- 10 built-in AI providers in `src/config/ai-providers.constants.ts`
- 9 built-in STT providers in `src/config/stt.constants.ts`
- Custom providers are added by users via the Dev Space UI, stored in localStorage
- At runtime, `@bany/curl-to-json` parses the cURL string, then `deepVariableReplacer` substitutes variables
- Streaming responses use SSE parsing; non-streaming use JSON path extraction via `getByPath`

### State Management

```
ThemeProvider (theme, transparency)
  └── AppProvider (all shared app state)
        ├── AI/STT providers (selected + all lists + custom)
        ├── System prompt
        ├── Screenshot configuration
        ├── Customizable state (icon visibility, always-on-top, autostart, cursor)
        ├── License status
        ├── Audio device selection
        └── Pluely API enabled flag
              │
              ├── useCompletion (overlay chat — owns its own conversation state)
              ├── useChatCompletion (chat page — receives conversation state from parent)
              ├── useSystemAudio (system audio pipeline — owns conversation state)
              └── other hooks consume context as needed
```

**Cross-component communication** uses `window.dispatchEvent(new CustomEvent(...))` for `conversationSelected`, `newConversation`, `conversationDeleted` — enabling communication between overlay and dashboard windows via localStorage events.

### Streaming AI Responses

`fetchAIResponse()` in `lib/functions/ai-response.function.ts` is an `async function*` generator. It yields individual tokens during streaming or the complete response for non-streaming providers. Consumed via `for await...of` in hooks.

## Key Files to Understand

If you need to make changes, start here:

| What you're changing | Files to read first |
|---|---|
| AI chat behavior | `src/hooks/useCompletion.ts`, `src/lib/functions/ai-response.function.ts` |
| Chat page | `src/hooks/useChatCompletion.ts`, `src/pages/chats/` |
| System audio | `src/hooks/useSystemAudio.ts`, `src-tauri/src/speaker/commands.rs`, `src-tauri/src/audio/vad.rs`, `src-tauri/src/audio/resampler.rs` |
| In-process STT | `src-tauri/src/transcription/engine.rs`, `src-tauri/src/transcription/model_manager.rs` |
| Adding a provider | `src/config/ai-providers.constants.ts` or `src/config/stt.constants.ts` |
| Custom providers UI | `src/pages/dev/components/ai-configs/` or `stt-configs/` |
| Model management UI | `src/pages/dev/components/model-manager/` |
| Screen capture | `src-tauri/src/capture.rs`, `src/components/Overlay.tsx` |
| Window behavior | `src-tauri/src/window.rs`, `src-tauri/src/lib.rs` (NSPanel init) |
| Global shortcuts | `src-tauri/src/shortcuts.rs`, `src/hooks/useGlobalShortcuts.ts`, `src/lib/storage/shortcuts.storage.ts` |
| License gating | `src/contexts/app.context.tsx` (`hasActiveLicense`), `src/components/GetLicense.tsx` |
| Database schema | `src-tauri/src/db/` (migrations), `src/lib/database/` (frontend operations) |
| Theme/appearance | `src/contexts/theme.context.tsx`, `src/global.css` |
| App initialization | `src/hooks/useApp.ts`, `src/contexts/app.context.tsx`, `src-tauri/src/lib.rs` |
| API proxy layer | `src-tauri/src/api.rs` (server communication, streaming, error handling) |
| Audio capture (native) | `src-tauri/src/speaker/macos.rs`, `windows.rs`, or `linux.rs` |

## Code Conventions

- **Path alias:** `@/` maps to `./src/` (configured in both tsconfig.json and vite.config.ts)
- **UI components:** shadcn/ui pattern — Radix UI primitives + Tailwind CSS + `class-variance-authority`. Components in `src/components/ui/` follow the shadcn structure exactly
- **Barrel exports:** `index.ts` files in hooks, contexts, components, and pages directories
- **Hook naming:** `use*.ts` for hooks, `*.hook.ts` for hook type definitions
- **Type definitions:** `src/types/` — one file per domain
- **Storage keys:** All centralized in `STORAGE_KEYS` constant in `src/config/constants.ts`
- **No comments:** Codebase does not use inline comments
- **No tests:** No test framework or test files exist
- **No linter:** No ESLint, Prettier, or other linting/formatting tools configured

## License Gating

Many features require an active license (checked via `hasActiveLicense` from `AppContext`):

- Custom keyboard shortcut rebinding (non-licensed can only toggle on/off)
- Screenshot selection mode (area capture)
- Response length/language customization
- AI-powered system prompt generation
- Conversation continuation in overlay
- Quick actions for system audio
- Theme and transparency controls
- Custom cursor
- Pluely hosted API access

The license is stored in `secure_storage.json` in the app data directory and validated against a payment server.

## Platform Differences

| Feature | macOS | Windows | Linux |
|---|---|---|---|
| Floating window | NSPanel via `tauri-nspanel` | Standard window, skip taskbar | Standard window, skip taskbar |
| System audio | CoreAudio (`cidre`), aggregate device + process tap | WASAPI (`wasapi`), render device loopback | PulseAudio, monitor source |
| App icon hiding | `NSApplicationActivationPolicy::Accessory` | `skip_taskbar` | `skip_taskbar` |
| Permissions | `tauri-plugin-macos-permissions` + info.plist entitlements | Standard Windows permissions | Standard Linux permissions |
| Dashboard size | 1200x800 with overlay title bar | 800x600 | 800x600 |
| Cursor hiding | Supported via custom cursor | Supported | Not supported |

## Build-Time Environment Variables

`src-tauri/build.rs` reads from `.env` and embeds these as compile-time constants in the Rust binary:

- `PAYMENT_ENDPOINT` — License payment server URL
- `API_ACCESS_KEY` — API access key
- `APP_ENDPOINT` — Pluely API server URL
- `POSTHOG_API_KEY` — PostHog analytics key

These are accessed in Rust via `option_env!()` with fallback to `std::env::var()`.
