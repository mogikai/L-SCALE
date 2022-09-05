/// Unit tests for file system module
/// TODO: Write more sensible tests
use std::path::Path;

use rand::{RngCore, thread_rng};
use crate::emulator::abstractions::handles::FSError;

use crate::emulator::system::filesystem::Filesystem;

#[test]
fn basic() -> Result<(), FSError> {
    let mut fs = Filesystem::new(2_621_440);
    fs.create_file(Path::new("/my_file.txt"), None)?;
    assert!(fs.create_file(Path::new("/my_file.txt"), None).is_err());
    fs.create_file(Path::new("/my_file2.txt"), None)?;
    assert!(fs.restore().is_err());
    fs.snapshot();

    assert!(fs.create_file(Path::new("/my_file2.txt"), None).is_err());
    fs.delete(Path::new("/my_file2.txt"))?;
    fs.create_file(Path::new("/my_file2.txt"), None)?;
    fs.delete(Path::new("/my_file2.txt"))?;
    fs.create_dir(Path::new("/bin"), None)?;
    fs.create_dir(Path::new("/home"), None)?;
    fs.create_dir(Path::new("/home/mark"), None)?;
    assert!(fs.delete(Path::new("/home")).is_err());
    fs.create_dir(Path::new("/home/mark/my_stuff"), None)?;
    fs.create_file(Path::new("/home/mark/my_stuff/my_file.txt"), None)?;
    fs.snapshot();

    fs.create_dir(Path::new("/bin/test"), None)?;
    fs.create_file(Path::new("/bin/test/my_file.bin"), None)?;
    assert!(fs.create_dir(Path::new("/my_file.txt/test"), None).is_err());
    assert!(fs.create_dir(Path::new("/bin/test"), None).is_err());
    assert!(fs.create_dir(Path::new("/doesnotexist/test"), None).is_err());

    fs.restore_and_discard()?;
    let resolved = fs.resolve_path(Path::new("/home/mark/my_stuff/my_file.txt"))?;
    assert_eq!(resolved.len(), 5);
    fs.restore()?;
    for i in 0..10_000 {
        if i % 1000 == 0 {
            fs.snapshot();
        }
        fs.create_file(Path::new(&format!("/test{}.txt", i)), None)?;
    }
    for i in 0..10_000 {
        if i % 1000 == 0 {
            fs.restore_and_discard()?;
        }
    }
    fs.restore()?;
    let my_file = fs.resolve_last(Path::new("/my_file.txt"))?;
    fs.write(my_file, 0, "asdf".as_bytes())?;
    fs.delete(Path::new("/my_file.txt"))?;
    assert!(fs.write(my_file, 0, "asdf".as_bytes()).is_err());
    Ok(())
}

#[test]
fn load_file_contents() -> Result<(), FSError> {
    let mut fs = Filesystem::new(1_073_741_824);
    let data = "This a sample text file\n It should contain two lines and a trailing newline\n".as_bytes();
    let data2 = "This a different sample text file\n It should contain two lines and a trailing newline and some fancy shit WOOOOO\n".as_bytes();
    let file_id = fs.create_file_with_contents(Path::new("/test.txt"), data, None)?;
    let file_id2 = fs.create_file_with_contents(Path::new("/test2.txt"), data2, None)?;

    assert_eq!(data, fs.read(file_id, 0, data.len())?.as_slice());
    assert_eq!(data2, fs.read(file_id2, 0, data2.len())?.as_slice());
    fs.write(file_id, 10000, data)?;
    fs.write(file_id2, 10000, data)?;
    fs.write(file_id, 10010, data)?;
    fs.snapshot();
    fs.write(file_id, 5000, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".as_bytes())?;
    fs.restore_and_discard()?;
    fs.snapshot();
    for i in 1..1_000 {
        let mut rdata = [0u8; 10000];
        thread_rng().fill_bytes(&mut rdata);
        fs.write(file_id, i * 10, &rdata)?;
        fs.read(file_id, (i - 1) * 10, 10000)?;
        if i % 1_000 == 0 {
            fs.restore()?;
        }
    }
    fs.restore()?;

    let data3 = "Beep Boop".as_bytes();
    let file_id3 = fs.create_file_with_contents(Path::new("/test3.txt"), data3, None)?;
    assert_eq!(data3, fs.read(file_id3, 0, data3.len())?);
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    fs.truncate(file_id3, 5000)?;
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    fs.snapshot();
    fs.truncate(file_id3, 4)?;
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    fs.truncate(file_id3, 5000)?;
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    fs.restore()?;
    fs.truncate(file_id3, 10)?;
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    Ok(())
}

#[test]
fn load_fs() -> Result<(), FSError> {
    let fs = Filesystem::from_disk(1_073_741_824, Path::new("/home/mark/projects/fraunhofer/fuzzing/prototypes/ffs"))?;
    // fs.print_tree();
    let lib_id = fs.resolve_last(Path::new("/src/lib.rs"))?;
    let data = fs.read(lib_id, 0, 100000)?;
    println!("{}", String::from_utf8(data).unwrap());
    Ok(())
}

#[test]
fn size_after_reset_is_correct() -> Result<(), FSError> {
    let mut fs = Filesystem::new(1000);
    let test_file = Path::new("/test.txt");
    fs.create_file(test_file, None)?;
    let test_file_id = fs.resolve_last(test_file)?;
    // TODO: Implement check
    Ok(())
}