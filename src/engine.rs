use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_SEQUENTIAL_SCAN;

#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub total_bytes: u64,
    pub copied_bytes: u64,
    pub total_files: usize,
    pub copied_files: usize,
    pub current_file: String,
    pub bytes_per_sec: f64,
    pub elapsed_secs: f64,
    pub is_finished: bool,
    pub error: Option<String>,
}

pub struct TransferStats {
    pub total_bytes: Arc<AtomicU64>,
    pub copied_bytes: Arc<AtomicU64>,
    pub total_files: Arc<AtomicU64>,
    pub copied_files: Arc<AtomicU64>,
    pub cancel_flag: Arc<AtomicBool>,
}

impl TransferStats {
    pub fn new() -> Self {
        Self {
            total_bytes: Arc::new(AtomicU64::new(0)),
            copied_bytes: Arc::new(AtomicU64::new(0)),
            total_files: Arc::new(AtomicU64::new(0)),
            copied_files: Arc::new(AtomicU64::new(0)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct FastCopier {
    pub buffer_size: usize,
    pub is_move: bool,
}

impl Default for FastCopier {
    fn default() -> Self {
        Self {
            buffer_size: 8 * 1024 * 1024, // 8MB buffer
            is_move: false,
        }
    }
}

pub struct ScanItem {
    pub src_path: PathBuf,
    pub rel_path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
}

impl FastCopier {
    pub fn scan_sources(sources: &[PathBuf]) -> Vec<ScanItem> {
        let mut items = Vec::new();
        for src in sources {
            if !src.exists() {
                continue;
            }
            let base_parent = src.parent().unwrap_or(src);
            Self::collect_recursive(src, base_parent, &mut items);
        }
        items
    }

    fn collect_recursive(current: &Path, base: &Path, items: &mut Vec<ScanItem>) {
        let rel_path = match current.strip_prefix(base) {
            Ok(p) => p.to_path_buf(),
            Err(_) => PathBuf::from(current.file_name().unwrap_or_default()),
        };

        if current.is_dir() {
            items.push(ScanItem {
                src_path: current.to_path_buf(),
                rel_path: rel_path.clone(),
                size: 0,
                is_dir: true,
            });

            if let Ok(entries) = fs::read_dir(current) {
                for entry in entries.flatten() {
                    Self::collect_recursive(&entry.path(), base, items);
                }
            }
        } else {
            let size = fs::metadata(current).map(|m| m.len()).unwrap_or(0);
            items.push(ScanItem {
                src_path: current.to_path_buf(),
                rel_path,
                size,
                is_dir: false,
            });
        }
    }

    pub fn execute<F>(
        &self,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        stats: Arc<TransferStats>,
        mut on_progress: F,
    ) where
        F: FnMut(TransferProgress),
    {
        let items = Self::scan_sources(&sources);

        let mut total_bytes = 0u64;
        let mut total_files = 0usize;

        for item in &items {
            if !item.is_dir {
                total_bytes += item.size;
                total_files += 1;
            }
        }

        stats.total_bytes.store(total_bytes, Ordering::Relaxed);
        stats.total_files.store(total_files as u64, Ordering::Relaxed);

        let start_time = Instant::now();
        let mut last_sample_time = Instant::now();
        let mut last_copied_bytes = 0u64;
        let mut current_speed = 0.0f64;

        for item in &items {
            if stats.cancel_flag.load(Ordering::Relaxed) {
                on_progress(TransferProgress {
                    total_bytes,
                    copied_bytes: stats.copied_bytes.load(Ordering::Relaxed),
                    total_files,
                    copied_files: stats.copied_files.load(Ordering::Relaxed) as usize,
                    current_file: "Cancelled by user".into(),
                    bytes_per_sec: 0.0,
                    elapsed_secs: start_time.elapsed().as_secs_f64(),
                    is_finished: true,
                    error: Some("Operation cancelled".into()),
                });
                return;
            }

            let target_path = destination.join(&item.rel_path);

            if item.is_dir {
                if let Err(e) = fs::create_dir_all(&target_path) {
                    on_progress(TransferProgress {
                        total_bytes,
                        copied_bytes: stats.copied_bytes.load(Ordering::Relaxed),
                        total_files,
                        copied_files: stats.copied_files.load(Ordering::Relaxed) as usize,
                        current_file: item.src_path.to_string_lossy().to_string(),
                        bytes_per_sec: current_speed,
                        elapsed_secs: start_time.elapsed().as_secs_f64(),
                        is_finished: true,
                        error: Some(format!("Failed to create folder {:?}: {}", target_path, e)),
                    });
                    return;
                }
            } else {
                if let Some(parent) = target_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                let file_name = item.src_path.file_name().unwrap_or_default().to_string_lossy().to_string();

                let res = self.copy_pipelined_file(
                    &item.src_path,
                    &target_path,
                    item.size,
                    &stats,
                    &mut |copied_so_far| {
                        let now = Instant::now();
                        let elapsed = now.duration_since(last_sample_time).as_secs_f64();
                        if elapsed >= 0.15 {
                            let delta_bytes = copied_so_far.saturating_sub(last_copied_bytes);
                            current_speed = (delta_bytes as f64) / elapsed;
                            last_sample_time = now;
                            last_copied_bytes = copied_so_far;

                            on_progress(TransferProgress {
                                total_bytes,
                                copied_bytes: copied_so_far,
                                total_files,
                                copied_files: stats.copied_files.load(Ordering::Relaxed) as usize,
                                current_file: file_name.clone(),
                                bytes_per_sec: current_speed,
                                elapsed_secs: start_time.elapsed().as_secs_f64(),
                                is_finished: false,
                                error: None,
                            });
                        }
                    },
                );

                if let Err(e) = res {
                    on_progress(TransferProgress {
                        total_bytes,
                        copied_bytes: stats.copied_bytes.load(Ordering::Relaxed),
                        total_files,
                        copied_files: stats.copied_files.load(Ordering::Relaxed) as usize,
                        current_file: file_name,
                        bytes_per_sec: current_speed,
                        elapsed_secs: start_time.elapsed().as_secs_f64(),
                        is_finished: true,
                        error: Some(format!("Error copying {:?}: {}", item.src_path, e)),
                    });
                    return;
                }

                stats.copied_files.fetch_add(1, Ordering::Relaxed);
            }
        }

        // If Move operation was requested, delete sources from deepest first
        if self.is_move && !stats.cancel_flag.load(Ordering::Relaxed) {
            for item in items.iter().rev() {
                if item.is_dir {
                    let _ = fs::remove_dir(&item.src_path);
                } else {
                    let _ = fs::remove_file(&item.src_path);
                }
            }
        }

        let total_elapsed = start_time.elapsed().as_secs_f64().max(0.001);
        let final_speed = (total_bytes as f64) / total_elapsed;

        on_progress(TransferProgress {
            total_bytes,
            copied_bytes: total_bytes,
            total_files,
            copied_files: total_files,
            current_file: "Completed Successfully!".into(),
            bytes_per_sec: final_speed,
            elapsed_secs: total_elapsed,
            is_finished: true,
            error: None,
        });
    }

    /// Pipelined Asynchronous Read/Write (Double-Buffered) with Windows Sequential Scan Hint
    fn copy_pipelined_file<F>(
        &self,
        src: &Path,
        dst: &Path,
        file_len: u64,
        stats: &TransferStats,
        on_chunk: &mut F,
    ) -> std::io::Result<()>
    where
        F: FnMut(u64),
    {
        if file_len == 0 {
            File::create(dst)?;
            return Ok(());
        }

        // Windows native hint: FILE_FLAG_SEQUENTIAL_SCAN for optimum OS Read-Ahead caching
        let mut reader = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN)
            .open(src)?;

        let mut writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN)
            .open(dst)?;

        // Pre-allocate space on disk to prevent filesystem fragmentation
        let _ = writer.set_len(file_len);

        // For small files (< 4MB), standard single-pass copy is faster than spawning threads
        if file_len < 4 * 1024 * 1024 {
            let mut buf = vec![0u8; 1024 * 1024];
            loop {
                if stats.cancel_flag.load(Ordering::Relaxed) {
                    return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled"));
                }
                let n = reader.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                writer.write_all(&buf[..n])?;
                let current = stats.copied_bytes.fetch_add(n as u64, Ordering::Relaxed) + (n as u64);
                on_chunk(current);
            }
            let _ = writer.flush();
            let _ = writer.sync_data();
            return Ok(());
        }

        // Double-buffering pipeline: Thread A reads, Thread B writes concurrently
        let (tx_chunks, rx_chunks) = crossbeam_channel::bounded::<Vec<u8>>(3);
        let cancel_flag = Arc::clone(&stats.cancel_flag);

        // Writer background worker
        let write_worker = std::thread::spawn(move || -> std::io::Result<u64> {
            let mut written_total = 0u64;
            while let Ok(chunk) = rx_chunks.recv() {
                if cancel_flag.load(Ordering::Relaxed) {
                    return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled"));
                }
                writer.write_all(&chunk)?;
                written_total += chunk.len() as u64;
            }
            writer.flush()?;
            let _ = writer.sync_data();
            Ok(written_total)
        });

        // Reader loop on calling thread
        let chunk_size = self.buffer_size;
        loop {
            if stats.cancel_flag.load(Ordering::Relaxed) {
                drop(tx_chunks);
                let _ = write_worker.join();
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled"));
            }

            let mut chunk = vec![0u8; chunk_size];
            let mut chunk_filled = 0;

            // Fill full buffer if possible
            while chunk_filled < chunk_size {
                let n = reader.read(&mut chunk[chunk_filled..])?;
                if n == 0 {
                    break;
                }
                chunk_filled += n;
            }

            if chunk_filled == 0 {
                break;
            }

            chunk.truncate(chunk_filled);
            let current = stats.copied_bytes.fetch_add(chunk_filled as u64, Ordering::Relaxed) + (chunk_filled as u64);
            on_chunk(current);

            if tx_chunks.send(chunk).is_err() {
                break;
            }
        }

        drop(tx_chunks); // Signal writer thread EOF

        match write_worker.join() {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(std::io::Error::new(std::io::ErrorKind::Other, "Writer thread panicked")),
        }
    }
}
