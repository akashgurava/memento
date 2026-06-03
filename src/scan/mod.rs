#[cfg(feature = "cli")]
pub(crate) mod cli;

use std::{
    collections::HashSet,
    fs::Metadata,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use ignore::{DirEntry, Error as IgnoreError, Walk, WalkBuilder};

use crate::{
    bus::{Message, Producer},
    config::AppConfig,
    error::MementoError,
};

/// Broad category of a scanned filesystem entry.
///
/// Used downstream to route files into the correct processing pipeline
/// (e.g. thumbnail generation for images, transcoding for videos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A recognised image file (extension is in `AppConfig::image_extensions`).
    Image,
    /// A recognised video file (extension is in `AppConfig::video_extensions`).
    Video,
    /// Any other regular file whose extension is not in either list.
    Other,
}

impl FileKind {
    /// Classify `path` by comparing its lowercased extension against the
    /// caller-supplied allow-lists.  Returns [`FileKind::Other`] when the
    /// path has no extension or the extension matches neither list.
    fn classify(path: &Path, image_exts: &HashSet<String>, video_exts: &HashSet<String>) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext {
            Some(ref e) if image_exts.contains(e) => FileKind::Image,
            Some(ref e) if video_exts.contains(e) => FileKind::Video,
            _ => FileKind::Other,
        }
    }
}

/// A regular file discovered during a directory walk.
///
/// Carries the scan root it belongs to, its absolute path, its broad
/// [`FileKind`] classification, and cached filesystem [`Metadata`].
#[derive(Debug)]
pub struct ScanFile {
    /// The scan root under which this file was discovered.
    root: Arc<Path>,
    /// Absolute path to the file.
    path: Arc<Path>,
    /// Broad file-type classification.
    kind: FileKind,
    /// Cached filesystem metadata (size, timestamps, permissions, …).
    metadata: Metadata,
}

impl ScanFile {
    /// Build a [`ScanFile`] from a raw [`DirEntry`].
    ///
    /// Fetches metadata from the entry and wraps any I/O failure in a
    /// [`MementoError::fs`] that includes the offending path.
    fn new(root: Arc<Path>, kind: FileKind, entry: DirEntry) -> Result<Self, MementoError> {
        let metadata = entry
            .metadata()
            .map_err(|e| MementoError::fs(entry.path().to_path_buf(), e))?;

        Ok(Self {
            root,
            path: Arc::from(entry.path()),
            kind,
            metadata,
        })
    }

    /// Returns a cheap clone of the file's absolute path.
    pub fn path(&self) -> Arc<Path> {
        self.path.clone()
    }

    /// Returns the file's absolute path as a `&str`.
    ///
    /// # Errors
    /// Returns [`MementoError::invalid_path`] when the path contains
    /// non-UTF-8 bytes.
    pub fn path_str(&self) -> Result<&str, MementoError> {
        self.path
            .to_str()
            .ok_or_else(|| MementoError::invalid_path(self.path.to_path_buf()))
    }

    /// Returns the scan root this file belongs to.
    pub fn root(&self) -> &Arc<Path> {
        &self.root
    }

    /// Returns the file's broad type classification.
    pub fn kind(&self) -> FileKind {
        self.kind
    }

    /// Returns cached filesystem metadata for the file.
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

/// Events emitted by [`Walker`] as it traverses the configured roots.
///
/// Consumers should handle all variants; in particular they must treat
/// [`ScanMessage::Cancelled`] and [`ScanMessage::Error`] differently —
/// the former signals an intentional, clean stop while the latter
/// indicates an unexpected filesystem problem.
#[derive(Debug)]
pub enum ScanMessage {
    /// A non-fatal filesystem or walk error occurred (e.g. permission denied).
    /// The walk continues after this message.
    Error { error: MementoError },

    /// The walk was cancelled by an external request.
    /// No further messages will be produced after this one.
    Cancelled,

    /// The walker is about to start scanning `path`.
    RootStart { path: PathBuf },

    /// A subdirectory was encountered inside the current root.
    Dir { path: PathBuf },

    /// A regular file was discovered and classified.
    File { file: ScanFile },

    /// The walker has finished scanning `path` (all entries yielded).
    RootDone { path: PathBuf },

    /// All configured roots have been fully scanned.
    Done,
}

impl ScanMessage {
    /// Produce a [`ScanMessage::Cancelled`] sentinel.
    fn cancel() -> Self {
        ScanMessage::Cancelled
    }

    /// Wrap an [`IgnoreError`] into the appropriate [`ScanMessage::Error`]
    /// variant, distinguishing I/O errors from walk-logic errors.
    fn error(root: PathBuf, error: IgnoreError) -> Self {
        let scan_err = if error.io_error().is_some() {
            MementoError::fs(root, &error)
        } else {
            MementoError::walk(root, &error)
        };
        ScanMessage::Error { error: scan_err }
    }

    /// Produce a [`ScanMessage::RootStart`] for `path`.
    fn root_start(path: PathBuf) -> Self {
        ScanMessage::RootStart { path }
    }

    /// Produce a [`ScanMessage::Dir`] for `path`.
    fn dir(path: PathBuf) -> Self {
        ScanMessage::Dir { path }
    }

    /// Classify `entry` and produce a [`ScanMessage::File`].
    ///
    /// # Errors
    /// Propagates any metadata read failure as a [`MementoError`].
    fn file(
        root: PathBuf,
        entry: DirEntry,
        image_exts: &HashSet<String>,
        video_exts: &HashSet<String>,
    ) -> Result<Self, MementoError> {
        let kind = FileKind::classify(entry.path(), image_exts, video_exts);
        Ok(ScanMessage::File {
            file: ScanFile::new(Arc::from(root), kind, entry)?,
        })
    }

    /// Produce a [`ScanMessage::RootDone`] for `path`.
    fn root_done(path: PathBuf) -> Self {
        ScanMessage::RootDone { path }
    }

    /// Produce a [`ScanMessage::Done`] sentinel.
    fn done() -> Self {
        ScanMessage::Done
    }
}

impl Message for ScanMessage {
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }
}

// ------------------------- Walker -------------------------

/// Iterates over all configured scan roots depth-first, emitting
/// [`ScanMessage`]s for every directory and file encountered.
///
/// # Message sequence per root
/// ```text
/// RootStart { path }
///   Dir  { path }   ← zero or more, one per sub-directory
///   File { file }   ← zero or more, one per regular file
/// RootDone { path }
/// ```
/// After every root has been processed the walker emits a single `Done`.
/// Non-fatal errors produce an `Error` message and the walk resumes.
pub struct Walker<'a> {
    /// Ordered list of roots to scan.
    roots: Vec<PathBuf>,
    /// Recognised image extensions (lower-case, without leading dot).
    image_exts: &'a HashSet<String>,
    /// Recognised video extensions (lower-case, without leading dot).
    video_exts: &'a HashSet<String>,
    /// The active [`Walk`] for `roots[current_index]`, or `None` when
    /// between roots.
    current_inner: Option<Walk>,
    /// Index into `roots` of the root currently being (or next to be) scanned.
    current_index: usize,
    /// External cancellation signal.
    cancel: Arc<AtomicBool>,
}

impl<'a> Walker<'a> {
    /// Construct a new `Walker` from `config`.
    ///
    /// # Errors
    /// Forwards any error returned by [`AppConfig::roots`].
    pub fn new(config: &'a AppConfig, cancel: Arc<AtomicBool>) -> Result<Self, MementoError> {
        Ok(Self {
            roots: config.roots()?,
            image_exts: config.image_extensions(),
            video_exts: config.video_extensions(),
            current_inner: None,
            current_index: 0,
            cancel,
        })
    }
}

impl<'a> Producer for Walker<'a> {
    type Message = ScanMessage;

    fn produce(&mut self) -> Self::Message {
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return ScanMessage::cancel();
            }

            if self.current_index >= self.roots.len() {
                return ScanMessage::done();
            }

            let root = &self.roots[self.current_index];

            let walk = match self.current_inner.as_mut() {
                Some(w) => w,
                None => {
                    self.current_inner = Some(WalkBuilder::new(root).hidden(false).build());
                    return ScanMessage::root_start(root.clone());
                }
            };

            match walk.next() {
                Some(Ok(entry)) => {
                    if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                        return ScanMessage::dir(entry.path().to_path_buf());
                    }

                    return match ScanMessage::file(
                        root.clone(),
                        entry,
                        self.image_exts,
                        self.video_exts,
                    ) {
                        Ok(msg) => msg,
                        Err(e) => ScanMessage::Error { error: e },
                    };
                }

                Some(Err(e)) => {
                    return ScanMessage::error(root.clone(), e);
                }

                None => {
                    let done_path = root.clone();
                    self.current_inner = None;
                    self.current_index += 1;
                    return ScanMessage::root_done(done_path);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Bus, Consumer};

    use super::*;
    use std::collections::HashMap;

    // ------------------------- FileKind::classify -------------------------

    fn exts(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn classify_by_extension() {
        let images = exts(&["jpg", "png"]);
        let videos = exts(&["mp4", "mkv"]);

        // Case-insensitive matching
        assert_eq!(
            FileKind::classify(Path::new("photo.jpg"), &images, &videos),
            FileKind::Image
        );
        assert_eq!(
            FileKind::classify(Path::new("photo.JPG"), &images, &videos),
            FileKind::Image
        );
        assert_eq!(
            FileKind::classify(Path::new("clip.MKV"), &images, &videos),
            FileKind::Video
        );

        // Unrecognised or missing extension → Other
        assert_eq!(
            FileKind::classify(Path::new("archive.zip"), &images, &videos),
            FileKind::Other
        );
        assert_eq!(
            FileKind::classify(Path::new("Makefile"), &images, &videos),
            FileKind::Other
        );

        // Empty allow-lists → everything is Other
        let empty: HashSet<String> = HashSet::new();
        assert_eq!(
            FileKind::classify(Path::new("photo.jpg"), &empty, &empty),
            FileKind::Other
        );

        // Only the last component's extension matters, not dots in dir names.
        let images = exts(&["img"]);
        let videos = exts(&["mp4"]);

        assert_eq!(
            FileKind::classify(Path::new("/abc/jii.isjs/abc.img"), &images, &videos),
            FileKind::Image
        );
        assert_eq!(
            FileKind::classify(Path::new("/abc/jii.mp4/readme.txt"), &images, &videos),
            FileKind::Other
        );
    }

    // ------------------------- ScanMessage helpers -------------------------

    #[test]
    fn terminal_message_constructors() {
        assert!(matches!(ScanMessage::cancel(), ScanMessage::Cancelled));
        assert!(matches!(ScanMessage::done(), ScanMessage::Done));
    }

    #[test]
    fn root_start_carries_path() {
        let p = PathBuf::from("/tmp/root");
        match ScanMessage::root_start(p.clone()) {
            ScanMessage::RootStart { path } => assert_eq!(path, p),
            other => panic!("expected RootStart, got {other:?}"),
        }
    }

    #[test]
    fn root_done_carries_path() {
        let p = PathBuf::from("/tmp/root");
        match ScanMessage::root_done(p.clone()) {
            ScanMessage::RootDone { path } => assert_eq!(path, p),
            other => panic!("expected RootDone, got {other:?}"),
        }
    }

    #[test]
    fn dir_carries_path() {
        let p = PathBuf::from("/tmp/root/sub");
        match ScanMessage::dir(p.clone()) {
            ScanMessage::Dir { path } => assert_eq!(path, p),
            other => panic!("expected Dir, got {other:?}"),
        }
    }

    // ------------------------- Walker integration -------------------------

    /// Drain a `Walker` into a vec of messages (safety cap at 1 000).
    fn drain(walker: &mut Walker<'_>) -> Vec<ScanMessage> {
        let mut msgs = Vec::new();
        loop {
            let msg = walker.produce();
            let terminal = msg.is_terminal();
            msgs.push(msg);
            if terminal || msgs.len() >= 1_000 {
                break;
            }
        }
        msgs
    }

    /// Count messages by a simple discriminant string.
    fn counts(msgs: &[ScanMessage]) -> HashMap<&'static str, usize> {
        let mut m: HashMap<&'static str, usize> = HashMap::new();
        for msg in msgs {
            let key = match msg {
                ScanMessage::RootStart { .. } => "RootStart",
                ScanMessage::RootDone { .. } => "RootDone",
                ScanMessage::Dir { .. } => "Dir",
                ScanMessage::File { .. } => "File",
                ScanMessage::Error { .. } => "Error",
                ScanMessage::Cancelled => "Cancelled",
                ScanMessage::Done => "Done",
            };
            *m.entry(key).or_insert(0) += 1;
        }
        m
    }

    // Build a minimal AppConfig stand-in using a real temp dir.
    // Because AppConfig is opaque to this module we use a small helper
    // that exercises the Walker directly with pre-built fields.
    struct FakeConfig {
        roots: Vec<PathBuf>,
        image_exts: HashSet<String>,
        video_exts: HashSet<String>,
    }

    /// A parallel of `Walker::new` that takes our fake config.
    fn make_walker(cfg: &FakeConfig) -> Walker<'_> {
        Walker {
            roots: cfg.roots.clone(),
            image_exts: &cfg.image_exts,
            video_exts: &cfg.video_exts,
            current_inner: None,
            current_index: 0,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn empty_roots_yields_only_done() {
        let cfg = FakeConfig {
            roots: vec![],
            image_exts: exts(&["jpg"]),
            video_exts: exts(&["mp4"]),
        };
        let mut w = make_walker(&cfg);
        let msgs = drain(&mut w);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], ScanMessage::Done));
    }

    #[test]
    fn single_root_emits_root_start_and_root_done() {
        let dir = tempfile::tempdir().unwrap();
        // Write one image and one "other" file.
        std::fs::write(dir.path().join("a.jpg"), b"img").unwrap();
        std::fs::write(dir.path().join("b.txt"), b"txt").unwrap();

        let cfg = FakeConfig {
            roots: vec![dir.path().to_path_buf()],
            image_exts: exts(&["jpg"]),
            video_exts: exts(&["mp4"]),
        };
        let mut w = make_walker(&cfg);
        let msgs = drain(&mut w);
        let c = counts(&msgs);

        assert_eq!(c.get("RootStart").copied().unwrap_or(0), 1, "one RootStart");
        assert_eq!(c.get("RootDone").copied().unwrap_or(0), 1, "one RootDone");
        assert_eq!(c.get("Done").copied().unwrap_or(0), 1, "one Done");
        // The root dir itself is yielded as a Dir entry by the ignore crate.
        assert!(c.get("Dir").copied().unwrap_or(0) >= 1, "at least one Dir");
        // Two files.
        assert_eq!(c.get("File").copied().unwrap_or(0), 2, "two File entries");
        assert_eq!(c.get("Error").copied().unwrap_or(0), 0, "no errors");
    }

    #[test]
    fn files_are_correctly_classified() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("photo.JPG"), b"").unwrap();
        std::fs::write(dir.path().join("clip.mp4"), b"").unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"").unwrap();

        let cfg = FakeConfig {
            roots: vec![dir.path().to_path_buf()],
            image_exts: exts(&["jpg"]),
            video_exts: exts(&["mp4"]),
        };
        let mut w = make_walker(&cfg);
        let msgs = drain(&mut w);

        let mut image_count = 0usize;
        let mut video_count = 0usize;
        let mut other_count = 0usize;

        for msg in &msgs {
            if let ScanMessage::File { file } = msg {
                match file.kind() {
                    FileKind::Image => image_count += 1,
                    FileKind::Video => video_count += 1,
                    FileKind::Other => other_count += 1,
                }
            }
        }

        assert_eq!(image_count, 1, "one image");
        assert_eq!(video_count, 1, "one video");
        assert_eq!(other_count, 1, "one other");
    }

    #[test]
    fn subdirectory_emits_dir_message() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("img.png"), b"").unwrap();

        let cfg = FakeConfig {
            roots: vec![dir.path().to_path_buf()],
            image_exts: exts(&["png"]),
            video_exts: exts(&[]),
        };
        let mut w = make_walker(&cfg);
        let msgs = drain(&mut w);
        let c = counts(&msgs);

        // Root dir + subdir = at least 2 Dir messages.
        assert!(
            c.get("Dir").copied().unwrap_or(0) >= 2,
            "root dir and subdir both emitted as Dir"
        );
        assert_eq!(c.get("File").copied().unwrap_or(0), 1, "one file");
    }

    #[test]
    fn multiple_roots_emit_correct_bookends() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        std::fs::write(dir1.path().join("a.jpg"), b"").unwrap();
        std::fs::write(dir2.path().join("b.jpg"), b"").unwrap();

        let cfg = FakeConfig {
            roots: vec![dir1.path().to_path_buf(), dir2.path().to_path_buf()],
            image_exts: exts(&["jpg"]),
            video_exts: exts(&[]),
        };
        let mut w = make_walker(&cfg);
        let msgs = drain(&mut w);
        let c = counts(&msgs);

        assert_eq!(c.get("RootStart").copied().unwrap_or(0), 2, "two RootStart");
        assert_eq!(c.get("RootDone").copied().unwrap_or(0), 2, "two RootDone");
        assert_eq!(c.get("Done").copied().unwrap_or(0), 1, "one Done");
        assert_eq!(c.get("File").copied().unwrap_or(0), 2, "two files total");
    }

    #[test]
    fn message_order_root_start_before_root_done() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.mp4"), b"").unwrap();

        let cfg = FakeConfig {
            roots: vec![dir.path().to_path_buf()],
            image_exts: exts(&[]),
            video_exts: exts(&["mp4"]),
        };
        let mut w = make_walker(&cfg);
        let msgs = drain(&mut w);

        let start_pos = msgs
            .iter()
            .position(|m| matches!(m, ScanMessage::RootStart { .. }))
            .expect("RootStart missing");
        let done_pos = msgs
            .iter()
            .position(|m| matches!(m, ScanMessage::RootDone { .. }))
            .expect("RootDone missing");

        assert!(start_pos < done_pos, "RootStart must precede RootDone");
    }

    #[test]
    fn done_is_last_message() {
        let dir = tempfile::tempdir().unwrap();

        let cfg = FakeConfig {
            roots: vec![dir.path().to_path_buf()],
            image_exts: exts(&[]),
            video_exts: exts(&[]),
        };
        let mut w = make_walker(&cfg);
        let msgs = drain(&mut w);

        assert!(
            matches!(msgs.last(), Some(ScanMessage::Done)),
            "Done must be the final message"
        );
    }

    // ------------------------- Bus -------------------------

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum MockMsg {
        Val(u32),
        End,
    }

    impl Message for MockMsg {
        fn is_terminal(&self) -> bool {
            matches!(self, Self::End)
        }
    }

    struct MockProducer {
        items: Vec<MockMsg>,
        pos: usize,
    }

    impl Producer for MockProducer {
        type Message = MockMsg;

        fn produce(&mut self) -> Self::Message {
            let val = self.items[self.pos];
            self.pos += 1;
            val
        }
    }

    struct MockConsumer {
        seen: Arc<std::sync::Mutex<Vec<MockMsg>>>,
    }

    impl Consumer for MockConsumer {
        type Message = MockMsg;

        fn consume(&mut self, message: &Self::Message) -> Result<(), MementoError> {
            self.seen
                .lock()
                .map_err(|e| MementoError::walk(PathBuf::new(), &e))?
                .push(*message);
            Ok(())
        }
    }

    #[test]
    fn bus_delivers_to_both_consumers() {
        let seen1 = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen2 = Arc::new(std::sync::Mutex::new(Vec::new()));

        let producer = MockProducer {
            items: vec![
                MockMsg::Val(1),
                MockMsg::Val(2),
                MockMsg::Val(3),
                MockMsg::End,
            ],
            pos: 0,
        };
        let c1 = MockConsumer {
            seen: seen1.clone(),
        };
        let c2 = MockConsumer {
            seen: seen2.clone(),
        };

        let handle = Bus::new(producer, c1, c2).run();
        handle.join().unwrap().unwrap();

        let expected = vec![
            MockMsg::Val(1),
            MockMsg::Val(2),
            MockMsg::Val(3),
            MockMsg::End,
        ];
        assert_eq!(*seen1.lock().unwrap(), expected);
        assert_eq!(*seen2.lock().unwrap(), expected);
    }
}
