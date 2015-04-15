#![allow(dead_code, unused_imports)]
use std::io::Read;
use std::default::Default;
use std::io::BufReader;
use std::path::Path;
use std::fs::File;
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
    start: &'a *const u8,
    length: usize,
    reader: &'a mut BufReader<std::fs::File>,
    position: usize,
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

#[test]
fn test_open_file() {
    let path = Path::new("test_samples/fs-242.mp3");
    let f = File::open(&path).unwrap();
    let mut reader = BufReader::new(f);
    let mut input_buffer = vec![0u8; 4096];
    reader.read(&mut input_buffer);
    let message = &mut mad_message {
        start: &input_buffer.as_ptr(),
        length: input_buffer.len(),
        reader: &mut reader,
        position: 0,
    };
    let mut decoder: mad_decoder = Default::default();
    let mut decoding_result: i32 = 42;

    extern fn input_callback (msg: &mut mad_message, stream: isize) -> mad_flow {
        let mut input_buffer = vec![0u8; 4096];
        let read_result = msg.reader.read(&mut input_buffer).unwrap();
        unsafe {
            mad_stream_buffer(stream, input_buffer.as_ptr(), input_buffer.len());
        }

        msg.position += read_result;
        if read_result == 0 {
            return mad_flow::mf_stop;
        }

        mad_flow::mf_continue
    }

    extern fn output_callback(msg: &mut mad_message, header: isize, pcm: &mad_pcm) -> mad_flow {
        println!("Position: {}", msg.position);
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
        decoding_result = mad_decoder_run(&mut decoder, mad_decoder_mode::MAD_DECODER_MODE_SYNC);
    }

    assert_eq!(decoding_result, 0);
}
