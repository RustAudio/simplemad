#![allow(dead_code, unused_imports)]
use std::io::Read;
use std::default::Default;
use std::io::BufReader;
use std::path::Path;
use std::fs::File;
mod decoder;

#[test]
fn test_open_file() {
    println!("");
    let path = Path::new("test_samples/tailtoddle_lo.mp3");
    let f = File::open(&path).unwrap();
    let decoder = decoder::decode(f);
    let mut frame_count = 0;
    for frame in decoder.iter() {
        println!("Got frame #{}: {}, {}, {}",
                 frame_count,
                 frame.sample_rate,
                 frame.channels,
                 frame.length);
        frame_count += 1;
    }
    assert_eq!(frame_count, 1570);
}
