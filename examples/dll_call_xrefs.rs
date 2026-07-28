use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let dll_path = args.next().ok_or_else(|| {
        "usage: cargo run --example dll_call_xrefs -- <dll-path> <addr> [addr ...]".to_string()
    })?;

    let targets: Vec<u64> = args
        .map(|arg| parse_addr(&arg))
        .collect::<Result<Vec<_>, _>>()?;

    if targets.is_empty() {
        return Err("need at least one target address".to_string());
    }

    let bytes = fs::read(&dll_path).map_err(|err| format!("read {}: {err}", dll_path))?;
    let pe = PeFile::parse(&bytes)?;
    let hits = pe.find_rel32_callers(&targets)?;

    println!("file: {}", Path::new(&dll_path).display());
    println!("image-base: {:#x}", pe.image_base);
    println!("executable-sections:");
    for section in pe.sections.iter().filter(|s| s.is_executable()) {
        println!(
            "  {} va={:#x} raw={:#x} size={:#x}",
            section.name,
            pe.image_base + u64::from(section.virtual_address),
            section.pointer_to_raw_data,
            section.size_of_raw_data
        );
    }

    for target in targets {
        let callers = hits.get(&target).cloned().unwrap_or_default();
        println!("target {target:#x}: {}", callers.len());
        for (caller, section_name) in callers {
            println!("  {caller:#x} ({section_name})");
        }
    }

    Ok(())
}

fn parse_addr(value: &str) -> Result<u64, String> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16).map_err(|err| format!("parse address {value}: {err}"));
    }

    if value.chars().all(|ch| ch.is_ascii_digit()) {
        return value
            .parse::<u64>()
            .map_err(|err| format!("parse address {value}: {err}"));
    }

    u64::from_str_radix(value, 16).map_err(|err| format!("parse address {value}: {err}"))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| "u16 offset overflow".to_string())?;
    let chunk = bytes
        .get(offset..end)
        .ok_or_else(|| format!("short read for u16 at {offset:#x}"))?;
    Ok(u16::from_le_bytes([chunk[0], chunk[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| "u32 offset overflow".to_string())?;
    let chunk = bytes
        .get(offset..end)
        .ok_or_else(|| format!("short read for u32 at {offset:#x}"))?;
    Ok(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

fn read_i32(bytes: &[u8], offset: usize) -> Result<i32, String> {
    Ok(read_u32(bytes, offset)? as i32)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| "u64 offset overflow".to_string())?;
    let chunk = bytes
        .get(offset..end)
        .ok_or_else(|| format!("short read for u64 at {offset:#x}"))?;
    Ok(u64::from_le_bytes([
        chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
    ]))
}

#[derive(Clone)]
struct Section {
    name: String,
    virtual_address: u32,
    size_of_raw_data: u32,
    pointer_to_raw_data: u32,
    characteristics: u32,
}

impl Section {
    fn is_executable(&self) -> bool {
        self.characteristics & 0x2000_0000 != 0
    }
}

struct PeFile {
    bytes: Vec<u8>,
    image_base: u64,
    sections: Vec<Section>,
}

impl PeFile {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.get(0..2) != Some(b"MZ") {
            return Err("missing MZ header".to_string());
        }

        let pe_offset = read_u32(bytes, 0x3c)? as usize;
        if bytes.get(pe_offset..pe_offset + 4) != Some(b"PE\0\0") {
            return Err("missing PE signature".to_string());
        }

        let coff = pe_offset + 4;
        let section_count = read_u16(bytes, coff + 2)? as usize;
        let optional_size = read_u16(bytes, coff + 16)? as usize;
        let optional = coff + 20;
        let optional_magic = read_u16(bytes, optional)?;
        let image_base = match optional_magic {
            0x20b => read_u64(bytes, optional + 24)?,
            0x10b => u64::from(read_u32(bytes, optional + 28)?),
            other => return Err(format!("unsupported optional header magic {other:#x}")),
        };

        let section_table = optional + optional_size;
        let mut sections = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let offset = section_table + index * 40;
            let name_bytes = bytes
                .get(offset..offset + 8)
                .ok_or_else(|| format!("short read for section header {index}"))?;
            let name_end = name_bytes
                .iter()
                .position(|b| *b == 0)
                .unwrap_or(name_bytes.len());
            let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();
            let virtual_address = read_u32(bytes, offset + 12)?;
            let size_of_raw_data = read_u32(bytes, offset + 16)?;
            let pointer_to_raw_data = read_u32(bytes, offset + 20)?;
            let characteristics = read_u32(bytes, offset + 36)?;
            sections.push(Section {
                name,
                virtual_address,
                size_of_raw_data,
                pointer_to_raw_data,
                characteristics,
            });
        }

        Ok(Self {
            bytes: bytes.to_vec(),
            image_base,
            sections,
        })
    }

    fn find_rel32_callers(
        &self,
        targets: &[u64],
    ) -> Result<BTreeMap<u64, Vec<(u64, String)>>, String> {
        let mut hits = BTreeMap::<u64, Vec<(u64, String)>>::new();
        let wanted = targets
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();

        for section in self
            .sections
            .iter()
            .filter(|section| section.is_executable())
        {
            let start = section.pointer_to_raw_data as usize;
            let size = section.size_of_raw_data as usize;
            let end = start
                .checked_add(size)
                .ok_or_else(|| format!("section {} size overflow", section.name))?;
            let data = self
                .bytes
                .get(start..end)
                .ok_or_else(|| format!("section {} raw range out of file", section.name))?;

            // Direct near calls on x86-64 use E8 <rel32>.
            for offset in 0..data.len().saturating_sub(5) {
                if data[offset] != 0xE8 {
                    continue;
                }
                let rel = read_i32(data, offset + 1)? as i64;
                let caller = self.image_base + u64::from(section.virtual_address) + offset as u64;
                let dest = (caller as i64) + 5 + rel;
                if dest < 0 {
                    continue;
                }
                let dest = dest as u64;
                if wanted.contains(&dest) {
                    hits.entry(dest)
                        .or_default()
                        .push((caller, section.name.clone()));
                }
            }
        }

        Ok(hits)
    }
}
