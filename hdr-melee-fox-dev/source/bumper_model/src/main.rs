// bumper_model dump <model dir> [anim files...]   -- print meshes, materials, meshex, anim visibility tracks
// bumper_model build <in dir> <out dir> <name=texture>...
//   For each variant, adds a copy of every mesh object named "<name>_*" that uses a copy of its
//   material with Texture0 swapped to <texture>. Renames the original meshes "normal_*".
// bumper_model sphere <in dir> <out dir> [rings] [segments]
//   Replaces the (single) mesh object with a UV sphere of the same center and size, so one
//   2:1 "world map" texture wraps the whole thing: left/right edges meet at the back, the top
//   edge is the top pole. Keeps the object's name, bone and attribute layout; copies the
//   other model files unchanged.
use ssbh_data::anim_data::{AnimData, GroupType, TrackValues};
use ssbh_data::matl_data::{MatlData, ParamId};
use ssbh_data::mesh_data::{calculate_tangents_vec4, AttributeData, BoneInfluence, MeshData, VectorData, VertexWeight};
use ssbh_data::meshex_data::MeshExData;
use ssbh_data::modl_data::{ModlData, ModlEntryData};
use ssbh_data::SsbhData;
use std::path::Path;

fn sphere(input: &Path, out: &Path, rings: u32, segments: u32) {
    std::fs::create_dir_all(out).unwrap();
    let mut mesh = MeshData::from_file(input.join("model.numshb")).unwrap();
    assert_eq!(mesh.objects.len(), 1, "expected a single mesh object");
    let o = &mut mesh.objects[0];

    // center and radius of the original
    let pts: Vec<[f32; 3]> = match &o.positions[0].data {
        VectorData::Vector3(v) => v.clone(),
        VectorData::Vector4(v) => v.iter().map(|p| [p[0], p[1], p[2]]).collect(),
        VectorData::Vector2(_) => panic!("2D positions"),
    };
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &pts {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let r = pts.iter().map(|p| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt()).fold(0.0f32, f32::max);
    println!("original: {} verts, center {:?}, radius {}", pts.len(), c, r);

    // UV sphere; the seam column and the pole rows are duplicated so UVs don't wrap
    let (mut pos, mut nrm, mut uv) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..=rings {
        let v = i as f32 / rings as f32;
        let theta = v * std::f32::consts::PI; // 0 at the top pole
        for j in 0..=segments {
            let u = j as f32 / segments as f32;
            let phi = u * 2.0 * std::f32::consts::PI;
            // u = 0.5 faces the camera (+z), so the middle of the texture is the front
            let n = [-theta.sin() * phi.sin(), theta.cos(), -theta.sin() * phi.cos()];
            pos.push([c[0] + r * n[0], c[1] + r * n[1], c[2] + r * n[2]]);
            nrm.push(n);
            uv.push([u, v]);
        }
    }
    let row = segments + 1;
    let mut idx = Vec::new();
    for i in 0..rings {
        for j in 0..segments {
            let a = i * row + j;
            let b = a + row;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    // make the winding counter-clockwise when seen from outside
    let tri_normal = |t: &[u32]| {
        let (p0, p1, p2) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        let (e1, e2) = ([p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]], [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]]);
        [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
    };
    let probe = &idx[(rings / 2 * segments * 6) as usize..][..3];
    let (tn, center_dir) = (tri_normal(probe), pos[probe[0] as usize]);
    let outward = tn[0] * (center_dir[0] - c[0]) + tn[1] * (center_dir[1] - c[1]) + tn[2] * (center_dir[2] - c[2]);
    if outward < 0.0 {
        for t in idx.chunks_mut(3) {
            t.swap(1, 2);
        }
    }
    let n = pos.len();

    let like = |old: &VectorData, v3: &[[f32; 3]], w: f32| match old {
        VectorData::Vector3(_) => VectorData::Vector3(v3.to_vec()),
        VectorData::Vector4(_) => VectorData::Vector4(v3.iter().map(|p| [p[0], p[1], p[2], w]).collect()),
        VectorData::Vector2(_) => panic!("2D attribute"),
    };
    let first4 = |d: &VectorData| match d {
        VectorData::Vector4(v) => v[0],
        VectorData::Vector3(v) => [v[0][0], v[0][1], v[0][2], 1.0],
        VectorData::Vector2(v) => [v[0][0], v[0][1], 0.0, 1.0],
    };
    let pos_data = VectorData::Vector3(pos.clone());
    let nrm_data = VectorData::Vector3(nrm.clone());
    let uv_data = VectorData::Vector2(uv.clone());
    for a in o.positions.iter_mut() {
        a.data = like(&a.data, &pos, 1.0);
    }
    for a in o.normals.iter_mut() {
        a.data = like(&a.data, &nrm, 0.0);
    }
    for a in o.texture_coordinates.iter_mut() {
        a.data = VectorData::Vector2(uv.clone());
    }
    let tangents = calculate_tangents_vec4(&pos_data, &nrm_data, &uv_data, &idx).unwrap();
    for a in o.tangents.iter_mut() {
        a.data = match a.data {
            VectorData::Vector3(_) => VectorData::Vector3(tangents.iter().map(|t| [t[0], t[1], t[2]]).collect()),
            _ => VectorData::Vector4(tangents.clone()),
        };
    }
    for a in o.binormals.iter_mut() {
        let b: Vec<[f32; 3]> = nrm
            .iter()
            .zip(&tangents)
            .map(|(n, t)| [(n[1] * t[2] - n[2] * t[1]) * t[3], (n[2] * t[0] - n[0] * t[2]) * t[3], (n[0] * t[1] - n[1] * t[0]) * t[3]])
            .collect();
        a.data = like(&a.data, &b, 0.0);
    }
    let colors: Vec<AttributeData> = o
        .color_sets
        .iter()
        .map(|a| AttributeData { name: a.name.clone(), data: VectorData::Vector4(vec![first4(&a.data); n]) })
        .collect();
    o.color_sets = colors;
    if let Some(bone) = o.bone_influences.first().map(|b| b.bone_name.clone()) {
        o.bone_influences = vec![BoneInfluence {
            bone_name: bone,
            vertex_weights: (0..n as u32).map(|i| VertexWeight { vertex_index: i, vertex_weight: 1.0 }).collect(),
        }];
    }
    o.vertex_indices = idx;
    println!("sphere: {} verts, {} triangles", n, o.vertex_indices.len() / 3);
    mesh.write_to_file(out.join("model.numshb")).unwrap();
    for f in ["model.numatb", "model.numdlb", "model.numshexb", "model.nuanmb"] {
        std::fs::copy(input.join(f), out.join(f)).unwrap();
    }
}

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
    // Lowercase (the plugin looks meshes up by hash40 of the name), except a trailing "Shape":
    // the game gets a mesh's short name by cutting off exactly "Shape", so "..._gamemodelshape"
    // never matched "..._gamemodel" and visibility did nothing.
    let rename = |name: &str, prefix: &str| {
        let n = format!("{}_{}", prefix, name.trim_start_matches("game_"));
        match n.len().checked_sub(5).filter(|&i| n[i..].eq_ignore_ascii_case("shape")) {
            Some(i) => format!("{}Shape", n[..i].to_lowercase()),
            None => n.to_lowercase(),
        }
    };
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
        Some("sphere") => sphere(
            Path::new(&a[2]),
            Path::new(&a[3]),
            a.get(4).map(|s| s.parse().unwrap()).unwrap_or(24),
            a.get(5).map(|s| s.parse().unwrap()).unwrap_or(48),
        ),
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
