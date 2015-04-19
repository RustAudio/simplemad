use std::thread;
use std::io;
use std::io::Read;
use std::sync::mpsc;
use std::sync::mpsc::{SyncSender, Receiver};
use std::default::Default;
use std::marker::Send;
use self::mad_decoder_mode::* ;
use std::option::Option::{None, Some};
use libc::types::common::c95::c_void;
use libc::types::common::c99::*;
use libc::types::os::arch::c95::*;

#[allow(unused)]
#[repr(C)]
static mut input_buffer: [u8; 16384] = [0; 16384];

#[allow(unused, improper_ctypes)]
#[link(name = "mad")]
extern {
    fn mad_decoder_init(decoder: *mut mad_decoder,
                        message: *mut c_void,
                        input_callback: extern fn(message: *mut MadMessage,
                                                  stream: &mad_stream) -> mad_flow,
                        header_callback: extern fn(),
                        filter_callback: extern fn(),
                        output_callback: extern fn(message: *mut MadMessage,
                                                   header: c_int,
                                                   pcm: &mad_pcm) -> mad_flow,
                        error_callback: extern fn(message: *mut MadMessage,
                                                  stream: &mad_stream,
                                                  frame: c_int) -> mad_flow,
                        message_callback: extern fn());
    #[allow(dead_code)]
    fn mad_decoder_run(input: &mut mad_decoder, mode: mad_decoder_mode) -> c_int;
    fn mad_stream_buffer(stream: &mad_stream, buf_start: *const u8, buf_length: size_t);
}

#[allow(unused)]
#[repr(C)]
enum mad_flow {
    mf_continue = 0x0000,    /* continue normally */
    mf_stop     = 0x0010,    /* stop decoding normally */
    mf_break    = 0x0011,    /* stop decoding and signal an error */
    mf_ignore   = 0x0020    /* ignore the current frame */
}

#[allow(unused)]
#[derive(Debug, Clone)]
#[repr(C)]
pub enum Error {
  mad_error_none           = 0x0000,    /* no error */

  mad_error_buflen         = 0x0001,    /* input buffer too small (or eof) */
  mad_error_bufptr         = 0x0002,    /* invalid (null) buffer pointer */

  mad_error_nomem          = 0x0031,    /* not enough memory */

  mad_error_lostsync       = 0x0101,    /* lost synchronization */
  mad_error_badlayer       = 0x0102,    /* reserved header layer value */
  mad_error_badbitrate     = 0x0103,    /* forbidden bitrate value */
  mad_error_badsamplerate  = 0x0104,    /* reserved sample frequency value */
  mad_error_bademphasis    = 0x0105,    /* reserved emphasis value */

  mad_error_badcrc         = 0x0201,    /* crc check failed */
  mad_error_badbitalloc    = 0x0211,    /* forbidden bit allocation value */
  mad_error_badscalefactor = 0x0221,    /* bad scalefactor index */
  mad_error_badmode        = 0x0222,    /* bad bitrate/mode combination */
  mad_error_badframelen    = 0x0231,    /* bad frame length */
  mad_error_badbigvalues   = 0x0232,    /* bad big_values count */
  mad_error_badblocktype   = 0x0233,    /* reserved block_type */
  mad_error_badscfsi       = 0x0234,    /* bad scalefactor selection info */
  mad_error_baddataptr     = 0x0235,    /* bad main_data_begin pointer */
  mad_error_badpart3len    = 0x0236,    /* bad audio data length */
  mad_error_badhufftable   = 0x0237,    /* bad huffman table select */
  mad_error_badhuffdata    = 0x0238,    /* huffman data overrun */
  mad_error_badstereo      = 0x0239,    /* incompatible block_type for js */
}

#[allow(unused)]
#[repr(C)]
struct mad_bitptr {
    byte: size_t,
    cache: uint16_t,
    left: uint16_t,
}

#[allow(unused)]
#[repr(C)]
struct mad_stream {
    buffer: size_t,
    buff_end: size_t,
    skip_len: c_ulong,
    sync: c_int,
    free_rate: c_ulong,
    this_frame: size_t,
    next_frame: size_t,
    ptr: mad_bitptr,
    anc_ptr: mad_bitptr,
    anc_bitlen: c_uint,
    buffer_mdlen: size_t,
    md_len: c_uint,
    options: c_int,
    error: Error,
}

#[allow(unused)]
#[repr(C)]
struct mad_pcm {
    sample_rate: c_uint,
    channels: uint16_t,
    length: uint16_t,
    samples: [[int32_t; 1152]; 2],
}

#[allow(unused)]
struct MadMessage<'a> {
    buffer: &'static mut [u8; 16384],
    reader: &'a mut (io::Read + 'a),
    sender: &'a SyncSender<Result<Frame, Error>>,
}

#[allow(unused)]
#[repr(C)]
enum mad_decoder_mode {
    MAD_DECODER_MODE_SYNC = 0,
    MAD_DECODER_MODE_ASYNC
}

impl Default for mad_decoder_mode {
    fn default() -> mad_decoder_mode {MAD_DECODER_MODE_SYNC}
}

#[derive(Default)]
#[repr(C)]
struct mad_async_parameters {
    pid: c_long,
    ain: c_int,
    aout: c_int,
}

#[derive(Default)]
#[repr(C)]
struct mad_decoder {
    mode: mad_decoder_mode,
    options: c_int,
    async: mad_async_parameters,
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
    pub samples: [[i32; 1152]; 2],
}

#[allow(unused)]
extern fn empty_callback() {

}

#[allow(unused)]
extern fn input_callback (msg: *mut MadMessage, stream: &mad_stream) -> mad_flow {
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
            bytes_read = Some((*msg).reader.read((*msg).buffer).unwrap());
        } else {
            let slice = &mut (*msg).buffer[unused_byte_count .. buffer_size];
            bytes_read = Some((*msg).reader.read(slice).unwrap());
        }

        match bytes_read {
            None    => return mad_flow::mf_stop,
            Some(0) => return mad_flow::mf_stop,
            Some(n) => mad_stream_buffer(stream, (*msg).buffer.as_ptr(), n as u64),
        }
    }

    mad_flow::mf_continue
}

#[allow(unused)]
extern fn error_callback(msg: *mut MadMessage,
                         stream: &mad_stream,
                         frame: c_int) -> mad_flow
{
    unsafe {
        let error_type = stream.error.clone();
        (*msg).sender.send(Err(error_type));
    }
    mad_flow::mf_continue
}

#[allow(unused)]
extern fn output_callback(msg: *mut MadMessage,
                          header: c_int,
                          pcm: &mad_pcm) -> mad_flow
{
    unsafe {
        let frame = Ok(Frame {sample_rate: pcm.sample_rate,
                              channels: pcm.channels,
                              length: pcm.length,
                              samples: pcm.samples});
        (*msg).sender.send(frame);
    }
    mad_flow::mf_continue
}

#[allow(unused)]
pub fn decode<T>(mut reader: T) -> Receiver<Result<Frame, Error>>
    where T: io::Read + Send + 'static {
    let (tx, rx): (SyncSender<Result<Frame, Error>>, Receiver<Result<Frame, Error>>) = mpsc::sync_channel::<Result<Frame, Error>>(2);
    thread::spawn(move || {
        unsafe {
            let mut message = MadMessage {
                buffer: &mut input_buffer,
                reader: &mut reader,
                sender: &tx,
            };
            let message_ptr = &mut message as *mut _ as *mut c_void;
            let mut decoder: mad_decoder = Default::default();
            mad_decoder_init(&mut decoder,
                             message_ptr,
                             input_callback,
                             empty_callback,
                             empty_callback,
                             output_callback,
                             error_callback,
                             empty_callback);
            mad_decoder_run(&mut decoder, mad_decoder_mode::MAD_DECODER_MODE_SYNC);
        }
    });
    rx
}

#[test]
fn data_sizes() {
    use std::mem::size_of;
    assert_eq!(size_of::<mad_bitptr>(), 16);
    assert_eq!(size_of::<Error>(), 4);
    assert_eq!(size_of::<mad_decoder>(), 88);
    assert_eq!(size_of::<mad_pcm>(), 9224);
    assert_eq!(size_of::<mad_stream>(), 120);
}

#[test]
fn test_short_file() {
    use std::path::Path;
    use std::fs::File;
    let path = Path::new("test_samples/tailtoddle_lo.mp3");
    let f = File::open(&path).unwrap();
    let decoder = decode(f);
    let mut frame_count = 0;
    let mut error_count = 0;
    for frame in decoder.iter() {
        match frame {
            Ok(_) => frame_count += 1,
            Err(_) => error_count += 1,
        }
    }
    assert_eq!(frame_count, 1656);
    assert_eq!(error_count, 1);
}

