extern crate libc;
mod decoder;

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
