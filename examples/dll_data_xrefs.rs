use std::collections::{BTreeMap, BTreeSet};
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
        "usage: cargo run --example dll_data_xrefs -- <dll-path> <query> [query ...]\n\
         query:\n\
           0x180012345           direct VA\n\
           ascii:Tdx_Encrypt     ASCII bytes\n\
           ascii0:penc           ASCII bytes + NUL terminator\n\
           utf16:网络包           UTF-16LE bytes\n\
           utf16z:网络包          UTF-16LE bytes + NUL terminator\n\
           str:Stock.字典        search both ASCII and UTF-16LE\n\
           str0:网络包           search both encodings with terminator"
            .to_string()
    })?;

    let queries = args
        .map(|arg| parse_query(&arg))
        .collect::<Result<Vec<_>, _>>()?;
    if queries.is_empty() {
        return Err("need at least one query".to_string());
    }

    let bytes = fs::read(&dll_path).map_err(|err| format!("read {}: {err}", dll_path))?;
    let pe = PeFile::parse(&bytes)?;

    println!("file: {}", Path::new(&dll_path).display());
    println!("image-base: {:#x}", pe.image_base);

    let mut all_targets = BTreeSet::new();
    let mut resolved = Vec::new();
    for query in &queries {
        let mut matches = pe.resolve_query(query)?;
        matches.sort_by_key(|m| (m.va, m.raw_offset));
        matches.dedup_by_key(|m| (m.va, m.raw_offset));
        for m in &matches {
            all_targets.insert(m.va);
        }
        resolved.push((query.clone(), matches));
    }

    let xrefs = pe.find_data_refs(&all_targets)?;

    for (query, matches) in resolved {
        println!("query {}: {} match(es)", query.label(), matches.len());
        for m in matches {
            println!(
                "  target {:#x} raw={:#x} kind={} section={}",
                m.va, m.raw_offset, m.kind, m.section_name
            );
            if let Some(text) = m.preview {
                println!("    preview: {text}");
            }
            if let Some(hex) = m.hex_head {
                println!("    hex-head: {hex}");
            }
            if !m.ascii_nearby.is_empty() {
                println!("    ascii-nearby: {}", m.ascii_nearby.join(" | "));
            }
            if !m.utf16_nearby.is_empty() {
                println!("    utf16-nearby: {}", m.utf16_nearby.join(" | "));
            }
            let refs = xrefs.get(&m.va).cloned().unwrap_or_default();
            println!("    xrefs: {}", refs.len());
            for xr in refs {
                println!("      {:#x} {} {}", xr.instr_va, xr.section_name, xr.kind);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum Query {
    Address(u64),
    Ascii(String),
    AsciiZ(String),
    Utf16(String),
    Utf16Z(String),
    Both(String),
    BothZ(String),
}

impl Query {
    fn label(&self) -> String {
        match self {
            Self::Address(addr) => format!("{addr:#x}"),
            Self::Ascii(text) => format!("ascii:{text}"),
            Self::AsciiZ(text) => format!("ascii0:{text}"),
            Self::Utf16(text) => format!("utf16:{text}"),
            Self::Utf16Z(text) => format!("utf16z:{text}"),
            Self::Both(text) => format!("str:{text}"),
            Self::BothZ(text) => format!("str0:{text}"),
        }
    }
}

fn parse_query(value: &str) -> Result<Query, String> {
    if let Some(text) = value.strip_prefix("ascii:") {
        return Ok(Query::Ascii(text.to_string()));
    }
    if let Some(text) = value.strip_prefix("ascii0:") {
        return Ok(Query::AsciiZ(text.to_string()));
    }
    if let Some(text) = value.strip_prefix("utf16:") {
        return Ok(Query::Utf16(text.to_string()));
    }
    if let Some(text) = value.strip_prefix("utf16z:") {
        return Ok(Query::Utf16Z(text.to_string()));
    }
    if let Some(text) = value.strip_prefix("str:") {
        return Ok(Query::Both(text.to_string()));
    }
    if let Some(text) = value.strip_prefix("str0:") {
        return Ok(Query::BothZ(text.to_string()));
    }
    Ok(Query::Address(parse_addr(value)?))
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
    virtual_size: u32,
    virtual_address: u32,
    size_of_raw_data: u32,
    pointer_to_raw_data: u32,
    characteristics: u32,
}

impl Section {
    fn is_executable(&self) -> bool {
        self.characteristics & 0x2000_0000 != 0
    }

    fn raw_range(&self) -> std::ops::Range<usize> {
        let start = self.pointer_to_raw_data as usize;
        let size = self.size_of_raw_data as usize;
        start..start + size
    }
}

struct PeFile {
    bytes: Vec<u8>,
    image_base: u64,
    sections: Vec<Section>,
    is_pe32_plus: bool,
}

#[derive(Clone)]
struct TargetMatch {
    va: u64,
    raw_offset: usize,
    kind: String,
    section_name: String,
    preview: Option<String>,
    hex_head: Option<String>,
    ascii_nearby: Vec<String>,
    utf16_nearby: Vec<String>,
}

#[derive(Clone)]
struct DataXref {
    instr_va: u64,
    section_name: String,
    kind: &'static str,
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
            let virtual_size = read_u32(bytes, offset + 8)?;
            let virtual_address = read_u32(bytes, offset + 12)?;
            let size_of_raw_data = read_u32(bytes, offset + 16)?;
            let pointer_to_raw_data = read_u32(bytes, offset + 20)?;
            let characteristics = read_u32(bytes, offset + 36)?;
            sections.push(Section {
                name,
                virtual_size,
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
            is_pe32_plus: optional_magic == 0x20b,
        })
    }

    fn raw_to_va(&self, raw_offset: usize) -> Option<(u64, &Section)> {
        self.sections.iter().find_map(|section| {
            let range = section.raw_range();
            if range.contains(&raw_offset) {
                let delta = raw_offset - range.start;
                Some((
                    self.image_base + u64::from(section.virtual_address) + delta as u64,
                    section,
                ))
            } else {
                None
            }
        })
    }

    fn resolve_query(&self, query: &Query) -> Result<Vec<TargetMatch>, String> {
        match query {
            Query::Address(addr) => {
                let raw_offset = self
                    .va_to_raw(*addr)
                    .ok_or_else(|| format!("address {addr:#x} not in file-backed section"))?;
                Ok(vec![TargetMatch {
                    va: *addr,
                    raw_offset,
                    kind: "address".to_string(),
                    section_name: self.section_name_for_va(*addr).unwrap_or("?").to_string(),
                    preview: Some(preview_at(&self.bytes, raw_offset, 0x20)),
                    hex_head: Some(hex_head_at(&self.bytes, raw_offset, 0x20)),
                    ascii_nearby: nearby_ascii_strings(&self.bytes, raw_offset, 0),
                    utf16_nearby: nearby_utf16_strings(&self.bytes, raw_offset, 0),
                }])
            }
            Query::Ascii(text) => self.find_ascii(text),
            Query::AsciiZ(text) => self.find_ascii_z(text),
            Query::Utf16(text) => self.find_utf16(text),
            Query::Utf16Z(text) => self.find_utf16_z(text),
            Query::Both(text) => {
                let mut out = self.find_ascii(text)?;
                out.extend(self.find_utf16(text)?);
                Ok(out)
            }
            Query::BothZ(text) => {
                let mut out = self.find_ascii_z(text)?;
                out.extend(self.find_utf16_z(text)?);
                Ok(out)
            }
        }
    }

    fn find_ascii(&self, text: &str) -> Result<Vec<TargetMatch>, String> {
        let needle = text.as_bytes();
        self.find_matches(needle, &format!("ascii:{}", sanitize(text)))
    }

    fn find_ascii_z(&self, text: &str) -> Result<Vec<TargetMatch>, String> {
        let mut needle = text.as_bytes().to_vec();
        needle.push(0);
        self.find_matches(&needle, &format!("ascii0:{}", sanitize(text)))
    }

    fn find_utf16(&self, text: &str) -> Result<Vec<TargetMatch>, String> {
        let mut needle = Vec::with_capacity(text.len() * 2);
        for unit in text.encode_utf16() {
            needle.extend_from_slice(&unit.to_le_bytes());
        }
        self.find_matches(&needle, &format!("utf16:{}", sanitize(text)))
    }

    fn find_utf16_z(&self, text: &str) -> Result<Vec<TargetMatch>, String> {
        let mut needle = Vec::with_capacity(text.len() * 2 + 2);
        for unit in text.encode_utf16() {
            needle.extend_from_slice(&unit.to_le_bytes());
        }
        needle.extend_from_slice(&0u16.to_le_bytes());
        self.find_matches(&needle, &format!("utf16z:{}", sanitize(text)))
    }

    fn find_matches(&self, needle: &[u8], kind: &str) -> Result<Vec<TargetMatch>, String> {
        if needle.is_empty() {
            return Err("empty search needle".to_string());
        }

        let mut out = Vec::new();
        for raw_offset in self
            .bytes
            .windows(needle.len())
            .enumerate()
            .filter_map(|(offset, window)| (window == needle).then_some(offset))
        {
            if let Some((va, section)) = self.raw_to_va(raw_offset) {
                out.push(TargetMatch {
                    va,
                    raw_offset,
                    kind: kind.to_string(),
                    section_name: section.name.clone(),
                    preview: Some(preview_at(&self.bytes, raw_offset, needle.len())),
                    hex_head: Some(hex_head_at(&self.bytes, raw_offset, needle.len().max(0x20))),
                    ascii_nearby: nearby_ascii_strings(&self.bytes, raw_offset, needle.len()),
                    utf16_nearby: nearby_utf16_strings(&self.bytes, raw_offset, needle.len()),
                });
            }
        }
        Ok(out)
    }

    fn va_to_raw(&self, va: u64) -> Option<usize> {
        self.sections.iter().find_map(|section| {
            let section_start = self.image_base + u64::from(section.virtual_address);
            let section_size = u64::from(section.virtual_size.max(section.size_of_raw_data));
            let section_end = section_start + section_size;
            if (section_start..section_end).contains(&va) {
                let delta = (va - section_start) as usize;
                let raw = section.pointer_to_raw_data as usize + delta;
                (raw < self.bytes.len()).then_some(raw)
            } else {
                None
            }
        })
    }

    fn section_name_for_va(&self, va: u64) -> Option<&str> {
        self.sections.iter().find_map(|section| {
            let start = self.image_base + u64::from(section.virtual_address);
            let size = u64::from(section.virtual_size.max(section.size_of_raw_data));
            let end = start + size;
            (start..end).contains(&va).then_some(section.name.as_str())
        })
    }

    fn find_data_refs(
        &self,
        targets: &BTreeSet<u64>,
    ) -> Result<BTreeMap<u64, Vec<DataXref>>, String> {
        if self.is_pe32_plus {
            self.find_rip_relative_refs(targets)
        } else {
            self.find_x86_absolute_refs(targets)
        }
    }

    fn find_rip_relative_refs(
        &self,
        targets: &BTreeSet<u64>,
    ) -> Result<BTreeMap<u64, Vec<DataXref>>, String> {
        let mut hits = BTreeMap::<u64, Vec<DataXref>>::new();

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
            let section_va = self.image_base + u64::from(section.virtual_address);

            for offset in 0..data.len() {
                for pattern in decode_rip_relative_patterns(data, offset)? {
                    let instr_va = section_va + offset as u64;
                    let dest = (instr_va as i64) + pattern.instr_len as i64 + pattern.disp as i64;
                    if dest < 0 {
                        continue;
                    }
                    let dest = dest as u64;
                    if targets.contains(&dest) {
                        hits.entry(dest).or_default().push(DataXref {
                            instr_va,
                            section_name: section.name.clone(),
                            kind: pattern.kind,
                        });
                    }
                }
            }
        }

        for refs in hits.values_mut() {
            refs.sort_by_key(|xr| xr.instr_va);
            refs.dedup_by_key(|xr| xr.instr_va);
        }

        Ok(hits)
    }

    fn find_x86_absolute_refs(
        &self,
        targets: &BTreeSet<u64>,
    ) -> Result<BTreeMap<u64, Vec<DataXref>>, String> {
        let mut hits = BTreeMap::<u64, Vec<DataXref>>::new();

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
            let section_va = self.image_base + u64::from(section.virtual_address);

            for offset in 0..data.len() {
                for pattern in decode_x86_absolute_patterns(data, offset)? {
                    if !targets.contains(&pattern.target) {
                        continue;
                    }
                    let instr_va = section_va + offset as u64;
                    hits.entry(pattern.target).or_default().push(DataXref {
                        instr_va,
                        section_name: section.name.clone(),
                        kind: pattern.kind,
                    });
                }
            }
        }

        for refs in hits.values_mut() {
            refs.sort_by_key(|xr| xr.instr_va);
            refs.dedup_by_key(|xr| xr.instr_va);
        }

        Ok(hits)
    }
}

fn sanitize(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_control()).collect()
}

fn preview_at(bytes: &[u8], raw_offset: usize, needle_len: usize) -> String {
    let start = raw_offset.saturating_sub(8);
    let end = (raw_offset + needle_len + 8).min(bytes.len());
    bytes[start..end]
        .iter()
        .map(|byte| {
            if byte.is_ascii_graphic() || *byte == b' ' {
                *byte as char
            } else {
                '.'
            }
        })
        .collect()
}

fn hex_head_at(bytes: &[u8], raw_offset: usize, len: usize) -> String {
    let end = raw_offset.saturating_add(len).min(bytes.len());
    bytes[raw_offset..end]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn nearby_ascii_strings(bytes: &[u8], raw_offset: usize, needle_len: usize) -> Vec<String> {
    nearby_strings(bytes, raw_offset, needle_len, false)
}

fn nearby_utf16_strings(bytes: &[u8], raw_offset: usize, needle_len: usize) -> Vec<String> {
    nearby_strings(bytes, raw_offset, needle_len, true)
}

fn nearby_strings(bytes: &[u8], raw_offset: usize, needle_len: usize, utf16: bool) -> Vec<String> {
    let start = raw_offset.saturating_sub(0x40);
    let end = (raw_offset + needle_len + 0x40).min(bytes.len());
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let min_len = if utf16 { 2 } else { 4 };

    if utf16 {
        let mut cursor = start + (start & 1);
        while cursor + 1 < end {
            let Some((text, next)) = scan_utf16_string(bytes, cursor, end) else {
                cursor += 2;
                continue;
            };
            cursor = next;
            if text.chars().count() < min_len || !seen.insert(text.clone()) {
                continue;
            }
            out.push(text);
            if out.len() >= 8 {
                break;
            }
        }
    } else {
        let mut cursor = start;
        while cursor < end {
            let Some((text, next)) = scan_ascii_string(bytes, cursor, end) else {
                cursor += 1;
                continue;
            };
            cursor = next;
            if text.len() < min_len || !seen.insert(text.clone()) {
                continue;
            }
            out.push(text);
            if out.len() >= 8 {
                break;
            }
        }
    }

    out
}

fn scan_ascii_string(bytes: &[u8], start: usize, end: usize) -> Option<(String, usize)> {
    let mut cursor = start;
    while cursor < end && !is_ascii_string_byte(bytes[cursor]) {
        cursor += 1;
    }
    if cursor >= end {
        return None;
    }
    let begin = cursor;
    while cursor < end && is_ascii_string_byte(bytes[cursor]) {
        cursor += 1;
    }
    let text = String::from_utf8_lossy(&bytes[begin..cursor]).into_owned();
    Some((text, cursor))
}

fn scan_utf16_string(bytes: &[u8], start: usize, end: usize) -> Option<(String, usize)> {
    let mut cursor = start + (start & 1);
    while cursor + 1 < end {
        let unit = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
        if is_utf16_string_unit(unit) {
            break;
        }
        cursor += 2;
    }
    if cursor + 1 >= end {
        return None;
    }

    let begin = cursor;
    let mut units = Vec::new();
    while cursor + 1 < end {
        let unit = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
        if !is_utf16_string_unit(unit) {
            break;
        }
        units.push(unit);
        cursor += 2;
    }
    let text = String::from_utf16_lossy(&units);
    Some((text, cursor.max(begin + 2)))
}

fn is_ascii_string_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'_' | b'-' | b'.' | b'%' | b'/' | b'\\' | b' ' | b'|' | b':'
        )
}

fn is_utf16_string_unit(unit: u16) -> bool {
    if unit == 0 {
        return false;
    }
    matches!(unit, 0x20..=0x7e | 0x4e00..=0x9fff | 0x3000..=0x303f | 0xff00..=0xffef)
}

struct RipPattern {
    instr_len: u8,
    disp: i32,
    kind: &'static str,
}

struct AbsPattern {
    target: u64,
    kind: &'static str,
}

fn decode_rip_relative_patterns(bytes: &[u8], offset: usize) -> Result<Vec<RipPattern>, String> {
    let mut out = Vec::new();

    for prefix_len in [0usize, 1usize] {
        if prefix_len == 1 && !matches!(bytes.get(offset), Some(0x40..=0x4f)) {
            continue;
        }
        let base = offset + prefix_len;
        let Some(&opcode) = bytes.get(base) else {
            continue;
        };

        if let Some(kind) = one_byte_rip_kind(opcode, bytes.get(base + 1).copied()) {
            let modrm = *bytes
                .get(base + 1)
                .ok_or_else(|| "short read for modrm".to_string())?;
            if is_rip_relative_modrm(modrm) {
                let disp = read_i32(bytes, base + 2)?;
                out.push(RipPattern {
                    instr_len: (prefix_len + 6) as u8,
                    disp,
                    kind,
                });
            }
        }

        if opcode == 0x0f {
            let Some(&opcode2) = bytes.get(base + 1) else {
                continue;
            };
            let modrm = *bytes
                .get(base + 2)
                .ok_or_else(|| "short read for modrm".to_string())?;
            if is_rip_relative_modrm(modrm)
                && let Some(kind) = two_byte_rip_kind(opcode2)
            {
                let disp = read_i32(bytes, base + 3)?;
                out.push(RipPattern {
                    instr_len: (prefix_len + 7) as u8,
                    disp,
                    kind,
                });
            }
        }
    }

    Ok(out)
}

fn decode_x86_absolute_patterns(bytes: &[u8], offset: usize) -> Result<Vec<AbsPattern>, String> {
    let mut out = Vec::new();
    let Some(&opcode) = bytes.get(offset) else {
        return Ok(out);
    };

    match opcode {
        0x68 => out.push(AbsPattern {
            target: u64::from(read_u32(bytes, offset + 1)?),
            kind: "push-imm32",
        }),
        0xb8..=0xbf => out.push(AbsPattern {
            target: u64::from(read_u32(bytes, offset + 1)?),
            kind: "mov-imm32",
        }),
        0xa1 => out.push(AbsPattern {
            target: u64::from(read_u32(bytes, offset + 1)?),
            kind: "mov-eax-moffs-load",
        }),
        0xa3 => out.push(AbsPattern {
            target: u64::from(read_u32(bytes, offset + 1)?),
            kind: "mov-eax-moffs-store",
        }),
        _ => {}
    }

    if let Some(pattern) = decode_x86_modrm_absolute(bytes, offset)? {
        out.push(pattern);
    }

    Ok(out)
}

fn decode_x86_modrm_absolute(bytes: &[u8], offset: usize) -> Result<Option<AbsPattern>, String> {
    let Some(&opcode) = bytes.get(offset) else {
        return Ok(None);
    };
    if opcode == 0x0f {
        let Some(&opcode2) = bytes.get(offset + 1) else {
            return Ok(None);
        };
        let modrm = *bytes
            .get(offset + 2)
            .ok_or_else(|| "short read for x86 modrm".to_string())?;
        if !is_abs32_modrm(modrm) {
            return Ok(None);
        }
        let Some(kind) = two_byte_rip_kind(opcode2) else {
            return Ok(None);
        };
        return Ok(Some(AbsPattern {
            target: u64::from(read_u32(bytes, offset + 3)?),
            kind,
        }));
    }

    let modrm = match bytes.get(offset + 1) {
        Some(value) => *value,
        None => return Ok(None),
    };
    if !is_abs32_modrm(modrm) {
        return Ok(None);
    }
    let Some(kind) = one_byte_x86_kind(opcode, modrm) else {
        return Ok(None);
    };
    Ok(Some(AbsPattern {
        target: u64::from(read_u32(bytes, offset + 2)?),
        kind,
    }))
}

fn is_rip_relative_modrm(modrm: u8) -> bool {
    ((modrm >> 6) & 0b11) == 0 && (modrm & 0b111) == 0b101
}

fn is_abs32_modrm(modrm: u8) -> bool {
    ((modrm >> 6) & 0b11) == 0 && (modrm & 0b111) == 0b101
}

fn one_byte_rip_kind(opcode: u8, modrm: Option<u8>) -> Option<&'static str> {
    match opcode {
        0x8d => Some("lea"),
        0x8b => Some("mov-load"),
        0x89 => Some("mov-store"),
        0x3b => Some("cmp-load"),
        0x39 => Some("cmp-store"),
        0x85 => Some("test"),
        0xff => match (modrm? >> 3) & 0b111 {
            2 => Some("call-indirect"),
            4 => Some("jmp-indirect"),
            _ => None,
        },
        _ => None,
    }
}

fn one_byte_x86_kind(opcode: u8, modrm: u8) -> Option<&'static str> {
    match opcode {
        0x8d => Some("lea"),
        0x8b => Some("mov-load"),
        0x89 => Some("mov-store"),
        0x3b => Some("cmp-load"),
        0x39 => Some("cmp-store"),
        0x85 => Some("test"),
        0xc7 => (((modrm >> 3) & 0b111) == 0).then_some("mov-imm-store"),
        0x81 => match (modrm >> 3) & 0b111 {
            0 => Some("add-imm"),
            5 => Some("sub-imm"),
            7 => Some("cmp-imm"),
            _ => None,
        },
        0xff => match (modrm >> 3) & 0b111 {
            2 => Some("call-indirect"),
            4 => Some("jmp-indirect"),
            _ => None,
        },
        _ => None,
    }
}

fn two_byte_rip_kind(opcode: u8) -> Option<&'static str> {
    match opcode {
        0xb6 => Some("movzx-byte"),
        0xb7 => Some("movzx-word"),
        0xbe => Some("movsx-byte"),
        0xbf => Some("movsx-word"),
        0xaf => Some("imul"),
        _ => None,
    }
}
