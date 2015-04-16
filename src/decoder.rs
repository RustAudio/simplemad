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
                        error_callback: extern fn(),
                        message_callback: extern fn());
    fn mad_decoder_run(input: &mut mad_decoder, mode: mad_decoder_mode) -> i32;
    fn mad_stream_buffer(stream: isize, buf_start: *const u8, buf_length: usize);
}

#[repr(C)]
enum mad_flow {
    mf_continue = 0x0000,	/* continue normally */
    mf_stop     = 0x0010,	/* stop decoding normally */
    mf_break    = 0x0011,	/* stop decoding and signal an error */
    mf_ignore   = 0x0020	/* ignore the current frame */
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
                             empty_callback,
                             empty_callback);
            mad_decoder_run(&mut decoder, mad_decoder_mode::MAD_DECODER_MODE_SYNC);
        }
    });
    rx
}

