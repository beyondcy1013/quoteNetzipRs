use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::Path;

use serde::Serialize;

use crate::{Auth7100FlowMatrix, analyze_auth_7100_flow_matrix, analyze_local_2000_log_file};

const LOCAL_SAMPLE_LOG_REL: &str =
    "windows_debug/tmp_netzip_probe_20260329/frida_ws2_trace_20260330_v11.log";
const AUTH_SAMPLE_PCAP_REL: &str = "tmp/netzip_full_tcp.pcap";

#[derive(Clone, Debug, Serialize)]
pub struct Local2000VsAuth7100Analysis {
    pub local_input: String,
    pub auth_input: String,
    pub local_complete_small_packets: usize,
    pub auth_packet_count: usize,
    pub local_prefixes: Vec<Local2000ComparablePrefix>,
    pub local_large_prefixes: Vec<Local2000LargeComparablePrefix>,
    pub auth_prefixes: Vec<Auth7100ComparablePrefix>,
    pub bridge_layout_matches: Vec<BridgeLayoutMatch>,
    pub large_bridge_layout_matches: Vec<BridgeLayoutMatch>,
    pub shared_findings: Vec<String>,
    pub local_only_findings: Vec<String>,
    pub auth_only_findings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Local2000ComparablePrefix {
    pub packet_len_declared: u32,
    pub packet_len_duplicate: u32,
    pub packet_len_matches: bool,
    pub field40_constant: u32,
    pub payload_len_hint: u32,
    pub field48: u32,
    pub attr_flags_hint: u32,
    pub object_span_len_hint: u32,
    pub fixed_overhead_len_hint: Option<u32>,
    pub header_len_hint: Option<u32>,
    pub outer_wrapper_len_hint: Option<u32>,
    pub penc_offset: Option<usize>,
    pub hypenc_offset: Option<usize>,
    pub penc_to_payload_boundary_delta: Option<i64>,
    pub hypenc_to_payload_boundary_delta: Option<i64>,
    pub penc_to_object_boundary_delta: Option<i64>,
    pub hypenc_to_object_boundary_delta: Option<i64>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Local2000LargeComparablePrefix {
    pub packet_len_declared: u32,
    pub packet_len_duplicate: Option<u32>,
    pub packet_len_matches: bool,
    pub field40_constant: u32,
    pub payload_len_hint: u32,
    pub field48: u32,
    pub attr_flags_hint: u32,
    pub object_span_len_hint: u32,
    pub fixed_overhead_len_hint: Option<u32>,
    pub header_len_hint: Option<u32>,
    pub outer_wrapper_len_hint: Option<u32>,
    pub penc_offset: Option<usize>,
    pub hypenc_offset: Option<usize>,
    pub penc_offsets: Vec<usize>,
    pub hypenc_offsets: Vec<usize>,
    pub penc_to_payload_boundary_delta: Option<i64>,
    pub hypenc_to_payload_boundary_delta: Option<i64>,
    pub penc_to_object_boundary_delta: Option<i64>,
    pub hypenc_to_object_boundary_delta: Option<i64>,
    pub nested_penc_to_payload_boundary_deltas: Vec<i64>,
    pub nested_hypenc_to_payload_boundary_deltas: Vec<i64>,
    pub segment_count: usize,
    pub recv_complete: bool,
    pub captured_complete: bool,
    pub stop_reason: String,
    pub oem_answer_summary: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Auth7100ComparablePrefix {
    pub direction: String,
    pub packet_role: String,
    pub object_type_id: u32,
    pub packet_len_declared: u32,
    pub packet_len_duplicate: u32,
    pub packet_len_matches: bool,
    pub field40_constant: u32,
    pub payload_len_hint: u32,
    pub attr_flags_hint: u32,
    pub object_span_len_hint: u32,
    pub fixed_overhead_len_hint: Option<u32>,
    pub header_len_hint: Option<u32>,
    pub outer_wrapper_len_hint: Option<u32>,
    pub layout_hint: String,
    pub penc_offsets: Vec<usize>,
    pub hypenc_offsets: Vec<usize>,
    pub first_penc_offset: Option<usize>,
    pub first_hypenc_offset: Option<usize>,
    pub first_penc_to_payload_boundary_delta: Option<i64>,
    pub first_hypenc_to_payload_boundary_delta: Option<i64>,
    pub first_penc_to_object_boundary_delta: Option<i64>,
    pub first_hypenc_to_object_boundary_delta: Option<i64>,
    pub second_penc_offset: Option<usize>,
    pub second_penc_to_payload_boundary_delta: Option<i64>,
    pub second_penc_to_object_boundary_delta: Option<i64>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct BridgeLayoutMatch {
    pub local_source_kind: String,
    pub local_packet_len_declared: u32,
    pub local_payload_len_hint: u32,
    pub local_attr_flags_hint: u32,
    pub local_object_span_len_hint: u32,
    pub local_header_len_hint: Option<u32>,
    pub local_outer_wrapper_len_hint: Option<u32>,
    pub local_penc_offset: Option<usize>,
    pub local_hypenc_offset: Option<usize>,
    pub local_penc_offsets: Vec<usize>,
    pub local_hypenc_offsets: Vec<usize>,
    pub local_penc_to_payload_boundary_delta: Option<i64>,
    pub local_hypenc_to_payload_boundary_delta: Option<i64>,
    pub local_penc_to_object_boundary_delta: Option<i64>,
    pub local_hypenc_to_object_boundary_delta: Option<i64>,
    pub local_nested_penc_to_payload_boundary_deltas: Vec<i64>,
    pub auth_direction: String,
    pub auth_packet_role: String,
    pub auth_object_type_id: u32,
    pub auth_layout_hint: String,
    pub auth_packet_len_declared: u32,
    pub auth_payload_len_hint: u32,
    pub auth_attr_flags_hint: u32,
    pub auth_object_span_len_hint: u32,
    pub auth_header_len_hint: Option<u32>,
    pub auth_outer_wrapper_len_hint: Option<u32>,
    pub auth_penc_offsets: Vec<usize>,
    pub auth_hypenc_offsets: Vec<usize>,
    pub auth_first_penc_to_payload_boundary_delta: Option<i64>,
    pub auth_first_hypenc_to_payload_boundary_delta: Option<i64>,
    pub auth_first_penc_to_object_boundary_delta: Option<i64>,
    pub auth_first_hypenc_to_object_boundary_delta: Option<i64>,
    pub auth_second_penc_offset: Option<usize>,
    pub auth_second_penc_to_payload_boundary_delta: Option<i64>,
    pub auth_second_penc_to_object_boundary_delta: Option<i64>,
    pub shared_signals: Vec<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LocalKey {
    packet_len_declared: u32,
    packet_len_duplicate: u32,
    field40_constant: u32,
    payload_len_hint: u32,
    field48: u32,
    attr_flags_hint: u32,
    object_span_len_hint: u32,
    penc_offset: Option<usize>,
    hypenc_offset: Option<usize>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AuthKey {
    direction: String,
    packet_role: String,
    object_type_id: u32,
    packet_len_declared: u32,
    packet_len_duplicate: u32,
    field40_constant: u32,
    payload_len_hint: u32,
    attr_flags_hint: u32,
    object_span_len_hint: u32,
    fixed_overhead_len_hint: Option<u32>,
    layout_hint: String,
    penc_offsets: Vec<usize>,
    hypenc_offsets: Vec<usize>,
}

pub fn analyze_local_2000_vs_auth7100_sample() -> Result<Local2000VsAuth7100Analysis, Box<dyn Error>>
{
    analyze_local_2000_vs_auth7100(
        crate::repository_fixture_path(LOCAL_SAMPLE_LOG_REL),
        2000,
        4096,
        20,
        crate::repository_fixture_path(AUTH_SAMPLE_PCAP_REL),
    )
}

pub fn analyze_local_2000_vs_auth7100(
    local_log_path: impl AsRef<Path>,
    local_port: u16,
    small_max: usize,
    code_preview_limit: usize,
    auth_pcap_path: impl AsRef<Path>,
) -> Result<Local2000VsAuth7100Analysis, Box<dyn Error>> {
    let local_log_path = local_log_path.as_ref();
    let auth_pcap_path = auth_pcap_path.as_ref();

    let local =
        analyze_local_2000_log_file(local_log_path, local_port, small_max, code_preview_limit)?;
    let auth = analyze_auth_7100_flow_matrix(auth_pcap_path)?;

    let local_prefixes = collect_local_prefixes(&local);
    let local_large_prefixes = collect_local_large_prefixes(&local);
    let auth_prefixes = collect_auth_prefixes(&auth);
    let bridge_layout_matches = collect_bridge_layout_matches(&local_prefixes, &auth_prefixes);
    let large_bridge_layout_matches =
        collect_large_bridge_layout_matches(&local_large_prefixes, &auth_prefixes);

    let shared_findings = summarize_shared_findings(
        &local_prefixes,
        &auth_prefixes,
        &bridge_layout_matches,
        &large_bridge_layout_matches,
    );
    let local_only_findings = summarize_local_only_findings(&local_prefixes, &local_large_prefixes);
    let auth_only_findings = summarize_auth_only_findings(&auth_prefixes);

    Ok(Local2000VsAuth7100Analysis {
        local_input: local.input,
        auth_input: auth.pcap_path,
        local_complete_small_packets: local.complete_small_packets,
        auth_packet_count: auth
            .sessions
            .iter()
            .map(|session| session.client_packets.len() + session.server_packets.len())
            .sum(),
        local_prefixes,
        local_large_prefixes,
        auth_prefixes,
        bridge_layout_matches,
        large_bridge_layout_matches,
        shared_findings,
        local_only_findings,
        auth_only_findings,
    })
}

fn collect_local_prefixes(
    analysis: &crate::Local2000LogAnalysis,
) -> Vec<Local2000ComparablePrefix> {
    let mut counts = BTreeMap::<LocalKey, usize>::new();

    for entry in &analysis.entries {
        for hit in &entry.packet_hits {
            if hit.kind != "complete-small" {
                continue;
            }
            let (
                Some(packet_len_declared),
                Some(packet_len_duplicate),
                Some(field40_constant),
                Some(payload_len_hint),
                Some(field48),
                Some(attr_flags_hint),
                Some(object_span_len_hint),
            ) = (
                hit.decl_a,
                hit.decl_b,
                hit.field40,
                hit.field44,
                hit.field48,
                hit.field52,
                hit.field56,
            )
            else {
                continue;
            };

            let key = LocalKey {
                packet_len_declared,
                packet_len_duplicate,
                field40_constant,
                payload_len_hint,
                field48,
                attr_flags_hint,
                object_span_len_hint,
                penc_offset: hit.penc_offset,
                hypenc_offset: hit.hypenc_offset,
            };
            *counts.entry(key).or_default() += 1;
        }
    }

    counts
        .into_iter()
        .map(|(key, count)| {
            let fixed_overhead_len_hint =
                key.object_span_len_hint.checked_sub(key.payload_len_hint);
            let header_len_hint = key.packet_len_declared.checked_sub(key.payload_len_hint);
            let outer_wrapper_len_hint = key
                .packet_len_declared
                .checked_sub(key.object_span_len_hint);

            Local2000ComparablePrefix {
                packet_len_matches: key.packet_len_declared == key.packet_len_duplicate,
                fixed_overhead_len_hint,
                header_len_hint,
                outer_wrapper_len_hint,
                packet_len_declared: key.packet_len_declared,
                packet_len_duplicate: key.packet_len_duplicate,
                field40_constant: key.field40_constant,
                payload_len_hint: key.payload_len_hint,
                field48: key.field48,
                attr_flags_hint: key.attr_flags_hint,
                object_span_len_hint: key.object_span_len_hint,
                penc_offset: key.penc_offset,
                hypenc_offset: key.hypenc_offset,
                penc_to_payload_boundary_delta: signed_offset_delta(
                    key.penc_offset,
                    header_len_hint,
                ),
                hypenc_to_payload_boundary_delta: signed_offset_delta(
                    key.hypenc_offset,
                    header_len_hint,
                ),
                penc_to_object_boundary_delta: signed_offset_delta(
                    key.penc_offset,
                    outer_wrapper_len_hint,
                ),
                hypenc_to_object_boundary_delta: signed_offset_delta(
                    key.hypenc_offset,
                    outer_wrapper_len_hint,
                ),
                count,
            }
        })
        .collect()
}

fn collect_local_large_prefixes(
    analysis: &crate::Local2000LogAnalysis,
) -> Vec<Local2000LargeComparablePrefix> {
    analysis
        .large_objects
        .iter()
        .filter_map(|item| {
            let header_len_hint = item.declared_len.checked_sub(item.field44?);
            let outer_wrapper_len_hint = item.declared_len.checked_sub(item.field56?);
            let nested_penc_to_payload_boundary_deltas = item
                .penc_offsets
                .iter()
                .copied()
                .skip(1)
                .filter_map(|offset| signed_offset_delta(Some(offset), header_len_hint))
                .collect::<Vec<_>>();
            let nested_hypenc_to_payload_boundary_deltas = item
                .hypenc_offsets
                .iter()
                .copied()
                .skip(1)
                .filter_map(|offset| signed_offset_delta(Some(offset), header_len_hint))
                .collect::<Vec<_>>();
            Some(Local2000LargeComparablePrefix {
                packet_len_declared: item.declared_len,
                packet_len_duplicate: item.declared_len_duplicate,
                packet_len_matches: item.declared_len_duplicate == Some(item.declared_len),
                field40_constant: item.field40?,
                payload_len_hint: item.field44?,
                field48: item.field48?,
                attr_flags_hint: item.field52?,
                object_span_len_hint: item.field56?,
                fixed_overhead_len_hint: item.field56?.checked_sub(item.field44?),
                header_len_hint,
                outer_wrapper_len_hint,
                penc_offset: item.penc_offsets.first().copied(),
                hypenc_offset: item.hypenc_offsets.first().copied(),
                penc_offsets: item.penc_offsets.clone(),
                hypenc_offsets: item.hypenc_offsets.clone(),
                penc_to_payload_boundary_delta: signed_offset_delta(
                    item.penc_offsets.first().copied(),
                    header_len_hint,
                ),
                hypenc_to_payload_boundary_delta: signed_offset_delta(
                    item.hypenc_offsets.first().copied(),
                    header_len_hint,
                ),
                penc_to_object_boundary_delta: signed_offset_delta(
                    item.penc_offsets.first().copied(),
                    outer_wrapper_len_hint,
                ),
                hypenc_to_object_boundary_delta: signed_offset_delta(
                    item.hypenc_offsets.first().copied(),
                    outer_wrapper_len_hint,
                ),
                nested_penc_to_payload_boundary_deltas,
                nested_hypenc_to_payload_boundary_deltas,
                segment_count: item.segment_count,
                recv_complete: item.recv_complete,
                captured_complete: item.captured_complete,
                stop_reason: item.stop_reason.clone(),
                oem_answer_summary: item.oem_answer_summary.clone(),
            })
        })
        .collect()
}

fn collect_auth_prefixes(matrix: &Auth7100FlowMatrix) -> Vec<Auth7100ComparablePrefix> {
    let mut counts = BTreeMap::<AuthKey, usize>::new();

    for session in &matrix.sessions {
        accumulate_auth_direction(&mut counts, "client", &session.client_packets);
        accumulate_auth_direction(&mut counts, "server", &session.server_packets);
    }

    counts
        .into_iter()
        .map(|(key, count)| {
            let header_len_hint = key.packet_len_declared.checked_sub(key.payload_len_hint);
            let outer_wrapper_len_hint = key
                .packet_len_declared
                .checked_sub(key.object_span_len_hint);
            let first_penc_offset = key.penc_offsets.first().copied();
            let first_hypenc_offset = key.hypenc_offsets.first().copied();
            let second_penc_offset = key.penc_offsets.get(1).copied();

            Auth7100ComparablePrefix {
                direction: key.direction,
                packet_role: key.packet_role,
                object_type_id: key.object_type_id,
                packet_len_declared: key.packet_len_declared,
                packet_len_duplicate: key.packet_len_duplicate,
                packet_len_matches: key.packet_len_declared == key.packet_len_duplicate,
                field40_constant: key.field40_constant,
                payload_len_hint: key.payload_len_hint,
                attr_flags_hint: key.attr_flags_hint,
                object_span_len_hint: key.object_span_len_hint,
                fixed_overhead_len_hint: key.fixed_overhead_len_hint,
                header_len_hint,
                outer_wrapper_len_hint,
                layout_hint: key.layout_hint,
                penc_offsets: key.penc_offsets,
                hypenc_offsets: key.hypenc_offsets,
                first_penc_offset,
                first_hypenc_offset,
                first_penc_to_payload_boundary_delta: signed_offset_delta(
                    first_penc_offset,
                    header_len_hint,
                ),
                first_hypenc_to_payload_boundary_delta: signed_offset_delta(
                    first_hypenc_offset,
                    header_len_hint,
                ),
                first_penc_to_object_boundary_delta: signed_offset_delta(
                    first_penc_offset,
                    outer_wrapper_len_hint,
                ),
                first_hypenc_to_object_boundary_delta: signed_offset_delta(
                    first_hypenc_offset,
                    outer_wrapper_len_hint,
                ),
                second_penc_offset,
                second_penc_to_payload_boundary_delta: signed_offset_delta(
                    second_penc_offset,
                    header_len_hint,
                ),
                second_penc_to_object_boundary_delta: signed_offset_delta(
                    second_penc_offset,
                    outer_wrapper_len_hint,
                ),
                count,
            }
        })
        .collect()
}

fn accumulate_auth_direction(
    counts: &mut BTreeMap<AuthKey, usize>,
    direction: &str,
    packets: &[crate::Auth7100FlowPacket],
) {
    for packet in packets {
        let hints = &packet.field_hints;
        let key = AuthKey {
            direction: direction.to_string(),
            packet_role: packet.packet_role.clone(),
            object_type_id: hints.object_type_id,
            packet_len_declared: hints.packet_len_declared,
            packet_len_duplicate: hints.packet_len_duplicate,
            field40_constant: hints.field40_constant,
            payload_len_hint: hints.payload_len_hint,
            attr_flags_hint: hints.attr_flags_hint,
            object_span_len_hint: hints.object_span_len_hint,
            fixed_overhead_len_hint: hints.fixed_overhead_len_hint,
            layout_hint: hints.layout_hint.clone(),
            penc_offsets: packet.penc_offsets.clone(),
            hypenc_offsets: packet.hypenc_offsets.clone(),
        };
        *counts.entry(key).or_default() += 1;
    }
}

fn summarize_shared_findings(
    local_prefixes: &[Local2000ComparablePrefix],
    auth_prefixes: &[Auth7100ComparablePrefix],
    bridge_layout_matches: &[BridgeLayoutMatch],
    large_bridge_layout_matches: &[BridgeLayoutMatch],
) -> Vec<String> {
    let mut findings = Vec::new();

    let local_field40 = local_prefixes
        .iter()
        .map(|item| item.field40_constant)
        .collect::<BTreeSet<_>>();
    let auth_field40 = auth_prefixes
        .iter()
        .map(|item| item.field40_constant)
        .collect::<BTreeSet<_>>();
    let shared_field40 = set_intersection(&local_field40, &auth_field40);
    if !shared_field40.is_empty() {
        findings.push(format!(
            "本地 2000 小包与 7100 前缀都复用了 field40 常量 {:?}",
            shared_field40
        ));
    }

    let local_overhead = local_prefixes
        .iter()
        .filter_map(|item| item.fixed_overhead_len_hint)
        .collect::<BTreeSet<_>>();
    let auth_overhead = auth_prefixes
        .iter()
        .filter_map(|item| item.fixed_overhead_len_hint)
        .collect::<BTreeSet<_>>();
    let shared_overhead = set_intersection(&local_overhead, &auth_overhead);
    if !shared_overhead.is_empty() {
        findings.push(format!(
            "本地 2000 小包与 7100 前缀都出现 fixed_overhead_len_hint {:?}",
            shared_overhead
        ));
    }

    if local_prefixes.iter().all(|item| item.packet_len_matches)
        && auth_prefixes.iter().all(|item| item.packet_len_matches)
    {
        findings.push(
            "本地 2000 小包与 7100 前缀都使用了重复长度字段，且当前样本里 declared/duplicate 始终相等"
                .to_string(),
        );
    }

    if !bridge_layout_matches.is_empty() {
        let layouts = bridge_layout_matches
            .iter()
            .map(|item| item.auth_layout_hint.clone())
            .collect::<BTreeSet<_>>();
        let object_types = bridge_layout_matches
            .iter()
            .map(|item| item.auth_object_type_id)
            .collect::<BTreeSet<_>>();
        findings.push(format!(
            "按 field40/attr/fixed_overhead/header_len/outer_wrapper 五组信号做精确桥接后，本地 2000 complete-small 当前只命中 7100 的 layout {:?} / object_type {:?}",
            layouts, object_types
        ));
        if layouts == BTreeSet::from(["download_file_record".to_string()])
            && object_types == BTreeSet::from([1])
        {
            findings.push(
                "当前证据更支持：本地 2000 complete-small 前缀族和 7100 download_file_record 同族，而不是 compressed_zstd/compressed_zstd_dict"
                    .to_string(),
            );
        }

        let shared_penc_payload_delta = bridge_layout_matches
            .iter()
            .filter_map(|item| {
                match (
                    item.local_penc_to_payload_boundary_delta,
                    item.auth_first_penc_to_payload_boundary_delta,
                ) {
                    (Some(local), Some(auth)) if local == auth => Some(local),
                    _ => None,
                }
            })
            .collect::<BTreeSet<_>>();
        if !shared_penc_payload_delta.is_empty() {
            let relations = shared_penc_payload_delta
                .into_iter()
                .map(describe_signed_delta)
                .collect::<Vec<_>>();
            findings.push(format!(
                "桥接命中的本地 2000 与 7100 download_file_record 首个 penc 都落在 {}",
                relations.join(" / ")
            ));
        }

        let shared_penc_offsets = bridge_layout_matches
            .iter()
            .filter_map(|item| {
                match (
                    item.local_penc_offset,
                    item.auth_penc_offsets.first().copied(),
                ) {
                    (Some(local), Some(auth)) if local == auth => Some(local),
                    _ => None,
                }
            })
            .collect::<BTreeSet<_>>();
        if !shared_penc_offsets.is_empty() {
            findings.push(format!(
                "桥接命中的本地 2000 与 7100 download_file_record 首个 penc 还共享精确偏移 {:?}",
                shared_penc_offsets
            ));
        }

        if bridge_layout_matches.iter().any(|item| {
            item.local_hypenc_to_payload_boundary_delta == Some(0)
                && item.auth_hypenc_offsets.is_empty()
        }) {
            findings.push(
                "本地 2000 当前已见到贴着 payload boundary 的 hypenc，但桥接命中的 7100 download_file_record 仍未见 hypenc"
                    .to_string(),
            );
        }

        let auth_second_penc_deltas = bridge_layout_matches
            .iter()
            .filter_map(|item| item.auth_second_penc_to_payload_boundary_delta)
            .collect::<BTreeSet<_>>();
        if !auth_second_penc_deltas.is_empty() {
            let relations = auth_second_penc_deltas
                .into_iter()
                .map(describe_signed_delta)
                .collect::<Vec<_>>();
            findings.push(format!(
                "桥接命中的 7100 download_file_record 还出现第二个 penc，当前落在 {}",
                relations.join(" / ")
            ));
        }
    }

    if !large_bridge_layout_matches.is_empty() {
        let layouts = large_bridge_layout_matches
            .iter()
            .map(|item| item.auth_layout_hint.clone())
            .collect::<BTreeSet<_>>();
        findings.push(format!(
            "本地大对象起始包按同一组信号做精确桥接后，也只命中 {:?}",
            layouts
        ));

        let local_nested_penc_deltas = large_bridge_layout_matches
            .iter()
            .flat_map(|item| {
                item.local_nested_penc_to_payload_boundary_deltas
                    .iter()
                    .copied()
            })
            .collect::<BTreeSet<_>>();
        if !local_nested_penc_deltas.is_empty() {
            let relations = local_nested_penc_deltas
                .into_iter()
                .map(describe_signed_delta)
                .collect::<Vec<_>>();
            findings.push(format!(
                "本地大对象起始包在首个 penc 对齐之后，payload 内还继续出现后续 penc，当前落在 {}",
                relations.join(" / ")
            ));
        }
    }

    findings
}

fn summarize_local_only_findings(
    local_prefixes: &[Local2000ComparablePrefix],
    local_large_prefixes: &[Local2000LargeComparablePrefix],
) -> Vec<String> {
    let mut findings = Vec::new();
    let header_len_hints = local_prefixes
        .iter()
        .filter_map(|item| item.header_len_hint)
        .collect::<BTreeSet<_>>();
    if !header_len_hints.is_empty() {
        findings.push(format!(
            "本地 2000 小包额外暴露了 packet_len_declared - payload_len_hint = {:?}，当前主样本固定为 68",
            header_len_hints
        ));
    }

    let attr_flags = local_prefixes
        .iter()
        .map(|item| item.attr_flags_hint)
        .collect::<BTreeSet<_>>();
    findings.push(format!(
        "本地 2000 完整小包当前只看到 attr_flags_hint {:?}",
        attr_flags
    ));

    let penc_offsets = local_prefixes
        .iter()
        .filter_map(|item| item.penc_offset)
        .collect::<BTreeSet<_>>();
    if !penc_offsets.is_empty() {
        findings.push(format!(
            "本地 2000 小包能直接定位到 penc 偏移 {:?}",
            penc_offsets
        ));
    }

    let hypenc_offsets = local_prefixes
        .iter()
        .filter_map(|item| item.hypenc_offset)
        .collect::<BTreeSet<_>>();
    if !hypenc_offsets.is_empty() {
        findings.push(format!(
            "本地 2000 小包在少数样本中还能直接定位到 hypenc 偏移 {:?}",
            hypenc_offsets
        ));
    }

    let penc_payload_deltas = local_prefixes
        .iter()
        .filter_map(|item| item.penc_to_payload_boundary_delta)
        .collect::<BTreeSet<_>>();
    if !penc_payload_deltas.is_empty() {
        findings.push(format!(
            "本地 2000 小包首个 penc 相对 payload boundary 的偏移当前固定为 {:?}",
            penc_payload_deltas
        ));
    }

    let hypenc_payload_deltas = local_prefixes
        .iter()
        .filter_map(|item| item.hypenc_to_payload_boundary_delta)
        .collect::<BTreeSet<_>>();
    if !hypenc_payload_deltas.is_empty() {
        findings.push(format!(
            "本地 2000 小包首个 hypenc 相对 payload boundary 的偏移当前固定为 {:?}",
            hypenc_payload_deltas
        ));
    }

    let field48_values = local_prefixes
        .iter()
        .map(|item| item.field48)
        .collect::<BTreeSet<_>>();
    findings.push(format!(
        "本地 2000 完整小包当前看到 field48 {:?}；它暂时不像远端 object_type_id，因为最强桥接命中的远端族是 object_type=1",
        field48_values
    ));

    if !local_large_prefixes.is_empty() {
        let segment_counts = local_large_prefixes
            .iter()
            .map(|item| item.segment_count)
            .collect::<BTreeSet<_>>();
        findings.push(format!(
            "本地大对象起始包当前样本覆盖的 segment_count {:?}，说明它和 complete-small 共享前缀族，但后续主体会跨 recv 连续搬运",
            segment_counts
        ));

        let large_penc_offsets = local_large_prefixes
            .iter()
            .flat_map(|item| item.penc_offsets.iter().copied())
            .collect::<BTreeSet<_>>();
        let large_hypenc_offsets = local_large_prefixes
            .iter()
            .flat_map(|item| item.hypenc_offsets.iter().copied())
            .collect::<BTreeSet<_>>();
        findings.push(format!(
            "本地大对象起始包当前已见到 penc offsets {:?} / hypenc offsets {:?}",
            large_penc_offsets, large_hypenc_offsets
        ));

        let large_nested_penc_payload_deltas = local_large_prefixes
            .iter()
            .flat_map(|item| item.nested_penc_to_payload_boundary_deltas.iter().copied())
            .collect::<BTreeSet<_>>();
        if !large_nested_penc_payload_deltas.is_empty() {
            findings.push(format!(
                "本地大对象在 payload 内继续出现后续 penc，相对 payload boundary 的偏移为 {:?}",
                large_nested_penc_payload_deltas
            ));
        }
    }

    findings
}

fn summarize_auth_only_findings(auth_prefixes: &[Auth7100ComparablePrefix]) -> Vec<String> {
    let mut findings = Vec::new();

    let attr_flags = auth_prefixes
        .iter()
        .map(|item| item.attr_flags_hint)
        .collect::<BTreeSet<_>>();
    findings.push(format!(
        "7100 前缀当前已出现 attr_flags_hint {:?}",
        attr_flags
    ));

    let layouts = auth_prefixes
        .iter()
        .map(|item| item.layout_hint.clone())
        .collect::<BTreeSet<_>>();
    findings.push(format!("7100 前缀已经能区分 layout_hint {:?}", layouts));

    let object_types = auth_prefixes
        .iter()
        .map(|item| item.object_type_id)
        .collect::<BTreeSet<_>>();
    findings.push(format!(
        "7100 前缀当前样本覆盖的 object_type_id {:?}",
        object_types
    ));

    if let Some(download) = auth_prefixes
        .iter()
        .find(|item| item.layout_hint == "download_file_record")
    {
        findings.push(format!(
            "7100 download_file_record 当前可直接定位到 penc 偏移 {:?}",
            download.penc_offsets
        ));
        if let Some(delta) = download.first_penc_to_payload_boundary_delta {
            findings.push(format!(
                "7100 download_file_record 首个 penc 相对 payload boundary 的偏移是 {delta}"
            ));
        }
        if let Some(offset) = download.second_penc_offset {
            findings.push(format!(
                "7100 download_file_record 还出现第二个 penc@{offset}，相对 payload boundary 的偏移是 {:?}",
                download.second_penc_to_payload_boundary_delta
            ));
        }
    }

    findings
}

fn set_intersection<T: Clone + Ord>(left: &BTreeSet<T>, right: &BTreeSet<T>) -> Vec<T> {
    left.intersection(right).cloned().collect()
}

fn signed_offset_delta(offset: Option<usize>, boundary: Option<u32>) -> Option<i64> {
    Some(offset? as i64 - i64::from(boundary?))
}

fn describe_signed_delta(delta: i64) -> String {
    match delta.cmp(&0) {
        std::cmp::Ordering::Less => format!("payload boundary 前 {} 字节", delta.unsigned_abs()),
        std::cmp::Ordering::Equal => "payload boundary 处".to_string(),
        std::cmp::Ordering::Greater => format!("payload boundary 后 {} 字节", delta),
    }
}

fn collect_bridge_layout_matches(
    local_prefixes: &[Local2000ComparablePrefix],
    auth_prefixes: &[Auth7100ComparablePrefix],
) -> Vec<BridgeLayoutMatch> {
    let mut matches = Vec::new();

    for local in local_prefixes {
        for auth in auth_prefixes {
            if local.field40_constant != auth.field40_constant
                || local.attr_flags_hint != auth.attr_flags_hint
                || local.fixed_overhead_len_hint != auth.fixed_overhead_len_hint
                || local.header_len_hint != auth.header_len_hint
                || local.outer_wrapper_len_hint != auth.outer_wrapper_len_hint
            {
                continue;
            }

            let mut shared_signals = Vec::new();
            shared_signals.push(format!("field40={}", local.field40_constant));
            shared_signals.push(format!("attr={}", local.attr_flags_hint));
            if let Some(value) = local.fixed_overhead_len_hint {
                shared_signals.push(format!("fixed_overhead={value}"));
            }
            if let Some(value) = local.header_len_hint {
                shared_signals.push(format!("header_len={value}"));
            }
            if let Some(value) = local.outer_wrapper_len_hint {
                shared_signals.push(format!("outer_wrapper={value}"));
            }
            if let (Some(local_offset), Some(auth_offset)) =
                (local.penc_offset, auth.penc_offsets.first().copied())
            {
                if local_offset == auth_offset {
                    shared_signals.push(format!("penc@{local_offset}"));
                }
            }
            if let (Some(local_delta), Some(auth_delta)) = (
                local.penc_to_payload_boundary_delta,
                auth.first_penc_to_payload_boundary_delta,
            ) {
                if local_delta == auth_delta {
                    shared_signals.push(format!("penc_to_payload_delta={local_delta}"));
                }
            }

            matches.push(BridgeLayoutMatch {
                local_source_kind: "complete-small".to_string(),
                local_packet_len_declared: local.packet_len_declared,
                local_payload_len_hint: local.payload_len_hint,
                local_attr_flags_hint: local.attr_flags_hint,
                local_object_span_len_hint: local.object_span_len_hint,
                local_header_len_hint: local.header_len_hint,
                local_outer_wrapper_len_hint: local.outer_wrapper_len_hint,
                local_penc_offset: local.penc_offset,
                local_hypenc_offset: local.hypenc_offset,
                local_penc_offsets: local.penc_offset.into_iter().collect(),
                local_hypenc_offsets: local.hypenc_offset.into_iter().collect(),
                local_penc_to_payload_boundary_delta: local.penc_to_payload_boundary_delta,
                local_hypenc_to_payload_boundary_delta: local.hypenc_to_payload_boundary_delta,
                local_penc_to_object_boundary_delta: local.penc_to_object_boundary_delta,
                local_hypenc_to_object_boundary_delta: local.hypenc_to_object_boundary_delta,
                local_nested_penc_to_payload_boundary_deltas: Vec::new(),
                auth_direction: auth.direction.clone(),
                auth_packet_role: auth.packet_role.clone(),
                auth_object_type_id: auth.object_type_id,
                auth_layout_hint: auth.layout_hint.clone(),
                auth_packet_len_declared: auth.packet_len_declared,
                auth_payload_len_hint: auth.payload_len_hint,
                auth_attr_flags_hint: auth.attr_flags_hint,
                auth_object_span_len_hint: auth.object_span_len_hint,
                auth_header_len_hint: auth.header_len_hint,
                auth_outer_wrapper_len_hint: auth.outer_wrapper_len_hint,
                auth_penc_offsets: auth.penc_offsets.clone(),
                auth_hypenc_offsets: auth.hypenc_offsets.clone(),
                auth_first_penc_to_payload_boundary_delta: auth
                    .first_penc_to_payload_boundary_delta,
                auth_first_hypenc_to_payload_boundary_delta: auth
                    .first_hypenc_to_payload_boundary_delta,
                auth_first_penc_to_object_boundary_delta: auth.first_penc_to_object_boundary_delta,
                auth_first_hypenc_to_object_boundary_delta: auth
                    .first_hypenc_to_object_boundary_delta,
                auth_second_penc_offset: auth.second_penc_offset,
                auth_second_penc_to_payload_boundary_delta: auth
                    .second_penc_to_payload_boundary_delta,
                auth_second_penc_to_object_boundary_delta: auth
                    .second_penc_to_object_boundary_delta,
                shared_signals,
            });
        }
    }

    matches
}

fn collect_large_bridge_layout_matches(
    local_prefixes: &[Local2000LargeComparablePrefix],
    auth_prefixes: &[Auth7100ComparablePrefix],
) -> Vec<BridgeLayoutMatch> {
    let mut matches = Vec::new();

    for local in local_prefixes {
        for auth in auth_prefixes {
            if local.field40_constant != auth.field40_constant
                || local.attr_flags_hint != auth.attr_flags_hint
                || local.fixed_overhead_len_hint != auth.fixed_overhead_len_hint
                || local.header_len_hint != auth.header_len_hint
                || local.outer_wrapper_len_hint != auth.outer_wrapper_len_hint
            {
                continue;
            }

            let mut shared_signals = Vec::new();
            shared_signals.push(format!("field40={}", local.field40_constant));
            shared_signals.push(format!("attr={}", local.attr_flags_hint));
            if let Some(value) = local.fixed_overhead_len_hint {
                shared_signals.push(format!("fixed_overhead={value}"));
            }
            if let Some(value) = local.header_len_hint {
                shared_signals.push(format!("header_len={value}"));
            }
            if let Some(value) = local.outer_wrapper_len_hint {
                shared_signals.push(format!("outer_wrapper={value}"));
            }
            if let (Some(local_offset), Some(auth_offset)) =
                (local.penc_offset, auth.penc_offsets.first().copied())
            {
                if local_offset == auth_offset {
                    shared_signals.push(format!("penc@{local_offset}"));
                }
            }
            if let (Some(local_delta), Some(auth_delta)) = (
                local.penc_to_payload_boundary_delta,
                auth.first_penc_to_payload_boundary_delta,
            ) {
                if local_delta == auth_delta {
                    shared_signals.push(format!("penc_to_payload_delta={local_delta}"));
                }
            }

            matches.push(BridgeLayoutMatch {
                local_source_kind: "large-start".to_string(),
                local_packet_len_declared: local.packet_len_declared,
                local_payload_len_hint: local.payload_len_hint,
                local_attr_flags_hint: local.attr_flags_hint,
                local_object_span_len_hint: local.object_span_len_hint,
                local_header_len_hint: local.header_len_hint,
                local_outer_wrapper_len_hint: local.outer_wrapper_len_hint,
                local_penc_offset: local.penc_offset,
                local_hypenc_offset: local.hypenc_offset,
                local_penc_offsets: local.penc_offsets.clone(),
                local_hypenc_offsets: local.hypenc_offsets.clone(),
                local_penc_to_payload_boundary_delta: local.penc_to_payload_boundary_delta,
                local_hypenc_to_payload_boundary_delta: local.hypenc_to_payload_boundary_delta,
                local_penc_to_object_boundary_delta: local.penc_to_object_boundary_delta,
                local_hypenc_to_object_boundary_delta: local.hypenc_to_object_boundary_delta,
                local_nested_penc_to_payload_boundary_deltas: local
                    .nested_penc_to_payload_boundary_deltas
                    .clone(),
                auth_direction: auth.direction.clone(),
                auth_packet_role: auth.packet_role.clone(),
                auth_object_type_id: auth.object_type_id,
                auth_layout_hint: auth.layout_hint.clone(),
                auth_packet_len_declared: auth.packet_len_declared,
                auth_payload_len_hint: auth.payload_len_hint,
                auth_attr_flags_hint: auth.attr_flags_hint,
                auth_object_span_len_hint: auth.object_span_len_hint,
                auth_header_len_hint: auth.header_len_hint,
                auth_outer_wrapper_len_hint: auth.outer_wrapper_len_hint,
                auth_penc_offsets: auth.penc_offsets.clone(),
                auth_hypenc_offsets: auth.hypenc_offsets.clone(),
                auth_first_penc_to_payload_boundary_delta: auth
                    .first_penc_to_payload_boundary_delta,
                auth_first_hypenc_to_payload_boundary_delta: auth
                    .first_hypenc_to_payload_boundary_delta,
                auth_first_penc_to_object_boundary_delta: auth.first_penc_to_object_boundary_delta,
                auth_first_hypenc_to_object_boundary_delta: auth
                    .first_hypenc_to_object_boundary_delta,
                auth_second_penc_offset: auth.second_penc_offset,
                auth_second_penc_to_payload_boundary_delta: auth
                    .second_penc_to_payload_boundary_delta,
                auth_second_penc_to_object_boundary_delta: auth
                    .second_penc_to_object_boundary_delta,
                shared_signals,
            });
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::analyze_local_2000_vs_auth7100_sample;

    #[test]
    fn sample_comparison_surfaces_shared_invariants() {
        let analysis = analyze_local_2000_vs_auth7100_sample().expect("sample comparison");
        assert!(analysis.local_complete_small_packets > 0);
        assert!(analysis.auth_packet_count > 0);
        assert!(
            analysis
                .shared_findings
                .iter()
                .any(|item| item.contains("field40"))
        );
        assert!(
            analysis
                .shared_findings
                .iter()
                .any(|item| item.contains("fixed_overhead_len_hint"))
        );
        assert!(
            analysis
                .shared_findings
                .iter()
                .any(|item| item.contains("download_file_record"))
        );
        assert!(!analysis.bridge_layout_matches.is_empty());
        assert!(!analysis.local_large_prefixes.is_empty());
        assert!(!analysis.large_bridge_layout_matches.is_empty());
        assert!(analysis.bridge_layout_matches.iter().all(|item| {
            item.auth_layout_hint == "download_file_record" && item.auth_object_type_id == 1
        }));
        assert!(analysis.bridge_layout_matches.iter().all(|item| {
            item.local_penc_to_payload_boundary_delta == Some(-8)
                && item.auth_first_penc_to_payload_boundary_delta == Some(-8)
                && item.auth_penc_offsets.first().copied() == Some(60)
                && item.auth_second_penc_offset == Some(416)
                && item.auth_second_penc_to_payload_boundary_delta == Some(348)
                && item.shared_signals.iter().any(|signal| signal == "penc@60")
        }));
        assert!(analysis.large_bridge_layout_matches.iter().all(|item| {
            item.auth_layout_hint == "download_file_record"
                && item.auth_object_type_id == 1
                && item.local_source_kind == "large-start"
                && item
                    .local_nested_penc_to_payload_boundary_deltas
                    .starts_with(&[2, 190])
        }));
        assert!(
            analysis
                .local_only_findings
                .iter()
                .any(|item| item.contains("68"))
        );
        assert!(
            analysis
                .shared_findings
                .iter()
                .any(|item| item.contains("payload boundary 前"))
        );
        assert!(
            analysis
                .shared_findings
                .iter()
                .any(|item| item.contains("第二个 penc"))
        );
        assert!(
            analysis
                .auth_only_findings
                .iter()
                .any(|item| item.contains("penc 偏移"))
        );
    }
}
