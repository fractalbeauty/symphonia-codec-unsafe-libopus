//! Example for testing. Takes a file path, decodes it, and outputs a wav file.

use symphonia::core::{
    codecs::{audio::AudioDecoderOptions, registry::CodecRegistry},
    formats::{probe::Hint, FormatOptions, TrackType},
    io::MediaSourceStream,
    meta::MetadataOptions,
};
use symphonia_codec_unsafe_libopus::UnsafeLibopusDecoder;

fn main() {
    // get file path
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("file path not provided");

    let src = std::fs::File::open(path).expect("failed to open media");

    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let hint = Hint::new();

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .expect("unsupported format");

    let track = format
        .default_track(TrackType::Audio)
        .expect("no audio track");

    let dec_opts: AudioDecoderOptions = Default::default();

    // the important part:
    let mut codec_registry = CodecRegistry::new();
    // register the Opus codec
    codec_registry.register_audio_decoder::<UnsafeLibopusDecoder>();
    // also register the default Symphonia codecs if needed
    symphonia::default::register_enabled_codecs(&mut codec_registry);

    let mut decoder = codec_registry
        .make_audio_decoder(
            track
                .codec_params
                .as_ref()
                .expect("codec parameters missing")
                .audio()
                .unwrap(),
            &dec_opts,
        )
        .expect("unsupported codec");

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    // buffer for decoded samples
    let mut sample_buffer = Vec::<f32>::new();

    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(symphonia::core::errors::Error::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(err) => {
                // A unrecoverable error occured, halt decoding.
                panic!("{}", err);
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(decoded) => {
                // Consume the decoded audio samples (see below).

                let mut decoded_buf = Vec::<f32>::new();
                decoded.copy_to_vec_interleaved(&mut decoded_buf);
                sample_buffer.extend(decoded_buf);
            }
            Err(symphonia::core::errors::Error::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                println!("io error");
                continue;
            }
            Err(symphonia::core::errors::Error::DecodeError(e)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                println!("decode error");
                dbg!(e);
                continue;
            }
            Err(err) => {
                // An unrecoverable error occured, halt decoding.
                panic!("{}", err);
            }
        }
    }

    // write wav file
    let spec = hound::WavSpec {
        channels: decoder
            .codec_params()
            .channels
            .as_ref()
            .map(|c| c.count())
            .unwrap() as u16,
        sample_rate: decoder.codec_params().sample_rate.unwrap(),
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create("output.wav", spec).unwrap();
    for s in sample_buffer {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
}
