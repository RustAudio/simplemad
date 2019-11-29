# simplemad [![](https://img.shields.io/crates/v/simplemad.svg)](https://crates.io/crates/simplemad)[![](https://github.com/RustAudio/simplemad/workflows/.github/workflows/ci.yml/badge.svg)](https://github.com/RustAudio/simplemad/actions?query=workflow%3A%22Continuous+Integration%22)

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
let partial_decoder = Decoder::decode_interval(file,
                                               Duration::from_secs(30),
                                               Duration::from_secs(60)).unwrap();
```

You can also elect to only decode the header of each frame. This is useful if
you want to quickly determine the length of a file.

```Rust
let headers = Decoder::decode_headers(file).unwrap();
let duration = headers.filter_map(|r| {
                          match r {
                              Ok(f) => Some(f.duration),
                              Err(_) => None,
                          }
                      }).fold(Duration::new(0, 0), |acc, dtn| acc + dtn);
```

# Documentation

https://docs.rs/simplemad

# License

MIT
