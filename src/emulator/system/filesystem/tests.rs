use std::path::Path;

use rand::{RngCore, thread_rng};

use crate::emulator::system::filesystem::Filesystem;

#[test]
fn basic() {
    let mut fs = Filesystem::new(2_621_440);
    assert!(fs.create_file(Path::new("/my_file.txt"), None).is_ok());
    assert!(fs.create_file(Path::new("/my_file.txt"), None).is_err());
    assert!(fs.create_file(Path::new("/my_file2.txt"), None).is_ok());
    println!("{:?}", fs);
    assert!(fs.restore().is_err());
    println!("Before Snapshot: ");
    fs.print_tree();
    fs.snapshot();

    assert!(fs.create_file(Path::new("/my_file2.txt"), None).is_err());
    assert!(fs.delete(Path::new("/my_file2.txt")).is_ok());
    assert!(fs.create_file(Path::new("/my_file2.txt"), None).is_ok());
    assert!(fs.delete(Path::new("/my_file2.txt")).is_ok());
    assert!(fs.create_dir(Path::new("/bin"), None).is_ok());
    assert!(fs.create_dir(Path::new("/home"), None).is_ok());
    assert!(fs.create_dir(Path::new("/home/mark"), None).is_ok());
    assert!(fs.delete(Path::new("/home")).is_err());
    assert!(fs.create_dir(Path::new("/home/mark/my_stuff"), None).is_ok());
    assert!(fs.create_file(Path::new("/home/mark/my_stuff/my_file.txt"), None).is_ok());
    println!("Before Snapshot: ");
    fs.print_tree();
    fs.snapshot();

    assert!(fs.create_dir(Path::new("/bin/test"), None).is_ok());
    assert!(fs.create_file(Path::new("/bin/test/my_file.bin"), None).is_ok());
    assert!(fs.create_dir(Path::new("/my_file.txt/test"), None).is_err());
    assert!(fs.create_dir(Path::new("/bin/test"), None).is_err());
    assert!(fs.create_dir(Path::new("/doesnotexist/test"), None).is_err());

    println!("Before restore: ");
    fs.print_tree();
    assert!(fs.restore_and_discard().is_ok());
    println!("After restore: ");
    fs.print_tree();
    let resolved = fs.resolve_path(Path::new("/home/mark/my_stuff/my_file.txt"));
    assert!(resolved.is_ok());
    let resolved = resolved.unwrap();
    assert_eq!(resolved.len(), 5);
    println!("After restore: ");
    assert!(fs.restore().is_ok());
    fs.print_tree();
    for i in 0..10_000 {
        if i % 1000 == 0 {
            fs.snapshot();
        }
        assert!(fs.create_file(Path::new(&format!("/test{}.txt", i)), None).is_ok())
    }
    for i in 0..10_000 {
        if i % 1000 == 0 {
            fs.restore_and_discard().unwrap();
        }
    }
    assert!(fs.restore().is_ok());
    fs.print_tree();
    println!("{:?}", fs);
    let my_file = fs.resolve_last(Path::new("/my_file.txt")).unwrap();
    assert!(fs.write(my_file, 0, "asdf".as_bytes()).is_ok());
    assert!(fs.delete(Path::new("/my_file.txt")).is_ok());
    assert!(fs.write(my_file, 0, "asdf".as_bytes()).is_err());
}

#[test]
fn load_file_contents() {
    let mut fs = Filesystem::new(1_073_741_824);
    let data = "This a sample text file\n It should contain two lines and a trailing newline\n".as_bytes();
    let data2 = "This a different sample text file\n It should contain two lines and a trailing newline and some fancy shit WOOOOO\n".as_bytes();
    let result = fs.create_file_with_contents(Path::new("/test.txt"), data, None);
    assert!(result.is_ok());
    let result2 = fs.create_file_with_contents(Path::new("/test2.txt"), data2, None);
    assert!(result2.is_ok());
    let file_id = result.unwrap();
    let file_id2 = result2.unwrap();
    assert_eq!(data, fs.read(file_id, 0, data.len()).unwrap().as_slice());
    assert_eq!(data2, fs.read(file_id2, 0, data2.len()).unwrap().as_slice());
    assert!(fs.write(file_id, 10000, data).is_ok());
    assert!(fs.write(file_id2, 10000, data).is_ok());
    assert!(fs.write(file_id, 10010, data).is_ok());
    fs.snapshot();
    fs.write(file_id, 5000, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".as_bytes()).unwrap();
    fs.restore_and_discard().unwrap();
    fs.snapshot();
    for i in 1..1_000 {
        let mut rdata = [0u8; 10000];
        thread_rng().fill_bytes(&mut rdata);
        fs.write(file_id, i * 10, &rdata).unwrap();
        fs.read(file_id, (i - 1) * 10, 10000).unwrap();
        if i % 1_000 == 0 {
            fs.restore().unwrap();
        }
    }
    fs.restore().unwrap();

    let data3 = "Beep Boop".as_bytes();
    let result3 = fs.create_file_with_contents(Path::new("/test3.txt"), data3, None);
    assert!(result3.is_ok());
    let file_id3 = result3.unwrap();
    assert_eq!(data3, fs.read(file_id3, 0, data3.len()).unwrap());
    println!("{:?}", fs.read(file_id3, 0, 10000));
    fs.truncate(file_id3, 5000).unwrap();
    println!("{:?}", fs.read(file_id3, 0, 10000));
    fs.snapshot();
    fs.truncate(file_id3, 4).unwrap();
    println!("{:?}", fs.read(file_id3, 0, 10000));
    fs.truncate(file_id3, 5000).unwrap();
    println!("{:?}", fs.read(file_id3, 0, 10000));
    fs.restore().unwrap();
    fs.truncate(file_id3, 10).unwrap();
    println!("{:?}", fs.read(file_id3, 0, 10000));
}

#[test]
fn load_fs() {
    let fs = Filesystem::from_disk(1_073_741_824, Path::new("/home/mark/projects/fraunhofer/fuzzing/prototypes/ffs")).unwrap();
    // fs.print_tree();
    let lib_id = fs.resolve_last(Path::new("/src/lib.rs")).unwrap();
    let data = fs.read(lib_id, 0, 100000).unwrap();
    println!("{}", String::from_utf8(data).unwrap());
}

#[test]
fn size_after_reset_is_correct() {
    let mut fs = Filesystem::new(1000);
    let test_file = Path::new("/test.txt");
    fs.create_file(test_file, None).unwrap();
    let test_file_id = fs.resolve_last(test_file).unwrap();
}