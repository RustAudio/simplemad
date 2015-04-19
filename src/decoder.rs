use std::thread;
use std::io;
use std::io::Read;
use std::sync::mpsc;
use std::sync::mpsc::{SyncSender, Receiver, RecvError};
use std::default::Default;
use std::marker::Send;
use std::option::Option::{None, Some};
use libc::types::common::c95::c_void;
use libc::types::common::c99::*;
use libc::types::os::arch::c95::*;

enum Error {
    Mad(MadError),
    Recv(RecvError),
}

struct Decoder {
    rx: Box<Receiver<Result<Frame, MadError>>>,
}

impl Decoder {
    #[allow(dead_code)]
    fn new(reader: Box<io::Read + Send + 'static>) -> Decoder {
        Decoder {
            rx: Box::new(decode(reader)),
        }
    }

    pub fn get_frame(&self) -> Result<Frame, Error> {
        match self.rx.recv() {
            Err(e) => Err(Error::Recv(e)),
            Ok(Err(e)) => Err(Error::Mad(e)),
            Ok(Ok(frame)) => Ok(frame),
        }
    }
}

impl Iterator for Decoder {
    type Item = Result<Frame, MadError>;
    fn next(&mut self) -> Option<Result<Frame, MadError>> {
        match self.get_frame() {
            Err(Error::Recv(_)) => None,
            Err(Error::Mad(e)) => Some(Err(e)),
            Ok(frame) => Some(Ok(frame)),
        }
    }
}

#[allow(unused, improper_ctypes)]
#[link(name = "mad")]
extern {
    fn mad_decoder_init(decoder: *mut MadDecoder,
                        message: *mut c_void,
                        input_cb: extern fn(message: *mut MadMessage,
                                                  stream: &MadStream) -> MadFlow,
                        header_cb: extern fn(),
                        filter_cb: extern fn(),
                        output_cb: extern fn(message: *mut MadMessage,
                                                   header: c_int,
                                                   pcm: &MadPcm) -> MadFlow,
                        error_cb: extern fn(message: *mut MadMessage,
                                                  stream: &MadStream,
                                                  frame: c_int) -> MadFlow,
                        message_cb: extern fn());
    #[allow(dead_code)]
    fn mad_decoder_run(input: &mut MadDecoder, mode: MadDecoderMode) -> c_int;
    fn mad_stream_buffer(stream: &MadStream, buf_start: *const u8, buf_length: size_t);
}

#[allow(unused)]
#[repr(C)]
enum MadFlow {
    Continue = 0x0000,    /* continue normally */
    Stop     = 0x0010,    /* stop decoding normally */
    Break    = 0x0011,    /* stop decoding and signal an error */
    Ignore   = 0x0020    /* ignore the current frame */
}

#[allow(unused)]
#[derive(Debug, Clone)]
#[repr(C)]
pub enum MadError {
  None           = 0x0000,    /* no error */

  BufLen         = 0x0001,    /* input buffer too small (or eof) */
  BufPtr         = 0x0002,    /* invalid (null) buffer pointer */

  NoMem          = 0x0031,    /* not enough memory */

  LostSync       = 0x0101,    /* lost synchronization */
  BadLayer       = 0x0102,    /* reserved header layer value */
  BadBitRate     = 0x0103,    /* forbidden bitrate value */
  BadSampleRate  = 0x0104,    /* reserved sample frequency value */
  BadEmphasis    = 0x0105,    /* reserved emphasis value */

  BadCRC         = 0x0201,    /* crc check failed */
  BadBitAlloc    = 0x0211,    /* forbidden bit allocation value */
  BadScaleFactor = 0x0221,    /* bad scalefactor index */
  BadMode        = 0x0222,    /* bad bitrate/mode combination */
  BadFrameLen    = 0x0231,    /* bad frame length */
  BadBigValues   = 0x0232,    /* bad big_values count */
  BadBlockType   = 0x0233,    /* reserved block_type */
  BadScFSI       = 0x0234,    /* bad scalefactor selection info */
  BadDataPtr     = 0x0235,    /* bad main_data_begin pointer */
  BadPart3Len    = 0x0236,    /* bad audio data length */
  BadHuffTable   = 0x0237,    /* bad huffman table select */
  BadHuffData    = 0x0238,    /* huffman data overrun */
  BadStereo      = 0x0239,    /* incompatible block_type for js */
}

#[allow(unused)]
#[repr(C)]
struct MadBitPtr {
    byte: size_t,
    cache: uint16_t,
    left: uint16_t,
}

#[allow(unused)]
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

#[allow(unused)]
#[repr(C)]
struct MadPcm {
    sample_rate: c_uint,
    channels: uint16_t,
    length: uint16_t,
    samples: [[int32_t; 1152]; 2],
}

#[allow(unused)]
struct MadMessage<'a> {
    buffer: Box<[u8; 16384]>,
    reader: &'a mut (io::Read + 'a),
    sender: &'a SyncSender<Result<Frame, MadError>>,
}

#[allow(unused)]
#[repr(C)]
enum MadDecoderMode {
    Sync = 0,
    Async
}

impl Default for MadDecoderMode {
    fn default() -> MadDecoderMode {MadDecoderMode::Sync}
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

#[allow(unused)]
pub struct Frame {
    pub sample_rate: u32,
    pub channels: uint16_t,
    pub length: uint16_t,
    pub samples: Vec<Vec<i32>>,
}

#[allow(unused)]
extern fn empty_cb() {

}

#[allow(unused)]
extern fn input_cb (msg: *mut MadMessage, stream: &MadStream) -> MadFlow {
    unsafe {
        let buffer_size = (*msg).buffer.len();
        let next_frame_position = (stream.next_frame - stream.buffer) as usize;
        let unused_byte_count = buffer_size - next_frame_position;

        if unused_byte_count > 0 {
            for idx in 0 .. unused_byte_count {
                (*msg).buffer[idx] = (*msg).buffer[idx + next_frame_position];
            }
        }

        let mut bytes_read = None;

        if next_frame_position == 0 {
            bytes_read = Some((*msg).reader.read(&mut *(*msg).buffer).unwrap());
        } else {
            let slice = &mut (*msg).buffer[unused_byte_count .. buffer_size];
            bytes_read = Some((*msg).reader.read(slice).unwrap());
        }

        match bytes_read {
            None    => return MadFlow::Stop,
            Some(0) => return MadFlow::Stop,
            Some(n) => mad_stream_buffer(stream, (*msg).buffer.as_ptr(), n as u64),
        }
    }

    MadFlow::Continue
}

#[allow(unused)]
extern fn error_cb(msg: *mut MadMessage,
                         stream: &MadStream,
                         frame: c_int) -> MadFlow
{
    unsafe {
        let error_type = stream.error.clone();
        (*msg).sender.send(Err(error_type));
    }
    MadFlow::Continue
}

#[allow(unused)]
extern fn output_cb(msg: *mut MadMessage, header: c_int, pcm: &MadPcm) -> MadFlow {
    let mut samples: Vec<Vec<i32>> = Vec::new();
    for channel_idx in 0..pcm.channels as usize {
        let mut channel: Vec<i32> = Vec::new();
        for sample_idx in 0 .. pcm.length as usize {
            channel.push(pcm.samples[channel_idx][sample_idx]);
        }
        samples.push(channel);
    }
    let frame = Ok(Frame {sample_rate: pcm.sample_rate,
                          channels: pcm.channels,
                          length: pcm.length,
                          samples: samples});
    unsafe {
        (*msg).sender.send(frame);
    }
    MadFlow::Continue
}

fn decode<T>(mut reader: T) -> Receiver<Result<Frame, MadError>>
    where T: io::Read + Send + 'static {
    let input_buffer = Box::new([0u8; 16384]);
    let (tx, rx) = mpsc::sync_channel::<Result<Frame, MadError>>(2);
    thread::spawn(move || {
        unsafe {
            let mut message = MadMessage {
                buffer: input_buffer,
                reader: &mut reader,
                sender: &tx,
            };
            let message_ptr = &mut message as *mut _ as *mut c_void;
            let mut decoder: MadDecoder = Default::default();
            mad_decoder_init(&mut decoder,
                             message_ptr,
                             input_cb,
                             empty_cb,
                             empty_cb,
                             output_cb,
                             error_cb,
                             empty_cb);
            mad_decoder_run(&mut decoder, MadDecoderMode::Sync);
        }
    });
    rx
}

#[test]
fn data_sizes() {
    use std::mem::size_of;
    assert_eq!(size_of::<MadError>(), 4);
    assert_eq!(size_of::<MadBitPtr>(), 16);
    assert_eq!(size_of::<MadDecoder>(), 88);
    assert_eq!(size_of::<MadPcm>(), 9224);
    assert_eq!(size_of::<MadStream>(), 120);
}

#[test]
fn test_short_file() {
    use std::path::Path;
    use std::fs::File;
    let path = Path::new("test_samples/tailtoddle_lo.mp3");
    let f = File::open(&path).unwrap();
    let decoder = Decoder::new(Box::new(f));
    let mut frame_count = 0;
    let mut error_count = 0;

    for item in decoder {
        match item {
            Err(_) => error_count += 1,
            Ok(f) => {
                frame_count += 1;
                assert_eq!(f.sample_rate, 11025);
                assert_eq!(f.channels, 2);
                assert_eq!(f.length, 576);
            }
        }
    }
    assert_eq!(frame_count, 1656);
    assert_eq!(error_count, 1);
}

