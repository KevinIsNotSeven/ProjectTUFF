// bumper_model dump <model dir> [anim files...]   -- print meshes, materials, meshex, anim visibility tracks
// bumper_model build <in dir> <out dir> <name=texture>...
//   For each variant, adds a copy of every mesh object named "<name>_*" that uses a copy of its
//   material with Texture0 swapped to <texture>. Renames the original meshes "normal_*".
use ssbh_data::anim_data::{AnimData, GroupType, TrackValues};
use ssbh_data::matl_data::{MatlData, ParamId};
use ssbh_data::mesh_data::MeshData;
use ssbh_data::meshex_data::MeshExData;
use ssbh_data::modl_data::{ModlData, ModlEntryData};
use ssbh_data::SsbhData;
use std::path::Path;

fn dump(dir: &Path, anims: &[String]) {
    let mesh = MeshData::from_file(dir.join("model.numshb")).unwrap();
    for o in &mesh.objects {
        println!(
            "mesh '{}' sub {} bone '{}' verts {} uvs {:?} colors {:?}",
            o.name,
            o.subindex,
            o.parent_bone_name,
            o.positions[0].data.len(),
            o.texture_coordinates.iter().map(|a| &a.name).collect::<Vec<_>>(),
            o.color_sets.iter().map(|a| &a.name).collect::<Vec<_>>()
        );
    }
    let modl = ModlData::from_file(dir.join("model.numdlb")).unwrap();
    for e in &modl.entries {
        println!("modl {} {} -> {}", e.mesh_object_name, e.mesh_object_subindex, e.material_label);
    }
    let matl = MatlData::from_file(dir.join("model.numatb")).unwrap();
    for m in &matl.entries {
        println!("matl '{}' shader {}", m.material_label, m.shader_label);
        for t in &m.textures {
            println!("   {:?} = {}", t.param_id, t.data);
        }
    }
    let mx = MeshExData::from_file(dir.join("model.numshexb")).unwrap();
    for g in &mx.mesh_object_groups {
        println!("meshex full '{}' name '{}' flags {:?}", g.mesh_object_full_name, g.mesh_object_name, g.entry_flags);
    }
    for a in anims {
        let anim = AnimData::from_file(a).unwrap();
        println!("anim {} frames {}", a, anim.final_frame_index);
        for g in &anim.groups {
            for n in &g.nodes {
                for t in &n.tracks {
                    let v = match &t.values {
                        TrackValues::Boolean(b) => format!("{:?}", b),
                        other => format!("{} values", track_len(other)),
                    };
                    println!("  {:?} node '{}' track '{}' {}", g.group_type, n.name, t.name, v);
                }
            }
        }
    }
}

fn track_len(v: &TrackValues) -> usize {
    match v {
        TrackValues::Transform(x) => x.len(),
        TrackValues::UvTransform(x) => x.len(),
        TrackValues::Float(x) => x.len(),
        TrackValues::PatternIndex(x) => x.len(),
        TrackValues::Boolean(x) => x.len(),
        TrackValues::Vector4(x) => x.len(),
    }
}

/// `variants` are (name, texture) pairs, e.g. ("tuff", "bumper_tuff_col"). For each one, every
/// mesh object gets a copy named "<name>_*" using a copy of its material ("<label>_<name>") with
/// Texture0 swapped. The original meshes are renamed "normal_*" and keep the stock materials.
fn build(input: &Path, out: &Path, variants: &[(String, String)]) {
    std::fs::create_dir_all(out).unwrap();
    let rename = |name: &str, prefix: &str| format!("{}_{}", prefix, name.trim_start_matches("game_"));
    let prefixes: Vec<&str> = std::iter::once("normal").chain(variants.iter().map(|(n, _)| n.as_str())).collect();

    let mut mesh = MeshData::from_file(input.join("model.numshb")).unwrap();
    let originals = mesh.objects.clone();
    mesh.objects.clear();
    for prefix in &prefixes {
        for o in &originals {
            let mut n = o.clone();
            n.name = rename(&o.name, prefix);
            mesh.objects.push(n);
        }
    }

    let mut matl = MatlData::from_file(input.join("model.numatb")).unwrap();
    let mut new_mats = Vec::new();
    for (name, tex) in variants {
        for m in &matl.entries {
            let mut t = m.clone();
            t.material_label = format!("{}_{}", m.material_label, name);
            for p in t.textures.iter_mut() {
                if p.param_id == ParamId::Texture0 {
                    p.data = tex.to_string();
                }
            }
            new_mats.push(t);
        }
    }
    matl.entries.extend(new_mats);

    let mut modl = ModlData::from_file(input.join("model.numdlb")).unwrap();
    let orig_entries = modl.entries.clone();
    modl.entries.clear();
    for prefix in &prefixes {
        for e in &orig_entries {
            modl.entries.push(ModlEntryData {
                mesh_object_name: rename(&e.mesh_object_name, prefix),
                mesh_object_subindex: e.mesh_object_subindex,
                material_label: if *prefix == "normal" {
                    e.material_label.clone()
                } else {
                    format!("{}_{}", e.material_label, prefix)
                },
            });
        }
    }

    // Keep the original bounding spheres and flags, duplicated for each group.
    let mx = MeshExData::from_file(input.join("model.numshexb")).unwrap();
    let mut new_mx = MeshExData { mesh_object_groups: Vec::new() };
    for prefix in &prefixes {
        for g in &mx.mesh_object_groups {
            let mut n = g.clone();
            n.mesh_object_full_name = rename(&g.mesh_object_full_name, prefix);
            n.mesh_object_name = rename(&g.mesh_object_name, prefix);
            new_mx.mesh_object_groups.push(n);
        }
    }

    // model.nuanmb: copy each material's tracks to its variant twins, and drop the
    // visibility track so it can't fight the plugin's per-item mesh visibility.
    let mut anim = AnimData::from_file(input.join("model.nuanmb")).unwrap();
    anim.groups.retain(|g| g.group_type != GroupType::Visibility);
    for g in anim.groups.iter_mut() {
        if g.group_type == GroupType::Material {
            let mut twins = Vec::new();
            for (name, _) in variants {
                for n in &g.nodes {
                    let mut t = n.clone();
                    t.name = format!("{}_{}", n.name, name);
                    twins.push(t);
                }
            }
            g.nodes.extend(twins);
        }
    }
    anim.write_to_file(out.join("model.nuanmb")).unwrap();

    mesh.write_to_file(out.join("model.numshb")).unwrap();
    matl.write_to_file(out.join("model.numatb")).unwrap();
    modl.write_to_file(out.join("model.numdlb")).unwrap();
    new_mx.write_to_file(out.join("model.numshexb")).unwrap();
    println!("wrote model to {}", out.display());
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(|s| s.as_str()) {
        Some("dump") => dump(Path::new(&a[2]), &a[3..]),
        // anim <file.nuanmb>: translation of every transform node, every 5 frames
        Some("anim") => {
            let anim = AnimData::from_file(&a[2]).unwrap();
            println!("frames {}", anim.final_frame_index);
            for g in &anim.groups {
                for n in &g.nodes {
                    for t in &n.tracks {
                        if let TrackValues::Transform(v) = &t.values {
                            let pts: Vec<String> = v
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| i % 5 == 0 || *i == v.len() - 1)
                                .map(|(i, x)| format!("{}:({:.1},{:.1},{:.1})", i, x.translation.x, x.translation.y, x.translation.z))
                                .collect();
                            println!("{} [{}] {}", n.name, v.len(), pts.join(" "));
                        }
                    }
                }
            }
        }
        Some("build") => {
            let variants: Vec<(String, String)> = a[4..]
                .iter()
                .map(|v| {
                    let (n, t) = v.split_once('=').expect("variant must be name=texture");
                    (n.to_string(), t.to_string())
                })
                .collect();
            build(Path::new(&a[2]), Path::new(&a[3]), &variants)
        }
        _ => eprintln!("usage: bumper_model dump <dir> [anims...] | build <in> <out> <name=texture>..."),
    }
}
