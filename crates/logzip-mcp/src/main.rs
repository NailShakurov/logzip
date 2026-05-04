mod mcp;
mod sandbox;
mod tools;

use std::path::PathBuf;
use sandbox::Sandbox;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match cmd {
        "compress"   => cmd_compress(&args[2..]),
        "decompress" => cmd_decompress(&args[2..]),
        "mcp"        => cmd_mcp(&args[2..]),
        "--version" | "-V" => {
            println!("logzip {}", env!("CARGO_PKG_VERSION"));
        }
        _ => {
            eprintln!("logzip {}", env!("CARGO_PKG_VERSION"));
            eprintln!("Usage:");
            eprintln!("  logzip compress   -i <file> [-o <file>] [--quality fast|balanced|max] [--bpe-passes N]");
            eprintln!("                    [--preamble] [--stats] [--preserve-ids] [--preserve-pattern <regex>]... [--debug]");
            eprintln!("  logzip decompress -i <file> [-o <file>]");
            eprintln!("  logzip mcp        [--allow-dir <dir>]...");
            if cmd != "help" && cmd != "--help" && cmd != "-h" {
                std::process::exit(1);
            }
        }
    }
}

fn cmd_compress(args: &[String]) {
    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut quality = "balanced";
    let mut bpe_passes: Option<usize> = None;
    let mut preamble = false;
    let mut stats = false;
    let mut preserve_ids = false;
    let mut preserve_patterns: Vec<String> = Vec::new();
    let mut debug = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input"       => { i += 1; input_path  = args.get(i).cloned(); }
            "-o" | "--output"      => { i += 1; output_path = args.get(i).cloned(); }
            "--quality"            => { i += 1; if let Some(q) = args.get(i) { quality = Box::leak(q.clone().into_boxed_str()); } }
            "--bpe-passes"         => { i += 1; bpe_passes = args.get(i).and_then(|s| s.parse().ok()); }
            "--preamble"           => { preamble = true; }
            "--stats"              => { stats = true; }
            "--preserve-ids"       => { preserve_ids = true; }
            "--preserve-pattern"   => { i += 1; if let Some(p) = args.get(i) { preserve_patterns.push(p.clone()); } }
            "--debug"              => { debug = true; }
            _ => {}
        }
        i += 1;
    }

    let (max_legend, mut passes) = match quality {
        "max"      => (512usize, 2usize),
        "balanced" => (128, 1),
        _          => (32, 1),
    };
    if let Some(p) = bpe_passes { passes = p; }

    let text = match input_path {
        Some(ref p) => std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", p, e);
            std::process::exit(1);
        }),
        None => {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).unwrap();
            s
        }
    };

    let preserve_cfg = (preserve_ids || !preserve_patterns.is_empty()).then(|| {
        logzip_core::PreserveConfig { preserve_ids, extra_patterns: preserve_patterns }
    });
    let result = logzip_core::compress(&text, 2, max_legend, true, None, true, passes, preserve_cfg.as_ref());
    let output = result.render(preamble);

    match output_path {
        Some(ref p) => std::fs::write(p, &output).unwrap_or_else(|e| {
            eprintln!("Error writing {}: {}", p, e);
            std::process::exit(1);
        }),
        None => print!("{}", output),
    }

    if stats {
        let s = &result.stats;
        let orig  = s.get("original_chars").map(|v| v.as_str()).unwrap_or("?");
        let comp  = s.get("compressed_chars").map(|v| v.as_str()).unwrap_or("?");
        let ratio = s.get("ratio_pct").map(|v| v.as_str()).unwrap_or("?");
        let preserved = s.get("preserved_candidates").map(|v| v.as_str()).unwrap_or("0");
        eprintln!("[logzip] {} → {} chars ({}% saved) | preserved={}", orig, comp, ratio, preserved);
    }
    if debug {
        let preserved = result.stats.get("preserved_candidates").map(|v| v.as_str()).unwrap_or("0");
        eprintln!("[logzip] debug: preserved_candidates={preserved}");
    }
}

fn cmd_decompress(args: &[String]) {
    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-i" | "--input"  => { i += 1; input_path  = args.get(i).cloned(); }
            "-o" | "--output" => { i += 1; output_path = args.get(i).cloned(); }
            _ => {}
        }
        i += 1;
    }

    let text = match input_path {
        Some(ref p) => std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", p, e);
            std::process::exit(1);
        }),
        None => {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).unwrap();
            s
        }
    };

    let output = logzip_core::decompress(&text).unwrap_or_else(|e| {
        eprintln!("Decompress error: {}", e);
        std::process::exit(1);
    });

    match output_path {
        Some(ref p) => std::fs::write(p, &output).unwrap_or_else(|e| {
            eprintln!("Error writing {}: {}", p, e);
            std::process::exit(1);
        }),
        None => print!("{}", output),
    }
}

fn cmd_mcp(args: &[String]) {
    let mut allow_dirs: Vec<PathBuf> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        if args[i] == "--allow-dir" {
            i += 1;
            if let Some(dir) = args.get(i) {
                allow_dirs.push(PathBuf::from(dir));
            }
        }
        i += 1;
    }

    if allow_dirs.is_empty() {
        eprintln!("[logzip-mcp] No --allow-dir specified, defaulting to CWD");
    }

    let sandbox = Sandbox::new(allow_dirs).unwrap_or_else(|e| {
        eprintln!("[logzip-mcp] Fatal: {}", e);
        std::process::exit(1);
    });

    mcp::run(sandbox);
}
