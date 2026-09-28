use crate::Error;

pub(crate) fn json<T: serde::Serialize>(value: &T, limit: usize) -> Result<String, Error> {
    struct Bounded {
        bytes: Vec<u8>,
        limit: usize,
        overflow: bool,
    }
    impl std::io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len().saturating_add(bytes.len()) > self.limit {
                self.overflow = true;
                return Err(std::io::Error::other("size limit"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Bounded {
        bytes: Vec::new(),
        limit,
        overflow: false,
    };
    let encoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        serde_json::to_writer(&mut output, value)
    }));
    match encoded {
        Ok(Ok(())) => Ok(String::from_utf8(output.bytes).expect("JSON is UTF-8")),
        _ => Err(Error::OutputUnavailable(
            if output.overflow {
                "size_limit"
            } else {
                "encoding_failed"
            }
            .into(),
        )),
    }
}
