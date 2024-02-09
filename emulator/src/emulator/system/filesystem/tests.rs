#![cfg(test)]

use hex::ToHex;
/// Unit tests for file system module
/// TODO: Write more sensible tests
use std::path::Path;

use rand::{
    thread_rng,
    RngCore,
};

use super::*;

#[test]
fn basic() -> Result<(), GenericError> {
    let mut fs = Filesystem::new(2_621_440);
    fs.create_file(Path::new("/my_file.txt"), None)?;
    assert!(fs.create_file(Path::new("/my_file.txt"), None).is_err());
    fs.create_file(Path::new("/my_file2.txt"), None)?;
    assert!(fs.restore_snapshot().is_err());
    fs.take_snapshot();

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
    fs.take_snapshot();

    fs.create_dir(Path::new("/bin/test"), None)?;
    fs.create_file(Path::new("/bin/test/my_file.bin"), None)?;
    assert!(fs.create_dir(Path::new("/my_file.txt/test"), None).is_err());
    assert!(fs.create_dir(Path::new("/bin/test"), None).is_err());
    assert!(fs
        .create_dir(Path::new("/doesnotexist/test"), None)
        .is_err());

    fs.restore_snapshot()?;
    let resolved = fs.resolve_path(Path::new("/home/mark/my_stuff/my_file.txt"), false)?;
    assert_eq!(resolved.len(), 5);
    fs.discard_current_and_restore_previous_snapshot()?;
    for i in 0..10_000 {
        if i % 1000 == 0 {
            fs.take_snapshot();
        }
        fs.create_file(Path::new(&format!("/test{}.txt", i)), None)?;
    }
    for i in 0..10_000 {
        if i % 1000 == 0 {
            fs.discard_current_and_restore_previous_snapshot()?;
        }
    }
    let my_file = fs.resolve_last(Path::new("/my_file.txt"), false)?;
    fs.write(my_file, 0, "asdf".as_bytes())?;
    fs.delete(Path::new("/my_file.txt"))?;
    assert!(fs.write(my_file, 0, "asdf".as_bytes()).is_err());
    Ok(())
}

#[test]
fn load_file_contents() -> Result<(), GenericError> {
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
    fs.take_snapshot();
    fs.write(file_id, 5000, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".as_bytes())?;
    fs.take_snapshot();
    fs.discard_current_and_restore_previous_snapshot()?;
    for i in 1..1_000 {
        let mut rdata = [0u8; 10000];
        thread_rng().fill_bytes(&mut rdata);
        fs.write(file_id, i * 10, &rdata)?;
        fs.read(file_id, (i - 1) * 10, 10000)?;
        if i % 1_000 == 0 {
            fs.restore_snapshot()?;
        }
    }
    fs.restore_snapshot()?;

    let data3 = "Beep Boop".as_bytes();
    let file_id3 = fs.create_file_with_contents(Path::new("/test3.txt"), data3, None)?;
    assert_eq!(data3, fs.read(file_id3, 0, data3.len())?);
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    fs.truncate(file_id3, 5000)?;
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    fs.take_snapshot();
    fs.truncate(file_id3, 4)?;
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    fs.truncate(file_id3, 5000)?;
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    fs.restore_snapshot()?;
    fs.truncate(file_id3, 10)?;
    println!("{:?}", fs.read(file_id3, 0, 10000)?);
    Ok(())
}

#[test]
fn load_fs() -> Result<(), FSError> {
    let reference = include_bytes!("../../../../tests/assets/mremap_private_anon.c");
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push(Path::new("tests"));
    let fs = Filesystem::from_disk(1_073_741_824, dir.as_path())?;
    // fs.print_tree();
    let lib_id = fs.resolve_last(Path::new("/assets/mremap_private_anon.c"), false)?;
    let data = fs.read(lib_id, 0, 100000)?;
    assert_eq!(data.as_slice(), reference.as_slice());
    Ok(())
}

#[test]
fn size_after_reset_is_correct() -> Result<(), FSError> {
    let mut fs = Filesystem::new(1000);
    let test_file = Path::new("/test.txt");
    fs.create_file(test_file, None)?;
    let test_file_id = fs.resolve_last(test_file, false)?;
    // TODO: Implement check
    Ok(())
}

#[test]
fn symlink() -> Result<(), FSError> {
    let mut fs = Filesystem::new(1000);
    let test_file = Path::new("/test.txt");
    fs.create_file(test_file, None)?;
    let test_file_id = fs.resolve_last(test_file, false)?;
    let test_link = Path::new("/link");
    fs.create_symlink(test_link, test_file.to_path_buf(), None)?;
    let test_link_id = fs.resolve_last(test_link, true)?;
    assert!(test_file_id == test_link_id);
    // TODO: Implement check
    Ok(())
}

#[test]
fn symlink_loop() -> Result<(), FSError> {
    let mut fs = Filesystem::new(1000);
    let test_link = Path::new("/link");
    fs.create_symlink(test_link, test_link.to_path_buf(), None)?;
    let err = fs.resolve_last(test_link, true);
    assert!(err == Err(FSError::Loop));
    Ok(())
}

#[test]
fn symlink_complex() -> Result<(), FSError> {
    let mut fs = Filesystem::new(1000);
    let realpath = Path::new("/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2");
    let dir = realpath.parent().expect("");
    let mut pb = PathBuf::new();
    for d in dir {
        pb.push(d);
        fs.create_dir(pb.as_path(), None);
    }
    let real_file_id = fs.create_file(realpath, None)?;
    fs.create_symlink(Path::new("/lib64"), PathBuf::from("usr/lib64"), None)?;
    fs.create_dir(Path::new("/usr/lib64"), None);
    fs.create_symlink(
        Path::new("/usr/lib64/ld-linux-x86-64.so.2"),
        PathBuf::from("/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2"),
        None,
    );
    fs.create_symlink(Path::new("/lib"), PathBuf::from("/usr/lib"), None)?;
    let link_id = fs.resolve_last(Path::new("/lib64/ld-linux-x86-64.so.2"), true)?;
    assert!(link_id == real_file_id);
    Ok(())
}
