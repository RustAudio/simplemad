/*!
This crate provides an interface to libmad, allowing the decoding of MPEG
audio files, including MP3s.

`simplemad::decode` takes a byte-oriented source and returns a channel that
yields `Result<Frame, MadError>`. `Frame` and `MadError` correspond to libmad's
struct types `mad_pcm` and `mad_error`, respectively. Samples are signed 32 bit
integers and are organized into channels. For stereo, the left channel is
channel 0.

MP3 files often begin with metadata, which will cause libmad to produce errors.
It is safe to ignore these errors until libmad reaches audio data and starts
producing frames.

# Examples
```
use simplemad::decode;
use std::fs::File;
use std::path::Path;

let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
let file = File::open(&path).unwrap();
let mut decoder = decode(file);

for item in decoder.iter() {
    match item {
        Err(e) => println!("Error: {:?}", e),
        Ok(frame) => {
          println!("Frame sample rate: {}", frame.sample_rate);
          println!("First audio sample (left channel): {}", frame.samples[0][0]);
          println!("First audio sample (right channel): {}", frame.samples[1][0]);
        }
    }
}
```
*/

#![crate_name = "simplemad"]

extern crate libc;
use std::thread;
use std::io;
use std::io::Read;
use std::sync::mpsc;
use std::sync::mpsc::{SyncSender, Receiver};
use std::default::Default;
use std::marker::Send;
use std::option::Option::None;
use libc::types::common::c95::c_void;
use libc::types::common::c99::*;
use libc::types::os::arch::c95::*;
use std::cmp::min;

/// A decoded frame
#[derive(Clone, Debug)]
pub struct Frame {
    /// Number of samples per second
    pub sample_rate: usize,
    /// Samples are signed 32 bit integers and are organized into channels.
    /// For stereo, the left channel is channel 0.
    pub samples: Vec<Vec<i32>>,
}

/// Decode a file in full
pub fn decode<T>(reader: T) -> Receiver<Result<Frame, MadError>>
    where T: io::Read + Send + 'static {
    spawn_decoder(reader, None, None)
}

/// Decode part of a file from `start_time` to `end_time`, measured in milliseconds
pub fn decode_interval<T>(reader: T, start_time: f32, end_time: f32)
    -> Receiver<Result<Frame, MadError>> where T: io::Read + Send + 'static {
    spawn_decoder(reader, Some(start_time), Some(end_time))
}

fn spawn_decoder<T>(mut reader: T, start_time: Option<f32>, end_time: Option<f32>)
    -> Receiver<Result<Frame, MadError>> where T: io::Read + Send + 'static {
    let (tx, rx) = mpsc::sync_channel::<Result<Frame, MadError>>(2);
    thread::spawn(move || {
        let input_buffer = Box::new([0u8; 32768]);
        let mut decoder: MadDecoder = Default::default();
        let mut message = MadMessage {
            buffer: input_buffer,
            reader: &mut reader,
            sender: &tx,
            start_time: start_time,
            end_time: end_time,
            current_time: 0.0,
        };
        unsafe {
            let message_ptr = &mut message as *mut _ as *mut c_void;
            mad_decoder_init(&mut decoder,
                             message_ptr,
                             input_cb,
                             header_cb,
                             empty_cb,
                             output_cb,
                             error_cb,
                             empty_cb);
            mad_decoder_run(&mut decoder, MadDecoderMode::Sync);
            mad_decoder_finish(&mut decoder);
        }
    });
    rx
}

struct MadMessage<'a> {
    buffer: Box<[u8]>,
    reader: &'a mut (io::Read + 'a),
    sender: &'a SyncSender<Result<Frame, MadError>>,
    start_time: Option<f32>,
    end_time: Option<f32>,
    current_time: f32,
}

#[link(name = "mad")]
extern {
    fn mad_decoder_init(decoder: *mut MadDecoder,
                        message: *mut c_void,
                        input_cb: extern fn(message: *mut c_void,
                                            stream: &MadStream) -> MadFlow,
                        header_cb: extern fn(message: *mut c_void,
                                             header: &MadHeader) -> MadFlow,
                        filter_cb: extern fn(),
                        output_cb: extern fn(message: *mut c_void,
                                             header: &MadHeader,
                                             pcm: &MadPcm) -> MadFlow,
                        error_cb: extern fn(message: *mut c_void,
                                            stream: &MadStream,
                                            frame: *const c_void) -> MadFlow,
                        message_cb: extern fn());
    fn mad_decoder_run(decoder: &mut MadDecoder, mode: MadDecoderMode) -> c_int;
    fn mad_decoder_finish(decoder: &mut MadDecoder) -> c_int;
    fn mad_stream_buffer(stream: &MadStream,
                         buf_start: *const u8,
                         buf_samples: size_t);
}

/// libmad callbacks return MadFlow values, which are used to control the decoding process
#[allow(dead_code)]
#[repr(C)]
enum MadFlow {
    /// continue normally
    Continue = 0x0000,

    /// stop decoding normally
    Stop = 0x0010,

    /// stop decoding and signal an error
    Break = 0x0011,

    /// ignore the current frame
    Ignore = 0x0020,

}

/// Errors generated by libmad
#[derive(Debug, Clone)]
#[repr(C)]
pub enum MadError {
    /// no error
    None = 0x0000,

    /// input buffer too small (or eof)
    BufLen = 0x0001,

    /// invalid (null) buffer pointer
    BufPtr = 0x0002,

    /// not enough memory
    NoMem = 0x0031,

    /// lost synchronization
    LostSync = 0x0101,

    /// reserved header layer value
    BadLayer = 0x0102,

    /// forbidden bitrate value
    BadBitRate = 0x0103,

    /// reserved sample frequency value
    BadSampleRate = 0x0104,

    /// reserved emphasis value
    BadEmphasis = 0x0105,

    /// crc check failed
    BadCRC = 0x0201,

    /// forbidden bit allocation value
    BadBitAlloc = 0x0211,

    /// bad scalefactor index
    BadScaleFactor = 0x0221,

    /// bad bitrate/mode combination
    BadMode = 0x0222,

    /// bad frame length
    BadFrameLen = 0x0231,

    /// bad big_values count
    BadBigValues = 0x0232,

    /// reserved block_type
    BadBlockType = 0x0233,

    /// bad scalefactor selection info
    BadScFSI = 0x0234,

    /// bad main_data_begin pointer
    BadDataPtr = 0x0235,

    /// bad audio data length
    BadPart3Len = 0x0236,

    /// bad huffman table select
    BadHuffTable = 0x0237,

    /// huffman data overrun
    BadHuffData = 0x0238,

    /// incompatible block_type for joint stereo
    BadStereo = 0x0239,
}

#[repr(C)]
struct MadBitPtr {
    byte: size_t,
    cache: uint16_t,
    left: uint16_t,
}

#[repr(C)]
struct MadStream {
    buffer: size_t,
    buff_end: size_t,
    skip_len: c_ulong,
    sync: c_int,
    free_rate: c_ulong,
    this_frame: size_t,
    next_frame: size_t,
    ptr: MadBitPtr,
    anc_ptr: MadBitPtr,
    anc_bitlen: c_uint,
    buffer_mdlen: size_t,
    md_len: c_uint,
    options: c_int,
    error: MadError,
}

#[allow(dead_code)]
#[repr(C)]
enum MadLayer {
    Layer1 = 1,
    Layer2 = 2,
    Layer3 = 3,
}

#[allow(dead_code)]
#[repr(C)]
enum MadMode {
    SingleChannel = 0,
    DualChannel = 1,
    JointStereo = 2,
    Stereo = 3,
}

#[allow(dead_code)]
#[repr(C)]
enum MadEmphasis {
    None = 0,
    Fifty15Us = 1,
    CcittJ17 = 3,
    Reserved = 2,
}

#[repr(C)]
struct MadTimer {
    seconds: c_long,
    fraction: c_ulong,
}

#[repr(C)]
struct MadHeader {
    layer: MadLayer,
    mode: MadMode,
    mode_extension: c_int,
    emphasis: MadEmphasis,
    bit_rate: c_ulong,
    sample_rate: c_uint,
    crc_check: c_ushort,
    crc_target: c_ushort,
    flags: c_int,
    private_bits: c_int,
    duration: MadTimer,
}

#[repr(C)]
struct MadPcm {
    sample_rate: c_uint,
    channels: uint16_t,
    length: uint16_t,
    samples: [[int32_t; 1152]; 2],
}

#[allow(dead_code)]
#[repr(C)]
enum MadDecoderMode {
    Sync = 0,
    Async
}

impl Default for MadDecoderMode {
    fn default() -> MadDecoderMode {
        MadDecoderMode::Sync
    }
}

#[derive(Default)]
#[repr(C)]
struct MadAsyncParameters {
    pid: c_long,
    ain: c_int,
    aout: c_int,
}

#[derive(Default)]
#[repr(C)]
struct MadDecoder {
    mode: MadDecoderMode,
    options: c_int,
    async: MadAsyncParameters,
    sync: size_t,
    cb_data: size_t,
    input_func: size_t,
    header_func: size_t,
    filter_func: size_t,
    output_func: size_t,
    error_func: size_t,
    message_func: size_t,
}

extern fn empty_cb() {

}

extern fn input_cb(msg_ptr: *mut c_void, stream: &MadStream) -> MadFlow {
    unsafe {
        let msg = &mut *(msg_ptr as *mut MadMessage);
        let buffer_size = (*msg).buffer.len();
        let next_frame_position = (stream.next_frame - stream.buffer) as usize;
        let unused_byte_count = buffer_size - min(next_frame_position, buffer_size);

        if unused_byte_count == buffer_size {
            mad_stream_buffer(stream, (*msg).buffer.as_ptr(), buffer_size as u64);
        } else {
            // Shift unused data to front of buffer
            for idx in 0 .. unused_byte_count {
                (*msg).buffer[idx] = (*msg).buffer[idx + next_frame_position];
            }

            // Refill rest of buffer
            let bytes_read = if next_frame_position == 0 {
                match (*msg).reader.read(&mut *(*msg).buffer) {
                    Ok(val) => val,
                    Err(_) => {return MadFlow::Stop;},
                }
            } else {
                let slice = &mut (*msg).buffer[unused_byte_count..buffer_size];
                match (*msg).reader.read(slice) {
                    Ok(val) => val,
                    Err(_) => {return MadFlow::Stop;},
                }
            };

            if bytes_read == 0 {
                return MadFlow::Stop;
            }

            let fresh_byte_count = (bytes_read + unused_byte_count) as u64;
            mad_stream_buffer(stream, (*msg).buffer.as_ptr(), fresh_byte_count);
        }
    }
    MadFlow::Continue
}

extern fn header_cb(msg_ptr: *mut c_void, header: &MadHeader) -> MadFlow {
    unsafe {
        let msg = &mut *(msg_ptr as *mut MadMessage);
        msg.current_time += (header.duration.seconds as f32) * 1000.0 +
                            (header.duration.fraction as f32) / 352800.0;
        match (msg.start_time, msg.end_time) {
            (Some(start_time), Some(end_time)) => {
                if msg.current_time > end_time {
                    MadFlow::Stop
                } else if msg.current_time >= start_time {
                    MadFlow::Continue
                } else {
                    MadFlow::Ignore
                }
            },
            _ => MadFlow::Continue,
        }
    }
}

#[allow(unused_variables)]
extern fn error_cb(msg_ptr: *mut c_void,
                   stream: &MadStream,
                   frame: *const c_void) -> MadFlow {
    unsafe {
        let error_type = stream.error.clone();
        let msg = &mut *(msg_ptr as *mut MadMessage);
        match (*msg).sender.send(Err(error_type)) {
            Ok(_) => {
                MadFlow::Continue
            },
            Err(_) => {
                MadFlow::Stop
            },
        }
    }
}

#[allow(unused_variables)]
extern fn output_cb(msg_ptr: *mut c_void, header: &MadHeader, pcm: &MadPcm) -> MadFlow {
    unsafe {
        let msg = &mut *(msg_ptr as *mut MadMessage);
        let frame = parse_frame(pcm);
        match (*msg).sender.send(Ok(frame)) {
            Ok(_) => {
                MadFlow::Continue
            },
            Err(_) => {
                MadFlow::Stop
            },
        }
    }
}

fn parse_frame(pcm: &MadPcm) -> Frame {
    let mut samples: Vec<Vec<i32>> = Vec::new();
    for channel_idx in 0..pcm.channels as usize {
        let mut channel: Vec<i32> = Vec::with_capacity(pcm.length as usize);
        for sample_idx in 0..pcm.length as usize {
            channel.push(pcm.samples[channel_idx][sample_idx]);
        }
        samples.push(channel);
    }
    Frame {sample_rate: pcm.sample_rate as usize, samples: samples}
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::mpsc::Receiver;

    fn create_decoder(path_str: &'static str) -> Receiver<Result<Frame, MadError>> {
        use std::path::Path;
        use std::fs::File;
        let path = Path::new(path_str);
        let file = File::open(&path).unwrap();
        decode(file)
    }

    #[test]
    fn constant_stereo_128() {
        let decoder = create_decoder("sample_mp3s/constant_stereo_128.mp3");
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder.iter() {
            match item {
                Err(_) => {
                    if frame_count > 0 { error_count += 1; }
                },
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    #[test]
    fn constant_joint_stereo_128() {
        let decoder = create_decoder("sample_mp3s/constant_joint_stereo_128.mp3");
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder.iter() {
            match item {
                Err(_) => {
                    if frame_count > 0 { error_count += 1; }
                },
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 950);
    }

    #[test]
    fn average_stereo_128() {
        let decoder = create_decoder("sample_mp3s/average_stereo_128.mp3");
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder.iter() {
            match item {
                Err(_) => {
                    if frame_count > 0 { error_count += 1; }
                },
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    #[test]
    fn constant_stereo_320() {
        let decoder = create_decoder("sample_mp3s/constant_stereo_320.mp3");
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder.iter() {
            match item {
                Err(_) => {
                    if frame_count > 0 { error_count += 1; }
                },
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    #[test]
    fn variable_joint_stereo() {
        let decoder = create_decoder("sample_mp3s/variable_joint_stereo.mp3");
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder.iter() {
            match item {
                Err(_) => {
                    if frame_count > 0 { error_count += 1 }
                },
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    #[test]
    fn variable_stereo() {
        let decoder = create_decoder("sample_mp3s/variable_stereo.mp3");
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder.iter() {
            match item {
                Err(_) => {
                    if frame_count > 0 { error_count += 1 }
                },
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    #[test]
    fn constant_stereo_16() {
        let decoder = create_decoder("sample_mp3s/constant_stereo_16.mp3");
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder.iter() {
            match item {
                Err(_) => {
                    if frame_count > 0 { error_count += 1; }
                },
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 24000);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 576);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 210);
    }

    #[test]
    fn constant_single_channel_128() {
        let decoder = create_decoder("sample_mp3s/constant_single_channel_128.mp3");
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder.iter() {
            match item {
                Err(_) => {
                    if frame_count > 0 { error_count += 1; }
                },
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 1);
                    assert_eq!(f.samples[0].len(), 1152);
                },
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    #[test]
    fn test_readme_code () {
        let decoder = create_decoder("sample_mp3s/constant_joint_stereo_128.mp3");

        for item in decoder.iter() {
            match item {
                Err(e) => println!("Error: {:?}", e),
                Ok(frame) => {
                  println!("Frame sample rate: {}", frame.sample_rate);
                  println!("First audio sample (left channel): {}", frame.samples[0][0]);
                  println!("First audio sample (right channel): {}", frame.samples[1][0]);
                }
            }
        }
    }
}
