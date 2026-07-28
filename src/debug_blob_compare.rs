use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::Path;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct BlobCompareResult {
    pub left_input: String,
    pub right_input: String,
    pub left_size: usize,
    pub right_size: usize,
    pub left_offset: usize,
    pub right_offset: usize,
    pub compare_len: usize,
    pub exact_match: bool,
    pub equal_prefix_len: usize,
    pub equal_suffix_len: usize,
    pub equal_bytes: usize,
    pub diff_bytes: usize,
    pub first_diff_offset: Option<usize>,
    pub left_head_hex: String,
    pub right_head_hex: String,
    pub xor_head_hex: String,
    pub diff_runs: Vec<DiffRun>,
    pub block_summary: Option<BlockCompareSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DiffRun {
    pub offset: usize,
    pub len: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockCompareSummary {
    pub block_size: usize,
    pub total_blocks: usize,
    pub equal_blocks: usize,
    pub differing_blocks: usize,
    pub equal_positions: Vec<usize>,
    pub differing_positions: Vec<usize>,
    pub left_unique_blocks: usize,
    pub right_unique_blocks: usize,
    pub xor_unique_blocks: usize,
    pub left_most_common_hex: String,
    pub left_most_common_count: usize,
    pub right_most_common_hex: String,
    pub right_most_common_count: usize,
    pub xor_most_common_hex: String,
    pub xor_most_common_count: usize,
}

#[derive(Clone, Debug)]
struct SlicePatternSummary {
    unique_blocks: usize,
    most_common_hex: String,
    most_common_count: usize,
}

pub fn compare_blob_files(
    left: impl AsRef<Path>,
    right: impl AsRef<Path>,
    left_offset: usize,
    right_offset: usize,
    compare_len: Option<usize>,
    block_size: usize,
) -> Result<BlobCompareResult, Box<dyn Error>> {
    let left_path = left.as_ref();
    let right_path = right.as_ref();
    let left_bytes = fs::read(left_path)?;
    let right_bytes = fs::read(right_path)?;

    compare_blob_bytes(
        left_path.display().to_string(),
        &left_bytes,
        right_path.display().to_string(),
        &right_bytes,
        left_offset,
        right_offset,
        compare_len,
        block_size,
    )
}

pub fn compare_blob_bytes(
    left_input: String,
    left_bytes: &[u8],
    right_input: String,
    right_bytes: &[u8],
    left_offset: usize,
    right_offset: usize,
    compare_len: Option<usize>,
    block_size: usize,
) -> Result<BlobCompareResult, Box<dyn Error>> {
    if left_offset > left_bytes.len() {
        return Err(format!(
            "left_offset {} exceeds left size {}",
            left_offset,
            left_bytes.len()
        )
        .into());
    }
    if right_offset > right_bytes.len() {
        return Err(format!(
            "right_offset {} exceeds right size {}",
            right_offset,
            right_bytes.len()
        )
        .into());
    }

    let left_slice = &left_bytes[left_offset..];
    let right_slice = &right_bytes[right_offset..];
    let available = left_slice.len().min(right_slice.len());
    let compare_len = compare_len.unwrap_or(available).min(available);

    if compare_len == 0 {
        return Err("compare length resolved to zero".into());
    }

    let left = &left_slice[..compare_len];
    let right = &right_slice[..compare_len];
    let xor = left
        .iter()
        .zip(right.iter())
        .map(|(lhs, rhs)| lhs ^ rhs)
        .collect::<Vec<_>>();

    let equal_prefix_len = left
        .iter()
        .zip(right.iter())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count();
    let equal_suffix_len = left
        .iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(lhs, rhs)| lhs == rhs)
        .count();

    let mut equal_bytes = 0usize;
    let mut first_diff_offset = None;
    let mut diff_runs = Vec::new();
    let mut run_start = None;

    for (index, (lhs, rhs)) in left.iter().zip(right.iter()).enumerate() {
        if lhs == rhs {
            equal_bytes += 1;
            if let Some(start) = run_start.take() {
                diff_runs.push(DiffRun {
                    offset: start,
                    len: index - start,
                });
            }
            continue;
        }

        first_diff_offset.get_or_insert(index);
        if run_start.is_none() {
            run_start = Some(index);
        }
    }

    if let Some(start) = run_start.take() {
        diff_runs.push(DiffRun {
            offset: start,
            len: compare_len - start,
        });
    }

    diff_runs.truncate(16);
    let diff_bytes = compare_len - equal_bytes;
    let exact_match = diff_bytes == 0;

    Ok(BlobCompareResult {
        left_input,
        right_input,
        left_size: left_bytes.len(),
        right_size: right_bytes.len(),
        left_offset,
        right_offset,
        compare_len,
        exact_match,
        equal_prefix_len,
        equal_suffix_len,
        equal_bytes,
        diff_bytes,
        first_diff_offset,
        left_head_hex: hex_dump(left, 64),
        right_head_hex: hex_dump(right, 64),
        xor_head_hex: hex_dump(&xor, 64),
        diff_runs,
        block_summary: summarize_block_compare(left, right, &xor, block_size),
    })
}

fn summarize_block_compare(
    left: &[u8],
    right: &[u8],
    xor: &[u8],
    block_size: usize,
) -> Option<BlockCompareSummary> {
    if block_size == 0 {
        return None;
    }

    let total_blocks = left.len() / block_size;
    if total_blocks < 2 {
        return None;
    }

    let mut equal_positions = Vec::new();
    let mut differing_positions = Vec::new();
    for index in 0..total_blocks {
        let start = index * block_size;
        let end = start + block_size;
        if left[start..end] == right[start..end] {
            equal_positions.push(index);
        } else {
            differing_positions.push(index);
        }
    }

    let left_summary = summarize_slice_patterns(left, block_size);
    let right_summary = summarize_slice_patterns(right, block_size);
    let xor_summary = summarize_slice_patterns(xor, block_size);

    Some(BlockCompareSummary {
        block_size,
        total_blocks,
        equal_blocks: equal_positions.len(),
        differing_blocks: differing_positions.len(),
        equal_positions: equal_positions.into_iter().take(32).collect(),
        differing_positions: differing_positions.into_iter().take(32).collect(),
        left_unique_blocks: left_summary.unique_blocks,
        right_unique_blocks: right_summary.unique_blocks,
        xor_unique_blocks: xor_summary.unique_blocks,
        left_most_common_hex: left_summary.most_common_hex,
        left_most_common_count: left_summary.most_common_count,
        right_most_common_hex: right_summary.most_common_hex,
        right_most_common_count: right_summary.most_common_count,
        xor_most_common_hex: xor_summary.most_common_hex,
        xor_most_common_count: xor_summary.most_common_count,
    })
}

fn summarize_slice_patterns(bytes: &[u8], block_size: usize) -> SlicePatternSummary {
    let mut counts = BTreeMap::<Vec<u8>, usize>::new();
    for chunk in bytes.chunks_exact(block_size) {
        *counts.entry(chunk.to_vec()).or_default() += 1;
    }

    let (most_common_hex, most_common_count) = counts
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(chunk, count)| (hex_line(chunk), *count))
        .unwrap_or_else(|| (String::new(), 0));

    SlicePatternSummary {
        unique_blocks: counts.len(),
        most_common_hex,
        most_common_count,
    }
}

fn hex_dump(bytes: &[u8], limit: usize) -> String {
    hex_line(&bytes[..bytes.len().min(limit)])
}

fn hex_line(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::compare_blob_bytes;

    #[test]
    fn compares_equal_blobs() {
        let left = b"ABCDEFGHABCDEFGH";
        let right = b"ABCDEFGHABCDEFGH";
        let result = compare_blob_bytes(
            "left".to_string(),
            left,
            "right".to_string(),
            right,
            0,
            0,
            None,
            8,
        )
        .expect("compare result");

        assert!(result.exact_match);
        assert_eq!(result.equal_bytes, 16);
        assert_eq!(result.diff_bytes, 0);
        let block = result.block_summary.expect("block summary");
        assert_eq!(block.total_blocks, 2);
        assert_eq!(block.equal_blocks, 2);
        assert_eq!(block.differing_blocks, 0);
    }

    #[test]
    fn compares_different_blobs() {
        let left = b"ABCDEFGHABCDEFGH12345678";
        let right = b"ABCDEFGHzzzzzzzz12345678";
        let result = compare_blob_bytes(
            "left".to_string(),
            left,
            "right".to_string(),
            right,
            0,
            0,
            None,
            8,
        )
        .expect("compare result");

        assert!(!result.exact_match);
        assert_eq!(result.first_diff_offset, Some(8));
        assert_eq!(result.equal_prefix_len, 8);
        assert_eq!(result.equal_suffix_len, 8);
        let block = result.block_summary.expect("block summary");
        assert_eq!(block.total_blocks, 3);
        assert_eq!(block.equal_blocks, 2);
        assert_eq!(block.differing_blocks, 1);
        assert_eq!(block.equal_positions, vec![0, 2]);
        assert_eq!(block.differing_positions, vec![1]);
    }
}
