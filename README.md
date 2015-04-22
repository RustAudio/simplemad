[![](http://meritbadge.herokuapp.com/simplemad)](https://crates.io/crates/simplemad)
[![Build Status](https://travis-ci.org/bendykst/simple-mad.rs.svg)](https://travis-ci.org/bendykst/simple-mad.rs)

simplemad is a simple interface for libmad, the MPEG audio decoding library. simplemad is useful for those who need to decode MP3 or other MPEG audio files in full.

# Installation

First, install libmad. Links to the source can be found below. It might be necessary to apply the patch found in [this guide](http://www.linuxfromscratch.org/blfs/view/svn/multimedia/libmad.html). Then add `simplemad = "0.1.0"` to the list of dependencies in your Cargo.toml.

# Use

Create a `Decoder` from a boxed `Reader`.

```Rust
let path = Path::new("some.mp3");
let f = File::open(&path).unwrap();
let decoder = Decoder::new(Box::new(f));
```

`Decoder` implements `Iterator` and yields a sequence of `Result<Frame, MadError>`.

```Rust
for item in decoder {
    match item {
        Err(e) => {
          println!("Error: {:?}", e);
        },
        Ok(frame) => {
          println!("Frame sample rate: {}", frame.sample_rate);
          println!("First audio sample (left channel): {}", frame.samples[0][0]);
          println!("First audio sample (right channel): {}", frame.samples[1][0]);
        }
    }
}
```

libmad samples are signed 32-bit integers. MP3 files often begin with metadata, which will cause libmad to complain. It is safe to ignore errors until libmad reaches audio data and starts producing frames.

# Documentation

TODO

# Dependencies

 * libmad (FTP: ftp://ftp.mars.org/pub/mpeg/ or [SourceForge](http://sourceforge.net/project/showfiles.php?group_id=12349))

 * libc

# License

MIT
