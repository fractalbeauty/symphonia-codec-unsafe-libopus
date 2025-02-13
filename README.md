# symphonia-codec-unsafe-libopus
This crate provides an Opus codec for [Symphonia](https://github.com/pdeljanov/Symphonia).
It uses [a fork](https://github.com/hazelmeow/opus-rs) of [opus-rs](https://github.com/SpaceManiac/opus-rs) which provides high-level bindings for libopus.
Instead of using `audiopus-sys`, the fork uses [unsafe-libopus](https://github.com/DCNick3/unsafe-libopus), which is a translation of libopus from C to unsafe Rust.
This avoids having to compile the C version of libopus, which makes cross-compilation much less painful.

This crate currently uses the unreleased `dev-0.6` branch of Symphonia and is not compatible with 0.5. Sorry!

## Usage
Instead of using `symphonia::default::get_codecs().make_audio_decoder()`, create a `CodecRegistry` and register the codecs yourself.

```rust
let mut codec_registry = CodecRegistry::new();

// register the Opus codec
codec_registry.register_audio_decoder::<UnsafeLibopusDecoder>();

// also register the default Symphonia codecs if needed
symphonia::default::register_enabled_codecs(&mut codec_registry);

// as you were
codec_registry.make_audio_decoder(/* ... */);
```

## License
MIT.
