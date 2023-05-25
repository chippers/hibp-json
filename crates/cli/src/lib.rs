use std::{
    ffi::OsStr,
    fs::File,
    io::{stdout, BufRead, BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use anyhow::Result;
use clap::{ArgAction, Parser};
use console::style;
use flate2::Compression;
use indicatif::{ParallelProgressIterator, ProgressStyle};
use mimalloc::MiMalloc;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use serde::Serialize;
use walkdir::WalkDir;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Generate JSON formatted files for HIBP password hash files
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    /// Path to existing hashes
    #[arg(long, default_value = "hashes")]
    hashes: PathBuf,

    /// Path to output to
    #[arg(short, long, default_value = "dist")]
    out: PathBuf,

    /// If the input should be strictly checked
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    strict: bool,

    /// If .gz files should be generated
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    gzip: bool,

    /// If .br files should be generated
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    brotli: bool,

    /// If .json files should be generated
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    json: bool,
}

#[derive(Serialize)]
pub struct Password {
    hash: String,
    count: usize,
}

pub fn walk1(path: impl AsRef<Path>) -> walkdir::IntoIter {
    WalkDir::new(path.as_ref())
        .max_depth(1)
        .min_depth(1)
        .into_iter()
}

pub fn generate_out_structure(out: &Path) -> Result<()> {
    #[rustfmt::skip]
    let hex = [
        "0", "1", "2", "3", "4", "5", "6", "7",
        "8", "9", "A", "B", "C", "D", "E", "F",
    ];

    for c1 in hex {
        let p1 = out.join(c1);
        for c2 in hex {
            let p2 = p1.join(c2);
            for c3 in hex {
                let p3 = p2.join(c3);
                for c4 in hex {
                    std::fs::create_dir_all(p3.join(c4))?;
                }
            }
        }
    }

    Ok(())
}

pub fn flush() -> Result<()> {
    Ok(stdout().lock().flush()?)
}

pub fn ensure_output_directories(dist: &Path) -> Result<()> {
    print!(
        "{} Ensuring 65,536 output directories",
        style("[1/3]").bold().dim()
    );
    flush()?;

    let start = Instant::now();
    generate_out_structure(dist)?;

    println!(
        "\r{} Ensured 65,536 output directories in {}ms",
        style("[1/3]").bold().dim(),
        style(start.elapsed().as_millis()).bold()
    );

    Ok(())
}

pub fn find_all_hash_files(hashes: &Path) -> Result<Vec<PathBuf>> {
    print!(
        "{} Finding all hash files in {}",
        style("[2/3]").bold().dim(),
        style(hashes.display()).bold()
    );
    flush()?;

    let start = Instant::now();
    let paths =
        walk1(hashes).try_fold(Vec::with_capacity(1_048_576), |mut acc: Vec<_>, item| {
            acc.push(item?.into_path());
            Ok::<_, walkdir::Error>(acc)
        })?;

    println!(
        "\r{} Found {} hash files in {} in {}ms",
        style("[2/3]").bold().dim(),
        style(paths.len()).bold(),
        style(hashes.display()).bold(),
        style(start.elapsed().as_millis()).bold(),
    );

    Ok(paths)
}

pub fn progress_style() -> ProgressStyle {
    ProgressStyle::with_template("{elapsed} {bar} {percent}% eta {eta} {per_sec} ")
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏  ")
}

pub fn format_prefix_to_dirs(prefix: &str) -> String {
    prefix
        .char_indices()
        .fold(String::with_capacity(9), |mut acc, (i, c)| {
            if i > 0 {
                acc.push('/');
            }
            acc.push(c);
            acc
        })
}

pub fn run() -> Result<()> {
    let very_start = Instant::now();
    let args = Args::parse();

    ensure_output_directories(&args.out)?;
    let paths = find_all_hash_files(&args.hashes)?;
    let count = paths.len() as u64;

    if args.strict {
        // HIBP has every single 5 character prefix of sha1
        assert!(count == 16_u64.pow(5));
    }

    let (json, brotli, gzip) = (args.json, args.brotli, args.gzip);

    println!(
        "{} Generating{}{}{} files ",
        style("[3/3]").bold().dim(),
        if json { " .json" } else { "" },
        if brotli { " .br" } else { "" },
        if gzip { " .gz" } else { "" }
    );

    let dist = args.out.as_path();

    let start = Instant::now();

    let total_json = AtomicU64::new(0);
    let total_gz = AtomicU64::new(0);
    let total_br = AtomicU64::new(0);

    paths
        .into_par_iter()
        .progress_with_style(progress_style())
        .for_each(|path| {
            let prefix = path.file_stem().and_then(OsStr::to_str).unwrap();
            let mut passwords = Vec::with_capacity(2048);

            let content = std::fs::read(&path).unwrap();
            for line in content.lines().map(Result::unwrap) {
                let mut hash = String::with_capacity(40);
                let (h, c) = line.split_once(':').unwrap();
                let count = c.parse().unwrap();
                hash.push_str(prefix);
                hash.push_str(h);
                passwords.push(Password { hash, count });
            }

            let prefix = format_prefix_to_dirs(prefix);

            let serialized = serde_json::to_vec(&passwords).unwrap();

            if json {
                let content = serialized.as_slice();
                std::fs::write(dist.join(format!("{prefix}.json")), content).unwrap();
                total_json.fetch_add(content.len() as u64, Ordering::SeqCst);
            }

            if gzip {
                let file = File::create(dist.join(format!("{prefix}.json.gz"))).unwrap();
                let mut buf: BufWriter<File> = BufWriter::new(file);
                let mut enc = flate2::write::GzEncoder::new(&mut buf, Compression::best());
                enc.write_all(&serialized).unwrap();
                enc.finish().unwrap();

                let f = buf.into_inner().unwrap();
                let size = f.metadata().unwrap().len();
                total_gz.fetch_add(size, Ordering::SeqCst);
            }

            if brotli {
                let mut serialized = std::io::Cursor::new(serialized);
                let file = File::create(dist.join(format!("{prefix}.json.br"))).unwrap();
                let mut buf: BufWriter<File> = BufWriter::new(file);
                let size = brotli::BrotliCompress(
                    &mut serialized,
                    &mut buf,
                    &brotli::enc::BrotliEncoderInitParams(),
                )
                .unwrap();
                total_br.fetch_add(size as u64, Ordering::SeqCst);
            }
        });

    println!(
        "Finished generating files in {}ms ({}ms total)",
        style(start.elapsed().as_millis()).bold(),
        very_start.elapsed().as_millis()
    );

    println!(
        "Bytes: json {} | br {} | gz {}",
        total_json.into_inner(),
        total_br.into_inner(),
        total_gz.into_inner()
    );

    Ok(())
}
