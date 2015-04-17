use std::io;
use std::io::Read;
use std::default::Default;
use std::io::BufReader;
use std::path::Path;
use std::fs::File;
use std::sync::mpsc::{SyncSender, Receiver};
use std::sync::mpsc;
use std::marker::Send;
use std::thread;
use std::io::Cursor;
use self::mad_decoder_mode::* ;

#[link(name = "mad")]
extern {
    fn mad_decoder_init(decoder: &mad_decoder,
                        message: &mad_message,
                        input_callback: extern fn(message: &mut mad_message, stream: isize) -> mad_flow,
                        header_callback: extern fn(),
                        filter_callback: extern fn(),
                        output_callback: extern fn(message: &mut mad_message,
                                                   header: isize,
                                                   pcm: &mad_pcm) -> mad_flow,
                        error_callback: extern fn(message: &mut mad_message,
                                                  stream: &mad_stream,
                                                  frame: isize) -> mad_flow,
                        message_callback: extern fn());
    fn mad_decoder_run(input: &mut mad_decoder, mode: mad_decoder_mode) -> i32;
    fn mad_stream_buffer(stream: isize, buf_start: *const u8, buf_length: usize);
}

#[repr(C)]
enum mad_flow {
    mf_continue = 0x0000,    /* continue normally */
    mf_stop     = 0x0010,    /* stop decoding normally */
    mf_break    = 0x0011,    /* stop decoding and signal an error */
    mf_ignore   = 0x0020    /* ignore the current frame */
}

#[derive(Show)]
#[repr(C)]
enum mad_error {
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

#[repr(C)]
struct mad_bitptr {
    byte: isize,
    cache: u16,
    left: u16,
}

#[repr(C)]
struct mad_stream {
    buffer: isize,
    buff_end: isize,
    skip_len: u32,
    sync: i16,
    free_rate: u32,
    this_frame: isize,
    next_frame: isize,
    ptr: mad_bitptr,
    anc_ptr: mad_bitptr,
    anc_bitlen: u16,
    buffer_mdlen: isize,
    md_len: u16,
    options: i16,
    error: mad_error,
}

#[repr(C)]
struct mad_pcm {
    sample_rate: u32,
    channels: u16,
    length: u16,
    samples: [[i32; 1152]; 2],
}

#[repr(C)]
struct mad_message<'a> {
    buffer: &'a mut [u8; 16384],
    reader: &'a mut (io::Read + 'a),
    sender: &'a SyncSender<Frame>,
}

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
    pid: u32,
    ain: isize,
    aout: isize,
}

#[derive(Default)]
#[repr(C)]
struct mad_decoder {
    mode: mad_decoder_mode,
    options: isize,
    async: mad_async_parameters,
    sync: usize,
    cb_data: usize,
    input_func: usize,
    header_func: usize,
    filter_func: usize,
    output_func: usize,
    error_func: usize,
    message_func: usize,
}

extern fn empty_callback() {

}

pub struct Frame {
    pub sample_rate: u32,
    pub channels: u16,
    pub length: u16,
    pub samples: [[i32; 1152]; 2],
}

pub fn decode<T>(mut reader: T) -> Receiver<Frame>
    where T: io::Read + Send + 'static {
    let (tx, rx) = mpsc::sync_channel::<Frame>(0);
    thread::spawn(move || {
        let mut input_buffer = [0u8; 16384];
        reader.read(&mut input_buffer);
        let message = &mut mad_message {
            buffer: &mut input_buffer,
            reader: &mut reader,
            sender: &tx,
        };
        let mut decoder: mad_decoder = Default::default();
        let mut decoding_result: i32 = 42;

        extern fn input_callback (msg: &mut mad_message, stream: isize) -> mad_flow {
            let read_result = msg.reader.read(msg.buffer).unwrap();
            // println!("Read {}", read_result);
            unsafe {
                mad_stream_buffer(stream, msg.buffer.as_ptr(), read_result);
            }

            if read_result == 0 {
                return mad_flow::mf_stop;
            }

            mad_flow::mf_continue
        }

        extern fn error_callback(msg: &mut mad_message,
                                 stream: &mad_stream,
                                 frame: isize) -> mad_flow
        {
            println!("Error: {:?}", stream.error);
            mad_flow::mf_continue
        }

        extern fn output_callback(msg: &mut mad_message,
                                  header: isize,
                                  pcm: &mad_pcm) -> mad_flow
        {
            msg.sender.send(Frame {sample_rate: pcm.sample_rate,
                                   channels: pcm.channels,
                                   length: pcm.length,
                                   samples: pcm.samples});
            mad_flow::mf_continue
        }

        unsafe {
            mad_decoder_init(&mut decoder,
                             message,
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

