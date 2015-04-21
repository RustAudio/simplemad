simple-mad is a simple interface for the MPEG audio decoding library libmad. simple-mad is useful for those who need to decode MP3 or other MPEG audio files in full.

[![Build Status](https://travis-ci.org/bendykst/simple-mad.rs.svg)](https://travis-ci.org/bendykst/simple-mad.rs)

# Use

Create a `Decoder` from a boxed `Reader`.

```Rust
let path = Path::new(path_str);
let f = File::open(&path).unwrap();
let decoder = Decoder::new(Box::new(f));
```

`Decoder` implements `Iterator` and yields a sequence of `Option<Result<Frame, MadError>>`.

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
