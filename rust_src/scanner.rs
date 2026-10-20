//! Non-blocking background library scanner.
//!
//! Walking + parsing runs on a worker thread; progress flows to the UI
//! through an mpsc channel ([`ScanEvent`]). The UI polls the receiver each
//! tick and never blocks.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use walkdir::WalkDir;

use crate::library::Library;
use crate::models::Track;

pub const SUPPORTED_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "ogg", "opus", "aac", "alac", "wma",
];

/// Result handed over from the worker thread when the scan completes.
pub struct ScanPayload {
    pub tracks: Vec<Track>,
    /// Raw .m3u/.m3u8 files as (path, contents); resolved into playlists by
    /// the main thread (needs the final track list for path matching).
    pub m3us: Vec<(PathBuf, String)>,
}

pub enum ScanEvent {
    /// Directory-walk heartbeat: `dirs` directories visited, `files`
    /// supported audio/playlist files collected so far. Emitted every few
    /// hundred entries so the UI shows life during the (potentially slow)
    /// walking phase instead of a frozen `0/0`.
    Walking { dirs: usize, files: usize },
    /// Directory walk finished; total audio files to parse.
    Listed(usize),
    Progress {
        files_done: usize,
        files_total: usize,
        tracks_found: usize,
        current: String,
    },
    Finished(Box<ScanPayload>),
}

pub struct ScanJob {
    pub receiver: Receiver<ScanEvent>,
    cancel: Arc<AtomicBool>,
}

impl ScanJob {
    /// Spawn a worker thread scanning `folder` recursively.
    pub fn start(folder: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_worker = cancel.clone();

                std::thread::Builder::new()
                    .name("walkz-library-scan".into())
                    .spawn(move || {
                        // Heartbeat at most every ~120 ms so the walk phase
                        // stays alive in the UI without flooding the channel;
                        // the first entry always reports so the overlay shows
                        // the walk has started even on fast, local disks.
                        let stride = std::time::Duration::from_millis(120);
                        let mut last_sent = std::time::Instant::now() - stride * 2;
                        let (audio_paths, m3u_paths) =
                            collect_files_with_progress(&folder, &cancel_worker, |dirs, files| {
                                let now = std::time::Instant::now();
                                if now.duration_since(last_sent) >= stride {
                                    last_sent = now;
                                    let _ = tx.send(ScanEvent::Walking { dirs, files });
                                }
                            });
                        if cancel_worker.load(Ordering::Relaxed) {
                            return;
                        }
                        let m3us = read_m3us(m3u_paths);

                        let files_total = audio_paths.len();
                        let _ = tx.send(ScanEvent::Listed(files_total));

                        let tracks = parse_tracks(&audio_paths, &folder, &cancel_worker, &|files_done, files_total, tracks_found, current| {
                            let _ = tx.send(ScanEvent::Progress {
                                files_done,
                                files_total,
                                tracks_found,
                                current,
                            });
                        });

                        if cancel_worker.load(Ordering::Relaxed) {
                            return;
                        }
                        let _ = tx.send(ScanEvent::Finished(Box::new(ScanPayload { tracks, m3us })));
                    })
                    .expect("failed to spawn scan worker");

        ScanJob {
            receiver: rx,
            cancel,
        }
    }

    /// Ask the worker to stop early (checked between directory-walk entries
    /// and between files during parsing, so large libraries abort promptly).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Assemble a job from an existing channel (tests inject synthetic
    /// worker lifecycles, e.g. a worker that dies without `Finished`).
    #[allow(dead_code)]
    pub fn from_parts(receiver: Receiver<ScanEvent>, cancel: Arc<AtomicBool>) -> Self {
        Self { receiver, cancel }
    }
}

/// Recursively collect supported audio files and playlist files. Stops
/// early when `cancel` is set (returning what has been gathered so far).
pub fn collect_files(folder: &Path, cancel: &AtomicBool) -> (Vec<PathBuf>, Vec<PathBuf>) {
    collect_files_with_progress(folder, cancel, |_, _| {})
}

/// Same as [`collect_files`], reporting walk progress as
/// `(directories_visited, files_collected_so_far)` along the way.
pub fn collect_files_with_progress(
    folder: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize),
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut audio_files = Vec::new();
    let mut playlist_files = Vec::new();
    let mut dirs_seen = 0usize;

    for entry in WalkDir::new(folder)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let p = entry.path();
        if p.is_dir() {
            dirs_seen += 1;
        } else if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()) {
                if SUPPORTED_EXTS.contains(&ext.as_str()) {
                    audio_files.push(p.to_path_buf());
                } else if ext == "m3u" || ext == "m3u8" {
                    playlist_files.push(p.to_path_buf());
                }
            }
        }
        progress(dirs_seen, audio_files.len() + playlist_files.len());
    }

    audio_files.sort();
    (audio_files, playlist_files)
}

/// Parse audio files into tracks, reporting progress as
/// `(files_done, files_total, tracks_found, current_file_name)`.
/// Stops early when `cancel` is set (returning the tracks so far).
///
/// Parsing runs on several worker threads (each metadata read — tag parse,
/// media probe — is I/O-bound, and on network/WSL mounts spawning `ffprobe`
/// per file dominates scan time). Files are split into contiguous chunks so
/// the returned track order matches the (sorted) input order.
pub fn parse_tracks(
    paths: &[PathBuf],
    base_folder: &Path,
    cancel: &AtomicBool,
    progress: &(impl Fn(usize, usize, usize, String) + Sync),
) -> Vec<Track> {
    let files_total = paths.len();
    if files_total == 0 {
        return Vec::new();
    }

    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 6)
        .min(files_total);

    // Sequential path for tiny libraries / single-core hosts.
    if workers <= 1 {
        let mut tracks = Vec::new();
        for (i, path) in paths.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let current = file_display_name(path);
            if let Some(track) = Library::parse_audio_file(path, base_folder) {
                tracks.push(track);
            }
            progress(i + 1, files_total, tracks.len(), current);
        }
        return tracks;
    }

    let done = AtomicUsize::new(0);
    let found = AtomicUsize::new(0);

    // Contiguous chunks (ceil division); thread i owns exactly one chunk.
    let chunk_len = files_total.div_ceil(workers);
    let mut results: Vec<Option<Vec<Track>>> = vec![None; workers];

    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for chunk in paths.chunks(chunk_len) {
            let base = base_folder;
            let cancel = &cancel;
            let done = &done;
            let found = &found;
            let progress = &progress;
            handles.push(scope.spawn(move || {
                let mut local = Vec::with_capacity(chunk.len());
                for path in chunk {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(track) = Library::parse_audio_file(path, base) {
                        local.push(track);
                        found.fetch_add(1, Ordering::Relaxed);
                    }
                    let current = file_display_name(path);
                    let files_done = done.fetch_add(1, Ordering::Relaxed) + 1;
                    progress(
                        files_done,
                        files_total,
                        found.load(Ordering::Relaxed),
                        current,
                    );
                }
                local
            }));
        }
        for (slot, handle) in results.iter_mut().zip(handles) {
            *slot = Some(handle.join().unwrap_or_default());
        }
    });

    let mut tracks = Vec::with_capacity(files_total);
    for chunk in results.into_iter().flatten() {
        tracks.extend(chunk);
    }
    tracks
}

fn file_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string()
}

/// Read playlist files into (path, contents) pairs.
pub fn read_m3us(paths: Vec<PathBuf>) -> Vec<(PathBuf, String)> {
    paths
        .into_iter()
        .filter_map(|p| {
            let content = fs::read_to_string(&p).ok()?;
            Some((p, content))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tmp tree:
    ///   root/My Artist/My Album/01 - Song One.mp3  (junk bytes; parser falls back)
    ///   root/My Artist/My Album/02 - Song Two.mp3
    ///   root/pl.m3u                                (references both)
    fn make_tmp_tree(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("walkz-scanner-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let album_dir = root.join("My Artist").join("My Album");
        fs::create_dir_all(&album_dir).unwrap();
        fs::write(album_dir.join("01 - Song One.mp3"), b"junk-not-audio").unwrap();
        fs::write(album_dir.join("02 - Song Two.mp3"), b"junk-not-audio").unwrap();
        fs::write(
            root.join("pl.m3u"),
            "My Artist/My Album/01 - Song One.mp3\nMy Artist/My Album/02 - Song Two.mp3\n",
        )
        .unwrap();
        root
    }

    fn cleanup(root: &Path) {
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn collect_files_finds_audio_and_m3u() {
        let root = make_tmp_tree("collect");
        let cancel = AtomicBool::new(false);
        let (audio, m3us) = collect_files(&root, &cancel);
        assert_eq!(audio.len(), 2);
        assert_eq!(m3us.len(), 1);
        assert!(audio.iter().all(|p| p.extension().unwrap() == "mp3"));
        cleanup(&root);
    }

    #[test]
    fn collect_files_reports_walk_progress() {
        let root = make_tmp_tree("collect-progress");
        let cancel = AtomicBool::new(false);
        let mut events = Vec::new();
        let (_, m3us) = collect_files_with_progress(&root, &cancel, |dirs, files| {
            events.push((dirs, files));
        });
        assert_eq!(m3us.len(), 1);
        // The walk visited the root + subdirectories and counted all files.
        let (last_dirs, last_files) = *events.last().unwrap();
        assert!(last_dirs >= 3, "root, artist dir, album dir visited");
        assert_eq!(last_files, 3, "2 audio + 1 playlist counted");
        assert!(events.len() > 1, "progress reported during the walk");
        cleanup(&root);
    }

    #[test]
    fn parse_tracks_reports_progress_and_uses_filename_heuristics() {
        let root = make_tmp_tree("parse");
        let cancel = AtomicBool::new(false);
        let (audio, _) = collect_files(&root, &cancel);
        let events = std::sync::Mutex::new(Vec::new());
        let tracks = parse_tracks(&audio, &root, &cancel, &|done, total, found, current| {
            events.lock().unwrap().push((done, total, found, current));
        });
        assert_eq!(tracks.len(), 2);
        // artist/album derived from folder structure
        assert!(tracks.iter().all(|t| t.artist == "My Artist"));
        assert!(tracks.iter().all(|t| t.album == "My Album"));
        // progress advanced to completion
        let events = events.into_inner().unwrap();
        let last = events.last().unwrap();
        assert_eq!(last.0, 2); // files_done
        assert_eq!(last.1, 2); // files_total
        assert_eq!(last.2, 2); // tracks_found
        cleanup(&root);
    }

    /// Parallel parsing must preserve the input (sorted) order.
    #[test]
    fn parse_tracks_parallel_preserves_order() {
        let root = std::env::temp_dir().join(format!("walkz-scanner-order-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut paths = Vec::new();
        for i in 0..40 {
            let p = root.join(format!("{i:03}.mp3"));
            fs::write(&p, b"junk-not-audio").unwrap();
            paths.push(p);
        }
        let cancel = AtomicBool::new(false);
        let tracks = parse_tracks(&paths, &root, &cancel, &|_, _, _, _| {});
        assert_eq!(tracks.len(), 40);
        for (i, track) in tracks.iter().enumerate() {
            let name = track.path.file_stem().unwrap().to_str().unwrap();
            assert_eq!(name, format!("{i:03}"), "order must match input");
        }
        cleanup(&root);
    }

    #[test]
    fn read_m3us_returns_contents() {
        let root = make_tmp_tree("m3u");
        let cancel = AtomicBool::new(false);
        let (_, m3u_paths) = collect_files(&root, &cancel);
        let m3us = read_m3us(m3u_paths);
        assert_eq!(m3us.len(), 1);
        let content = &m3us[0].1;
        assert!(content.contains("Song One"));
        cleanup(&root);
    }

    #[test]
    fn scan_job_delivers_listed_progress_finished() {
        let root = make_tmp_tree("job");
        let job = ScanJob::start(root.clone());
        let mut saw_listed = None;
        let mut progress_count = 0usize;
        let mut finished = None;
        while finished.is_none() {
            match job.receiver.recv_timeout(std::time::Duration::from_secs(30)).unwrap() {
                ScanEvent::Walking { .. } => {}
                ScanEvent::Listed(total) => saw_listed = Some(total),
                ScanEvent::Progress { .. } => progress_count += 1,
                ScanEvent::Finished(payload) => finished = Some(*payload),
            }
        }
        assert_eq!(saw_listed, Some(2));
        assert_eq!(progress_count, 2);
        let payload = finished.unwrap();
        assert_eq!(payload.tracks.len(), 2);
        assert_eq!(payload.m3us.len(), 1);
        cleanup(&root);
    }

    /// The walking phase must emit heartbeats so the UI never shows a dead
    /// `0/0` while directories are still being visited. The tiny test tree
    /// finishes the walk in one heartbeat window, so force heartbeats by
    /// scanning a deeper tree.
    #[test]
    fn scan_job_emits_walking_heartbeats_on_large_tree() {
        let root = std::env::temp_dir().join(format!("walkz-scanner-walk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        // 40 dirs × 8 files = 360 walk entries > the 256-entry heartbeat stride.
        for d in 0..40 {
            let dir = root.join(format!("d{d:02}"));
            fs::create_dir_all(&dir).unwrap();
            for f in 0..8 {
                fs::write(dir.join(format!("t{f}.mp3")), b"junk").unwrap();
            }
        }
        let job = ScanJob::start(root.clone());
        let mut walking = 0usize;
        let mut saw_listed = false;
        while !saw_listed {
            match job.receiver.recv_timeout(std::time::Duration::from_secs(30)).unwrap() {
                ScanEvent::Walking { dirs, .. } => {
                    walking += 1;
                    assert!(dirs > 0, "heartbeat carries a directory count");
                }
                ScanEvent::Listed(_) => saw_listed = true,
                ScanEvent::Progress { .. } => panic!("no progress before Listed"),
                ScanEvent::Finished(_) => panic!("no finished before Listed"),
            }
        }
        assert!(walking > 0, "at least one Walking heartbeat before Listed");
        // Drain the rest so the worker exits cleanly.
        loop {
            match job.receiver.recv_timeout(std::time::Duration::from_secs(30)).unwrap() {
                ScanEvent::Finished(_) => break,
                _ => {}
            }
        }
        cleanup(&root);
    }

    // ------------------------------------------------------------------
    // Cancellation (review: Important #2 — checked inside walk and parse)
    // ------------------------------------------------------------------

    #[test]
    fn collect_files_stops_when_cancelled() {
        let root = make_tmp_tree("cancel-collect");
        let cancel = AtomicBool::new(true); // pre-cancelled
        let (audio, m3us) = collect_files(&root, &cancel);
        assert!(audio.is_empty(), "no audio collected after cancel");
        assert!(m3us.is_empty(), "no playlists collected after cancel");
        cleanup(&root);
    }

    #[test]
    fn parse_tracks_stops_when_cancelled() {
        let root = make_tmp_tree("cancel-parse");
        let cancel = AtomicBool::new(false);
        let (audio, _) = collect_files(&root, &cancel);
        assert_eq!(audio.len(), 2);

        let cancel = AtomicBool::new(true); // pre-cancelled before parsing
        let progress_calls = std::sync::Mutex::new(0usize);
        let tracks = parse_tracks(&audio, &root, &cancel, &|_, _, _, _| {
            *progress_calls.lock().unwrap() += 1;
        });
        assert!(tracks.is_empty(), "no tracks parsed after cancel");
        assert_eq!(progress_calls.into_inner().unwrap(), 0, "no progress events after cancel");
        cleanup(&root);
    }

    #[test]
    fn cancelled_scan_job_terminates_promptly() {
        let root = make_tmp_tree("cancel-job");
        let job = ScanJob::start(root.clone());
        job.cancel();

        // The worker must exit on its own (either silently — cancelled
        // before emitting — or having raced partway through a small tree);
        // it must NEVER hang.
        loop {
            match job.receiver.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(_) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    panic!("cancelled scan worker hung instead of exiting")
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        cleanup(&root);
    }
}
