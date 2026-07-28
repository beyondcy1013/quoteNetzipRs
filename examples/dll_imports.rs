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
        "usage: cargo run --example dll_imports -- <dll-path> [filter ...]".to_string()
    })?;
    let filters = args.map(Filter::parse).collect::<Result<Vec<_>, _>>()?;

    let bytes = fs::read(&dll_path).map_err(|err| format!("read {}: {err}", dll_path))?;
    let pe = PeFile::parse(&bytes)?;
    let imports = pe.imports()?;

    println!("file: {}", Path::new(&dll_path).display());
    println!("image-base: {:#x}", pe.image_base);

    for import in imports
        .iter()
        .filter(|entry| filters.is_empty() || filters.iter().any(|f| f.matches(entry)))
    {
        println!(
            "{:#x} {}!{}",
            import.iat_va, import.dll_name, import.symbol_name
        );
    }

    Ok(())
}

#[derive(Clone)]
enum Filter {
    Address(u64),
    Text(String),
}

impl Filter {
    fn parse(value: String) -> Result<Self, String> {
        if let Ok(addr) = parse_addr(&value) {
            return Ok(Self::Address(addr));
        }
        Ok(Self::Text(value.to_ascii_lowercase()))
    }

    fn matches(&self, entry: &ImportEntry) -> bool {
        match self {
            Self::Address(addr) => entry.iat_va == *addr,
            Self::Text(text) => {
                entry.dll_name.to_ascii_lowercase().contains(text)
                    || entry.symbol_name.to_ascii_lowercase().contains(text)
            }
        }
    }
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
    Err(format!("not an address: {value}"))
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
    virtual_address: u32,
    virtual_size: u32,
    size_of_raw_data: u32,
    pointer_to_raw_data: u32,
}

struct PeFile {
    bytes: Vec<u8>,
    image_base: u64,
    is_pe32_plus: bool,
    import_rva: u32,
    sections: Vec<Section>,
}

#[derive(Clone)]
struct ImportEntry {
    iat_va: u64,
    dll_name: String,
    symbol_name: String,
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
        let magic = read_u16(bytes, optional)?;
        let (image_base, data_dir_offset, is_pe32_plus) = match magic {
            0x10b => (
                u64::from(read_u32(bytes, optional + 28)?),
                optional + 96,
                false,
            ),
            0x20b => (read_u64(bytes, optional + 24)?, optional + 112, true),
            other => return Err(format!("unsupported optional header magic {other:#x}")),
        };
        let import_rva = read_u32(bytes, data_dir_offset + 8)?;

        let section_table = optional + optional_size;
        let mut sections = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let offset = section_table + index * 40;
            let virtual_size = read_u32(bytes, offset + 8)?;
            let virtual_address = read_u32(bytes, offset + 12)?;
            let size_of_raw_data = read_u32(bytes, offset + 16)?;
            let pointer_to_raw_data = read_u32(bytes, offset + 20)?;
            sections.push(Section {
                virtual_address,
                virtual_size,
                size_of_raw_data,
                pointer_to_raw_data,
            });
        }

        Ok(Self {
            bytes: bytes.to_vec(),
            image_base,
            is_pe32_plus,
            import_rva,
            sections,
        })
    }

    fn imports(&self) -> Result<Vec<ImportEntry>, String> {
        let mut results = Vec::new();
        let mut descriptor_rva = self.import_rva;
        let thunk_size = if self.is_pe32_plus { 8 } else { 4 };
        let ordinal_flag = if self.is_pe32_plus {
            0x8000_0000_0000_0000u64
        } else {
            0x8000_0000u64
        };

        while descriptor_rva != 0 {
            let descriptor = self.rva_to_offset(descriptor_rva)?;
            let original_first_thunk = read_u32(&self.bytes, descriptor)?;
            let name_rva = read_u32(&self.bytes, descriptor + 12)?;
            let first_thunk = read_u32(&self.bytes, descriptor + 16)?;

            if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
                break;
            }

            let dll_name = self.read_c_string(name_rva)?;
            let thunk_rva = if original_first_thunk != 0 {
                original_first_thunk
            } else {
                first_thunk
            };

            let mut index = 0usize;
            loop {
                let thunk_entry_rva = thunk_rva
                    .checked_add((index * thunk_size) as u32)
                    .ok_or_else(|| "thunk rva overflow".to_string())?;
                let thunk_value = self.read_thunk_value(thunk_entry_rva)?;
                if thunk_value == 0 {
                    break;
                }

                let iat_rva = first_thunk
                    .checked_add((index * thunk_size) as u32)
                    .ok_or_else(|| "iat rva overflow".to_string())?;
                let iat_va = self.image_base + u64::from(iat_rva);

                let symbol_name = if thunk_value & ordinal_flag != 0 {
                    format!("ord:{}", thunk_value & 0xffff)
                } else {
                    let name_offset = self.rva_to_offset(thunk_value as u32)?;
                    let hint = read_u16(&self.bytes, name_offset)?;
                    let name = self.read_c_string((thunk_value as u32) + 2)?;
                    format!("{name} (hint {hint})")
                };

                results.push(ImportEntry {
                    iat_va,
                    dll_name: dll_name.clone(),
                    symbol_name,
                });
                index += 1;
            }

            descriptor_rva = descriptor_rva
                .checked_add(20)
                .ok_or_else(|| "import descriptor overflow".to_string())?;
        }

        Ok(results)
    }

    fn read_thunk_value(&self, rva: u32) -> Result<u64, String> {
        let offset = self.rva_to_offset(rva)?;
        if self.is_pe32_plus {
            read_u64(&self.bytes, offset)
        } else {
            Ok(u64::from(read_u32(&self.bytes, offset)?))
        }
    }

    fn read_c_string(&self, rva: u32) -> Result<String, String> {
        let start = self.rva_to_offset(rva)?;
        let bytes = self
            .bytes
            .get(start..)
            .ok_or_else(|| format!("string rva {rva:#x} out of file"))?;
        let len = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..len]).into_owned())
    }

    fn rva_to_offset(&self, rva: u32) -> Result<usize, String> {
        for section in &self.sections {
            let start = section.virtual_address;
            let span = section.virtual_size.max(section.size_of_raw_data);
            let end = start
                .checked_add(span)
                .ok_or_else(|| "section span overflow".to_string())?;
            if rva >= start && rva < end {
                let delta = rva - start;
                return usize::try_from(section.pointer_to_raw_data + delta)
                    .map_err(|_| "offset conversion failed".to_string());
            }
        }
        Err(format!("rva {rva:#x} not found in any section"))
    }
}
