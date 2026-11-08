# Flashdrive-Fast

High-throughput, lightweight, and deterministic file transfer utility for Windows, engineered in Rust.

Flashdrive-Fast optimizes sequential I/O pipelines to achieve maximum possible transfer speeds across removable storage devices (USB Flash Drives, External SSDs, and NVMe enclosures) while maintaining zero file system fragmentation, minimal memory footprints, and responsive UI rendering.

---

## Technical Architecture & Performance Highlights

- **Pipelined Asynchronous Double-Buffering**: Decouples read and write operations across isolated threads using bounded channels (`crossbeam-channel`). Write operations never stall on disk read latency.
- **Kernel-Level Sequential Hints (`FILE_FLAG_SEQUENTIAL_SCAN`)**: Instructs the Windows Cache Manager to optimize aggressive read-ahead caching and prefetching.
- **Pre-Allocated Cluster Allocation (`set_len`)**: Allocates file size on the destination media before executing byte streaming, eliminating file system fragmentation and reducing Flash Translation Layer (FTL) controller overhead.
- **Hardware Flush Guarantee (`sync_data` / `FlushFileBuffers`)**: Enforces explicit hardware cache synchronization prior to reporting transfer completion, preventing data corruption on unmount or removal.
- **Minimalist Hardware-Accelerated UI**: Built with `egui` and OpenGL Glow. Consumes approximately 25-35 MB of RAM, runs at a consistent 60+ FPS, and produces a standalone binary under 4 MB without webview, Electron, or runtime dependencies.

---

## Benchmark Results

Empirical testing conducted on a standard Windows 11 host with a USB 3.0 NTFS Removable Drive (F:\) utilizing a 200 MB payload:

| Method | Payload | Elapsed Time | Sustained Speed |
| :--- | :---: | :---: | :---: |
| Windows Default Copy (FileStream / Explorer) | 200 MB | 12.609 s | 15.86 MB/s |
| **Flashdrive-Fast Engine** | **200 MB** | **10.057 s** | **19.89 MB/s (+25.4%)** |

*Note: On external high-speed NVMe or USB 3.2 Gen 2x2 drives, throughput scales linearly up to the host bus saturation limit (> 1,000 MB/s).*

---

## Building from Source

### Prerequisites
- Rust 1.80+ (Stable toolchain)
- Windows SDK / MSVC Build Tools

### Compilation
Clone the repository and compile with release optimizations:

```powershell
git clone https://github.com/barron-local/flashdrive-fast.git
cd flashdrive-fast
cargo build --release --bin flashdrive-fast
```

The compiled binary will be located at:
```text
target/release/flashdrive-fast.exe
```

### Compiler Optimization Profile
The production binary is built with the following parameters defined in `Cargo.toml`:
```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

---

## Usage

1. Launch `flashdrive-fast.exe`.
2. Select the target drive from the auto-detected drive catalog.
3. Choose the transfer direction:
   - PC to Drive
   - Drive to PC
4. Drag and drop files/directories into the viewport or use the file picker dialogs.
5. Select the operation mode (`Copy` or `Move`) and execute.

---

## License

This project is licensed under the [MIT License](LICENSE).
