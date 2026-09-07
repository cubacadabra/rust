# Game-owned audio

The runtime exposes a deliberately small one-shot sound seam for game packages.
Games name sounds in their manifest and trigger those names from Luau; scripts do
not receive file paths or control host audio objects directly.

```json
{
  "assets": {
    "audio": {
      "charm-learned": {
        "path": "assets/audio/charm-learned.wav",
        "volume": 0.72
      }
    }
  }
}
```

```luau
api.audio:play("charm-learned")
api.audio:play("charm-learned", { volume = 0.5 })
```

Audio ids are 1–64 ASCII letters, numbers, dots, dashes, or underscores. The
asset and command volumes are each between `0` and `1` and are multiplied by
the host. Package audio currently uses 48 kHz PCM WAV files so the same source
asset is practical on browser and native hosts. The builder accepts at most 64
declared sounds, each no larger than 4 MiB and located under `assets/`.

The host polls JSON commands through the C/WASM ABI:

```json
{"type":"play","id":"charm-learned","volume":1.0}
```

The command queue is bounded. When a host is slow or has not implemented audio,
the runtime discards the oldest pending sound rather than interrupting the game.
Missing assets and playback failures are likewise non-fatal host concerns.

This API is experimental while package `sdkVersion` is below `1.0`. Spatial
placement, looping music, fades, mixer buses, and streaming are intentionally
outside this first contract; they should be designed from demonstrated game
needs rather than inferred from the one-shot API.
