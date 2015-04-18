#![allow(dead_code, unused_imports)]
use std::io::Read;
use std::default::Default;
use std::io::BufReader;
use std::path::Path;
use std::fs::File;
use std::mem::size_of;
mod decoder;

#[test]
fn test_open_file() {
    println!("");
    let path = Path::new("test_samples/tailtoddle_lo.mp3");
    let f = File::open(&path).unwrap();
    let decoder = decoder::decode(f);
    let mut frame_count = 0;
    for frame in decoder.iter() {
        frame_count += 1;
    }
    assert_eq!(frame_count, 1656);
}

/*
#[test]
fn test_long_file() {
    println!("");
    let path = Path::new("test_samples/fs-242.mp3");
    let f = File::open(&path).unwrap();
    let decoder = decoder::decode(f);
    let mut frame_count = 0;
    for frame in decoder.iter() {
        frame_count += 1;
    }
    assert!(true);
}
*/

/*
#[test]
fn test_bad_file() {
    println!("");
    let path = Path::new("test_samples/1280.jpg");
    let f = File::open(&path).unwrap();
    let decoder = decoder::decode(f);
    let mut frame_count = 0;
    for frame in decoder.iter() {
        frame_count += 1;
    }
    assert!(true);
}
*/
