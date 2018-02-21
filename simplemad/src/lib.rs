/*!
 This crate provides an interface to libmad, the MPEG audio decoding library.

 To begin, create a `Decoder` from a byte-oriented source using `Decoder::decode`
 or `Decoder::decode_interval`. Fetch results using `get_frame` or the `Iterator`
 interface. MP3 files often begin or end with metadata, which will cause libmad
 to produce errors. It is safe to ignore these errors until libmad reaches the
 start of the audio data or the end of the file.

 # Examples
 ```no_run
 #![allow(unused_variables)]
 use simplemad::{Decoder, Frame};
 use std::time::Duration;
 use std::fs::File;
 use std::path::Path;

 let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
 let file = File::open(&path).unwrap();
 let decoder = Decoder::decode(file).unwrap();

 for decoding_result in decoder {
     match decoding_result {
         Err(e) => println!("Error: {:?}", e),
         Ok(frame) => {
             println!("Frame sample rate: {}", frame.sample_rate);
             println!("First audio sample (left channel): {:?}", frame.samples[0][0]);
             println!("First audio sample (right channel): {:?}", frame.samples[1][0]);
         },
     }
 }

 // Decode the interval from 1s to 2s (to the nearest frame),
 let file_b = File::open(&path).unwrap();
 let partial_decoder = Decoder::decode_interval(file_b,
                                                Duration::from_secs(1),
                                                Duration::from_secs(2));
 let frames: Vec<Frame> = partial_decoder.unwrap()
                                         .filter_map(|r| match r {
                                             Ok(f) => Some(f),
                                             Err(_) => None})
                                         .collect();

 // Decode only the headers to quickly calculate the file's length
 let file_c = File::open(&path).unwrap();
 let headers = Decoder::decode_headers(file_c).unwrap();
 let duration = headers.filter_map(|r| {
                           match r {
                               Ok(f) => Some(f.duration),
                               Err(_) => None,
                           }
                       }).fold(Duration::new(0, 0), |acc, dtn| acc + dtn);
*/

#![crate_name = "simplemad"]
#![deny(missing_docs,
        trivial_casts,
        unstable_features,
        unused_import_braces)]

extern crate simplemad_sys;
use std::io::{self};
use std::default::Default;
use std::cmp::{min, max};
use std::time::Duration;
use simplemad_sys::*;

/// A decoded frame
#[derive(Clone, Debug)]
pub struct Frame {
    /// Number of samples per second
    pub sample_rate: u32,
    /// Stream bit rate
    pub bit_rate: u32,
    /// Audio layer (I, II or III)
    pub layer: MadLayer,
    /// Single Channel, Dual Channel, Joint Stereo or Stereo
    pub mode: MadMode,
    /// Samples are organized into a vector of channels. For
    /// stereo, the left channel is channel 0.
    pub samples: Vec<Vec<MadFixed32>>,
    /// The duration of the frame
    pub duration: Duration,
    /// The position at the start of the frame
    pub position: Duration,
}

/// An interface for the decoding operation
///
/// Create a decoder using `decode` or `decode_interval`. Fetch
/// results with `get_frame` or the `Iterator` interface.
pub struct Decoder<R>
    where R: io::Read
{
    reader: R,
    buffer: Box<[u8; 0x8000]>,
    stream: MadStream,
    synth: MadSynth,
    frame: MadFrame,
    position: Duration,
    headers_only: bool,
    start_time: Option<Duration>,
    end_time: Option<Duration>,
}

impl<R> Decoder<R> where R: io::Read {
    fn new(reader: R,
           start_time: Option<Duration>,
           end_time: Option<Duration>,
           headers_only: bool)
           -> Result<Decoder<R>, SimplemadError> {
        let mut new_decoder = Decoder {
            reader: reader,
            buffer: Box::new([0u8; 0x8000]),
            stream: Default::default(),
            synth: Default::default(),
            frame: Default::default(),
            position: Duration::new(0, 0),
            headers_only: headers_only,
            start_time: start_time,
            end_time: end_time,
        };

        let bytes_read = try!(new_decoder.reader.read(&mut *new_decoder.buffer));

        unsafe {
            mad_stream_init(&mut new_decoder.stream);
            mad_frame_init(&mut new_decoder.frame);
            mad_synth_init(&mut new_decoder.synth);
            mad_stream_buffer(&new_decoder.stream,
                              new_decoder.buffer.as_ptr(),
                              bytes_read as c_ulong);
        }

        Ok(new_decoder)
    }

    /// Decode a file in full
    pub fn decode(reader: R) -> Result<Decoder<R>, SimplemadError> {
        Decoder::new(reader, None, None, false)
    }

    /// Decode only the header information of each frame
    pub fn decode_headers(reader: R) -> Result<Decoder<R>, SimplemadError> {
        Decoder::new(reader, None, None, true)
    }

    /// Decode part of a file from `start_time` to `end_time`
    pub fn decode_interval(reader: R,
                           start_time: Duration,
                           end_time: Duration)
                           -> Result<Decoder<R>, SimplemadError> {
        Decoder::new(reader, Some(start_time), Some(end_time), false)
    }

    /// Get the next decoding result, either a `Frame` or a `SimplemadError`
    pub fn get_frame(&mut self) -> Result<Frame, SimplemadError> {
        if let Some(t) = self.start_time {
            if self.position < t {
                return self.seek_to_start();
            }
        }

        if let Some(t) = self.end_time {
            if self.position >= t {
                return Err(SimplemadError::EOF);
            }
        }

        let decoding_result = if self.headers_only {
            self.decode_header_only()
        } else {
            self.decode_frame()
        };

        match decoding_result {
            Ok(frame) => {
                self.position += frame_duration(&self.frame);
                Ok(frame)
            }
            Err(SimplemadError::Mad(MadError::BufLen)) => {
                // Refill buffer and try again
                if try!(self.refill_buffer()) == 0 {
                    Err(SimplemadError::EOF)
                } else {
                    self.get_frame()
                }
            }
            Err(e) => Err(e),
        }
    }

    fn seek_to_start(&mut self) -> Result<Frame, SimplemadError> {
        if let Some(start_time) = self.start_time {
            while self.position < start_time {
                match self.decode_header_only() {
                    Ok(frame) => {
                        self.position += frame.duration;
                    }
                    Err(SimplemadError::Mad(MadError::BufLen)) => {
                        if try!(self.refill_buffer()) == 0 {
                            return Err(SimplemadError::EOF);
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        self.get_frame()
    }

    fn decode_header_only(&mut self) -> Result<Frame, SimplemadError> {
        unsafe {
            mad_header_decode(&mut self.frame.header, &mut self.stream);
        }

        if let Some(error) = self.check_error() {
            return Err(SimplemadError::Mad(error));
        }

        Ok(Frame {
            sample_rate: self.frame.header.sample_rate,
            mode: self.frame.header.mode,
            layer: self.frame.header.layer,
            bit_rate: self.frame.header.bit_rate as u32,
            samples: Vec::new(),
            duration: frame_duration(&self.frame),
            position: self.position,
        })
    }

    fn decode_frame(&mut self) -> Result<Frame, SimplemadError> {
        unsafe {
            mad_frame_decode(&mut self.frame, &mut self.stream);
        }

        if let Some(error) = self.check_error() {
            return Err(SimplemadError::Mad(error));
        }

        unsafe {
            mad_synth_frame(&mut self.synth, &mut self.frame);
        }

        if let Some(error) = self.check_error() {
            return Err(SimplemadError::Mad(error));
        }

        let pcm = &self.synth.pcm;
        let samples = pcm.samples
                         .into_iter()
                         .take(pcm.channels as usize)
                         .map(|ch| {
                             ch.into_iter()
                               .take(pcm.length as usize)
                               .map(|sample| MadFixed32::new(*sample))
                               .collect()
                         })
                         .collect();

        Ok(Frame {
            sample_rate: pcm.sample_rate,
            duration: frame_duration(&self.frame),
            mode: self.frame.header.mode,
            layer: self.frame.header.layer,
            bit_rate: self.frame.header.bit_rate as u32,
            position: self.position,
            samples: samples,
        })
    }

    fn refill_buffer(&mut self) -> Result<usize, io::Error> {
        let buffer_len = self.buffer.len();
        let next_frame_position = self.stream.next_frame as usize - self.stream.buffer as usize;
        let unused_byte_count = buffer_len - min(next_frame_position, buffer_len);

        // Shift unused data to front of buffer
        for idx in 0..unused_byte_count {
            self.buffer[idx] = self.buffer[idx + next_frame_position];
        }

        // Refill rest of buffer
        let mut free_region_start = unused_byte_count;
        while free_region_start != buffer_len {
            let slice = &mut self.buffer[free_region_start..buffer_len];
            match try!(self.reader.read(slice)) {
                0 => break,
                n => free_region_start += n,
            }
        }

        unsafe {
            mad_stream_buffer(&self.stream,
                              self.buffer.as_ptr(),
                              free_region_start as c_ulong);
        }

        let bytes_read = free_region_start - unused_byte_count;
        Ok(bytes_read)
    }

    fn check_error(&mut self) -> Option<MadError> {
        if self.stream.error != MadError::None {
            let error = self.stream.error;
            self.stream.error = MadError::None;
            Some(error)
        } else {
            None
        }
    }
}

impl<R> Iterator for Decoder<R> where R: io::Read {
    type Item = Result<Frame, SimplemadError>;
    fn next(&mut self) -> Option<Result<Frame, SimplemadError>> {
        match self.get_frame() {
            Ok(f) => Some(Ok(f)),
            Err(SimplemadError::EOF) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

impl<R> Drop for Decoder<R> where R: io::Read {
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

#[derive(Debug)]
/// An error encountered during the decoding process
pub enum SimplemadError {
    /// An `io::Error` generated by the `Reader`
    Read(io::Error),
    /// A `MadError` generated by libmad
    Mad(MadError),
    /// The `Reader` has stopped producing data
    EOF,
}

impl From<MadError> for SimplemadError {
    fn from(err: MadError) -> SimplemadError {
        SimplemadError::Mad(err)
    }
}

impl From<io::Error> for SimplemadError {
    fn from(err: io::Error) -> SimplemadError {
        SimplemadError::Read(err)
    }
}

fn frame_duration(frame: &MadFrame) -> Duration {
    let duration = &frame.header.duration;
    Duration::new(duration.seconds as u64,
                  ((duration.fraction as u64) * 1_000_000_000 / 352_800_000) as u32)
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
/// libmad's native fixed-point sample format
///
/// A 32-bit value comprised of a sign bit,
/// three whole number bits and 28 fractional
/// bits.
pub struct MadFixed32 {
    value: i32,
}

impl MadFixed32 {
    /// Construct a new MadFixed32 from a value in libmad's fixed-point format
    pub fn new(v: i32) -> MadFixed32 {
        MadFixed32 { value: v }
    }

    /// Get the raw fixed-point representation
    pub fn to_raw(&self) -> i32 {
        self.value
    }

    /// Convert to i16
    pub fn to_i16(&self) -> i16 {
        let frac_bits = 28;
        let unity_value = 0x1000_0000;
        let rounded_value = self.value + (1 << (frac_bits - 16));
        let clipped_value = max(-unity_value, min(rounded_value, unity_value - 1));
        let quantized_value = clipped_value >> (frac_bits + 1 - 16);

        quantized_value as i16
    }

    /// Convert to i32
    pub fn to_i32(&self) -> i32 {
        // clip only
        if self.value > i32::max_value() / 8 {
            i32::max_value()
        } else if self.value < i32::min_value() / 8 {
            i32::min_value()
        } else {
            self.value * 8
        }
    }

    /// Convert to f32
    pub fn to_f32(&self) -> f32 {
        // The big number is 2^28, as 28 is the fractional bit count)
        f32::max(-1.0, f32::min(1.0, (self.value as f32) / 268435456.0))
    }

    /// Convert to f64
    pub fn to_f64(&self) -> f64 {
        // The big number is 2^28, as 28 is the fractional bit count)
        f64::max(-1.0, f64::min(1.0, (f64::from(self.value)) / 268435456.0))
    }
}

impl From<i16> for MadFixed32 {
    fn from(v: i16) -> MadFixed32 {
        MadFixed32 { value: i32::from(v) * 0x2000 }
    }
}

impl From<i32> for MadFixed32 {
    fn from(v: i32) -> MadFixed32 {
        MadFixed32 { value: v / 8 }
    }
}

impl From<f32> for MadFixed32 {
    fn from(v: f32) -> MadFixed32 {
        MadFixed32 {
            // The big number is 2^28, as
            // 28 is the fractional bit count)
            value: (v * 268435456.0) as i32,
        }
    }
}

impl From<f64> for MadFixed32 {
    fn from(v: f64) -> MadFixed32 {
        MadFixed32 {
            // The big number is 2^28, as
            // 28 is the fractional bit count)
            value: (v * 268435456.0) as i32,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use simplemad_sys::*;
    use std::io::BufReader;
    use std::fs::File;
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn test_find_duration() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let bufreader = BufReader::new(file);
        let decoder = Decoder::decode_headers(bufreader).unwrap();

        let duration = decoder.filter_map(|r| {
                                  match r {
                                      Ok(f) => Some(f.duration),
                                      Err(_) => None,
                                  }
                              })
                              .fold(Duration::new(0, 0), |acc, dtn| acc + dtn);

        assert_eq!(duration, Duration::new(5, 41632464));
    }

    #[test]
    fn test_decode_headers() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let bufreader = BufReader::new(file);
        let decoder = Decoder::decode_headers(bufreader).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.mode, MadMode::Stereo);
                    assert_eq!(f.layer, MadLayer::LayerIII);
                    assert_eq!(f.bit_rate, 128000);
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 0);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    #[test]
    fn test_bufreader() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let bufreader = BufReader::new(file);
        let decoder = Decoder::decode(bufreader).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.mode, MadMode::Stereo);
                    assert_eq!(f.layer, MadLayer::LayerIII);
                    assert_eq!(f.bit_rate, 128000);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    #[test]
    fn test_decode_interval() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let decoder = Decoder::decode_interval(file,
                                               Duration::from_secs(3),
                                               Duration::from_secs(4)).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 39);
    }

    #[test]
    fn test_interval_beyond_eof() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let mut decoder = Decoder::decode_interval(file,
                                                   Duration::from_secs(60),
                                                   Duration::from_secs(65)).unwrap();

        assert!(decoder.next().is_none());
    }

    #[test]
    fn test_decode_empty_interval() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let decoder = Decoder::decode_interval(file,
                                               Duration::from_secs(2),
                                               Duration::from_secs(2)).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 0);
    }

    #[test]
    fn test_decode_overlong_interval() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let decoder = Decoder::decode_interval(file,
                                               Duration::from_secs(3),
                                               Duration::from_secs(45)).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.samples.len(), 2);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 77);
    }

    #[test]
    fn constant_stereo_128() {
        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.mode, MadMode::Stereo);
                    assert_eq!(f.layer, MadLayer::LayerIII);
                    assert_eq!(f.bit_rate, 128000);
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
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.mode, MadMode::JointStereo);
                    assert_eq!(f.layer, MadLayer::LayerIII);
                    assert_eq!(f.bit_rate, 128000);
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
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.mode, MadMode::Stereo);
                    assert_eq!(f.layer, MadLayer::LayerIII);
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
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.mode, MadMode::Stereo);
                    assert_eq!(f.layer, MadLayer::LayerIII);
                    assert_eq!(f.bit_rate, 320000);
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
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.mode, MadMode::JointStereo);
                    assert_eq!(f.layer, MadLayer::LayerIII);
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
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1
                    }
                }
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
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 24000);
                    assert_eq!(f.mode, MadMode::Stereo);
                    assert_eq!(f.layer, MadLayer::LayerIII);
                    assert_eq!(f.bit_rate, 16000);
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
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.mode, MadMode::SingleChannel);
                    assert_eq!(f.layer, MadLayer::LayerIII);
                    assert_eq!(f.bit_rate, 128000);
                    assert_eq!(f.samples.len(), 1);
                    assert_eq!(f.samples[0].len(), 1152);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 193);
    }

    fn gather_min_max<'a, I: IntoIterator<Item=&'a MadFixed32>>(iter: I, min_max: &mut (i32, i32)) {
        // to_raw() is used, because other conversions do clipping to [-1, 1]
        for s in iter.into_iter().map(|v| v.to_raw()) {
            if min_max.0 > s {
                min_max.0 = s;
            } else if min_max.1 < s {
                min_max.1 = s;
            }
        }
    }

    fn assert_min_max(min_max: (i32, i32), expected: (f64, f64)) {
        // A MadFixed32 constant equal to 1
        const ONE: i32 = 0x10000000;

        let (min_sample, max_sample) = min_max;
        let min_sample = min_sample as f64 / ONE as f64;
        let max_sample = max_sample as f64 / ONE as f64;

        assert!((min_sample - expected.0).abs() < 0.05, "Min sample not close to {}: {}", expected.0, min_sample);
        assert!((max_sample - expected.1).abs() < 0.05, "Max sample not close to {}: {}", expected.1, max_sample);
    }

    #[test]
    fn constant_single_channel_320_11025hz_tone() {
        let path = Path::new("sample_mp3s/constant_single_channel_320_11025hz_tone.mp3");
        let file = File::open(&path).unwrap();
        let decoder = Decoder::decode(file).unwrap();
        let mut frame_count = 0;
        let mut error_count = 0;
        let mut min_max_samples = (0, 0);

        for item in decoder {
            match item {
                Err(_) => {
                    if frame_count > 0 {
                        error_count += 1;
                    }
                }
                Ok(f) => {
                    frame_count += 1;
                    assert_eq!(f.sample_rate, 44100);
                    assert_eq!(f.mode, MadMode::SingleChannel);
                    assert_eq!(f.layer, MadLayer::LayerIII);
                    assert_eq!(f.bit_rate, 320000);
                    assert_eq!(f.samples.len(), 1);
                    assert_eq!(f.samples[0].len(), 1152);

                    gather_min_max(f.samples[0].iter(), &mut min_max_samples);
                }
            }
        }
        assert_eq!(error_count, 0);
        assert_eq!(frame_count, 40);

        // Check that the raw output signal is approximately within [-1, 1] interval
        assert_min_max(min_max_samples, (-1.0, 1.0));
    }

    #[allow(unused_variables)]
    #[test]
    fn test_readme_md() {
        use std::fs::File;
        use std::path::Path;

        let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
        let file = File::open(&path).unwrap();
        let file2 = File::open(&path).unwrap();
        let decoder = Decoder::decode(file).unwrap();

        for decoding_result in decoder {
            match decoding_result {
                Err(e) => println!("Error: {:?}", e),
                Ok(frame) => {
                    println!("Frame sample rate: {}", frame.sample_rate);
                    println!("First audio sample (left channel): {:?}",
                             frame.samples[0][0]);
                    println!("First audio sample (right channel): {:?}",
                             frame.samples[1][0]);
                }
            }
        }
        let partial_decoder = Decoder::decode_interval(file2,
                                                       Duration::from_secs(30),
                                                       Duration::from_secs(60)).unwrap();
    }
}
