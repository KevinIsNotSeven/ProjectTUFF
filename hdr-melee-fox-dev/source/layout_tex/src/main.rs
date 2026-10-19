//! In-place texture swapper for the BNTX inside Smash Ultimate UI layout.arc (SARC) files.
//!
//!   layout_tex list    <layout.arc>
//!   layout_tex extract <layout.arc> <texture name> <out.dds>
//!   layout_tex replace <layout.arc> <texture name> <in.dds> <out layout.arc>
//!
//! `replace` takes a DDS already encoded in the texture's own format and size (made with
//! ultimate_tex_cli), swizzles it, and overwrites only that texture's bytes. Nothing else in the
//! file moves, so the SARC/BNTX headers stay valid.
use std::path::Path;
use tegra_swizzle::surface::{deswizzle_surface, swizzle_surface, BlockDim};
use tegra_swizzle::BlockHeight;

fn u16_at(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(d[o..o + 2].try_into().unwrap())
}
fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn u64_at(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(d[o..o + 8].try_into().unwrap())
}

struct Tex {
    name: String,
    format: u32,
    width: u32,
    height: u32,
    mips: u32,
    block_height_log2: u32,
    data_offset: usize, // absolute offset in the layout.arc
    image_size: usize,
}

/// (block dim, bytes per block/pixel, DXGI format) for a BNTX surface format.
fn format_info(format: u32) -> Option<(BlockDim, u32, u32)> {
    let bc = BlockDim::block_4x4();
    let un = BlockDim::uncompressed();
    Some(match format {
        0x0b01 => (un, 4, 28),
        0x0b06 => (un, 4, 29),
        0x0c01 => (un, 4, 87),
        0x0c06 => (un, 4, 91),
        0x1a01 => (bc, 8, 71),
        0x1a06 => (bc, 8, 72),
        0x1b01 => (bc, 16, 74),
        0x1b06 => (bc, 16, 75),
        0x1c01 => (bc, 16, 77),
        0x1c06 => (bc, 16, 78),
        0x1d01 => (bc, 8, 80),
        0x1e01 => (bc, 16, 83),
        0x2001 => (bc, 16, 98),
        0x2006 => (bc, 16, 99),
        _ => return None,
    })
}

fn find(d: &[u8], magic: &[u8], from: usize) -> Option<usize> {
    d[from..].windows(magic.len()).position(|w| w == magic).map(|p| p + from)
}

fn textures(d: &[u8]) -> Vec<Tex> {
    let mut out = Vec::new();
    let mut search = 0;
    // A layout.arc may hold more than one BNTX.
    while let Some(b) = find(d, b"BNTX", search) {
        search = b + 4;
        let nx = b + 0x20;
        if &d[nx..nx + 4] != b"NX  " {
            continue;
        }
        let count = u32_at(d, nx + 4) as usize;
        let ptrs = b + u64_at(d, nx + 8) as usize;
        for i in 0..count {
            let t = b + u64_at(d, ptrs + i * 8) as usize;
            assert_eq!(&d[t..t + 4], b"BRTI");
            let name_off = b + u64_at(d, t + 0x60) as usize;
            let len = u16_at(d, name_off) as usize;
            let name = String::from_utf8_lossy(&d[name_off + 2..name_off + 2 + len]).to_string();
            let mip_ptrs = b + u64_at(d, t + 0x70) as usize;
            out.push(Tex {
                name,
                format: u32_at(d, t + 0x1C),
                width: u32_at(d, t + 0x24),
                height: u32_at(d, t + 0x28),
                mips: u16_at(d, t + 0x16) as u32,
                block_height_log2: u32_at(d, t + 0x34),
                data_offset: b + u64_at(d, mip_ptrs) as usize,
                image_size: u32_at(d, t + 0x50) as usize,
            });
        }
    }
    out
}

fn get<'a>(texs: &'a [Tex], name: &str) -> &'a Tex {
    texs.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no texture named {name}"))
}

fn write_dds(path: &Path, t: &Tex, data: &[u8]) {
    let (_, _, dxgi) = format_info(t.format).expect("unsupported format");
    let mut h = Vec::new();
    h.extend_from_slice(b"DDS ");
    let mut hdr = [0u32; 31];
    hdr[0] = 124;
    hdr[1] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x20000;
    hdr[2] = t.height;
    hdr[3] = t.width;
    hdr[6] = t.mips;
    hdr[18] = 32; // pixel format size
    hdr[19] = 0x4; // FOURCC
    hdr[20] = u32::from_le_bytes(*b"DX10");
    hdr[26] = 0x1000 | 0x400000 | 0x8;
    for v in hdr {
        h.extend_from_slice(&v.to_le_bytes());
    }
    for v in [dxgi, 3, 0, 1, 0] {
        h.extend_from_slice(&v.to_le_bytes());
    }
    h.extend_from_slice(data);
    std::fs::write(path, h).unwrap();
}

/// Linear (unswizzled) size of `mips` levels of a surface.
fn linear_size(t: &Tex, mips: u32) -> usize {
    let (dim, bpp, _) = format_info(t.format).unwrap();
    let (bw, bh) = (dim.width.get() as u32, dim.height.get() as u32);
    (0..mips)
        .map(|m| {
            let w = (t.width >> m).max(1);
            let h = (t.height >> m).max(1);
            (w.div_ceil(bw) * h.div_ceil(bh) * bpp) as usize
        })
        .sum()
}

/// (name, absolute start, absolute end) of each file in a little-endian SARC.
fn sarc_files(d: &[u8]) -> Vec<(String, usize, usize)> {
    assert_eq!(&d[0..4], b"SARC");
    let data_off = u32_at(d, 0x0C) as usize;
    let sfat = u16_at(d, 0x04) as usize;
    assert_eq!(&d[sfat..sfat + 4], b"SFAT");
    let count = u16_at(d, sfat + 6) as usize;
    let nodes = sfat + 0x0C;
    let sfnt = nodes + count * 16;
    assert_eq!(&d[sfnt..sfnt + 4], b"SFNT");
    let names = sfnt + 8;
    (0..count)
        .map(|i| {
            let n = nodes + i * 16;
            let attr = u32_at(d, n + 4);
            let name_at = names + ((attr & 0xFFFF) as usize) * 4;
            let len = d[name_at..].iter().position(|&b| b == 0).unwrap();
            let name = String::from_utf8_lossy(&d[name_at..name_at + len]).to_string();
            (name, data_off + u32_at(d, n + 8) as usize, data_off + u32_at(d, n + 12) as usize)
        })
        .collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d = std::fs::read(&a[2]).expect("read layout");
    let texs = textures(&d);
    match a[1].as_str() {
        "list" => {
            for t in &texs {
                println!(
                    "{:40} {:#06x} {}x{} mips {} bh {} size {}",
                    t.name, t.format, t.width, t.height, t.mips, 1 << t.block_height_log2, t.image_size
                );
            }
        }
        "extract" => {
            let t = get(&texs, &a[3]);
            let (dim, bpp, _) = format_info(t.format).expect("unsupported format");
            let linear = deswizzle_surface(
                t.width, t.height, 1,
                &d[t.data_offset..t.data_offset + t.image_size],
                dim, BlockHeight::new(1 << t.block_height_log2), bpp, t.mips, 1,
            )
            .expect("deswizzle");
            write_dds(Path::new(&a[4]), t, &linear);
            println!("wrote {}", a[4]);
        }
        "replace" => {
            let t = get(&texs, &a[3]);
            let (dim, bpp, _) = format_info(t.format).expect("unsupported format");
            let dds = std::fs::read(&a[4]).expect("read dds");
            assert_eq!(&dds[0..4], b"DDS ");
            let w = u32_at(&dds, 16);
            let h = u32_at(&dds, 12);
            assert!(w == t.width && h == t.height, "dds is {w}x{h}, texture is {}x{}", t.width, t.height);
            let start = if &dds[84..88] == b"DX10" { 148 } else { 128 };
            let need = linear_size(t, t.mips);
            assert!(dds.len() - start >= need, "dds has too few mipmaps");
            let swizzled = swizzle_surface(
                t.width, t.height, 1, &dds[start..start + need],
                dim, BlockHeight::new(1 << t.block_height_log2), bpp, t.mips, 1,
            )
            .expect("swizzle");
            assert!(swizzled.len() <= t.image_size, "swizzled {} > slot {}", swizzled.len(), t.image_size);
            let mut out = d.clone();
            out[t.data_offset..t.data_offset + swizzled.len()].copy_from_slice(&swizzled);
            std::fs::write(&a[5], out).unwrap();
            println!("replaced {} ({} bytes) -> {}", t.name, swizzled.len(), a[5]);
        }
        // sarc <layout.arc> <out dir>: extract every file in the SARC (to inspect layouts/anims)
        "sarc" => {
            for (name, start, end) in sarc_files(&d) {
                let p = Path::new(&a[3]).join(&name);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, &d[start..end]).unwrap();
                println!("{name} @ {start:#x} ({} bytes)", end - start);
            }
        }
        _ => eprintln!("usage: layout_tex list|extract|replace|sarc ..."),
    }
}
