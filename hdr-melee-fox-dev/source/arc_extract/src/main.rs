// Usage: arc_extract <data.arc file OR folder of split parts 00,01,...> <out_dir> <arc path>...
// Extracts each arc path (e.g. item/bumper/model/body/c00/model.numshb) to out_dir/<arc path>.
use smash_arc::{ArcFile, ArcLookup, Hash40, Region};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Reads a sequence of split files (nxdumptool's 00, 01, ... parts) as one stream.
struct SplitReader {
    parts: Vec<(u64, File)>, // (start offset, file)
    total: u64,
    pos: u64,
}

impl SplitReader {
    fn open(dir: &Path) -> std::io::Result<Self> {
        let mut names: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        names.sort();
        let mut parts = Vec::new();
        let mut total = 0;
        for n in names {
            let f = File::open(&n)?;
            let len = f.metadata()?.len();
            parts.push((total, f));
            total += len;
        }
        Ok(Self { parts, total, pos: 0 })
    }
}

impl Read for SplitReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.total {
            return Ok(0);
        }
        let idx = self.parts.iter().rposition(|(start, _)| *start <= self.pos).unwrap();
        let next_start = self.parts.get(idx + 1).map(|p| p.0).unwrap_or(self.total);
        let (start, f) = &mut self.parts[idx];
        f.seek(SeekFrom::Start(self.pos - *start))?;
        let max = ((next_start - self.pos) as usize).min(buf.len());
        let n = f.read(&mut buf[..max])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for SplitReader {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.pos = match from {
            SeekFrom::Start(o) => o,
            SeekFrom::End(o) => (self.total as i64 + o) as u64,
            SeekFrom::Current(o) => (self.pos as i64 + o) as u64,
        };
        Ok(self.pos)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: arc_extract <data.arc or split folder> <out_dir> <arc path>...");
        std::process::exit(1);
    }
    let src = Path::new(&args[1]);
    let arc = if src.is_dir() {
        ArcFile::from_reader(BufReader::new(SplitReader::open(src).expect("failed to open parts")))
    } else {
        ArcFile::open(src)
    }
    .expect("failed to read data.arc");
    // arc_extract <arc> --finddir <file path> [candidate dir names...]
    // Prints which directory (file group) holds the file, and which candidate name matches it.
    if args[2] == "--finddir" {
        let info = arc.get_file_info_from_hash(Hash40::from(args[3].as_str())).expect("file not found");
        let base = arc.get_file_infos().as_ptr();
        let idx = (info as *const _ as usize - base as usize) / std::mem::size_of_val(info);
        for d in arc.get_dir_infos() {
            let start = d.file_info_start_index as usize;
            if idx >= start && idx < start + d.file_count as usize {
                let path = d.path.hash40();
                println!("dir path {:#x} name {:#x} parent {:#x} files {}", path.as_u64(), d.name.as_u64(), d.parent.as_u64(), d.file_count);
                for c in &args[4..] {
                    let h = Hash40::from(c.as_str());
                    if h == path || h == d.name || h == d.parent {
                        println!("  matches '{}'", c);
                    }
                }
            }
        }
        return;
    }
    let out = Path::new(&args[2]);
    let mut failed = 0;
    for p in &args[3..] {
        match arc.get_file_contents(Hash40::from(p.as_str()), Region::UsEnglish) {
            Ok(data) => {
                let dest = out.join(p);
                std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
                std::fs::write(&dest, &data).unwrap();
                println!("ok   {} ({} bytes)", p, data.len());
            }
            Err(e) => {
                failed += 1;
                println!("FAIL {} ({:?})", p, e);
            }
        }
    }
    std::process::exit(if failed > 0 { 2 } else { 0 });
}
