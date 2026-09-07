use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use netzip_fullpull::{
    Official5188BulkEnvelope, Official5188Frame, Official5188Kind, Official5188Reassembler,
    Official5188ZlibObjectEnvelope, assemble_bulk_envelopes,
};
use netzipapi_rust_demo::extract_official_5188_frames;
use serde::Serialize;

#[derive(Serialize)]
struct NestedEntry {
    stream: usize,
    frame_index: usize,
    wire_kind: String,
    payload_len: usize,
    payload_file: String,
    decoded_zlib_file: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let usage = "usage: official_5188_bulk_extract CAPTURE OUTPUT_DIR";
    let capture = args.next().map(PathBuf::from).ok_or(usage)?;
    let output_dir = args.next().map(PathBuf::from).ok_or(usage)?;
    if args.next().is_some() {
        return Err(usage.into());
    }
    fs::create_dir_all(&output_dir)?;

    let mut streams: HashMap<(u32, u32), BTreeMap<u32, Official5188BulkEnvelope>> = HashMap::new();
    for captured in extract_official_5188_frames(capture)? {
        if captured.wire_kind != "3e04" {
            continue;
        }
        let frame = Official5188Frame {
            kind: Official5188Kind::SERVER_META,
            metadata: captured.metadata,
            payload: captured.payload,
        };
        let envelope = Official5188BulkEnvelope::decode(&frame)?;
        let key = (envelope.header_word, envelope.sequence_word);
        let by_offset = streams.entry(key).or_default();
        if let Some(existing) = by_offset.get(&envelope.block_offset) {
            if existing != &envelope {
                return Err(format!("conflicting 3e04 block at {}", envelope.block_offset).into());
            }
        } else {
            by_offset.insert(envelope.block_offset, envelope);
        }
    }

    let mut manifest = Vec::new();
    for (stream_index, ((_header, _length), blocks)) in streams.into_iter().enumerate() {
        let envelopes = blocks.into_values().collect::<Vec<_>>();
        let assembled = assemble_bulk_envelopes(&envelopes)?;
        fs::write(
            output_dir.join(format!("stream-{stream_index:02}.body.bin")),
            &assembled.body,
        )?;
        let mut decoder = Official5188Reassembler::new();
        let frames = decoder.push(&assembled.body)?;
        if decoder.buffered_len() != 0 {
            return Err(format!("nested stream {stream_index} has trailing bytes").into());
        }
        for (frame_index, frame) in frames.into_iter().enumerate() {
            let stem = format!(
                "stream-{stream_index:02}-frame-{frame_index:02}-{}",
                frame.kind.wire_hex()
            );
            let payload_file = format!("{stem}.payload.bin");
            fs::write(output_dir.join(&payload_file), &frame.payload)?;
            let decoded_zlib_file = match Official5188ZlibObjectEnvelope::decode(&frame) {
                Ok(object) => {
                    let name = format!("{stem}.zlib.bin");
                    fs::write(output_dir.join(&name), object.decoded)?;
                    Some(name)
                }
                Err(_) => None,
            };
            manifest.push(NestedEntry {
                stream: stream_index,
                frame_index,
                wire_kind: frame.kind.wire_hex(),
                payload_len: frame.payload.len(),
                payload_file,
                decoded_zlib_file,
            });
        }
    }
    fs::write(
        output_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("exported {} nested frames", manifest.len());
    Ok(())
}
