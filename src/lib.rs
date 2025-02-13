use opus::Decoder;
use std::sync::Mutex;
use symphonia_core::{
    audio::{AsGenericAudioBufferRef, AudioBuffer, AudioMut, AudioSpec, GenericAudioBufferRef},
    codecs::{
        audio::{
            well_known::CODEC_ID_OPUS, AudioCodecParameters, AudioDecoder, AudioDecoderOptions,
            FinalizeResult,
        },
        registry::{RegisterableAudioDecoder, SupportedAudioCodec},
        CodecInfo,
    },
    errors::{decode_error, unsupported_error, Result},
    formats::Packet,
    support_audio_codec,
};

// TODO: this struct needs to be Send+Sync. is the mutex ok?
pub struct UnsafeLibopusDecoder {
    // Codec paramters.
    params: AudioCodecParameters,
    // Inner decoder.
    inner: Mutex<Decoder>,
    // Buffer for interleaved decoder output.
    interleaved_buf: Vec<f32>,
    // Output buffer.
    buf: AudioBuffer<f32>,
}

impl UnsafeLibopusDecoder {
    pub fn try_new(params: &AudioCodecParameters, _opts: &AudioDecoderOptions) -> Result<Self> {
        // Verify codec ID.
        if params.codec != CODEC_ID_OPUS {
            return unsupported_error("unsafe-libopus: invalid codec");
        }

        // get sample rate and channels from codec params
        // most likely set by symphonia-format-ogg
        let Some(sample_rate) = params.sample_rate else {
            return unsupported_error("unsafe-libopus: missing sample rate in params");
        };
        let Some(channels) = params.channels.as_ref() else {
            return unsupported_error("unsafe-libopus: missing channels in params");
        };

        // make opus::Decoder
        let opus_channels = match channels.count() {
            1 => opus::Channels::Mono,
            2 => opus::Channels::Stereo,
            _ => return unsupported_error("unsafe-libopus: unsupported channel configuration"),
        };
        let Ok(inner) = Decoder::new(sample_rate, opus_channels) else {
            return unsupported_error("unsafe-libopus: decoder create error");
        };

        let spec = AudioSpec::new(sample_rate, channels.clone());

        // TODO: ?
        let buf_capacity = 4096;
        let buf = AudioBuffer::new(spec, buf_capacity);

        // TODO: handle delay/preskip?
        Ok(UnsafeLibopusDecoder {
            params: params.clone(),
            inner: Mutex::new(inner),
            interleaved_buf: Vec::new(),
            buf,
        })
    }

    fn decode_inner(&mut self, packet: &Packet) -> Result<()> {
        // Fill the audio buffer with silence.
        self.buf.clear();
        self.buf.render_silence(None);

        // Checked in try_new.
        let num_channels = self.params.channels.as_ref().map(|c| c.count()).unwrap();

        // Lock the decoder. This is needed because AudioDecoder requires Send + Sync.
        let Ok(mut decoder) = self.inner.lock() else {
            return decode_error("unsafe-libopus: failed to lock decoder");
        };

        // Get the number of samples in this packet.
        let Ok(packet_samples_per_ch) = decoder.get_nb_samples(packet.buf()) else {
            return decode_error("unsafe-libopus: decode error");
        };

        // Resize the buffer to have space for the packet samples.
        self.interleaved_buf
            .resize(packet_samples_per_ch * num_channels, 0.0);

        // Decode into interleaved buf.
        let Ok(decoded_samples_per_ch) =
            decoder.decode_float(packet.buf(), &mut self.interleaved_buf, false)
        else {
            return decode_error("unsafe-libopus: decode error");
        };

        // Resize output buffer.
        self.buf.resize_with_silence(decoded_samples_per_ch);

        // Copy from interleaved buf to AudioBuffer planes.
        let decoded_samples_total = decoded_samples_per_ch * num_channels;
        for (i, frame) in self.interleaved_buf[0..decoded_samples_total]
            .chunks_exact(num_channels)
            .enumerate()
        {
            for (channel, sample) in frame.iter().enumerate() {
                self.buf.plane_mut(channel).unwrap()[i] = *sample;
            }
        }

        Ok(())
    }
}

impl AudioDecoder for UnsafeLibopusDecoder {
    fn reset(&mut self) {
        // Lock the decoder. This is required because AudioDecoder requires Send + Sync.
        let Ok(mut decoder) = self.inner.lock() else {
            return;
        };

        let _ = decoder.reset_state();
    }

    fn codec_info(&self) -> &CodecInfo {
        // Only one codec is supported.
        &Self::supported_codecs().first().unwrap().info
    }

    fn codec_params(&self) -> &AudioCodecParameters {
        &self.params
    }

    fn decode(&mut self, packet: &Packet) -> Result<GenericAudioBufferRef<'_>> {
        if let Err(e) = self.decode_inner(packet) {
            self.buf.clear();
            Err(e)
        } else {
            Ok(self.buf.as_generic_audio_buffer_ref())
        }
    }

    fn finalize(&mut self) -> FinalizeResult {
        Default::default()
    }

    fn last_decoded(&self) -> GenericAudioBufferRef<'_> {
        self.buf.as_generic_audio_buffer_ref()
    }
}

impl RegisterableAudioDecoder for UnsafeLibopusDecoder {
    fn try_registry_new(
        params: &AudioCodecParameters,
        opts: &AudioDecoderOptions,
    ) -> Result<Box<dyn AudioDecoder>>
    where
        Self: Sized,
    {
        Ok(Box::new(UnsafeLibopusDecoder::try_new(params, opts)?))
    }

    fn supported_codecs() -> &'static [SupportedAudioCodec] {
        &[support_audio_codec!(CODEC_ID_OPUS, "opus", "Opus")]
    }
}
