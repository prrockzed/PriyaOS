# Phase 10 — Voice

**Goal.** Talk to PriyaOS: push-to-talk and an optional "Hey Priya" wake word, local speech recognition and spoken answers, voice approvals — private by default.

**Depends on:** Phases 5 and 9.
**Features:** VOX-01 … VOX-07.
**Read first:** [07 §3](../architecture/07-interfaces.md).

## Build

- [ ] `voice` module (process module) with `SpeechToText` and `TextToSpeech` ports
- [ ] whisper.cpp adapter (model size chosen by hardware; multilingual model for Hindi), streaming partial transcripts into the Priya bar
- [ ] Piper TTS adapter with English and Hindi voices; speak short answers only, full answer in the bar
- [ ] Push-to-talk (hold Super+V) via Hyprland bind → PipeWire capture
- [ ] Optional wake word (openWakeWord, local, off by default; on-device only; visible mic indicator while listening)
- [ ] Voice approvals: `ask` tier by "yes/no"; `confirm` tier requires the displayed confirmation word
- [ ] Voice kill switch phrase ("Priya, stop everything")
- [ ] Privacy settings: cloud STT allowed or not, keep voice history or not (default: delete audio after transcription)
- [ ] Cloud STT/TTS adapters (optional) through Priya AI role `transcribe`

## Tests

- L0: command phrase matching, confirmation-word logic.
- L1: STT/TTS contract suites with recorded audio fixtures (English, Hindi, noisy room), transcript accuracy thresholds.
- L4: wake word false-trigger rate over an hour of normal room audio (target < 1/hour); latency from end of speech to first answer token < 2 s with local models on the Zenbook.

## Done when

- [ ] Owner can run a full task by voice (ask → approve → review summary spoken → merge by voice)
- [ ] No audio leaves the device with default settings (verified by egress logs)

**Demo.** Hold Super+V: "what's using my battery?" → spoken short answer; "Hey Priya, install VLC" → "Ready to review" → "merge" → done.
