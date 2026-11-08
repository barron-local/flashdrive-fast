use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use flashdrive_fast::engine::{FastCopier, TransferStats};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: bench <source_file> <dest_dir>");
        std::process::exit(1);
    }

    let src = PathBuf::from(&args[1]);
    let dst = PathBuf::from(&args[2]);

    let stats = Arc::new(TransferStats::new());
    let copier = FastCopier {
        buffer_size: 4 * 1024 * 1024,
        is_move: false,
    };

    let start = Instant::now();
    copier.execute(vec![src.clone()], dst, stats, |_| {});
    let elapsed = start.elapsed().as_secs_f64();

    let meta = fs::metadata(&src).unwrap();
    let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
    let speed = size_mb / elapsed;

    println!("RESULT: elapsed={:.3}s, speed={:.2}MB/s, size={:.2}MB", elapsed, speed, size_mb);
}
