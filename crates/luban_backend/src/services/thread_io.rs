use std::io::{BufReader, Read};
use std::thread::{self, JoinHandle};

pub(super) fn spawn_read_to_string<R: Read + Send + 'static>(input: R) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let mut reader = BufReader::new(input);
        let _ = reader.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).to_string()
    })
}
