/*!
This crate provides an interface to libmad, the MPEG audio decoding library.

`simplemad::decode` takes a byte-oriented source and returns a channel that
yields `Result<Frame, MadError>`. If you only need to decode part of a file,
you can also use `simplemad::decode_interval`.

`Frame` and `MadError` correspond to libmad's struct types `mad_pcm` and
`mad_error`, respectively. Samples are signed 32 bit integers and are organized
into channels. For stereo, the left channel is channel 0.

MP3 files often begin with metadata, which will cause libmad to produce errors.
It is safe to ignore these errors until libmad reaches audio data and starts
producing frames.

# Examples
```no_run
use simplemad::{decode, decode_interval, Frame};
use std::fs::File;
use std::path::Path;

let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
let file = File::open(&path).unwrap();
let file_b = File::open(&path).unwrap();
let mut decoder = decode(file);

for decoding_result in decoder {
    match decoding_result {
        Err(e) => println!("Error: {:?}", e),
        Ok(frame) => {
          println!("Frame sample rate: {}", frame.sample_rate);
          println!("First audio sample (left channel): {}", frame.samples[0][0]);
          println!("First audio sample (right channel): {}", frame.samples[1][0]);
        }
    }
}

// Decode the interval from 1s to 2s (to the nearest frame)
let mut partial_decoder = decode_interval(file_b, 1_000_f64, 2_000_f64);
let frames: Vec<Frame> = partial_decoder.filter_map(|r| match r {
                                            Ok(f) => Some(f),
                                            Err(_) => None})
                                        .collect();
```
*/

#![crate_name = "simplemad"]

extern crate simplemad_sys;
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
use std::cmp::min;
use simplemad_sys::*;

/// A decoded frame
#[derive(Clone, Debug)]
pub struct Frame {
    /// Number of samples per second
    pub sample_rate: u32,
    /// Samples are signed 32 bit integers and are organized into channels.
    /// For stereo, the left channel is channel 0.
    pub samples: Vec<Vec<i32>>,
    /// The position in milliseconds at the start of the frame
    pub position: f64,
}

pub struct Decoder<T> where T: io::Read + Send + 'static {
    reader: T,
    buffer: Box<[u8; 32_768]>,
    stream: MadStream,
    synth: MadSynth,
    frame: MadFrame,
    start_ms: Option<f64>,
    end_ms: Option<f64>,
    position_ms: f64,
    terminated: bool,
}

impl<T> Decoder<T> where T: io::Read + Send + 'static {
    pub fn new(reader: T, start_ms: Option<f64>, end_ms: Option<f64>) -> Decoder<T> {
        let mut new_decoder =
            Decoder {
                reader: reader,
                buffer: Box::new([0u8; 32_768]),
                stream: Default::default(),
                synth: Default::default(),
                frame: Default::default(),
                start_ms: start_ms,
                end_ms: end_ms,
                position_ms: 0.0,
                terminated: false,
            };

        let bytes_read = match new_decoder.reader.read(&mut *new_decoder.buffer) {
            Ok(v) => v,
            Err(e) => panic!("{:?}", e),
        };

        unsafe {
            mad_stream_init(&mut new_decoder.stream);
            mad_frame_init(&mut new_decoder.frame);
            mad_synth_init(&mut new_decoder.synth);
            mad_stream_buffer(&mut new_decoder.stream,
                              new_decoder.buffer.as_ptr(),
                              bytes_read as u64);
        }

        new_decoder
    }

    fn get_frame(&mut self) -> Result<Frame, MadError> {
       match self.decode_frame() {
           Ok(frame) => Ok(frame),
           Err(MadError::BufLen) => {
               // Refill buffer and try again
               if self.refill_buffer() > 0 {
                   self.decode_frame()
               } else {
                   // Out of data
                   Err(MadError::BufLen)
               }
           },
           Err(e) => Err(e)
       }
    }

    fn decode_frame(&mut self) -> Result<Frame, MadError> {
        unsafe {
            mad_header_decode(&mut self.frame.header, &mut self.stream);
        }

        if self.stream.error != MadError::None {
            return Err(self.stream.error.clone());
        }

        unsafe {
            mad_frame_decode(&mut self.frame, &mut self.stream);
        }

        if self.stream.error != MadError::None {
            return Err(self.stream.error.clone());
        }

        unsafe {
            mad_synth_frame(&mut self.synth, &mut self.frame);
        }

        if self.stream.error != MadError::None {
            return Err(self.stream.error.clone());
        }

        let pcm = &self.synth.pcm;
        let mut samples: Vec<Vec<i32>> = Vec::new();

        for channel_idx in 0..pcm.channels as usize {
            let mut channel: Vec<i32> = Vec::with_capacity(pcm.length as usize);
            for sample_idx in 0..pcm.length as usize {
                channel.push(pcm.samples[channel_idx][sample_idx]);
            }
            samples.push(channel);
        }

        let frame =
            Frame {sample_rate: self.synth.pcm.sample_rate as u32,
                   samples: samples,
                   position: self.position_ms -
                             (self.frame.header.duration.seconds as f64) * 1000.0 -
                             (self.frame.header.duration.fraction as f64) / 352800.0};
        Ok(frame)
    }

    pub fn refill_buffer(&mut self) -> usize {
        let buffer_size = self.buffer.len();
        let next_frame_position = (self.stream.next_frame - self.stream.buffer) as usize;
        let unused_byte_count = buffer_size - min(next_frame_position, buffer_size);

        if unused_byte_count == buffer_size {
            unsafe {
                mad_stream_buffer(&mut self.stream, self.buffer.as_ptr(), buffer_size as u64);
            }
            panic!("Full buffer during refill attempt.");
        } else {
            // Shift unused data to front of buffer
            for idx in 0 .. unused_byte_count {
                self.buffer[idx] = self.buffer[idx + next_frame_position];
            }

            // Refill rest of buffer
            let bytes_read = if next_frame_position == 0 {
                match self.reader.read(&mut *self.buffer) {
                    Ok(val) => val,
                    Err(e) => panic!("{:?}", e),
                }
            } else {
                let slice = &mut self.buffer[unused_byte_count..buffer_size];
                match self.reader.read(slice) {
                    Ok(val) => val,
                    Err(e) => panic!("{:?}", e),
                }
            };

            if bytes_read == 0 {
                return 0;
            }

            let fresh_byte_count = (bytes_read + unused_byte_count) as u64;
            unsafe {
                mad_stream_buffer(&mut self.stream, self.buffer.as_ptr(), fresh_byte_count);
            }

            if self.stream.error == MadError::BufLen {
                self.stream.error = MadError::None;
            }

            bytes_read
        }
    }
}

impl<T> Iterator for Decoder<T> where T: io::Read + Send + 'static {
    type Item = Result<Frame, MadError>;
    fn next(&mut self) -> Option<Result<Frame, MadError>> {
        if self.terminated {
            return None;
        }

        match self.get_frame() {
            Ok(f) => Some(Ok(f)),
            Err(MadError::BufLen) => None, // EOF condition
            Err(e) => {
                if !error_is_recoverable(&e) {
                    self.terminated = true;
                }
                Some(Err(e))
            }
        }
    }
}

impl<T> Drop for Decoder<T> where T: io::Read + Send + 'static {
    fn drop(&mut self) {
        unsafe {
            mad_stream_finish(&mut self.stream);
            mad_frame_finish(&mut self.frame);
            // mad_synth_finish is present in the libmad docs
            // but is defined as nothing in the library
            // mad_synth_finish(&mut self.synth);
        }
    }
}

fn error_is_recoverable(err: &MadError) -> bool {
    (err.clone() as u16) & 0xff00 != 0
}

/// Decode a file in full
pub fn decode<T>(reader: T) -> Decoder<T>
    where T: io::Read + Send + 'static {
    Decoder::new(reader, None, None)
}

/// Decode part of a file from `start_time` to `end_time`, measured in milliseconds
pub fn decode_interval<T>(reader: T, start_time: f64, end_time: f64)
    -> Decoder<T> where T: io::Read + Send + 'static {
    Decoder::new(reader, Some(start_time), Some(end_time))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::path::Path;
    use simplemad_sys::*;
    use std::sync::mpsc::Receiver;

    #[test]
    fn constant_stereo_128() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let decoder = decode(file);
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
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
        let path = Path::new("sample_mp3s/constant_joint_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let decoder = decode(file);
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
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
        let path = Path::new("sample_mp3s/average_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let decoder = decode(file);
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
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
        let path = Path::new("sample_mp3s/constant_stereo_320.mp3");
        let file = File::open(&path).unwrap();
        let decoder = decode(file);
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
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
        let path = Path::new("sample_mp3s/variable_joint_stereo.mp3");
        let file = File::open(&path).unwrap();
        let decoder = decode(file);
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
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
        let path = Path::new("sample_mp3s/variable_stereo.mp3");
        let file = File::open(&path).unwrap();
        let decoder = decode(file);
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
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
        let path = Path::new("sample_mp3s/constant_stereo_16.mp3");
        let file = File::open(&path).unwrap();
        let decoder = decode(file);
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
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
        let path = Path::new("sample_mp3s/constant_single_channel_128.mp3");
        let file = File::open(&path).unwrap();
        let decoder = decode(file);
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
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
}
