# Phase 10 — Voice

**Goal.** Talk to PriyaOS: hold Super+V (or, if enabled, say "Hey Priya"), speak in English or Hindi, see the words appear live, hear a short spoken answer, approve or stop work by voice. Speech recognition and speech synthesis run locally; no audio leaves the device unless the user turns on cloud voice. The whole UI is also available in Hindi.

**Usable at the end:** Hold Super+V → "what's using my battery?" → live transcript in the bar → short spoken answer; "install VLC" → plan → "yes" → review → "keep it" — all hands-free, in English or Hindi, with the UI in Hindi if chosen.

**Depends on:** Phase 3 (approvals, kill switch), Phase 5 (Priya bar complete, OSDs, Settings app), Phase 9 (PipeWire defaults, onboarding "voice on/off", fcitx5 + Noto Sans Devanagari).

**Features:** VOX-01, VOX-02, VOX-03, VOX-04, VOX-05, VOX-06, VOX-07, DS-20.

**Read first:** [07 §3 Voice](../architecture/07-interfaces.md), [04 §4 roles (`transcribe`, `fast`)](../architecture/04-priya-ai.md), [05 §4 approvals, §7 kill switch](../architecture/05-security.md), [11 §2.2 typography, §4 Priya bar states, §5 autonomy, §6 writing, §7 accessibility](../architecture/11-design-system-and-ux.md), [02 §5 modules](../architecture/02-core.md), [09 §2–3 suites and fixtures](../architecture/09-testing.md).

## Sub-phase overview

| ID | Title | Depends on | Usable outcome |
|---|---|---|---|
| 10.1 | Audio capture and push-to-talk | Phase 9 | Holding Super+V records the mic (OSD shows "mic live"); `priya voice record` saves a clip |
| 10.2 | Speech-to-text | 10.1 | `priya voice transcribe clip.wav` and push-to-talk produce text locally |
| 10.3 | Listening UI and live transcripts | 10.2 | Hold Super+V → words appear in the bar while speaking → released → normal answer |
| 10.4 | Spoken answers | 10.3 | Voice-started questions get a short spoken reply; speaking again interrupts it |
| 10.5 | Wake word | 10.3 | Optional "Hey Priya" opens the bar in listening mode |
| 10.6 | Voice approvals and stop phrase | 10.3, 10.4 | "Yes" / "no" answers approvals; the confirm word works; "Priya, stop everything" triggers the kill switch |
| 10.7 | Hindi | 10.2, 10.4 | Speak Hindi or Hinglish, hear Hindi answers; whole UI switchable to Hindi |
| 10.8 | Privacy controls | 10.2 | Voice settings page; audio deleted after transcription by default; optional voice history |
| 10.9 | Optional cloud voice | 10.8 | User can opt in to a cloud transcription or voice provider; local stays default and fallback |
| 10.10 | Phase gate | all | Full task by voice on the Zenbook |

---

### 10.1 — Audio capture and push-to-talk
**Outcome:** holding Super+V captures 16 kHz mono audio from the default PipeWire source, with a visible "mic live" OSD; releasing it hands the clip to the voice pipeline (`priya voice record --out clip.wav` for testing).
**Features:** VOX-01.
**Depends on:** Phase 9.

**Tasks**
- [ ] `modules/voice` process module skeleton: `module.toml` (`kind = "process"`, methods `voice.ptt_start`, `voice.ptt_stop`, `voice.status`; capability `audio:capture`; settings schema section `[voice]`), supervised by priyad.
- [ ] `AudioSource` port with a PipeWire adapter (`pipewire` crate stream, 16 kHz mono s16; follows the default source and WirePlumber device changes); ring buffer in memory, clip written only to `$XDG_RUNTIME_DIR/priya/voice/` (tmpfs).
- [ ] Hyprland binds in `/usr/share/priya/desktop/hyprland.conf`: `bind = SUPER, V, exec, priya voice ptt start` and `bindr = SUPER, V, exec, priya voice ptt stop` (release); CLI `priya voice ptt start|stop` calls the module through `v1.module.call`.
- [ ] Voice activity detection (Silero VAD as used by whisper.cpp, or webrtc-vad) to trim leading/trailing silence and detect "no speech"; hard cap 60 s per clip (setting `voice.max_clip_secs`).
- [ ] Mic-live OSD (Phase 5 OSD family) and a mic dot in the top bar whenever capture is active; capture refused if the source is muted, with a plain message.
- [ ] Events (no audio content): `module.event` `voice.capture_started`, `voice.capture_stopped {duration_ms, speech_detected}`.
- [ ] Hardware probe in `priya doctor`: default source present, sample rate, `sof-firmware` loaded on Intel laptops.

**UI**
- Mic-live OSD and top-bar mic dot; states: live, muted source, no input device, busy (another app has exclusive capture), error.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: Super+V hold/release; Esc cancels a capture without sending. Screen-reader names: "Microphone on", "Microphone muted", "No microphone found".
- 👤 Mockup approval (`ui/mockups/10.1/`) before building; gallery sign-off after.

**Tests**
- Unit: clip-length cap, VAD trimming on fixture audio (silence, speech, speech with long pause), ring-buffer wrap.
- Contract: `AudioSource` port suite against a fake source (WAV replay) and the PipeWire adapter (Linux CI with a PipeWire null sink/source).
- Integration (fake machine): `priya voice record` with a virtual PipeWire source playing `fixtures/audio/en/hello.wav` → clip matches within tolerance; no file outside `$XDG_RUNTIME_DIR`.
- UI: OSD screenshots for every state × theme × scale; a11y lint.
- Perf: key-down to capture-start < 150 ms (p95); idle module RSS < 30 MiB with no capture.
- Regression: `voice_ptt_records_clip`, `voice_clip_never_written_to_disk`, `voice_osd_states`.

**Done when**
- [ ] Holding Super+V on the Zenbook records clean audio from the built-in mic; releasing ends capture within 100 ms.
- [ ] No audio file exists outside tmpfs after a capture (asserted in integration).
- [ ] 👤 OSD signed off in the gallery.

---

### 10.2 — Speech-to-text
**Outcome:** speech is turned into text locally by whisper.cpp; `priya voice transcribe clip.wav` prints a transcript, and a push-to-talk clip becomes `v1.intent.submit {text, audio_ref}`.
**Features:** VOX-03.
**Depends on:** 10.1.

**Tasks**
- [ ] `SpeechToText` port in `modules/voice` (`transcribe(clip, lang_hint) -> Transcript {text, lang, segments, confidence}`, `stream(partial_cb)`), plus contract suite.
- [ ] whisper.cpp adapter (via `whisper-rs` bindings, multilingual ggml models, CPU by default, Vulkan backend where available).
- [ ] Model selection by hardware: `priya voice benchmark` measures real-time factor for `base`, `small`, `large-v3-turbo` (quantised); picks the largest with RTF < 0.5; stored as `voice.stt_model`.
- [ ] Model fetch: on first enable, download the chosen model from the pinned upstream URL with SHA-256 verification into `~/.local/share/priya/voice/models/` (or install from a `[priyaos]` package); offline state explained.
- [ ] Partial transcripts: re-decode a sliding window every ~500 ms while capturing; final decode on release.
- [ ] Hand-off to the intent pipeline: final transcript → `v1.intent.submit {text, audio_ref, context}` with `source = "voice"` so answers know to speak (10.4); low confidence (< 0.5) → shown for confirmation instead of submitted.
- [ ] Vocabulary hints: initial prompt built from app names, project names and agent names in the tool index (improves "open kitty", "Codex", "Hyprland").

**Tests**
- Unit: confidence gating, prompt-hint builder (bounded length), segment merging.
- Contract: `SpeechToText` suite with recorded fixtures under `modules/voice/tests/fixtures/audio/` (`RECORDED.md`: mic, room, model, version): quiet room, laptop fan noise, music in background, accented English, short commands ("stop", "yes").
- Integration: PTT clip → intent submitted with correct text; "no speech" clip → nothing submitted, gentle message.
- Eval: WER on the 200-utterance English set ≤ 10 % with `small`, ≤ 7 % with `large-v3-turbo`; command-word accuracy (yes/no/stop/keep/discard) ≥ 98 %.
- Perf: end of speech → final transcript < 800 ms for a 5 s utterance on the Zenbook with the chosen model; RTF recorded in the perf dashboard.
- Regression: `stt_contract_en`, `ptt_transcript_becomes_intent`, `stt_low_confidence_asks`.

**Done when**
- [ ] English eval thresholds met on the Zenbook's selected model.
- [ ] STT works with networking disabled (asserted in integration with network namespace off).

---

### 10.3 — Listening UI and live transcripts
**Outcome:** the Priya bar has a listening state: while Super+V is held the Priya mark animates, words appear live, and on release the request runs exactly like a typed one.
**Features:** VOX-01, VOX-03.
**Depends on:** 10.2.

**Tasks**
- [ ] Bar `listening` state (11 §4): Priya mark listening animation, live transcript line, level meter, "Release to send · Esc to cancel" hint.
- [ ] Mic chip in the bar: click or Enter on it toggles a capture (for users who can't hold keys — accessibility); same pipeline as push-to-talk.
- [ ] Transcript editing: after release the transcript is selected text in the input for 1.5 s ("Edit" hint) if confidence is low; otherwise submitted.
- [ ] Transcript shown as the user turn in the answer history, with a small mic glyph.
- [ ] Reduced-motion variant: level meter becomes a static "listening" label.
- [ ] `PriyaUI` components `MicChip`, `LevelMeter`, `LiveTranscript` added to the library and gallery first.

**UI**
- States: idle, listening (no speech yet), hearing speech, transcribing, low-confidence edit, no speech heard, mic busy/denied, model missing (download needed), error, offline (local STT still works — no change), long transcript (wraps, 6 lines max then scrolls), Hindi text.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0.
- Keyboard path: Super+V, Esc, Tab to mic chip, Enter toggles. Screen-reader names: "Listening", "Transcript: …", "Microphone button, toggle listening"; live region announces the final transcript.
- 👤 Mockup approval (`ui/mockups/10.3/`) before building; journey sign-off ("hold → speak → release → answer").

**Human in the loop:** the user sees exactly what was heard before anything runs if confidence is low; Esc cancels; nothing is sent while the key is held.

**Tests**
- UI: component tests for `MicChip`/`LevelMeter`/`LiveTranscript`; screenshot every state × theme × scale; a11y lint; journey test against mock priyad with scripted partial transcripts.
- Integration: PTT → partials streamed to the bar subscription in order; Esc during capture → no intent event.
- Perf: first partial on screen < 700 ms after speech starts; bar remains at 60 fps during partial updates (frame-time trace).
- Regression: `bar_listening_states`, `voice_journey_question`, `esc_cancels_capture`.

**Done when**
- [ ] Owner asks three questions by voice from the bar without touching the mouse.
- [ ] 👤 Gallery and journey signed off.

---

### 10.4 — Spoken answers
**Outcome:** questions asked by voice get a short spoken answer (Piper, local); the full answer stays in the bar; speaking or pressing Esc stops playback.
**Features:** VOX-04.
**Depends on:** 10.3.

**Tasks**
- [ ] `TextToSpeech` port (`speak(text, voice) -> AudioStream`, `voices()`) and contract suite; Piper adapter (ONNX voices, streamed synthesis, PipeWire playback).
- [ ] Spoken summary: answers to `source = "voice"` intents carry a `spoken` field — the first one or two sentences of the plain-language answer, ≤ 30 words; never commands, paths or code (replaced by "details are in the bar").
- [ ] Setting `voice.speak_answers = "voice_only" | "always" | "never"` (default `voice_only`).
- [ ] Barge-in: new push-to-talk, wake word or Esc stops playback within 150 ms; PipeWire echo-cancel module (`libpipewire-module-echo-cancel`) enabled for the capture path so playback isn't transcribed.
- [ ] Task milestones optionally spoken for voice-started tasks ("Ready to review", "Needs your approval"), respecting Do Not Disturb and quiet hours.
- [ ] Voice choice in settings with a "Play sample" button; English default voice picked by 👤 listening test.

**UI**
- Bar "speaking" indicator on the answer (wave glyph + Stop button); Voice settings: voice picker, speed, sample. States: speaking, stopped, voice missing (download), error.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0. Keyboard: Esc stops; Tab to Stop. Screen-reader name "Stop speaking"; when Orca is running, spoken answers default to `never` to avoid two voices (user can override).
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: spoken-summary extractor (drops code/paths, word cap) on 50 answer fixtures; DND suppression.
- Contract: `TextToSpeech` suite (Piper + fake): produces audio, honours stop, lists voices.
- Integration: voice intent → answer → audio played to a PipeWire null sink; typed intent → no audio.
- Perf: answer text ready → first audio < 400 ms; barge-in stop < 150 ms.
- Eval: 30 spoken answers rated by owner for clarity (target ≥ 4/5 average).
- Regression: `voice_answer_spoken`, `typed_answer_silent`, `barge_in_stops_tts`, `orca_disables_tts_default`.

**Done when**
- [ ] End of speech → first spoken word < 2 s with a local fast model on the Zenbook (p50 over 20 questions).
- [ ] Priya's own voice is never transcribed as a new request (echo test with speakers at 70 % volume).

---

### 10.5 — Wake word
**Outcome:** with the setting on, saying "Hey Priya" opens the bar in listening mode; detection is local, off by default, and the mic indicator stays visible while armed.
**Features:** VOX-02.
**Depends on:** 10.3.

**Tasks**
- [ ] openWakeWord runner (ONNX, 80 ms frames) in the voice module, reading the same PipeWire stream; only the wake-word model sees audio until it fires.
- [ ] Custom "Hey Priya" model: training recipe in `modules/voice/wakeword/` (synthetic positives generated with Piper voices in English and Hindi accents, negatives from public speech/noise sets per openWakeWord's training notebook); trained model shipped with its `RECORDED.md` (data, seed, version).
- [ ] Setting `voice.wake_word = false` (default); threshold and sensitivity slider; armed only when the session is unlocked, not on battery saver unless the user allows it.
- [ ] Persistent armed indicator (top-bar mic outline) and a one-click "Stop listening for Hey Priya".
- [ ] After detection: short chime (respects DND), bar opens in `listening`, capture ends on 1 s silence (VAD) or 15 s cap.
- [ ] Event `voice.wake_detected {score}` (no audio); false-trigger feedback ("That wasn't for you") lowers sensitivity suggestion.

**UI**
- Top-bar armed indicator; Voice settings toggle with plain explanation ("Your mic listens on this computer only for 'Hey Priya'. Nothing is recorded until then."). States: off, armed, triggered, paused (locked screen / battery saver), model missing.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard-operable toggle; screen-reader names "Wake word listening on/off".
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** the user turns it on explicitly; the indicator is always visible while armed; one click disarms.

**Tests**
- Unit: arming rules (locked, battery saver, setting), silence end-pointing.
- Contract: `WakeWordDetector` port suite with fixture clips (positives in 6 voices/accents, near-misses "hey Siri", "hey Pria-nka", TV audio).
- Eval: false-accepts < 1 per hour on 10 h of recorded room/TV/meeting audio; false-rejects ≤ 10 % on 200 owner + synthetic positives.
- Perf: armed CPU ≤ 3 % of one core (p95) on the Zenbook; added power draw measured on L4.
- Security: wake word inactive on the lock screen (assert no detection events while locked).
- Regression: `wakeword_off_by_default`, `wakeword_not_armed_when_locked`, `wakeword_fixture_eval`.

**Done when**
- [ ] Eval thresholds met; owner keeps it on for a day with ≤ 2 false triggers.

---

### 10.6 — Voice approvals and stop phrase
**Outcome:** approval cards can be answered by voice ("yes" / "no" for `ask`, the displayed confirmation word for `confirm`), and "Priya, stop everything" triggers the kill switch while the mic is active.
**Features:** VOX-05.
**Depends on:** 10.3, 10.4.

**Tasks**
- [ ] Command grammar (EN + HI) in `modules/voice/grammar.toml`: allow (`yes`, `allow`, `haan`, `ji haan`), deny (`no`, `deny`, `nahin`), `keep it`, `discard`, `undo that`, `stop`; matched on the final transcript before the intent pipeline.
- [ ] `ask` tier: voice answer accepted only while that approval card is visible or was just spoken, and only within 15 s of capture start (PTT or wake word).
- [ ] `confirm` tier: card shows a random confirmation word from a list of distinct, easy-to-say words (e.g. "marigold", "river"); only that word confirms; 3 wrong attempts → falls back to typed confirm.
- [ ] Voice decisions recorded as `approval.granted {via: "voice", transcript}`; remembered approvals ("always allow here") are not offered by voice.
- [ ] Stop phrase: "Priya, stop everything" / "Priya, sab roko" → `priya stop-all` (05 §7) whenever capture is active; "stop" alone stops only TTS and the current answer.
- [ ] Approval prompts spoken for voice-started tasks ("Install VLC in the sandbox? Say yes or no."), batched as in 11 §5.

**UI**
- Approval card variant with voice hint ("Say yes or no", or "Say the word **marigold** to confirm"); states: waiting for voice, heard (shows transcript), accepted, not understood, expired.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard keys Y/A/N still work; screen-reader announces the expected word.
- 👤 Mockup approval before building; journey sign-off ("install by voice → approve by voice → keep by voice").

**Human in the loop:** voice never grants more than a click would; `confirm` needs the on-screen word, so a stray "yes" from a video cannot confirm; unanswered approvals still pause the task and never auto-allow.

**Tests**
- Unit: grammar matching (EN, HI, Hinglish, filler words "uh yes"), time-window rules, confirm-word generation (no homophones in the list), attempt counting.
- Integration: voice "yes" on an `ask` card → `approval.granted via voice`; "yes" on a `confirm` card → rejected with hint; stop phrase during two running tasks → both runs stopped within 5 s and tasks `paused`.
- Security: TTS playback of "yes" through speakers does not approve (echo cancel + window); approval from a transcript older than 15 s rejected; voice cannot create a remembered approval.
- UI: card variants screenshots; journey test.
- Regression: `voice_ask_yes_allows`, `voice_confirm_requires_word`, `voice_stop_everything_kills`, `voice_cannot_remember_approval`.

**Done when**
- [ ] Owner completes ask → approve → keep by voice; kill-switch phrase works from wake word and from push-to-talk.

---

### 10.7 — Hindi
**Outcome:** the owner can speak Hindi or Hinglish and get Hindi answers spoken in a Hindi voice, and can switch every PriyaOS screen to Hindi.
**Features:** VOX-06, DS-20.
**Depends on:** 10.2, 10.4.

**Tasks**
- [ ] STT language: `voice.language = "auto" | "en" | "hi"`; auto uses whisper language detection per clip; Hinglish handled by keeping the multilingual model and normalising Devanagari/Latin output per setting `voice.hindi_script = "devanagari" | "latin"`.
- [ ] Answer language: intents carry `lang`; Priya AI system prompt asks for answers in the user's language; classifier and intent fixtures extended with Hindi/Hinglish examples.
- [ ] Piper Hindi voice (`hi_IN`, chosen by 👤 listening test); TTS voice follows answer language.
- [ ] UI translation pipeline: `lupdate` extracts `qsTr` strings to `ui/i18n/priya_hi.ts`; `lrelease` builds `.qm` in the package; language setting `general.ui_language` switches live.
- [ ] Backend strings: ADR for localising `user_message` and CLI text (proposed: Fluent `.ftl` files per crate, locale from settings); implement for priyad errors, notifications and approval texts.
- [ ] Hindi translation of all shipped strings (draft by Priya AI, reviewed and corrected by the owner 👤); glossary `ui/i18n/glossary-hi.md` for fixed terms (Sandbox, Rewind, mode names keep English names with Hindi explanation).
- [ ] Pseudo-locale (`xx-long`, +40 % length) CI job to catch clipping.

**UI**
- Every existing screen in Hindi; Language section in Settings → General; voice language in Settings → Voice. States as each screen defines, plus mixed-script text and long Hindi strings.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0 (Noto Sans Devanagari, line height checked for matras).
- Keyboard path unchanged; screen-reader names translated (Orca with Hindi voice via speech-dispatcher).
- 👤 Mockup approval for the language switcher; gallery sign-off of the full gallery in Hindi.

**Tests**
- Unit: language routing, script normalisation, Fluent fallback to English for missing keys.
- Contract: STT suite with Hindi and Hinglish fixtures; TTS suite with Hindi text (numbers, dates, mixed English words).
- Eval: Hindi WER ≤ 25 % (`small`) / ≤ 18 % (`large-v3-turbo`) on a 150-utterance set; intent classification accuracy in Hindi within 5 points of English on the translated intent set.
- UI: full gallery screenshots in Hindi × theme × scale; pseudo-locale clipping check; untranslated-string check (no English in Hindi builds except glossary terms).
- Regression: `hindi_voice_question`, `ui_language_switch_live`, `no_untranslated_strings_hi`, `pseudolocale_no_clipping`.

**Done when**
- [ ] 100 % of shipped UI strings translated and owner-reviewed.
- [ ] Owner completes the Phase 10 demo in Hindi.

---

### 10.8 — Privacy controls
**Outcome:** a Voice settings page makes every voice privacy choice visible; by default audio is deleted right after transcription and never leaves the device.
**Features:** VOX-07.
**Depends on:** 10.2.

**Tasks**
- [ ] Defaults: `voice.keep_audio = false`, `voice.keep_transcripts = true` (transcripts are the intent text, already in task history), `voice.cloud = false`.
- [ ] Deletion: clip removed from tmpfs right after the final transcript; `voice.audio_deleted` event; on crash, startup sweeps `$XDG_RUNTIME_DIR/priya/voice/`.
- [ ] Optional voice history (opt-in): clips stored encrypted at rest under `~/.local/share/priya/voice/history/` with retention (default 7 days), viewable and deletable in Settings and by `priya voice history list|delete`.
- [ ] Cedar policy shipped by default: `forbid UseModel` for data class `audio` on non-local providers unless `voice.cloud` is on.
- [ ] Settings → Voice page: push-to-talk key, wake word, language, voice, speak answers, keep audio, cloud voice, "Delete all voice data".
- [ ] Natural-language settings coverage: "stop listening for Hey Priya", "never send my voice to the cloud".

**UI**
- Voice settings page; states: default, edited, invalid, locked by admin, wake word armed, history present/empty.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names for every control.
- 👤 Mockup approval before building; gallery sign-off after.

**Tests**
- Unit: retention pruning, policy evaluation for `audio` class.
- Integration: 50 PTT requests with default settings → zero audio files remain; history opt-in → files encrypted, pruned after retention (fake clock).
- Security: with default settings, egress capture during 20 voice requests shows no traffic to any STT/TTS endpoint and no audio payloads in model calls; event log contains no audio blobs.
- UI: settings page screenshots; NL settings fixtures.
- Regression: `voice_audio_deleted_default`, `voice_no_cloud_default`, `voice_history_retention`.

**Done when**
- [ ] Default-settings privacy test green; "Delete all voice data" removes clips, history and wake-word feedback.

---

### 10.9 — Optional cloud voice
**Outcome:** users who want better accuracy can opt in to a cloud STT/TTS provider; Priya AI's `transcribe` role routes to it; local whisper/Piper remain the fallback offline.
**Features:** VOX-03, VOX-04, VOX-07.
**Depends on:** 10.8.

**Tasks**
- [ ] Cloud STT adapter behind `SpeechToText` using Priya AI role `transcribe` (any enabled provider whose catalog entry lists `audio` input); request via `v1.ai.ask`-style module call so keys stay in priyad.
- [ ] Cloud TTS adapter behind `TextToSpeech` (provider speech endpoints); catalog gets `output = ["audio"]` modalities.
- [ ] Opt-in flow: plain explanation of what is sent and to whom, per-provider choice, cost estimate per minute; `voice.cloud = true` only after confirmation.
- [ ] Fallback: offline, provider error or budget hit → local adapter with a one-line notice.
- [ ] Costs recorded as `model.called` with role `transcribe`, visible in Usage & cost.

**UI**
- Cloud voice section in Settings → Voice; opt-in sheet; states: off, choosing provider, on, offline fallback, budget hit.
- Light, dark, OLED-black, high-contrast; scale 1.0 and 2.0; keyboard and screen-reader names.
- 👤 Mockup approval before building; gallery sign-off after.

**Human in the loop:** cloud voice is never turned on implicitly; the bar shows a cloud glyph on the mic chip while it is in use.

**Tests**
- Contract: cloud adapters pass the same STT/TTS suites with recorded HTTP cassettes (secrets scrubbed).
- Integration: provider down → local fallback, transcript still produced; budget exhausted → local fallback.
- Live: one 3-second clip per enabled provider, budget ≤ $0.02.
- Regression: `cloud_stt_fallback_local`, `cloud_voice_requires_optin`.

**Done when**
- [ ] With cloud voice on, WER improvement is reported in `priya voice benchmark`; turning it off restores local-only behaviour (verified by egress test).

---

### 10.10 — Phase gate
**Outcome:** voice works end to end on the owner's Zenbook in English and Hindi.
**Features:** all Phase 10 features.
**Depends on:** 10.1–10.9.

**Tasks**
- [ ] Run the full regression suite and nightly suites; fix failures.
- [ ] L4 checklist on the Zenbook (below); record results in this file.
- [ ] Update `features.md` statuses, `docs/human/05-the-phases.md`, and `docs/human/04-everything-priyaos-will-do.md` voice section.

**Done when**
- [ ] See phase gate below.

## Phase gate

- [ ] Whole regression suite green on `main`, plus nightly e2e (VM with virtual PipeWire devices), perf, security and eval suites.
- [ ] Owner demo: hold Super+V → "what's using my battery?" → spoken answer; "Hey Priya, install VLC" → plan → "yes" → "ready to review" → "keep it"; the same in Hindi with the UI in Hindi; "Priya, stop everything" during a run.
- [ ] L4 checklist (Zenbook): built-in mic and headset mic both work; Bluetooth headset (HFP) capture works; echo test with speakers; wake-word false triggers ≤ 2 over a normal working day; battery drain with wake word armed measured and documented; latency end of speech → first spoken word < 2 s (local).
- [ ] Privacy check: no audio leaves the device with default settings (egress log), no audio files on disk after a day of use.
- [ ] Docs: [07 §3](../architecture/07-interfaces.md) updated with final model choices; ADR for backend localisation added; `features.md` statuses for VOX-01…07 and DS-20 set; `docs/human/05-the-phases.md` status updated.

## Out of scope

- Speaker identification / voice biometrics (anyone at the keyboard can talk to it; voice never grants more than a click).
- Continuous dictation into other apps (typing by voice into arbitrary text fields).
- Languages other than English and Hindi (pipeline supports them; translations and fixtures come later).
- Voice on the phone companion (Phase 15).
- Meeting transcription or always-on recording of any kind.
