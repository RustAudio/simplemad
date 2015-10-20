[![](https://img.shields.io/crates/v/simplemad.svg)](https://crates.io/crates/simplemad)

[![Build Status](https://travis-ci.org/bendykst/simple-mad.rs.svg?branch=master)](https://travis-ci.org/bendykst/simple-mad.rs)

simplemad is a simple interface for libmad, the MPEG audio decoding library.

# Use and examples

To begin, create a `Decoder` from a byte-oriented source using
`Decoder::decode` or `Decoder::decode_interval`. Fetch results using
`get_frame` or the `Iterator` interface.

MP3 files often begin or end with metadata, which will cause libmad to produce
errors. It is safe to ignore these errors until libmad reaches the start of the
audio data or the end of the file.

```Rust
use simplemad::Decoder;
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
```

Decode the interval from 30 seconds to 60 seconds:

```Rust
let partial_decoder = Decoder::decode_interval(file, 30_000_f64, 60_000_f64).unwrap();
```

# Documentation

http://bendykst.github.io/doc/simplemad/index.html

# License

MIT
