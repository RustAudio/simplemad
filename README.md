[![](https://img.shields.io/crates/v/simplemad.svg)](https://crates.io/crates/simplemad)

[![Build Status](https://travis-ci.org/bendykst/simple-mad.rs.svg?branch=master)](https://travis-ci.org/bendykst/simple-mad.rs)

simplemad is a simple interface for libmad, the MPEG audio decoding library.

# Use and examples

`simplemad::decode` takes a byte-oriented source and returns a channel that
yields `Result<Frame, MadError>`. If you only need to decode part of a file,
you can also use `simplemad::decode_interval`.

`Frame` and `MadError` correspond to libmad's struct types `mad_pcm` and
`mad_error`, respectively. Samples are signed 32 bit integers and are organized
into channels. For stereo, the left channel is channel 0.

MP3 files often begin with metadata, which will cause libmad to produce errors.
It is safe to ignore these errors until libmad reaches audio data and starts
producing frames.

```Rust
use simplemad::decode;
use std::fs::File;
use std::path::Path;

let path = Path::new("sample_mp3s/constant_stereo_128.mp3");
let file = File::open(&path).unwrap();
let mut decoder = decode(file);

for decoding_result in decoder.iter() {
    match decoding_result {
        Err(e) => println!("Error: {:?}", e),
        Ok(frame) => {
          println!("Frame sample rate: {}", frame.sample_rate);
          println!("First audio sample (left channel): {}", frame.samples[0][0]);
          println!("First audio sample (right channel): {}", frame.samples[1][0]);
        }
    }
}
```

To decode from 30 seconds to 60 seconds, use `decode_interval` and provide the start and end time in milliseconds.

```Rust
let mut partial_decoder = decode_interval(file, 30_000_f32, 60_000_f32);
```

# Installation

First, install libmad. Links to the source can be found below. It might be necessary to apply the patch found in [this guide](http://www.linuxfromscratch.org/blfs/view/svn/multimedia/libmad.html). Then add `simplemad = "0.3.0"` to the list of dependencies in your Cargo.toml.

# Documentation

http://bendykst.github.io/doc/simplemad/index.html

# Dependencies

 * libmad (FTP: ftp://ftp.mars.org/pub/mpeg/ or [SourceForge](http://sourceforge.net/project/showfiles.php?group_id=12349))

 * libc

# License

MIT
