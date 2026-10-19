use nus3audio::{AudioFile, Nus3audioFile};
use std::env;

fn magic(data: &[u8]) -> String {
    data.iter().take(4).map(|&b| if b.is_ascii_graphic() { b as char } else { '.' }).collect()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        // list <file>
        Some("list") => {
            let f = Nus3audioFile::open(&args[2]).expect("open");
            println!("{} tracks", f.files.len());
            for t in &f.files {
                println!("{:5} {:>9} {} {}", t.id, t.data.len(), magic(&t.data), t.name);
            }
        }
        // merge <base> <donor> <out> <track names...>
        // Copies <base>, then replaces (or adds) each named track with the donor's copy.
        Some("merge") => {
            let mut base = Nus3audioFile::open(&args[2]).expect("open base");
            let donor = Nus3audioFile::open(&args[3]).expect("open donor");
            for name in &args[5..] {
                let d = donor.files.iter().find(|t| &t.name == name).unwrap_or_else(|| panic!("donor has no track {name}"));
                if let Some(t) = base.files.iter_mut().find(|t| &t.name == name) {
                    println!("replace {name} ({} -> {} bytes)", t.data.len(), d.data.len());
                    t.data = d.data.clone();
                } else {
                    let id = base.files.iter().map(|t| t.id).max().map_or(0, |m| m + 1);
                    println!("add {name} as id {id} ({} bytes; donor id {})", d.data.len(), d.id);
                    base.files.push(AudioFile { id, name: name.clone(), data: d.data.clone() });
                }
            }
            let mut out = Vec::new();
            base.write(&mut out);
            std::fs::write(&args[4], out).expect("write");
            println!("wrote {}", args[4]);
        }
        // patch <base> <out> <track name> <encoded audio file> [<track name> <file>...]
        // Writes a patch3audio holding just the named tracks (ids taken from <base>) with the given data.
        Some("patch") => {
            let base = Nus3audioFile::open(&args[2]).expect("open base");
            let mut files = Vec::new();
            for pair in args[4..].chunks(2) {
                let name = &pair[0];
                let t = base.files.iter().find(|t| &t.name == name).unwrap_or_else(|| panic!("base has no track {name}"));
                let data = std::fs::read(&pair[1]).expect("read audio");
                println!("{name} id {} ({} -> {} bytes)", t.id, t.data.len(), data.len());
                files.push(AudioFile { id: t.id, name: name.clone(), data });
            }
            let patch = Nus3audioFile { files };
            let mut out = Vec::new();
            patch.write(&mut out);
            std::fs::write(&args[3], out).expect("write");
            println!("wrote {}", args[3]);
        }
        // extract <file> <track name> <out>
        Some("extract") => {
            let f = Nus3audioFile::open(&args[2]).expect("open");
            let t = f.files.iter().find(|t| t.name == args[3]).unwrap_or_else(|| panic!("no track {}", args[3]));
            std::fs::write(&args[4], &t.data).expect("write");
            println!("wrote {} ({} bytes)", args[4], t.data.len());
        }
        _ => eprintln!("usage: nus3tool list <file> | merge <base> <donor> <out> <names...> | patch <base> <out> <name> <audio> [...] | extract <file> <name> <out>"),
    }
}
