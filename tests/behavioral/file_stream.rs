use eaglemode_rs::emCore::emFileStream::emFileStream;

fn tmp_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("eaglemode_test_filestream");
    std::fs::create_dir_all(&dir).ok();
    dir.join(name)
}

#[test]
fn open_read_close() {
    let path = tmp_path("open_read_close.bin");
    std::fs::write(&path, b"hello").unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();
    assert!(fs.IsOpen());

    let mut buf = vec![0u8; 5];
    fs.TryRead(&mut buf).unwrap();
    assert_eq!(&buf, b"hello");

    fs.TryClose().unwrap();
    assert!(!fs.IsOpen());
    std::fs::remove_file(&path).ok();
}

#[test]
fn open_write_close_reread() {
    let path = tmp_path("write_reread.bin");

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "wb").unwrap();
    fs.TryWrite(b"world").unwrap();
    fs.TryClose().unwrap();

    let contents = std::fs::read(&path).unwrap();
    assert_eq!(&contents, b"world");
    std::fs::remove_file(&path).ok();
}

#[test]
fn seek_and_tell() {
    let path = tmp_path("seek_tell.bin");
    std::fs::write(&path, b"abcdefghij").unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();

    assert_eq!(fs.TryTell().unwrap(), 0);
    fs.TrySeek(5).unwrap();
    assert_eq!(fs.TryTell().unwrap(), 5);

    let mut buf = vec![0u8; 3];
    fs.TryRead(&mut buf).unwrap();
    assert_eq!(&buf, b"fgh");
    assert_eq!(fs.TryTell().unwrap(), 8);

    fs.TryClose().unwrap();
    std::fs::remove_file(&path).ok();
}

#[test]
fn read_at_most() {
    let path = tmp_path("read_at_most.bin");
    std::fs::write(&path, b"abc").unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();

    let mut buf = vec![0u8; 10];
    let n = fs.TryReadAtMost(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf[..n], b"abc");

    fs.TryClose().unwrap();
    std::fs::remove_file(&path).ok();
}

#[test]
fn read_line() {
    let path = tmp_path("read_line.bin");
    std::fs::write(&path, b"line1\nline2\nline3").unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();

    assert_eq!(fs.TryReadLine(true).unwrap(), "line1");
    assert_eq!(fs.TryReadLine(true).unwrap(), "line2");
    assert_eq!(fs.TryReadLine(true).unwrap(), "line3");

    fs.TryClose().unwrap();
    std::fs::remove_file(&path).ok();
}

#[test]
fn buffered_small_reads() {
    let path = tmp_path("buffered_reads.bin");
    let data: Vec<u8> = (0..=255).collect();
    std::fs::write(&path, &data).unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();

    // Read one byte at a time — should be served from buffer
    for i in 0..=255u8 {
        assert_eq!(fs.TryReadUInt8().unwrap(), i);
    }

    fs.TryClose().unwrap();
    std::fs::remove_file(&path).ok();
}

#[test]
fn endian_uint16_le_round_trip() {
    let path = tmp_path("endian_u16le.bin");

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "wb").unwrap();
    fs.TryWriteUInt16LE(0x1234).unwrap();
    fs.TryWriteUInt16LE(0xABCD).unwrap();
    fs.TryClose().unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();
    assert_eq!(fs.TryReadUInt16LE().unwrap(), 0x1234);
    assert_eq!(fs.TryReadUInt16LE().unwrap(), 0xABCD);
    fs.TryClose().unwrap();
    std::fs::remove_file(&path).ok();
}

#[test]
fn endian_int32_be_round_trip() {
    let path = tmp_path("endian_i32be.bin");

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "wb").unwrap();
    fs.TryWriteInt32BE(-1).unwrap();
    fs.TryWriteInt32BE(0x12345678).unwrap();
    fs.TryClose().unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();
    assert_eq!(fs.TryReadInt32BE().unwrap(), -1);
    assert_eq!(fs.TryReadInt32BE().unwrap(), 0x12345678);
    fs.TryClose().unwrap();
    std::fs::remove_file(&path).ok();
}

#[test]
fn endian_uint64_le_round_trip() {
    let path = tmp_path("endian_u64le.bin");

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "wb").unwrap();
    fs.TryWriteUInt64LE(0x0102030405060708).unwrap();
    fs.TryClose().unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();
    assert_eq!(fs.TryReadUInt64LE().unwrap(), 0x0102030405060708);
    fs.TryClose().unwrap();
    std::fs::remove_file(&path).ok();
}

#[test]
fn endian_all_types() {
    let path = tmp_path("endian_all.bin");

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "wb").unwrap();
    fs.TryWriteInt8(-42).unwrap();
    fs.TryWriteUInt8(200).unwrap();
    fs.TryWriteInt16LE(-1000).unwrap();
    fs.TryWriteInt16BE(-1000).unwrap();
    fs.TryWriteUInt16LE(60000).unwrap();
    fs.TryWriteUInt16BE(60000).unwrap();
    fs.TryWriteInt32LE(-100000).unwrap();
    fs.TryWriteInt32BE(-100000).unwrap();
    fs.TryWriteUInt32LE(3_000_000_000).unwrap();
    fs.TryWriteUInt32BE(3_000_000_000).unwrap();
    fs.TryWriteInt64LE(-1_000_000_000_000).unwrap();
    fs.TryWriteInt64BE(-1_000_000_000_000).unwrap();
    fs.TryWriteUInt64LE(10_000_000_000_000).unwrap();
    fs.TryWriteUInt64BE(10_000_000_000_000).unwrap();
    fs.TryClose().unwrap();

    let mut fs = emFileStream::new();
    fs.TryOpen(&path, "rb").unwrap();
    assert_eq!(fs.TryReadInt8().unwrap(), -42);
    assert_eq!(fs.TryReadUInt8().unwrap(), 200);
    assert_eq!(fs.TryReadInt16LE().unwrap(), -1000);
    assert_eq!(fs.TryReadInt16BE().unwrap(), -1000);
    assert_eq!(fs.TryReadUInt16LE().unwrap(), 60000);
    assert_eq!(fs.TryReadUInt16BE().unwrap(), 60000);
    assert_eq!(fs.TryReadInt32LE().unwrap(), -100000);
    assert_eq!(fs.TryReadInt32BE().unwrap(), -100000);
    assert_eq!(fs.TryReadUInt32LE().unwrap(), 3_000_000_000);
    assert_eq!(fs.TryReadUInt32BE().unwrap(), 3_000_000_000);
    assert_eq!(fs.TryReadInt64LE().unwrap(), -1_000_000_000_000);
    assert_eq!(fs.TryReadInt64BE().unwrap(), -1_000_000_000_000);
    assert_eq!(fs.TryReadUInt64LE().unwrap(), 10_000_000_000_000);
    assert_eq!(fs.TryReadUInt64BE().unwrap(), 10_000_000_000_000);
    fs.TryClose().unwrap();
    std::fs::remove_file(&path).ok();
}
