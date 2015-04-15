#![allow(dead_code, unused_imports)]
extern crate libc;

use std::io::Read;
use std::default::Default;
use std::io::BufReader;
use std::path::Path;
use std::fs::File;
use self::mad_decoder_mode::* ;
use libc::{c_int, size_t};

#[link(name = "mad")]
extern {
    fn mad_decoder_init(decoder: &mad_decoder,
                        message: &mad_message,
                        input_callback: extern fn(message: &mut mad_message, stream: isize),
                        header_callback: extern fn(),
                        filter_callback: extern fn(),
                        output_callback: extern fn(message: &mut mad_message,
                                                   header: isize,
                                                   pcm: &mad_pcm),
                        error_callback: extern fn(),
                        message_callback: extern fn());
    fn mad_decoder_run(input: &mut mad_decoder, mode: mad_decoder_mode) -> i32;
    fn mad_stream_buffer(stream: isize, buf_start: &[u8], buf_length: usize);
}

#[repr(C)]
struct mad_pcm {
    sample_rate: usize,
    channels: u16,
    length: u16,
    samples: [[u32; 1152]; 2],
}

#[repr(C)]
struct mad_message<'a> {
    start: &'a mut [u8; 4096],
    length: u32,
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
    let mut input_buffer = [0u8; 4096];
    let mut reader = BufReader::new(&f);

    let message = &mut mad_message {
        start: &mut input_buffer,
        length: 4096u32,
    };

    let mut decoder: mad_decoder = Default::default();
    let mut decoding_result: i32 = 42;

    extern fn input_callback (msg: &mut mad_message, stream: isize) {
        panic!("In INPUT!");
    }

    extern fn output_callback(message: &mut mad_message, header: isize, pcm: &mad_pcm) {
        panic!("In OUTPUT!");
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
