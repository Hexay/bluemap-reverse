//! Wavefront OBJ debug export: `<stem>.obj` + `<stem>.mtl` + `textures/*.png`, world coordinates, one object per tile.
//! Vertex colours (`v x y z r g b`) carry the tint, optionally shaded by AO and light like the webapp.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};

use crate::textures::Texture;
use crate::tile::{Face, Tile};

pub struct PlacedTile<'a> {
    pub name: String,
    pub tile: &'a Tile,
    /// World x/z of the tile's min corner (positions are tile-relative in x/z).
    pub origin: [i32; 2],
}

#[derive(Clone, Copy)]
pub struct ObjOptions {
    /// Multiply vertex colours by AO and `(1-ambient)*max(sun,block)/15+ambient`.
    pub shade: bool,
    pub ambient: f32,
}

pub struct ObjStats {
    pub faces: usize,
    pub materials: usize,
}

pub fn write_obj(dir: &Path, stem: &str, tiles: &[PlacedTile], textures: &[Texture], opts: ObjOptions) -> Result<ObjStats> {
    fs::create_dir_all(dir.join("textures"))?;
    let mut out = BufWriter::new(File::create(dir.join(format!("{stem}.obj")))?);
    writeln!(out, "mtllib {stem}.mtl")?;

    let mut used = BTreeSet::new();
    let mut next_vertex = 1usize;
    let mut faces = 0;
    for placed in tiles {
        writeln!(out, "o {}", placed.name)?;
        let mut current = None;
        for face in placed.tile.faces() {
            if current != Some(face.material) {
                current = Some(face.material);
                used.insert(face.material);
                writeln!(out, "usemtl {}", material_name(textures, face.material))?;
            }
            write_face(&mut out, &face, placed.origin, next_vertex, opts)?;
            next_vertex += 3;
            faces += 1;
        }
    }
    out.flush()?;
    write_mtl(dir, stem, &used, textures)?;
    Ok(ObjStats { faces, materials: used.len() })
}

fn write_face(out: &mut impl Write, f: &Face, [ox, oz]: [i32; 2], first: usize, opts: ObjOptions) -> Result<()> {
    let light = (1.0 - opts.ambient) * f.sunlight.max(f.blocklight) as f32 / 15.0 + opts.ambient;
    for k in 0..3 {
        let [x, y, z] = f.pos[k];
        let shade = if opts.shade { light * f.ao[k] as f32 / 255.0 } else { 1.0 };
        let [r, g, b] = f.color.map(|c| c as f32 / 255.0 * shade);
        writeln!(out, "v {} {} {} {r:.4} {g:.4} {b:.4}", x + ox as f32, y, z + oz as f32)?;
    }
    for [u, v] in f.uv {
        writeln!(out, "vt {u} {}", 1.0 - v)?;
    }
    let (a, b, c) = (first, first + 1, first + 2);
    writeln!(out, "f {a}/{a} {b}/{b} {c}/{c}")?;
    Ok(())
}

fn material_name(textures: &[Texture], material: u32) -> String {
    textures.get(material as usize).map_or_else(|| format!("material_{material}"), Texture::file_stem)
}

fn write_mtl(dir: &Path, stem: &str, used: &BTreeSet<u32>, textures: &[Texture]) -> Result<()> {
    let mut mtl = BufWriter::new(File::create(dir.join(format!("{stem}.mtl")))?);
    for &m in used {
        let name = material_name(textures, m);
        writeln!(mtl, "newmtl {name}\nKd 1 1 1\nillum 1")?;
        if let Some(t) = textures.get(m as usize) {
            let file = format!("textures/{name}.png");
            fs::write(dir.join(&file), t.frame_png()?).with_context(|| file.clone())?;
            writeln!(mtl, "map_Kd {file}")?;
            if t.half_transparent || t.color[3] < 1.0 {
                writeln!(mtl, "map_d {file}")?;
            }
        }
        writeln!(mtl)?;
    }
    mtl.flush()?;
    Ok(())
}
