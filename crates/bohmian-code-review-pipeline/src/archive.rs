//! Push-driven tar reader for the streamed commit archive.
//!
//! The archive arrives as async chunks, so the reader is fed decompressed
//! bytes through [`Write`] (behind `flate2::write::GzDecoder`) instead of
//! pulling from a blocking [`std::io::Read`]. It keeps at most one header
//! block, one bounded PAX/long-name record, and one wanted file's bytes in
//! memory. A file is buffered only when its path is wanted and its size fits
//! the file bound; its bytes are released as verified only when they hash to
//! the listed blob id, so an archive can never substitute content.

use crate::github::git_blob_sha;
use crate::identity::ObjectSha;
use bohmian_code_review::RelativePath;
use std::collections::BTreeMap;
use std::io::{self, Write};

/// Tar block size.
const BLOCK: usize = 512;

/// Largest PAX or GNU long-name record accepted; real ones are a few hundred bytes.
const MAX_METADATA_BYTES: u64 = 64 * 1024;

/// A verified archive entry: path, listed blob id, and bytes hashing to it.
pub(crate) type VerifiedEntry = (RelativePath, ObjectSha, Vec<u8>);

/// Where the data bytes of the current entry go.
enum Target {
    /// Discarded: not wanted, not a regular file, or over the file bound.
    Skip,
    /// A wanted file being buffered.
    File(RelativePath, ObjectSha, Vec<u8>),
    /// A PAX extended header for the next entry.
    Pax(Vec<u8>),
    /// A GNU long name for the next entry.
    LongName(Vec<u8>),
}

/// Incremental tar parser over decompressed archive bytes.
pub(crate) struct TarStream {
    wanted: BTreeMap<RelativePath, ObjectSha>,
    max_file_bytes: u64,
    expansion_left: u64,
    header: Vec<u8>,
    /// Current entry target, data bytes left, then padding bytes left.
    entry: Option<(Target, u64, u64)>,
    next_path: Option<String>,
    next_size: Option<u64>,
    ended: bool,
    /// Entries verified since the caller last drained them.
    pub(crate) verified: Vec<VerifiedEntry>,
}

impl TarStream {
    /// A reader that buffers only `wanted` paths no larger than
    /// `max_file_bytes` and fails once more than `max_expanded_bytes`
    /// decompressed bytes were written.
    pub(crate) fn new(
        wanted: BTreeMap<RelativePath, ObjectSha>,
        max_file_bytes: u64,
        max_expanded_bytes: u64,
    ) -> TarStream {
        TarStream {
            wanted,
            max_file_bytes,
            expansion_left: max_expanded_bytes,
            header: Vec::with_capacity(BLOCK),
            entry: None,
            next_path: None,
            next_size: None,
            ended: false,
            verified: Vec::new(),
        }
    }

    /// Consume `input`, completing headers and entries as they fill.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::InvalidData`] for an unreadable size field
    /// or an oversized or malformed PAX/long-name record.
    fn feed(&mut self, mut input: &[u8]) -> io::Result<()> {
        while !input.is_empty() && !self.ended {
            let Some((target, data_left, padding_left)) = &mut self.entry else {
                let take = (BLOCK - self.header.len()).min(input.len());
                self.header.extend_from_slice(&input[..take]);
                input = &input[take..];
                if self.header.len() == BLOCK {
                    let block = std::mem::take(&mut self.header);
                    self.start_entry(&block)?;
                    self.header = block;
                    self.header.clear();
                }
                continue;
            };
            if *data_left > 0 {
                let take = usize::try_from(*data_left).map_or(input.len(), |n| n.min(input.len()));
                match target {
                    Target::Skip => {}
                    Target::File(_, _, bytes) | Target::Pax(bytes) | Target::LongName(bytes) => {
                        bytes.extend_from_slice(&input[..take]);
                    }
                }
                *data_left -= take as u64;
                input = &input[take..];
                if *data_left == 0 {
                    let target = std::mem::replace(target, Target::Skip);
                    self.finish_entry(target)?;
                }
            } else {
                let take =
                    usize::try_from(*padding_left).map_or(input.len(), |n| n.min(input.len()));
                *padding_left -= take as u64;
                input = &input[take..];
            }
            if matches!(self.entry, Some((_, 0, 0))) {
                self.entry = None;
            }
        }
        Ok(())
    }

    /// Parse one header block and choose where the entry's data goes.
    ///
    /// An all-zero block ends the archive. PAX `x` and GNU `L` records set
    /// the path (and PAX the size) of the following entry; every other
    /// entry consumes them.
    ///
    /// # Errors
    ///
    /// As [`TarStream::feed`].
    fn start_entry(&mut self, block: &[u8]) -> io::Result<()> {
        if block.iter().all(|&byte| byte == 0) {
            self.ended = true;
            return Ok(());
        }
        let kind = block[156];
        let header_size = parse_size(&block[124..136])?;
        let target = match kind {
            b'x' | b'L' if header_size > MAX_METADATA_BYTES => {
                return Err(invalid("tar metadata record exceeds its bound"));
            }
            b'x' => Target::Pax(Vec::new()),
            b'L' => Target::LongName(Vec::new()),
            b'g' | b'K' => Target::Skip,
            _ => {
                let size = self.next_size.take().unwrap_or(header_size);
                let path = self.next_path.take().unwrap_or_else(|| header_path(block));
                let target = self.file_target(&path, size, kind);
                return self.begin(target, size);
            }
        };
        self.begin(target, header_size)
    }

    /// Enter the data section of an entry of `size` bytes; an empty entry
    /// completes at once.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::InvalidData`] when the padded size overflows
    /// or an empty PAX record is malformed.
    fn begin(&mut self, target: Target, size: u64) -> io::Result<()> {
        if size == 0 {
            return self.finish_entry(target);
        }
        let padded = size
            .checked_next_multiple_of(BLOCK as u64)
            .ok_or_else(|| invalid("tar size overflows"))?;
        self.entry = Some((target, size, padded - size));
        Ok(())
    }

    /// Buffer a regular file whose repository path (the archive path minus
    /// its top-level `owner-repo-sha/` directory) is wanted and fits the bound.
    fn file_target(&self, path: &str, size: u64, kind: u8) -> Target {
        if !matches!(kind, b'0' | 0 | b'7') || size > self.max_file_bytes {
            return Target::Skip;
        }
        let Some(path) = path
            .split_once('/')
            .and_then(|(_, rest)| RelativePath::new(rest).ok())
        else {
            return Target::Skip;
        };
        match self.wanted.get(&path) {
            Some(blob) => {
                let blob = blob.clone();
                Target::File(path, blob, Vec::with_capacity(size as usize))
            }
            None => Target::Skip,
        }
    }

    /// Apply a completed metadata record or verify a completed file.
    ///
    /// # Errors
    ///
    /// Returns [`io::ErrorKind::InvalidData`] for a malformed PAX record.
    fn finish_entry(&mut self, target: Target) -> io::Result<()> {
        match target {
            Target::Skip => {}
            Target::Pax(bytes) => {
                for (key, value) in pax_records(&bytes)? {
                    match key {
                        "path" => self.next_path = Some(value.to_owned()),
                        "size" => {
                            self.next_size =
                                Some(value.parse().map_err(|_| invalid("bad PAX size"))?);
                        }
                        _ => {}
                    }
                }
            }
            Target::LongName(bytes) => self.next_path = Some(c_string(&bytes)),
            Target::File(path, blob, bytes) => {
                if git_blob_sha(&bytes) == blob.as_str() {
                    self.wanted.remove(&path);
                    self.verified.push((path, blob, bytes));
                }
            }
        }
        Ok(())
    }
}

impl Write for TarStream {
    /// Consume every byte or fail; bytes after the end-of-archive block are ignored.
    ///
    /// # Errors
    ///
    /// Returns an error naming `archive expansion` once the decompressed
    /// stream exceeds its bound, or a parse error from [`TarStream::feed`].
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if input.len() as u64 > self.expansion_left {
            return Err(io::Error::other("archive expansion exceeds its byte limit"));
        }
        self.expansion_left -= input.len() as u64;
        self.feed(input)?;
        Ok(input.len())
    }

    /// Nothing is buffered for a downstream writer.
    ///
    /// # Errors
    ///
    /// Never.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// An [`io::ErrorKind::InvalidData`] error with `detail`.
fn invalid(detail: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, detail.to_owned())
}

/// The text of a NUL-terminated header field.
fn c_string(field: &[u8]) -> String {
    let end = field
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// Entry path from the header: `prefix/name` for POSIX ustar, else `name`.
fn header_path(block: &[u8]) -> String {
    let name = c_string(&block[..100]);
    let prefix = c_string(&block[345..500]);
    if &block[257..263] == b"ustar\0" && !prefix.is_empty() {
        format!("{prefix}/{name}")
    } else {
        name
    }
}

/// Decode a size field: octal text, or GNU base-256 when the high bit is set.
///
/// # Errors
///
/// Returns [`io::ErrorKind::InvalidData`] for non-octal text or a value
/// that does not fit `u64`.
fn parse_size(field: &[u8]) -> io::Result<u64> {
    if field[0] & 0x80 != 0 {
        return field[1..]
            .iter()
            .try_fold(u64::from(field[0] & 0x7f), |acc, &byte| {
                acc.checked_mul(256)
                    .map(|acc| acc | u64::from(byte))
                    .ok_or_else(|| invalid("tar size overflows"))
            });
    }
    let text = c_string(field);
    let text = text.trim_matches(|ch: char| ch == ' ' || ch == '\0');
    if text.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(text, 8).map_err(|_| invalid("bad tar size"))
}

/// Split PAX `"<len> <key>=<value>\n"` records.
///
/// # Errors
///
/// Returns [`io::ErrorKind::InvalidData`] when a length is missing, points
/// past the data, or a record lacks `=` or its newline.
fn pax_records(mut data: &[u8]) -> io::Result<Vec<(&str, &str)>> {
    let mut records = Vec::new();
    while !data.is_empty() {
        let space = data
            .iter()
            .position(|&byte| byte == b' ')
            .ok_or_else(|| invalid("bad PAX record"))?;
        let len: usize = std::str::from_utf8(&data[..space])
            .ok()
            .and_then(|len| len.parse().ok())
            .filter(|&len| len > space + 1 && len <= data.len())
            .ok_or_else(|| invalid("bad PAX record length"))?;
        let record = std::str::from_utf8(&data[space + 1..len])
            .map_err(|_| invalid("PAX record is not UTF-8"))?;
        let (key, value) = record
            .strip_suffix('\n')
            .and_then(|record| record.split_once('='))
            .ok_or_else(|| invalid("bad PAX record"))?;
        records.push((key, value));
        data = &data[len..];
    }
    Ok(records)
}

#[cfg(test)]
/// Owner tests for the push tar reader: chunk-boundary independence, PAX and
/// GNU long names, verification, and bounds.
mod tests {
    use super::*;
    use tar::{Builder, EntryType, Header};

    /// Blob id of `bytes`.
    fn blob(bytes: &[u8]) -> ObjectSha {
        ObjectSha::new(&git_blob_sha(bytes)).expect("sha")
    }

    /// A PAX (long ustar path), a GNU long name, a plain entry, a symlink,
    /// and an entry whose bytes do not match their listed blob.
    fn fixture() -> (Vec<u8>, BTreeMap<RelativePath, ObjectSha>) {
        let long = format!("{}/deep.ts", "d".repeat(120));
        let mut builder = Builder::new(Vec::new());
        let mut add = |header: &mut Header, path: &str, bytes: &[u8]| {
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            builder
                .append_data(header, format!("top/{path}"), bytes)
                .expect("append");
        };
        add(&mut Header::new_ustar(), &long, b"long ustar");
        add(&mut Header::new_gnu(), &format!("{long}.gnu"), b"long gnu");
        add(&mut Header::new_gnu(), "a.ts", b"plain");
        add(&mut Header::new_gnu(), "bad.ts", b"substituted");
        let mut link = Header::new_gnu();
        link.set_entry_type(EntryType::Symlink);
        link.set_size(0);
        builder
            .append_link(&mut link, "top/link.ts", "a.ts")
            .expect("link");
        let tar = builder.into_inner().expect("tar");
        let wanted = [
            (long.clone(), blob(b"long ustar")),
            (format!("{long}.gnu"), blob(b"long gnu")),
            ("a.ts".to_owned(), blob(b"plain")),
            ("bad.ts".to_owned(), blob(b"original")),
            ("link.ts".to_owned(), blob(b"")),
        ]
        .into_iter()
        .map(|(path, sha)| (RelativePath::new(&path).expect("path"), sha))
        .collect();
        (tar, wanted)
    }

    /// Any chunking yields the same verified entries; mismatched bytes and
    /// non-regular entries are never released.
    #[test]
    fn entries_verify_identically_across_chunk_boundaries() {
        let (tar, wanted) = fixture();
        for chunk in [1, 7, 511, 512, 513, tar.len()] {
            let mut stream = TarStream::new(wanted.clone(), 1024, u64::MAX);
            for part in tar.chunks(chunk) {
                stream.write_all(part).expect("write");
            }
            let paths: Vec<&str> = stream
                .verified
                .iter()
                .map(|(path, _, _)| path.as_str())
                .collect();
            assert_eq!(paths.len(), 3, "chunk {chunk}");
            assert!(paths.contains(&"a.ts"));
            assert!(paths.iter().any(|path| path.ends_with("deep.ts")));
            assert!(paths.iter().any(|path| path.ends_with("deep.ts.gnu")));
        }
    }

    /// Files above the file bound are not buffered, and the expansion bound
    /// fails the stream instead of truncating it.
    #[test]
    fn file_and_expansion_bounds_hold() {
        let (tar, wanted) = fixture();
        let mut stream = TarStream::new(wanted.clone(), 5, u64::MAX);
        stream.write_all(&tar).expect("write");
        let paths: Vec<&str> = stream
            .verified
            .iter()
            .map(|(path, _, _)| path.as_str())
            .collect();
        assert_eq!(paths, ["a.ts"]);
        let error = TarStream::new(wanted, 1024, 600)
            .write_all(&tar)
            .expect_err("bounded");
        assert!(error.to_string().contains("archive expansion"));
    }

    /// Octal and base-256 sizes decode; a PAX record past its data fails.
    #[test]
    fn sizes_and_pax_records_are_validated() {
        assert_eq!(parse_size(b"00000000012\0").expect("octal"), 10);
        let mut base256 = [0u8; 12];
        base256[0] = 0x80;
        base256[11] = 3;
        assert_eq!(parse_size(&base256).expect("base-256"), 3);
        assert!(parse_size(b"zz\0").is_err());
        assert_eq!(
            pax_records(b"12 path=a/b\n").expect("pax"),
            [("path", "a/b")]
        );
        assert!(pax_records(b"99 path=a\n").is_err());
    }
}
