use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

/// Decode a `.schem` written by `export_schem` into (size, palette-name per cell): for tests and `--verify`.
pub fn read_schem(path: &Path) -> Result<([usize; 3], Vec<String>)> {
    let mut nbt = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path)?).read_to_end(&mut nbt)?;
    let root: fastnbt::Value = fastnbt::from_bytes(&nbt)?;
    let fastnbt::Value::Compound(root) = root else { bail!("root is not a compound") };
    let Some(fastnbt::Value::Compound(s)) = root.get("Schematic") else { bail!("no Schematic compound") };
    let dim = |k: &str| match s.get(k) {
        Some(fastnbt::Value::Short(v)) => Ok(*v as u16 as usize),
        _ => bail!("missing {k}"),
    };
    let size = [dim("Width")?, dim("Height")?, dim("Length")?];
    let Some(fastnbt::Value::Compound(blocks)) = s.get("Blocks") else { bail!("no Blocks") };
    let Some(fastnbt::Value::Compound(pal)) = blocks.get("Palette") else { bail!("no Palette") };
    let Some(fastnbt::Value::ByteArray(data)) = blocks.get("Data") else { bail!("no Data") };
    let mut names = vec![String::new(); pal.len()];
    for (k, v) in pal {
        let fastnbt::Value::Int(i) = v else { bail!("palette value not int") };
        *names.get_mut(*i as usize).context("palette index out of range")? = k.clone();
    }
    let mut cells = Vec::with_capacity(size.iter().product());
    let (mut v, mut shift) = (0u32, 0);
    for &b in data.iter() {
        let b = b as u8;
        v |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            cells.push(names.get(v as usize).context("data index out of range")?.clone());
            (v, shift) = (0, 0);
        } else {
            shift += 7;
        }
    }
    ensure!(cells.len() == size.iter().product::<usize>(), "data has {} cells, expected {:?}", cells.len(), size);
    Ok((size, cells))
}
