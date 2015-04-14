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
                        input_callback: extern fn(),
                        header_callback: extern fn(),
                        filter_callback: extern fn(),
                        output_callback: extern fn(),
                        error_callback: extern fn(),
                        message_callback: extern fn());
    fn mad_decoder_run(input: u8);
}

#[derive(Default)]
#[repr(C)]
struct mad_message<'a> {
    start: &'a [u8],
    length: u32
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
}

extern fn empty_callback_fn () {

}

#[test]
fn test_open_file() {
    let path = Path::new("test_samples/fs-242.mp3");
    let f = File::open(&path).unwrap();
    let mut reader = BufReader::new(&f);
    let input_buffer = &mut [0u8; 4096];
    reader.read(input_buffer);

    let message = &mad_message {
        start: input_buffer,
        length: input_buffer.len() as u32,
    };

    let mut decoder: mad_decoder = Default::default();

    unsafe {
        mad_decoder_init(&mut decoder,
                         message,
                         empty_callback_fn,
                         empty_callback_fn,
                         empty_callback_fn,
                         empty_callback_fn,
                         empty_callback_fn,
                         empty_callback_fn);
    }
}
