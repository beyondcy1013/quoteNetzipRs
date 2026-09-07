const state = {
  resultTitle: "尚未发起请求",
  resultSummary: "使用左侧表单调用服务。",
  resultMeta: [],
  resultBody: "",
  logs: ["网页界面已就绪"],
};

const HEALTH_TITLE = "健康检查";
const DEFAULT_SERVICE_ORIGIN = "http://127.0.0.1:16893";
const DEFAULT_REMOTE_HOST = "";

const elements = {
  serviceStatus: document.querySelector("#serviceStatus"),
  resultTitle: document.querySelector("#resultTitle"),
  resultSummary: document.querySelector("#resultSummary"),
  resultMeta: document.querySelector("#resultMeta"),
  resultBody: document.querySelector("#resultBody"),
  logList: document.querySelector("#logList"),
};

const legacyElements = {
  accountExpiry: document.querySelector("#legacyAccountExpiry"),
  connectionState: document.querySelector("#legacyConnectionState"),
  statusBar: document.querySelector("#legacyStatusBar"),
  protocolNote: document.querySelector("#legacyProtocolNote"),
  authServer: document.querySelector("#legacyAuthServer"),
  stockMainServer: document.querySelector("#legacyStockMainServer"),
  stockBackupServer: document.querySelector("#legacyStockBackupServer"),
  reservedServer: document.querySelector("#legacyReservedServer"),
  wenhuaServer: document.querySelector("#legacyWenhuaServer"),
  boyiServer: document.querySelector("#legacyBoyiServer"),
  connectRadio: document.querySelector("#legacyConnectRadio"),
  disconnectRadio: document.querySelector("#legacyDisconnectRadio"),
};

const legacyState = {
  passwordInfoUrlTemplate: null,
  authServers: [],
  dataServers: [],
};

const pageTabButtons = Array.from(document.querySelectorAll("[data-page-tab-target]"));
const pageTabPanels = Array.from(document.querySelectorAll("[data-page-tab-panel]"));
const toolTabButtons = Array.from(document.querySelectorAll("[data-tool-tab-target]"));
const toolTabPanels = Array.from(document.querySelectorAll("[data-tool-tab-panel]"));

function render() {
  elements.resultTitle.textContent = state.resultTitle;
  elements.resultSummary.textContent = state.resultSummary;
  elements.resultMeta.innerHTML = "";
  for (const itemText of state.resultMeta) {
    const item = document.createElement("span");
    item.className = "meta-pill";
    item.textContent = itemText;
    elements.resultMeta.appendChild(item);
  }
  elements.resultBody.value = state.resultBody;
  elements.logList.innerHTML = "";
  for (const line of state.logs.slice(0, 20)) {
    const item = document.createElement("div");
    item.textContent = line;
    elements.logList.appendChild(item);
  }
}

function pushLog(message) {
  state.logs.unshift(`${new Date().toLocaleTimeString()}  ${message}`);
  render();
}

function baseUrl() {
  if (window.location.origin && window.location.origin !== "null") {
    return window.location.origin.replace(/\/$/, "");
  }
  return DEFAULT_SERVICE_ORIGIN;
}

function hexOf(value) {
  return `0x${Number(value).toString(16)}`;
}

function joinHexList(values) {
  return values.map((value) => hexOf(value)).join(", ");
}

function probeSummary(probe) {
  if (!probe || typeof probe !== "object") {
    return "?";
  }
  if (probe.reachable) {
    return `可达 ${probe.latency_ms ?? 0}ms`;
  }
  return probe.error || "不可达";
}

async function callApi(title, path, options = {}) {
  pushLog(`${options.method || "GET"} ${path}`);
  try {
    const data = await requestJson(path, options);
    applyResult(title, data);
    if (title === HEALTH_TITLE) {
      elements.serviceStatus.textContent = state.resultSummary;
    }
    pushLog(`${title}: 成功`);
  } catch (error) {
    state.resultTitle = title;
    state.resultSummary = String(error);
    state.resultMeta = [];
    state.resultBody = "";
    if (title === HEALTH_TITLE) {
      elements.serviceStatus.textContent = `服务错误: ${error}`;
    }
    pushLog(`${title}: 失败`);
  }
  render();
}

async function requestJson(path, options = {}) {
  const response = await fetch(`${baseUrl()}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(options.headers || {}),
    },
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${text}`);
  }
  return JSON.parse(text);
}

function applyResult(title, data) {
  state.resultTitle = title;
  state.resultSummary = summarize(data);
  state.resultMeta = metaSummary(data);
  state.resultBody = JSON.stringify(data, null, 2);
  render();
}

function summarize(data) {
  if (typeof data?.config_source === "string" && Array.isArray(data?.auth_servers)) {
    const parts = [
      `认证服务器 = ${data.auth_servers.length}`,
      `数据服务器 = ${Array.isArray(data.data_servers) ? data.data_servers.length : 0}`,
    ];
    if (typeof data?.config?.account === "string") {
      parts.push(`账号 = ${data.config.account}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.recommended_auth_server === "string" && Array.isArray(data?.auth_servers)) {
    const parts = [
      `认证候选 = ${data.auth_servers.length}`,
      `推荐认证 = ${data.recommended_auth_server}`,
    ];
    if (typeof data.recommended_stock_backup_server === "string") {
      parts.push(`推荐备用 = ${data.recommended_stock_backup_server}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.protocol_login_state === "string" && typeof data?.login_status_text === "string") {
    return `连接试探 = ${data.login_status_text}, 状态 = ${data.protocol_login_state}`;
  }
  if (typeof data?.saved === "boolean" && typeof data?.status_text === "string" && data?.config) {
    return `本地配置保存 = ${data.saved}, 状态 = ${data.status_text}`;
  }
  if (typeof data?.status === "string") {
    return `服务状态 = ${data.status}`;
  }
  if (typeof data?.ok === "boolean") {
    return `健康检查 = ${data.ok}`;
  }
  if (typeof data?.records_total === "number") {
    const parts = [`记录总数 = ${data.records_total}`];
    if (typeof data.record_size === "number") {
      parts.push(`记录大小 = ${data.record_size}`);
    }
    if (typeof data.reply_frames === "number") {
      parts.push(`回复帧数 = ${data.reply_frames}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.packet_kind === "string") {
    return `数据包类型 = ${data.packet_kind}, 大小 = ${data.size_bytes ?? "?"}`;
  }
  if (typeof data?.matches_total === "number" && Array.isArray(data?.preview)) {
    const parts = [
      `查询 = ${data.query ?? ""}`,
      `匹配数 = ${data.matches_total}`,
      `记录总数 = ${data.records_total ?? "?"}`,
    ];
    if (typeof data.preview?.[0]?.decimal_point === "number") {
      parts.push(`小数位 = ${data.preview[0].decimal_point}`);
    }
    if (typeof data.preview?.[0]?.pre_close === "number") {
      parts.push(`昨收 = ${data.preview[0].pre_close}`);
    }
    if (typeof data.reply_frames === "number") {
      parts.push(`回复帧数 = ${data.reply_frames}`);
    }
    return parts.join(", ");
  }
  if (Array.isArray(data?.requested_symbols) && typeof data?.quote_reply_frames === "number") {
    const parts = [
      `代码 = ${data.requested_symbols.join(", ")}`,
      `匹配 = ${data.matched_records ?? "?"}`,
    ];
    if (Array.isArray(data.unmatched_symbols) && data.unmatched_symbols.length) {
      parts.push(`未匹配 = ${data.unmatched_symbols.join(", ")}`);
    }
    if (typeof data.parsed_quote_bodies === "number") {
      parts.push(`行情体数 = ${data.parsed_quote_bodies}`);
    }
    if (typeof data.quote_reply_frames === "number") {
      parts.push(`行情帧数 = ${data.quote_reply_frames}`);
    }
    const first = data.first_match || data.preview?.[0];
    if (typeof first?.normalized_quote_head?.price === "number") {
      parts.push(`归一价 = ${first.normalized_quote_head.price.toFixed(2)}`);
    }
    if (
      typeof first?.normalized_change_value === "number" &&
      typeof first?.normalized_change_percent === "number"
    ) {
      parts.push(
        `涨跌 = ${first.normalized_change_value >= 0 ? "+" : ""}${first.normalized_change_value.toFixed(2)} (${first.normalized_change_percent >= 0 ? "+" : ""}${first.normalized_change_percent.toFixed(2)}%)`,
      );
    }
    return parts.join(", ");
  }
  if (typeof data?.bars_total === "number" && Array.isArray(data?.preview)) {
    const parts = [
      `股票 = ${data.symbol ?? "?"}`,
      `K线 = ${data.kline_type ?? data.category ?? "?"}`,
      `条数 = ${data.bars_total}`,
    ];
    if (typeof data.kline_reply_frames === "number") {
      parts.push(`帧数 = ${data.kline_reply_frames}`);
    }
    const last = data.last_bar || data.preview[data.preview.length - 1];
    if (last?.datetime) {
      parts.push(`最后时间 = ${last.datetime}`);
    }
    if (typeof last?.close === "number") {
      parts.push(`收盘 = ${last.close.toFixed(3)}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.categories_total === "number" && Array.isArray(data?.preview)) {
    const parts = [
      `股票 = ${data.symbol ?? "?"}`,
      `类目数 = ${data.categories_total}`,
    ];
    const first = data.first_category || data.preview[0];
    if (first?.name) {
      parts.push(`首项 = ${first.name}`);
    }
    if (typeof data.category_reply_frames === "number") {
      parts.push(`帧数 = ${data.category_reply_frames}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.content_chars_total === "number" && typeof data?.content_preview === "string") {
    const parts = [
      `股票 = ${data.symbol ?? "?"}`,
      `文件名 = ${data.filename ?? "?"}`,
      `字符数 = ${data.content_chars_total}`,
    ];
    if (data.resolved_category_name) {
      parts.push(`类目 = ${data.resolved_category_name}`);
    }
    if (typeof data.content_reply_frames === "number") {
      parts.push(`帧数 = ${data.content_reply_frames}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.matched_records === "number" && Array.isArray(data?.preview)) {
    const parts = [
      `查询 = ${data.query ?? ""}`,
      `匹配 = ${data.matched_records}`,
    ];
    const decimalPointSource = quote0547DecimalPointSource(data);
    if (decimalPointSource) {
      parts.push(decimalPointSource);
    }
    if (typeof data.xor93_count === "number") {
      parts.push(`xor93 数量 = ${data.xor93_count}`);
    }
    if (typeof data.filtered_records === "number") {
      parts.push(`过滤后 = ${data.filtered_records}`);
    }
    if (typeof data.count_matches_records === "boolean") {
      parts.push(`计数匹配 = ${data.count_matches_records}`);
    }
    const first = data.first_match || data.preview[0];
    const normalizedPrice = first?.normalized_quote_head?.price;
    const rawPrice = first?.quote_head?.price;
    const normalizedChangeValue = first?.normalized_change_value;
    const normalizedChangePercent = first?.normalized_change_percent;
    const normalizedAmplitudePercent = first?.normalized_amplitude_percent;
    const normalizedOpenGapValue = first?.normalized_open_gap_value;
    const normalizedOpenGapPercent = first?.normalized_open_gap_percent;
    const normalizedReturnFromOpenPercent = first?.normalized_return_from_open_percent;
    const normalizedDrawdownFromHighPercent = first?.normalized_drawdown_from_high_percent;
    if (typeof normalizedPrice === "number") {
      parts.push(`归一价 = ${normalizedPrice.toFixed(2)}`);
    }
    if (typeof normalizedChangeValue === "number" && typeof normalizedChangePercent === "number") {
      parts.push(
        `涨跌 = ${normalizedChangeValue >= 0 ? "+" : ""}${normalizedChangeValue.toFixed(2)} (${normalizedChangePercent >= 0 ? "+" : ""}${normalizedChangePercent.toFixed(2)}%)`,
      );
    }
    if (typeof normalizedAmplitudePercent === "number") {
      parts.push(`振幅 = ${normalizedAmplitudePercent >= 0 ? "+" : ""}${normalizedAmplitudePercent.toFixed(2)}%`);
    }
    if (typeof normalizedOpenGapValue === "number" && typeof normalizedOpenGapPercent === "number") {
      parts.push(
        `开盘缺口 = ${normalizedOpenGapValue >= 0 ? "+" : ""}${normalizedOpenGapValue.toFixed(2)} (${normalizedOpenGapPercent >= 0 ? "+" : ""}${normalizedOpenGapPercent.toFixed(2)}%)`,
      );
    }
    if (typeof normalizedReturnFromOpenPercent === "number") {
      parts.push(`相对开盘 = ${normalizedReturnFromOpenPercent >= 0 ? "+" : ""}${normalizedReturnFromOpenPercent.toFixed(2)}%`);
    }
    if (typeof normalizedDrawdownFromHighPercent === "number") {
      parts.push(`相对高点 = ${normalizedDrawdownFromHighPercent >= 0 ? "+" : ""}${normalizedDrawdownFromHighPercent.toFixed(2)}%`);
    }
    if (typeof first?.code_table_pre_close_matches === "boolean") {
      const delta = first?.code_table_pre_close_delta;
      const detail =
        typeof delta === "number" ? ` Δ昨收=${delta >= 0 ? "+" : ""}${delta.toFixed(6)}` : "";
      parts.push(`昨收匹配 = ${first.code_table_pre_close_matches}${detail}`);
    }
    if (typeof rawPrice === "number") {
      parts.push(`原始价 = ${rawPrice.toFixed(2)}`);
    }
    return parts.join(", ");
  }
  if (quote0547ExtraProfileSummary(data)) {
    const parts = [];
    if (typeof data.path === "string" && data.path.length) {
      parts.push(`路径 = ${data.path}`);
    }
    if (typeof data.filtered_records === "number") {
      parts.push(`过滤后 = ${data.filtered_records}`);
    }
    if (typeof data.records_with_complete_extra_tuple === "number") {
      parts.push(`完整元组 = ${data.records_with_complete_extra_tuple}`);
    }
    if (typeof data.default_pattern_count === "number") {
      parts.push(`默认模式 = ${data.default_pattern_count}`);
    }
    if (typeof data.anomaly_count === "number") {
      parts.push(`异常数 = ${data.anomaly_count}`);
    }
    if (typeof data.code_table_lookup_error === "string" && data.code_table_lookup_error.length) {
      parts.push("代码表查询 = 错误");
    }
    return parts.join(", ");
  }
  if (typeof data?.record_found === "boolean" && data.field_id === undefined) {
    const parts = [
      `股票 = ${data.record?.symbol || data.symbol || "?"}`,
      `找到记录 = ${data.record_found}`,
    ];
    if (data.record && typeof data.record.quarter !== "undefined") {
      parts.push(`季度 = ${data.record.quarter}`);
    }
    if (data.record && typeof data.record.mg_shou_yi === "number") {
      parts.push(`每股收益 = ${data.record.mg_shou_yi}`);
    }
    if (data.record && typeof data.record.jing_li_run === "number") {
      parts.push(`净利润 = ${data.record.jing_li_run}`);
    }
    return parts.join(", ");
  }
  if (Array.isArray(data?.packets) && typeof data?.probe_hello_len === "number") {
    const parts = [
      `probe_hello 长度 = ${data.probe_hello_len}`,
      `启动包数 = ${data.packets.length}`,
    ];
    if (data.packets[0]?.label) {
      parts.push(`首包 = ${data.packets[0].label}`);
    }
    if (data.packets[data.packets.length - 1]?.label) {
      parts.push(`末包 = ${data.packets[data.packets.length - 1].label}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.expected_plain_len === "number" && typeof data?.expected_cipher_len === "number") {
    const parts = [
      `明文长度 = 0x${Number(data.expected_plain_len).toString(16)}`,
      `密文长度 = 0x${Number(data.expected_cipher_len).toString(16)}`,
      `就绪 = ${Boolean(data.ready)}`,
    ];
    if (typeof data.status === "string") {
      parts.push(`状态 = ${data.status}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.compare_len === "number" && typeof data?.diff_bytes === "number") {
    const parts = [
      `对比长度 = ${data.compare_len}`,
      `差异字节 = ${data.diff_bytes}`,
      `完全一致 = ${data.exact_match}`,
    ];
    if (typeof data.first_diff_offset === "number") {
      parts.push(`首个差异 = ${data.first_diff_offset}`);
    }
    return parts.join(", ");
  }
  if (Array.isArray(data?.specs)) {
    const parts = [`取值器规格数 = ${data.specs.length}`];
    if (typeof data.confirmed_count === "number") {
      parts.push(`已确认 = ${data.confirmed_count}`);
    }
    if (typeof data.derived_count === "number") {
      parts.push(`推导 = ${data.derived_count}`);
    }
    if (typeof data.unresolved_count === "number") {
      parts.push(`未解 = ${data.unresolved_count}`);
    }
    const first = data.specs[0];
    const last = data.specs[data.specs.length - 1];
    if (first?.field_id !== undefined) {
      parts.push(`首项 = ${hexOf(first.field_id)}`);
    }
    if (last?.field_id !== undefined) {
      parts.push(`末项 = ${hexOf(last.field_id)}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.field_id === "number" && typeof data?.record_found === "boolean") {
    const parts = [
      `股票 = ${data.symbol ?? "?"}`,
      `字段 ID = 0x${Number(data.field_id).toString(16)}`,
      `找到记录 = ${data.record_found}`,
    ];
    if (typeof data.getter_name === "string") {
      parts.push(`取值器 = ${data.getter_name}`);
    }
    if (typeof data.value === "number") {
      parts.push(`值 = ${data.value}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.segment_count === "number" && typeof data?.reply_bytes === "number") {
    const parts = [
      `目标 = ${data.target ?? "?"}`,
      `分段数 = ${data.segment_count}`,
      `回复字节 = ${data.reply_bytes}`,
    ];
    if (typeof data.sent_bytes === "number") {
      parts.push(`发送字节 = ${data.sent_bytes}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.summary === "object" && data.summary) {
    const parts = [];
    if (typeof data.summary.client_frames === "number") {
      parts.push(`客户端帧 = ${data.summary.client_frames}`);
    }
    if (typeof data.summary.server_frames === "number") {
      parts.push(`服务端帧 = ${data.summary.server_frames}`);
    }
    if (typeof data.summary.labeled_client_frames === "number") {
      parts.push(`已标注客户端帧 = ${data.summary.labeled_client_frames}`);
    }
    if (typeof data.summary.labeled_server_frames === "number") {
      parts.push(`已标注服务端帧 = ${data.summary.labeled_server_frames}`);
    }
    if (data.summary.main_site_validate_after_tag_block8) {
      const block = data.summary.main_site_validate_after_tag_block8;
      parts.push(`0x7b00 块统计 = ${block.total_blocks}/${block.unique_blocks}`);
    }
    return parts.length ? parts.join(", ") : "帧扫描完成";
  }
  if (typeof data?.request_bytes === "number" && typeof data?.reply_bytes === "number") {
    const parts = [
      `目标 = ${data.target ?? "?"}`,
      `请求字节 = ${data.request_bytes}`,
      `回复字节 = ${data.reply_bytes}`,
    ];
    if (typeof data.parsed_kind === "string") {
      parts.push(`解析类型 = ${data.parsed_kind}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.pcap_packets === "number") {
    const parts = [`PCAP 包数 = ${data.pcap_packets}`];
    if (typeof data.unique_packets === "number") {
      parts.push(`唯一包数 = ${data.unique_packets}`);
    }
    if (typeof data.truncated_unique_packets === "number") {
      parts.push(`截断包数 = ${data.truncated_unique_packets}`);
    }
    if (Array.isArray(data.flows)) {
      parts.push(`流数量 = ${data.flows.length}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.byte_profile?.entropy === "number") {
    const parts = [
      `大小 = ${data.size ?? "?"}`,
      `熵 = ${data.byte_profile.entropy.toFixed(3)}`,
    ];
    if (Array.isArray(data.netpacket_runs)) {
      parts.push(`netpacket 段数 = ${data.netpacket_runs.length}`);
    }
    if (Array.isArray(data.zlib_hits)) {
      parts.push(`zlib 命中 = ${data.zlib_hits.length}`);
    }
    return parts.join(", ");
  }
  if (typeof data?.mode === "string" && (Array.isArray(data.server_frames) || Array.isArray(data.client_frames))) {
    const parts = [
      `模式 = ${data.mode}`,
      `服务端帧 = ${Array.isArray(data.server_frames) ? data.server_frames.length : 0}`,
      `客户端帧 = ${Array.isArray(data.client_frames) ? data.client_frames.length : 0}`,
    ];
    return parts.join(", ");
  }
  if (typeof data?.service === "string") {
    return `服务 = ${data.service}`;
  }
  return "请求完成";
}

function metaSummary(data) {
  if (typeof data?.config_source === "string" && Array.isArray(data?.auth_servers)) {
    const meta = [`配置来源: ${data.config_source}`];
    if (typeof data.servers_source === "string") {
      meta.push(`服务器列表: ${data.servers_source}`);
    }
    if (typeof data.state_path === "string") {
      meta.push(`本地状态: ${data.state_path}`);
    }
    if (typeof data?.config?.protocol_note === "string") {
      meta.push(`提示: ${data.config.protocol_note}`);
    }
    return meta;
  }
  if (typeof data?.recommended_auth_server === "string" && Array.isArray(data?.auth_servers)) {
    const meta = [];
    const auth = data.auth_servers.find((item) => item.value === data.recommended_auth_server);
    if (auth) {
      meta.push(`推荐认证: ${auth.display_name}`);
      meta.push(`6100: ${probeSummary(auth.probe_6100)} / 7100: ${probeSummary(auth.login_7100)}`);
    }
    const backup = Array.isArray(data.data_servers)
      ? data.data_servers.find((item) => item.value === data.recommended_stock_backup_server)
      : null;
    if (backup) {
      meta.push(`备用: ${backup.display_name}`);
      meta.push(`7709: ${probeSummary(backup.probe)}`);
    }
    return meta;
  }
  if (typeof data?.protocol_login_state === "string" && typeof data?.status_text === "string") {
    const meta = [];
    if (data.effective_auth_server?.display_name) {
      meta.push(`认证: ${data.effective_auth_server.display_name}`);
    }
    if (data.auth_probe) {
      meta.push(`6100: ${probeSummary(data.auth_probe)}`);
    }
    if (data.auth_login_probe) {
      meta.push(`7100: ${probeSummary(data.auth_login_probe)}`);
    }
    if (data.stock_backup_probe) {
      meta.push(`7709: ${probeSummary(data.stock_backup_probe)}`);
    }
    return meta;
  }
  if (Array.isArray(data?.specs) && data.specs.length) {
    const meta = [];
    if (Array.isArray(data.unresolved_field_hex) && data.unresolved_field_hex.length) {
      meta.push(`未解字段: ${data.unresolved_field_hex.join(", ")}`);
    }
    const first = data.specs[0];
    const last = data.specs[data.specs.length - 1];
    if (first?.display_name || first?.name) {
      meta.push(`首个取值器: ${first.display_name || first.name}`);
    }
    if (last?.display_name || last?.name) {
      meta.push(`最后取值器: ${last.display_name || last.name}`);
    }
    return meta;
  }
  if (typeof data?.matches_total === "number" && Array.isArray(data?.preview)) {
    const meta = [];
    if (Array.isArray(data.preview) && data.preview.length) {
      const first = data.preview[0];
      let line = `首条匹配: ${first.code} ${first.name}`;
      if (typeof first.decimal_point === "number") {
        line += ` 小数位=${first.decimal_point}`;
      }
      if (typeof first.pre_close === "number") {
        line += ` 昨收=${first.pre_close}`;
      }
      meta.push(line);
    }
    if (typeof data.reply_bytes === "number") {
      meta.push(`回复字节: ${data.reply_bytes}`);
    }
    return meta;
  }
  if (Array.isArray(data?.requested_symbols) && typeof data?.quote_reply_frames === "number") {
    const meta = [];
    if (typeof data.host === "string" && typeof data.port === "number") {
      meta.push(`目标: ${data.host}:${data.port}`);
    }
    if (typeof data.code_table_reply_frames === "number") {
      meta.push(`代码表帧数: ${data.code_table_reply_frames}`);
    }
    if (typeof data.code_table_records_total === "number") {
      meta.push(`代码表记录数: ${data.code_table_records_total}`);
    }
    if (typeof data.quote_reply_bytes === "number") {
      meta.push(`行情字节: ${data.quote_reply_bytes}`);
    }
    if (typeof data.parsed_quote_records === "number") {
      meta.push(`已解析记录: ${data.parsed_quote_records}`);
    }
    if (Array.isArray(data.unmatched_symbols) && data.unmatched_symbols.length) {
      meta.push(`未匹配: ${data.unmatched_symbols.join(", ")}`);
    }
    return meta;
  }
  if (typeof data?.bars_total === "number" && Array.isArray(data?.preview)) {
    const meta = [];
    if (typeof data.host === "string" && typeof data.port === "number") {
      meta.push(`目标: ${data.host}:${data.port}`);
    }
    if (typeof data.code_table_reply_frames === "number") {
      meta.push(`代码表帧数: ${data.code_table_reply_frames}`);
    }
    if (typeof data.code_table_records_total === "number") {
      meta.push(`代码表记录数: ${data.code_table_records_total}`);
    }
    if (typeof data.kline_reply_bytes === "number") {
      meta.push(`K线字节: ${data.kline_reply_bytes}`);
    }
    const first = data.first_bar || data.preview[0];
    const last = data.last_bar || data.preview[data.preview.length - 1];
    if (first?.datetime) {
      meta.push(`首根 K 线: ${first.datetime}`);
    }
    if (last?.datetime) {
      meta.push(`末根 K 线: ${last.datetime}`);
    }
    return meta;
  }
  if (typeof data?.categories_total === "number" && Array.isArray(data?.preview)) {
    const meta = [];
    if (typeof data.host === "string" && typeof data.port === "number") {
      meta.push(`目标: ${data.host}:${data.port}`);
    }
    if (typeof data.code_table_reply_frames === "number") {
      meta.push(`代码表帧数: ${data.code_table_reply_frames}`);
    }
    if (typeof data.category_reply_bytes === "number") {
      meta.push(`类目字节: ${data.category_reply_bytes}`);
    }
    if (Array.isArray(data.preview) && data.preview.length) {
      meta.push(`首个类目: ${data.preview[0].name}`);
    }
    return meta;
  }
  if (typeof data?.content_chars_total === "number" && typeof data?.content_preview === "string") {
    const meta = [];
    if (typeof data.host === "string" && typeof data.port === "number") {
      meta.push(`目标: ${data.host}:${data.port}`);
    }
    if (data.resolved_category_name) {
      meta.push(`解析类目: ${data.resolved_category_name}`);
    }
    if (typeof data.content_reply_bytes === "number") {
      meta.push(`正文字节: ${data.content_reply_bytes}`);
    }
    if (typeof data.length === "number") {
      meta.push(`请求字节: ${data.length}`);
    }
    return meta;
  }
  if (typeof data?.matched_records === "number" && Array.isArray(data?.preview)) {
    const meta = [];
    const first = data.first_match || data.preview[0];
    const last = data.last_match || data.preview[data.preview.length - 1];
    if (typeof data.source_mode === "string" && data.source_mode.length) {
      meta.push(`来源模式: ${data.source_mode}`);
    }
    if (typeof data.source_files_total === "number") {
      meta.push(`来源文件数: ${data.source_files_total}`);
    }
    const decimalPointSource = quote0547DecimalPointSource(data);
    if (decimalPointSource) {
      meta.push(decimalPointSource);
    }
    if (typeof data.xor93_count === "number") {
      meta.push(`xor93 数量: ${data.xor93_count}`);
    }
    if (typeof data.filtered_records === "number") {
      meta.push(`过滤记录数: ${data.filtered_records}`);
    }
    if (typeof data.decimal_point_filter === "number") {
      meta.push(`小数位过滤: ${data.decimal_point_filter}`);
    }
    if (typeof data.prefix3_filter === "string" && data.prefix3_filter.length) {
      meta.push(`prefix3 过滤: ${data.prefix3_filter}`);
    }
    if (typeof data.pattern_bucket_filter === "string" && data.pattern_bucket_filter.length) {
      meta.push(`模式桶过滤: ${data.pattern_bucket_filter}`);
    }
    if (typeof data.pattern_subbucket_filter === "string" && data.pattern_subbucket_filter.length) {
      meta.push(`模式子桶过滤: ${data.pattern_subbucket_filter}`);
    }
    if (typeof data.state_matrix_filter === "string" && data.state_matrix_filter.length) {
      meta.push(`状态过滤: ${data.state_matrix_filter}`);
    }
    if (typeof data.quote_head_state_filter === "string" && data.quote_head_state_filter.length) {
      meta.push(`报价头过滤: ${data.quote_head_state_filter}`);
    }
    if (typeof data.time_presence_filter === "string" && data.time_presence_filter.length) {
      meta.push(`时间过滤: ${data.time_presence_filter}`);
    }
    if (typeof data.name_keyword_tag_filter === "string" && data.name_keyword_tag_filter.length) {
      meta.push(`关键词过滤: ${data.name_keyword_tag_filter}`);
    }
    meta.push(`匹配记录数: ${data.matched_records}`);
    if (typeof data.count_matches_records === "boolean") {
      meta.push(`计数匹配: ${data.count_matches_records}`);
    }
    if (typeof data.matched_prefix3_top !== "undefined") {
      meta.push(`匹配 prefix3 高频: ${summarizeProfileValue(data.matched_prefix3_top)}`);
    }
    if (typeof data.matched_decimal_point_top !== "undefined") {
      meta.push(`匹配小数位高频: ${summarizeProfileValue(data.matched_decimal_point_top)}`);
    }
    if (typeof data.matched_pattern_bucket_top !== "undefined") {
      meta.push(`匹配模式桶高频: ${summarizeProfileValue(data.matched_pattern_bucket_top)}`);
    }
    if (typeof data.matched_pattern_subbucket_top !== "undefined") {
      meta.push(`匹配模式子桶高频: ${summarizeProfileValue(data.matched_pattern_subbucket_top)}`);
    }
    if (typeof data.matched_quote_head_state_top !== "undefined") {
      meta.push(`匹配报价头高频: ${summarizeProfileValue(data.matched_quote_head_state_top)}`);
    }
    if (typeof data.matched_time_presence_top !== "undefined") {
      meta.push(`匹配时间高频: ${summarizeProfileValue(data.matched_time_presence_top)}`);
    }
    if (typeof data.matched_state_matrix_top !== "undefined") {
      meta.push(`匹配状态矩阵: ${summarizeProfileValue(data.matched_state_matrix_top)}`);
    }
    if (typeof data.matched_name_keyword_top !== "undefined") {
      meta.push(`匹配关键词高频: ${summarizeProfileValue(data.matched_name_keyword_top)}`);
    }
    if (typeof data.matched_source_top !== "undefined") {
      meta.push(`匹配来源高频: ${summarizeProfileValue(data.matched_source_top)}`);
    }
    if (first?.market_name || first?.market || first?.code) {
      meta.push(`首条匹配: ${recordSummary(first)}`);
    }
    if (last && last !== first && (last.market_name || last.market || last.code)) {
      meta.push(`最后匹配: ${recordSummary(last)}`);
    }
    return meta;
  }
  if (quote0547ExtraProfileSummary(data)) {
    const meta = [];
    if (typeof data.source_mode === "string" && data.source_mode.length) {
      meta.push(`来源模式: ${data.source_mode}`);
    }
    if (typeof data.source_files_total === "number") {
      meta.push(`来源文件数: ${data.source_files_total}`);
    }
    if (typeof data.code_table_lookup_error === "string" && data.code_table_lookup_error.length) {
      meta.push(`代码表查询错误: ${data.code_table_lookup_error}`);
    }
    if (typeof data.xor93_count === "number") {
      meta.push(`xor93 数量: ${data.xor93_count}`);
    }
    if (typeof data.filtered_records === "number") {
      meta.push(`过滤记录数: ${data.filtered_records}`);
    }
    if (typeof data.records_with_complete_extra_tuple === "number") {
      meta.push(`完整元组数: ${data.records_with_complete_extra_tuple}`);
    }
    if (typeof data.default_pattern_count === "number") {
      meta.push(`默认模式数: ${data.default_pattern_count}`);
    }
    if (typeof data.default_quote_head_count === "number") {
      meta.push(`默认报价头: ${data.default_quote_head_count}`);
    }
    if (typeof data.default_quote_head_time_present_count === "number") {
      meta.push(`默认报价头且有时间: ${data.default_quote_head_time_present_count}`);
    }
    if (typeof data.default_quote_head_time_absent_count === "number") {
      meta.push(`默认报价头且无时间: ${data.default_quote_head_time_absent_count}`);
    }
    if (typeof data.default_no_quote_head_count === "number") {
      meta.push(`默认无报价头: ${data.default_no_quote_head_count}`);
    }
    if (typeof data.anomaly_count === "number") {
      meta.push(`异常数: ${data.anomaly_count}`);
    }
    if (typeof data.anomaly_extra3_positive_count === "number") {
      meta.push(`extra3>0: ${data.anomaly_extra3_positive_count}`);
    }
    if (typeof data.anomaly_special_zero_bucket_count === "number") {
      meta.push(`特殊零桶: ${data.anomaly_special_zero_bucket_count}`);
    }
    if (typeof data.extra0_top !== "undefined") {
      meta.push(`extra0 高频: ${summarizeProfileValue(data.extra0_top)}`);
    }
    if (typeof data.extra1_top !== "undefined") {
      meta.push(`extra1 高频: ${summarizeProfileValue(data.extra1_top)}`);
    }
    if (typeof data.extra2_top !== "undefined") {
      meta.push(`extra2 高频: ${summarizeProfileValue(data.extra2_top)}`);
    }
    if (typeof data.extra3_top !== "undefined") {
      meta.push(`extra3 高频: ${summarizeProfileValue(data.extra3_top)}`);
    }
    if (typeof data.pattern_top !== "undefined") {
      meta.push(`模式高频: ${summarizeProfileValue(data.pattern_top)}`);
    }
    if (typeof data.source_top !== "undefined") {
      meta.push(`来源高频: ${summarizeProfileValue(data.source_top)}`);
    }
    if (typeof data.default_pattern_subbucket_top !== "undefined") {
      meta.push(`默认子桶高频: ${summarizeProfileValue(data.default_pattern_subbucket_top)}`);
    }
    if (typeof data.default_pattern_subbucket_correlations !== "undefined") {
      meta.push(`默认子桶关联: ${summarizeProfileValue(data.default_pattern_subbucket_correlations)}`);
    }
    if (typeof data.default_state_matrix_top !== "undefined") {
      meta.push(`默认状态矩阵: ${summarizeProfileValue(data.default_state_matrix_top)}`);
    }
    if (typeof data.default_quote_head_subbucket_top !== "undefined") {
      meta.push(`默认报价头高频: ${summarizeProfileValue(data.default_quote_head_subbucket_top)}`);
    }
    if (typeof data.default_quote_head_name_keyword_top !== "undefined") {
      meta.push(`默认报价头关键词高频: ${summarizeProfileValue(data.default_quote_head_name_keyword_top)}`);
    }
    if (typeof data.default_no_quote_head_subbucket_top !== "undefined") {
      meta.push(`默认无报价头高频: ${summarizeProfileValue(data.default_no_quote_head_subbucket_top)}`);
    }
    if (typeof data.default_no_quote_head_name_keyword_top !== "undefined") {
      meta.push(`默认无报价头关键词高频: ${summarizeProfileValue(data.default_no_quote_head_name_keyword_top)}`);
    }
    if (typeof data.anomaly_prefix3_top !== "undefined") {
      meta.push(`prefix3 高频: ${summarizeProfileValue(data.anomaly_prefix3_top)}`);
    }
    if (typeof data.anomaly_market_top !== "undefined") {
      meta.push(`市场高频: ${summarizeProfileValue(data.anomaly_market_top)}`);
    }
    if (typeof data.anomaly_decimal_point_top !== "undefined") {
      meta.push(`小数位高频: ${summarizeProfileValue(data.anomaly_decimal_point_top)}`);
    }
    if (typeof data.anomaly_pattern_bucket_top !== "undefined") {
      meta.push(`模式桶高频: ${summarizeProfileValue(data.anomaly_pattern_bucket_top)}`);
    }
    if (typeof data.anomaly_pattern_subbucket_top !== "undefined") {
      meta.push(`模式子桶高频: ${summarizeProfileValue(data.anomaly_pattern_subbucket_top)}`);
    }
    if (typeof data.anomaly_name_keyword_top !== "undefined") {
      meta.push(`关键词高频: ${summarizeProfileValue(data.anomaly_name_keyword_top)}`);
    }
    if (typeof data.anomaly_time_hhmmss_top !== "undefined") {
      meta.push(`时间高频: ${summarizeProfileValue(data.anomaly_time_hhmmss_top)}`);
    }
    if (typeof data.anomaly_extra0_time_hint_top !== "undefined") {
      meta.push(`extra0 时间高频: ${summarizeProfileValue(data.anomaly_extra0_time_hint_top)}`);
    }
    if (typeof data.anomaly_pattern_correlations !== "undefined") {
      meta.push(`模式关联: ${summarizeProfileValue(data.anomaly_pattern_correlations)}`);
    }
    if (Array.isArray(data.default_no_quote_head_examples) && data.default_no_quote_head_examples.length) {
      meta.push(`默认无报价头示例: ${summarizeProfileValue(data.default_no_quote_head_examples[0])}`);
    }
    if (Array.isArray(data.default_quote_head_time_present_examples) && data.default_quote_head_time_present_examples.length) {
      meta.push(`默认报价头有时间示例: ${summarizeProfileValue(data.default_quote_head_time_present_examples[0])}`);
    }
    if (Array.isArray(data.default_quote_head_time_absent_examples) && data.default_quote_head_time_absent_examples.length) {
      meta.push(`默认报价头无时间示例: ${summarizeProfileValue(data.default_quote_head_time_absent_examples[0])}`);
    }
    if (Array.isArray(data.anomaly_special_zero_bucket_examples) && data.anomaly_special_zero_bucket_examples.length) {
      meta.push(`特殊零桶示例: ${summarizeProfileValue(data.anomaly_special_zero_bucket_examples[0])}`);
    }
    const firstExample = Array.isArray(data.anomaly_examples) ? data.anomaly_examples[0] : null;
    if (firstExample !== null && firstExample !== undefined) {
      meta.push(`首个异常: ${summarizeProfileValue(firstExample)}`);
    }
    return meta;
  }
  if (typeof data?.record_found === "boolean" && data.field_id === undefined) {
    if (!data.record || typeof data.record.symbol !== "string") {
      return [];
    }
    return [
      `市场: ${data.record.market || "?"}`,
      `代码: ${data.record.code || "?"}`,
      `报告期: ${data.record.bao_gao || "?"}`,
    ];
  }
  if (Array.isArray(data?.packets) && data.packets.length) {
    return [
      `标签: ${data.packets.map((item) => item.label).join(" | ")}`,
      `probe_hello: ${data.probe_hello_hex ?? "?"}`,
    ];
  }
  if (
    typeof data?.expected_plain_len === "number" &&
    typeof data?.plain_source_hint === "string"
  ) {
    const meta = [
      `明文来源: ${data.plain_source_hint}`,
      `密文目标: ${data.cipher_target_hint || "?"}`,
    ];
    if (Array.isArray(data.suggested_artifacts) && data.suggested_artifacts.length) {
      meta.push(`建议产物: ${data.suggested_artifacts.slice(0, 3).join(" | ")}`);
    }
    return meta;
  }
  if (typeof data?.compare_len === "number" && typeof data?.diff_bytes === "number") {
    const meta = [];
    if (data.block_summary) {
      meta.push(
        `块${data.block_summary.block_size}: 相同 ${data.block_summary.equal_blocks}，不同 ${data.block_summary.differing_blocks}`,
      );
      meta.push(
        `异或高频: ${data.block_summary.xor_most_common_hex || "?"} x${data.block_summary.xor_most_common_count ?? 0}`,
      );
    }
    if (Array.isArray(data.diff_runs) && data.diff_runs.length) {
      const first = data.diff_runs[0];
      meta.push(`首个差异段: @${first.offset} 长度 ${first.len}`);
    }
    return meta;
  }
  if (typeof data?.summary === "object" && data.summary) {
    const meta = [];
    if (typeof data.summary.code_table_requests === "number") {
      meta.push(`代码表请求数: ${data.summary.code_table_requests}`);
    }
    if (Array.isArray(data.summary.bootstrap_labels) && data.summary.bootstrap_labels.length) {
      meta.push(`启动序列: ${data.summary.bootstrap_labels.slice(0, 4).join(" | ")}`);
    }
    if (data.summary.main_site_validate_after_tag_block8) {
      const block = data.summary.main_site_validate_after_tag_block8;
      meta.push(
        `0x7b00 去 tag 后: ${block.total_blocks}x${block.block_size}，唯一 ${block.unique_blocks}，Top ${block.most_common_count}`,
      );
    }
    return meta;
  }
  return [];
}

function value(id) {
  return document.querySelector(id).value.trim();
}

function valueOrNull(id) {
  const v = value(id);
  return v.length ? v : null;
}

function numberOrNull(id) {
  const v = value(id);
  if (!v.length) {
    return null;
  }
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function boolValue(id) {
  return document.querySelector(id).value === "true";
}

function checked(id) {
  return document.querySelector(id).checked;
}

function setChecked(id, isChecked) {
  const node = document.querySelector(id);
  if (node) {
    node.checked = Boolean(isChecked);
  }
}

function setInputValue(id, nextValue) {
  const node = document.querySelector(id);
  if (node) {
    node.value = nextValue ?? "";
  }
}

function setText(node, text) {
  if (node) {
    node.textContent = text ?? "";
  }
}

function replaceSelectOptions(select, options, selectedValue) {
  if (!select) {
    return;
  }
  select.innerHTML = "";
  for (const option of options) {
    const element = document.createElement("option");
    element.value = option.value;
    element.textContent = option.label;
    element.selected = option.value === selectedValue;
    select.appendChild(element);
  }
}

function legacyAuthOptionLabel(server, probe) {
  if (!probe) {
    return server.display_name;
  }
  return `${server.display_name} (${probeSummary(probe.probe_6100)} / ${probeSummary(probe.login_7100)})`;
}

function legacyDataOptionLabel(server, probe) {
  if (!probe) {
    return server.display_name;
  }
  return `${server.display_name} (${probeSummary(probe.probe)})`;
}

function fillPlaceholderSelect(select, label) {
  replaceSelectOptions(select, [
    {
      value: label,
      label,
    },
  ], label);
}

function applyLegacyServerLists(config, probeData = null) {
  const authOptions = [
    {
      value: "智能选择",
      label:
        probeData && probeData.recommended_auth_server
          ? `智能选择 -> ${probeData.recommended_auth_server}`
          : "智能选择",
    },
    ...legacyState.authServers.map((server) => {
      const probe = probeData?.auth_servers?.find((item) => item.value === server.value) || null;
      return {
        value: server.value,
        label: legacyAuthOptionLabel(server, probe),
      };
    }),
  ];
  replaceSelectOptions(legacyElements.authServer, authOptions, config.selected_auth_server || "智能选择");

  const dataOptions = legacyState.dataServers.map((server) => {
    const probe = probeData?.data_servers?.find((item) => item.value === server.value) || null;
    return {
      value: server.value,
      label: legacyDataOptionLabel(server, probe),
    };
  });
  replaceSelectOptions(
    legacyElements.stockBackupServer,
    dataOptions,
    config.selected_stock_backup_server || dataOptions[0]?.value,
  );

  fillPlaceholderSelect(
    legacyElements.stockMainServer,
    config.selected_stock_main_server || "网络断开 - 等待更新服务器列表",
  );
  fillPlaceholderSelect(
    legacyElements.reservedServer,
    config.selected_reserved_server || "",
  );
  fillPlaceholderSelect(
    legacyElements.wenhuaServer,
    config.selected_wenhua_server || "网络断开 - 等待更新服务器列表",
  );
  fillPlaceholderSelect(
    legacyElements.boyiServer,
    config.selected_boyi_server || "网络断开 - 等待更新服务器列表",
  );
}

function applyLegacyConfig(config) {
  setInputValue("#legacyAccount", config.account);
  setInputValue("#legacyPassword", config.password);
  setText(legacyElements.accountExpiry, config.account_expiry);
  setInputValue("#legacyDataInterval", String(config.data_interval ?? 10));
  setInputValue("#legacyOperatorName", config.operator_name);
  setInputValue("#legacyFillDailyCount", String(config.fill_daily_count ?? 1000));
  setInputValue("#legacyFill5MinCount", String(config.fill_5min_count ?? 3000));
  setInputValue("#legacyFill1MinCount", String(config.fill_1min_count ?? 2000));
  setInputValue("#legacyFillTicksCount", String(config.fill_ticks_count ?? 1));
  setChecked("#legacyEnableStockMain", config.enable_stock_main);
  setChecked("#legacyEnableStockBackup", config.enable_stock_backup);
  setChecked("#legacyEnableReserved", config.enable_reserved);
  setChecked("#legacyEnableWenhua", config.enable_wenhua);
  setChecked("#legacyEnableBoyi", config.enable_boyi);
  setChecked("#legacyFillDaily", config.fill_daily);
  setChecked("#legacyFill5Min", config.fill_5min);
  setChecked("#legacyFill1Min", config.fill_1min);
  setChecked("#legacyFillTicks", config.fill_ticks);
  setChecked("#legacyFillF10", config.fill_f10);
  setChecked("#legacyFillAllStocks", config.fill_all_stocks);
  setChecked("#legacyNetworkTime", config.network_time);
  setChecked("#legacyOperatorEnabled", config.operator_enabled);
  legacyElements.connectRadio.checked = Boolean(config.connect_selected);
  legacyElements.disconnectRadio.checked = Boolean(config.disconnect_selected);
  setText(legacyElements.connectionState, config.login_status_text);
  setText(legacyElements.statusBar, config.status_bar_text);
  setText(legacyElements.protocolNote, config.protocol_note);
}

function applyLegacyBootstrapData(data) {
  legacyState.passwordInfoUrlTemplate = data?.config?.password_info_url_template || null;
  legacyState.authServers = Array.isArray(data?.auth_servers) ? data.auth_servers : [];
  legacyState.dataServers = Array.isArray(data?.data_servers) ? data.data_servers : [];
  applyLegacyServerLists(data.config || {}, null);
  applyLegacyConfig(data.config || {});
}

function legacyConfigPayload() {
  return {
    account: value("#legacyAccount"),
    password: value("#legacyPassword"),
    account_expiry: legacyElements.accountExpiry.textContent.trim(),
    analysis_software: "自定义",
    analysis_version: "510",
    auto_upgrade: "稳定版",
    selected_auth_server: legacyElements.authServer.value || "智能选择",
    selected_stock_main_server: legacyElements.stockMainServer.value || "",
    selected_stock_backup_server: legacyElements.stockBackupServer.value || "",
    selected_reserved_server: legacyElements.reservedServer.value || "",
    selected_wenhua_server: legacyElements.wenhuaServer.value || "",
    selected_boyi_server: legacyElements.boyiServer.value || "",
    enable_stock_main: checked("#legacyEnableStockMain"),
    enable_stock_backup: checked("#legacyEnableStockBackup"),
    enable_reserved: checked("#legacyEnableReserved"),
    enable_wenhua: checked("#legacyEnableWenhua"),
    enable_boyi: checked("#legacyEnableBoyi"),
    fill_daily: checked("#legacyFillDaily"),
    fill_daily_count: Number(value("#legacyFillDailyCount") || "1000"),
    fill_5min: checked("#legacyFill5Min"),
    fill_5min_count: Number(value("#legacyFill5MinCount") || "3000"),
    fill_1min: checked("#legacyFill1Min"),
    fill_1min_count: Number(value("#legacyFill1MinCount") || "2000"),
    fill_ticks: checked("#legacyFillTicks"),
    fill_ticks_count: Number(value("#legacyFillTicksCount") || "1"),
    fill_f10: checked("#legacyFillF10"),
    fill_all_stocks: checked("#legacyFillAllStocks"),
    network_time: checked("#legacyNetworkTime"),
    data_interval: Number(value("#legacyDataInterval") || "10"),
    operator_enabled: checked("#legacyOperatorEnabled"),
    operator_name: value("#legacyOperatorName"),
    connect_selected: legacyElements.connectRadio.checked,
    disconnect_selected: legacyElements.disconnectRadio.checked,
    login_status_text: legacyElements.connectionState.textContent.trim(),
    status_bar_text: legacyElements.statusBar.textContent.trim(),
    password_info_url_template: legacyState.passwordInfoUrlTemplate,
    protocol_note: legacyElements.protocolNote.textContent.trim(),
  };
}

async function bootstrapLegacyPanel(updateResult = false) {
  pushLog("GET /api/legacy/panel/bootstrap");
  const data = await requestJson("/api/legacy/panel/bootstrap");
  applyLegacyBootstrapData(data);
  if (updateResult) {
    applyResult("旧连接面板配置", data);
  }
  pushLog("旧连接面板配置: 成功");
  return data;
}

async function saveLegacyPanel() {
  const data = await requestJson("/api/legacy/panel/save", {
    method: "POST",
    body: JSON.stringify({
      config: legacyConfigPayload(),
    }),
  });
  applyLegacyConfig(data.config);
  applyResult("旧连接面板配置保存", data);
  pushLog("旧连接面板配置保存: 成功");
  return data;
}

async function probeLegacyPanel(updateResult = true) {
  const data = await requestJson("/api/legacy/panel/probe", {
    method: "POST",
    body: JSON.stringify({
      config: legacyConfigPayload(),
      timeout_ms: 1800,
    }),
  });
  applyLegacyServerLists(legacyConfigPayload(), data);
  setText(legacyElements.statusBar, data.status_text);
  setText(legacyElements.connectionState, "测速完成");
  if (legacyElements.authServer.value === "智能选择" && data.recommended_auth_server) {
    legacyElements.authServer.value = "智能选择";
  }
  if (updateResult) {
    applyResult("旧连接面板测速", data);
  }
  pushLog("旧连接面板测速: 成功");
  return data;
}

async function connectLegacyPanel() {
  const data = await requestJson("/api/legacy/panel/connect", {
    method: "POST",
    body: JSON.stringify({
      config: legacyConfigPayload(),
      timeout_ms: 1800,
    }),
  });
  legacyState.passwordInfoUrlTemplate = data?.config?.password_info_url_template || legacyState.passwordInfoUrlTemplate;
  applyLegacyConfig(data.config);
  applyLegacyServerLists(data.config, {
    auth_servers: data.effective_auth_server && data.auth_probe && data.auth_login_probe
      ? [
          {
            value: data.effective_auth_server.value,
            probe_6100: data.auth_probe,
            login_7100: data.auth_login_probe,
          },
        ]
      : [],
    data_servers: data.effective_stock_backup_server && data.stock_backup_probe
      ? [
          {
            value: data.effective_stock_backup_server.value,
            probe: data.stock_backup_probe,
          },
        ]
      : [],
    recommended_auth_server: data.config.selected_auth_server,
  });
  applyResult("旧连接面板连接试探", data);
  pushLog("旧连接面板连接试探: 成功");
  return data;
}

async function disconnectLegacyPanel() {
  const data = await requestJson("/api/legacy/panel/disconnect", {
    method: "POST",
    body: JSON.stringify({
      config: legacyConfigPayload(),
    }),
  });
  applyLegacyConfig(data.config);
  applyResult("旧连接面板断开", data);
  pushLog("旧连接面板断开: 成功");
  return data;
}

function recordSummary(record) {
  if (!record || typeof record !== "object") {
    return "?";
  }
  const market = record.market_name ?? record.market;
  const code = record.code || record.symbol || "?";
  const pieces = [];
  if (typeof record.source_name === "string" && record.source_name.length) {
    pieces.push(`来源=${record.source_name}`);
  }
  if (market !== undefined && market !== null && market !== "") {
    pieces.push(String(market));
  }
  pieces.push(String(code));
  if (typeof record.len === "number") {
    pieces.push(`长度=${record.len}`);
  }
  if (typeof record.code_table_name === "string" && record.code_table_name.length) {
    pieces.push(`名称=${record.code_table_name}`);
  }
  if (
    Array.isArray(record.code_table_name_keyword_tags) &&
    record.code_table_name_keyword_tags.length
  ) {
    pieces.push(`标签=${record.code_table_name_keyword_tags.join("+")}`);
  }
  if (
    typeof record.code_table_name_keyword_tag === "string" &&
    record.code_table_name_keyword_tag.length
    && !(
      Array.isArray(record.code_table_name_keyword_tags) &&
      record.code_table_name_keyword_tags.length
    )
  ) {
    pieces.push(`标签=${record.code_table_name_keyword_tag}`);
  }
  if (typeof record.active1_raw === "number") {
    pieces.push(`a1=${record.active1_raw}`);
  }
  if (typeof record.decimal_point_used === "number") {
    pieces.push(`小数位=${record.decimal_point_used}`);
  }
  if (typeof record.code_table_pre_close === "number") {
    pieces.push(`昨收=${record.code_table_pre_close}`);
  }
  if (typeof record.code_table_pre_close_matches === "boolean") {
    const delta =
      typeof record.code_table_pre_close_delta === "number"
        ? `${record.code_table_pre_close_delta >= 0 ? "+" : ""}${record.code_table_pre_close_delta.toFixed(6)}`
        : "?";
    pieces.push(`昨收匹配=${record.code_table_pre_close_matches}(${delta})`);
  }
  if (typeof record.time_hhmmss === "string" && record.time_hhmmss.length) {
    pieces.push(`时间=${record.time_hhmmss}`);
  }
  if (typeof record.pattern_bucket === "string" && record.pattern_bucket.length) {
    pieces.push(`模式桶=${record.pattern_bucket}`);
  }
  if (typeof record.pattern_subbucket === "string" && record.pattern_subbucket.length) {
    pieces.push(`模式子桶=${record.pattern_subbucket}`);
  }
  if (typeof record.time_fields_match === "boolean") {
    const delta =
      typeof record.time_fields_delta_seconds === "number"
        ? `${record.time_fields_delta_seconds >= 0 ? "+" : ""}${record.time_fields_delta_seconds}s`
        : "?";
    pieces.push(`时间匹配=${record.time_fields_match}(${delta})`);
  }
  if (typeof record.extra0_raw === "number") {
    pieces.push(`extra0=${record.extra0_raw}`);
  }
  if (typeof record.extra0_time_hhmmss === "string" && record.extra0_time_hhmmss.length) {
    pieces.push(`提示=${record.extra0_time_hhmmss}`);
  }
  if (
    typeof record.extra1_raw === "number" &&
    typeof record.extra2_raw === "number" &&
    typeof record.extra3_raw === "number" &&
    !(
      record.extra1_raw === 2 &&
      record.extra2_raw === 0 &&
      record.extra3_raw === 0
    )
  ) {
    pieces.push(`x1=${record.extra1_raw}`);
    pieces.push(`x2=${record.extra2_raw}`);
    pieces.push(`x3=${record.extra3_raw}`);
  }
  if (typeof record.extra3_positive === "boolean" && record.extra3_positive) {
    pieces.push("x3>0");
  }
  if (record.special_zero_bucket === true) {
    pieces.push("特殊零桶");
  }
  if (record.normalized_quote_head && typeof record.normalized_quote_head.price === "number") {
    pieces.push(`归一价=${record.normalized_quote_head.price.toFixed(2)}`);
  }
  if (
    typeof record.normalized_change_value === "number" &&
    typeof record.normalized_change_percent === "number"
  ) {
    const value = record.normalized_change_value;
    const percent = record.normalized_change_percent;
    pieces.push(
      `涨跌=${value >= 0 ? "+" : ""}${value.toFixed(2)}(${percent >= 0 ? "+" : ""}${percent.toFixed(2)}%)`,
    );
  }
  if (typeof record.normalized_amplitude_percent === "number") {
    pieces.push(`振幅=${record.normalized_amplitude_percent >= 0 ? "+" : ""}${record.normalized_amplitude_percent.toFixed(2)}%`);
  }
  if (
    typeof record.normalized_open_gap_value === "number" &&
    typeof record.normalized_open_gap_percent === "number"
  ) {
    const value = record.normalized_open_gap_value;
    const percent = record.normalized_open_gap_percent;
    pieces.push(
      `开盘缺口=${value >= 0 ? "+" : ""}${value.toFixed(2)}(${percent >= 0 ? "+" : ""}${percent.toFixed(2)}%)`,
    );
  }
  if (typeof record.normalized_return_from_open_percent === "number") {
    pieces.push(`相对开盘=${record.normalized_return_from_open_percent >= 0 ? "+" : ""}${record.normalized_return_from_open_percent.toFixed(2)}%`);
  }
  if (typeof record.normalized_drawdown_from_high_percent === "number") {
    pieces.push(`相对高点=${record.normalized_drawdown_from_high_percent >= 0 ? "+" : ""}${record.normalized_drawdown_from_high_percent.toFixed(2)}%`);
  }
  if (record.quote_head && typeof record.quote_head.price === "number") {
    pieces.push(`价格=${record.quote_head.price.toFixed(2)}`);
  }
  return pieces.join(" ");
}

function quote0547DecimalPointSource(data) {
  if (typeof data?.matched_records !== "number" || !Array.isArray(data?.preview)) {
    return null;
  }
  const manual = typeof data.decimal_point_used === "number" ? data.decimal_point_used : null;
  const first = data.first_match || data.preview[0];
  const recordDecimalPoint =
    typeof first?.decimal_point_used === "number" ? first.decimal_point_used : null;
  const decimalPoint = manual ?? recordDecimalPoint;
  if (decimalPoint === null) {
    if (data.auto_decimal_lookup === true) {
      return "小数位来源 = 自动(7709)";
    }
    return null;
  }
  if (manual !== null) {
    return `小数位来源 = 手工(${decimalPoint})`;
  }
  if (data.auto_decimal_lookup === true) {
    return `小数位来源 = 自动(7709:${decimalPoint})`;
  }
  return `小数位来源 = 记录(${decimalPoint})`;
}

function quote0547ExtraProfileSummary(data) {
  return (
    typeof data?.filtered_records === "number" ||
    typeof data?.records_with_complete_extra_tuple === "number" ||
    typeof data?.default_pattern_count === "number" ||
    typeof data?.anomaly_count === "number" ||
    typeof data?.anomaly_extra3_positive_count === "number" ||
    typeof data?.anomaly_special_zero_bucket_count === "number" ||
    typeof data?.code_table_lookup_error === "string" ||
    Array.isArray(data?.anomaly_examples) ||
    data?.extra0_top !== undefined ||
    data?.extra1_top !== undefined ||
    data?.extra2_top !== undefined ||
    data?.extra3_top !== undefined ||
    data?.pattern_top !== undefined ||
    data?.default_pattern_subbucket_top !== undefined ||
    data?.anomaly_prefix3_top !== undefined ||
    data?.anomaly_market_top !== undefined ||
    data?.anomaly_decimal_point_top !== undefined ||
    data?.anomaly_pattern_bucket_top !== undefined ||
    data?.anomaly_pattern_subbucket_top !== undefined ||
    data?.anomaly_name_keyword_top !== undefined ||
    data?.anomaly_time_hhmmss_top !== undefined ||
    data?.anomaly_extra0_time_hint_top !== undefined ||
    data?.anomaly_pattern_correlations !== undefined ||
    data?.anomaly_special_zero_bucket_examples !== undefined
  );
}

function summarizeProfileValue(value) {
  if (value === null || value === undefined) {
    return "?";
  }
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    return value.slice(0, 3).map((item) => summarizeProfileValue(item)).join(" | ");
  }
  if (typeof value === "object") {
    const pieces = [];
    if (typeof value.code === "string") {
      pieces.push(value.code);
    }
    if (typeof value.market_name === "string" || typeof value.market === "number") {
      pieces.push(String(value.market_name ?? value.market));
    }
    if (typeof value.count === "number") {
      pieces.push(`数量=${value.count}`);
    }
    if (typeof value.value === "number") {
      pieces.push(`值=${value.value}`);
    }
    if (typeof value.extra0_raw === "number") {
      pieces.push(`x0=${value.extra0_raw}`);
    }
    if (typeof value.extra1_raw === "number") {
      pieces.push(`x1=${value.extra1_raw}`);
    }
    if (typeof value.extra2_raw === "number") {
      pieces.push(`x2=${value.extra2_raw}`);
    }
    if (typeof value.extra3_raw === "number") {
      pieces.push(`x3=${value.extra3_raw}`);
    }
    if (typeof value.pattern === "string") {
      pieces.push(value.pattern);
    }
    if (
      typeof value.extra1_raw === "number" &&
      typeof value.extra2_raw === "number" &&
      typeof value.extra3_raw === "number" &&
      typeof value.count === "number" &&
      value.prefix3_top !== undefined
    ) {
      pieces.push(`模式=${value.extra1_raw}/${value.extra2_raw}/${value.extra3_raw}`);
      pieces.push(`前三位=${summarizeProfileValue(value.prefix3_top)}`);
      if (value.decimal_point_top !== undefined) {
        pieces.push(`小数位=${summarizeProfileValue(value.decimal_point_top)}`);
      }
      if (value.time_hhmmss_top !== undefined) {
        pieces.push(`时间=${summarizeProfileValue(value.time_hhmmss_top)}`);
      }
      if (value.extra0_time_hint_top !== undefined) {
        pieces.push(`提示=${summarizeProfileValue(value.extra0_time_hint_top)}`);
      }
      return pieces.join(" ");
    }
    if (typeof value.label === "string") {
      pieces.push(value.label);
    }
    if (Array.isArray(value.example_codes) && value.example_codes.length) {
      pieces.push(value.example_codes.slice(0, 3).join(","));
    }
    return pieces.length ? pieces.join(" ") : JSON.stringify(value);
  }
  return String(value);
}

function initTabs(buttons, panels, config) {
  const { buttonKey, panelKey, storageKey, fallbackTab } = config;
  if (!buttons.length || !panels.length) {
    return () => {};
  }

  function setActiveTab(tabName) {
    const knownTabs = new Set(buttons.map((button) => button.dataset[buttonKey]));
    const safeTabName = knownTabs.has(tabName) ? tabName : fallbackTab;
    for (const button of buttons) {
      const isActive = button.dataset[buttonKey] === safeTabName;
      button.classList.toggle("is-active", isActive);
      button.setAttribute("aria-selected", isActive ? "true" : "false");
    }
    for (const panel of panels) {
      const isActive = panel.dataset[panelKey] === safeTabName;
      panel.classList.toggle("is-active", isActive);
    }
    try {
      window.localStorage.setItem(storageKey, safeTabName);
    } catch (_error) {
      // Ignore localStorage failures in restricted browsers.
    }
  }

  for (const button of buttons) {
    button.addEventListener("click", () => {
      setActiveTab(button.dataset[buttonKey] || fallbackTab);
    });
  }

  let initialTab = fallbackTab;
  try {
    initialTab = window.localStorage.getItem(storageKey) || initialTab;
  } catch (_error) {
    initialTab = fallbackTab;
  }
  setActiveTab(initialTab);
  return setActiveTab;
}

const setActivePageTab = initTabs(pageTabButtons, pageTabPanels, {
  buttonKey: "pageTabTarget",
  panelKey: "pageTabPanel",
  storageKey: "netzip-active-page-tab",
  fallbackTab: "server",
});

initTabs(toolTabButtons, toolTabPanels, {
  buttonKey: "toolTabTarget",
  panelKey: "toolTabPanel",
  storageKey: "netzip-active-tool-tab",
  fallbackTab: "quote",
});

async function runLegacyAction(title, action) {
  pushLog(title);
  try {
    await action();
  } catch (error) {
    state.resultTitle = title;
    state.resultSummary = String(error);
    state.resultMeta = [];
    state.resultBody = "";
    render();
    setText(legacyElements.statusBar, `操作失败: ${error}`);
    setText(legacyElements.connectionState, "操作失败");
    pushLog(`${title}: 失败`);
  }
}

function openLegacyPasswordPage() {
  if (!legacyState.passwordInfoUrlTemplate) {
    setText(legacyElements.statusBar, "当前配置未提供密码修改地址。");
    return;
  }
  const account = encodeURIComponent(value("#legacyAccount"));
  const password = encodeURIComponent(value("#legacyPassword"));
  const url = legacyState.passwordInfoUrlTemplate
    .replace("%s", account)
    .replace("%s", password);
  window.open(url, "_blank", "noopener");
  pushLog(`打开密码页面: ${url}`);
}

document.querySelector("#legacyRefreshBtn").addEventListener("click", () => {
  runLegacyAction("旧连接面板加载配置", () => bootstrapLegacyPanel(true));
});

document.querySelector("#legacySaveBtn").addEventListener("click", () => {
  runLegacyAction("旧连接面板保存配置", saveLegacyPanel);
});

document.querySelector("#legacyProbeBtn").addEventListener("click", () => {
  runLegacyAction("旧连接面板测速", probeLegacyPanel);
});

document.querySelector("#legacyPasswordBtn").addEventListener("click", openLegacyPasswordPage);

legacyElements.connectRadio.addEventListener("change", () => {
  if (legacyElements.connectRadio.checked) {
    runLegacyAction("旧连接面板连接试探", connectLegacyPanel);
  }
});

legacyElements.disconnectRadio.addEventListener("change", () => {
  if (legacyElements.disconnectRadio.checked) {
    runLegacyAction("旧连接面板断开", disconnectLegacyPanel);
  }
});

document.querySelector("#healthBtn").addEventListener("click", () => {
  callApi(HEALTH_TITLE, "/health");
});

document.querySelector("#capabilitiesBtn").addEventListener("click", () => {
  setActivePageTab("console");
  callApi("能力列表", "/api/capabilities");
});

document.querySelector("#finBtn").addEventListener("click", () => {
  callApi("FIN 解析", "/api/fin/parse", {
    method: "POST",
    body: JSON.stringify({
      path: value("#finPath"),
      limit: Number(value("#finLimit") || "10"),
      out_csv_path: valueOrNull("#finCsvPath"),
    }),
  });
});

document.querySelector("#getterValueBtn").addEventListener("click", () => {
  callApi("FIN 字段取值", "/api/fin/getter-value", {
    method: "POST",
    body: JSON.stringify({
      path: value("#getterPath"),
      symbol: value("#getterSymbol"),
      field_id: Number(value("#getterFieldId")),
    }),
  });
});

document.querySelector("#getterSpecsBtn").addEventListener("click", () => {
  callApi("FIN 取值器规格", "/api/fin/getter-specs", {
    method: "POST",
    body: JSON.stringify({}),
  });
});

document.querySelector("#finQueryBtn").addEventListener("click", () => {
  callApi("FIN 记录查询", "/api/fin/query-record", {
    method: "POST",
    body: JSON.stringify({
      path: value("#finQueryPath"),
      symbol: value("#finQuerySymbol"),
    }),
  });
});

document.querySelector("#tdxBtn").addEventListener("click", () => {
  callApi("7709 同步", "/api/tdx7709/sync-code-table", {
    method: "POST",
    body: JSON.stringify({
      host: DEFAULT_REMOTE_HOST,
      port: Number(value("#tdxPort") || "7709"),
      preview_limit: Number(value("#tdxPreviewLimit") || "10"),
      read_timeout_ms: Number(value("#tdxReadTimeoutMs") || "1000"),
      connect_timeout_ms: Number(value("#tdxConnectTimeoutMs") || "5000"),
      settle_ms: Number(value("#tdxSettleMs") || "120"),
      out_csv_path: valueOrNull("#tdxCsvPath"),
    }),
  });
});

document.querySelector("#tdxBootstrapBtn").addEventListener("click", () => {
  callApi("7709 启动方案", "/api/tdx7709/bootstrap-plan");
});

document.querySelector("#tdxQueryBtn").addEventListener("click", () => {
  callApi("7709 代码表查询", "/api/tdx7709/query-code-table", {
    method: "POST",
    body: JSON.stringify({
      host: DEFAULT_REMOTE_HOST,
      port: Number(value("#tdxQueryPort") || "7709"),
      query: valueOrNull("#tdxQueryText"),
      limit: Number(value("#tdxQueryLimit") || "20"),
    }),
  });
});

document.querySelector("#tdxLiveQuoteBtn").addEventListener("click", () => {
  const rawSymbols = value("#tdxLiveSymbols")
    .split(/[,\n\s]+/)
    .map((item) => item.trim())
    .filter(Boolean);
  callApi("7709 实时行情", "/api/tdx7709/live-quote", {
    method: "POST",
    body: JSON.stringify({
      host: DEFAULT_REMOTE_HOST,
      port: Number(value("#tdxLivePort") || "7709"),
      symbols: rawSymbols,
      limit: Number(value("#tdxLiveLimit") || "20"),
      read_timeout_ms: Number(value("#tdxLiveReadTimeoutMs") || "1000"),
      connect_timeout_ms: Number(value("#tdxLiveConnectTimeoutMs") || "5000"),
      settle_ms: Number(value("#tdxLiveSettleMs") || "300"),
    }),
  });
});

document.querySelector("#tdxKlineBtn").addEventListener("click", () => {
  callApi("7709 K线", "/api/tdx7709/kline", {
    method: "POST",
    body: JSON.stringify({
      host: DEFAULT_REMOTE_HOST,
      port: Number(value("#tdxKlinePort") || "7709"),
      symbol: value("#tdxKlineSymbol"),
      kline_type: valueOrNull("#tdxKlineType"),
      category: numberOrNull("#tdxKlineCategory"),
      start: Number(value("#tdxKlineStart") || "0"),
      count: Number(value("#tdxKlineCount") || "32"),
      limit: Number(value("#tdxKlineLimit") || "20"),
      read_timeout_ms: Number(value("#tdxKlineReadTimeoutMs") || "1000"),
      connect_timeout_ms: Number(value("#tdxKlineConnectTimeoutMs") || "5000"),
      settle_ms: Number(value("#tdxKlineSettleMs") || "300"),
    }),
  });
});

document.querySelector("#tdxF10CategoriesBtn").addEventListener("click", () => {
  callApi("7709 F10 类目", "/api/tdx7709/f10/categories", {
    method: "POST",
    body: JSON.stringify({
      host: DEFAULT_REMOTE_HOST,
      port: Number(value("#tdxF10Port") || "7709"),
      symbol: value("#tdxF10Symbol"),
      limit: Number(value("#tdxF10Limit") || "20"),
      read_timeout_ms: Number(value("#tdxF10ReadTimeoutMs") || "1000"),
      connect_timeout_ms: Number(value("#tdxF10ConnectTimeoutMs") || "5000"),
      settle_ms: Number(value("#tdxF10SettleMs") || "300"),
    }),
  });
});

document.querySelector("#tdxF10ContentBtn").addEventListener("click", () => {
  callApi("7709 F10 正文", "/api/tdx7709/f10/content", {
    method: "POST",
    body: JSON.stringify({
      host: DEFAULT_REMOTE_HOST,
      port: Number(value("#tdxF10Port") || "7709"),
      symbol: value("#tdxF10Symbol"),
      filename: valueOrNull("#tdxF10Filename"),
      category_name: valueOrNull("#tdxF10CategoryName"),
      start: numberOrNull("#tdxF10Start"),
      length: numberOrNull("#tdxF10Length"),
      preview_chars: Number(value("#tdxF10PreviewChars") || "600"),
      read_timeout_ms: Number(value("#tdxF10ReadTimeoutMs") || "1000"),
      connect_timeout_ms: Number(value("#tdxF10ConnectTimeoutMs") || "5000"),
      settle_ms: Number(value("#tdxF10SettleMs") || "300"),
    }),
  });
});

document.querySelector("#quote0547Btn").addEventListener("click", () => {
  callApi("0x0547 正文查询", "/api/quote/0547/query", {
    method: "POST",
    body: JSON.stringify({
      path: value("#quote0547Path"),
      query: value("#quote0547Symbol"),
      limit: Number(value("#quote0547Limit") || "10"),
      decimal_point: numberOrNull("#quote0547DecimalPoint"),
      decimal_point_filter: numberOrNull("#quote0547DecimalPointFilter"),
      prefix3: valueOrNull("#quote0547Prefix3"),
      pattern_bucket: valueOrNull("#quote0547PatternBucket"),
      state_matrix: valueOrNull("#quote0547StateMatrix"),
      quote_head_state: valueOrNull("#quote0547QuoteHeadState"),
      time_presence: valueOrNull("#quote0547TimePresence"),
      name_keyword_tag: valueOrNull("#quote0547NameKeywordTag"),
      pattern_subbucket: valueOrNull("#quote0547PatternSubbucket"),
    }),
  });
});

document.querySelector("#quote0547ExtraProfileBtn").addEventListener("click", () => {
  callApi("0x0547 附加字段画像", "/api/quote/0547/extra-profile", {
    method: "POST",
    body: JSON.stringify({
      path: value("#quote0547ExtraProfilePath"),
      anomaly_limit: Number(value("#quote0547ExtraProfileLimit") || "10"),
    }),
  });
});

document.querySelector("#compare118Btn").addEventListener("click", () => {
  callApi("0x118 转储对比方案", "/api/debug/tdx118-dump-compare-plan");
});

document.querySelector("#blobCompareBtn").addEventListener("click", () => {
  callApi("二进制对比", "/api/debug/blob-compare", {
    method: "POST",
    body: JSON.stringify({
      left_path: value("#blobLeftPath"),
      right_path: value("#blobRightPath"),
      left_offset: Number(value("#blobLeftOffset") || "0"),
      right_offset: Number(value("#blobRightOffset") || "0"),
      compare_len: Number(value("#blobCompareLen") || "280"),
      block_size: Number(value("#blobBlockSize") || "8"),
    }),
  });
});

document.querySelector("#answerBtn").addEventListener("click", () => {
  callApi("应答汇总", "/api/debug/answer-summary", {
    method: "POST",
    body: JSON.stringify({
      path: value("#answerPath"),
    }),
  });
});

document.querySelector("#pcapBtn").addEventListener("click", () => {
  callApi("PCAP 汇总", "/api/debug/pcap-summary", {
    method: "POST",
    body: JSON.stringify({
      path: value("#pcapPath"),
      segment_limit: Number(value("#pcapSegmentLimit") || "12"),
    }),
  });
});

document.querySelector("#streamBtn").addEventListener("click", () => {
  callApi("流分析", "/api/debug/stream-analyze", {
    method: "POST",
    body: JSON.stringify({
      path: value("#streamPath"),
      is_hex: value("#streamMode") === "hex",
      preview_limit: Number(value("#streamPreviewLimit") || "8"),
    }),
  });
});

document.querySelector("#quoteFrameBtn").addEventListener("click", () => {
  callApi("行情帧扫描", "/api/debug/quote-frame-scan", {
    method: "POST",
    body: JSON.stringify({
      path: value("#quoteFramePath"),
    }),
  });
});

document.querySelector("#quoteBtn").addEventListener("click", () => {
  callApi("行情回放", "/api/debug/quote-replay", {
    method: "POST",
    body: JSON.stringify({
      path: value("#quotePath"),
      host: DEFAULT_REMOTE_HOST,
      port: Number(value("#quotePort") || "7719"),
      segments: valueOrNull("#quoteSegments"),
      recv_ms: Number(value("#quoteRecvMs") || "250"),
      connect_timeout_ms: Number(value("#quoteConnectTimeoutMs") || "5000"),
      save_path: valueOrNull("#quoteSavePath"),
    }),
  });
});

document.querySelector("#probeBtn").addEventListener("click", () => {
  callApi("协议探测", "/api/debug/proto-probe", {
    method: "POST",
    body: JSON.stringify({
      host: DEFAULT_REMOTE_HOST,
      port: Number(value("#probePort") || "6100"),
      payload: valueOrNull("#probePayload"),
      encoding: value("#probeEncoding") || "utf16le",
      nul_terminate: boolValue("#probeNul"),
      prefix_u32le: boolValue("#probePrefixU32le"),
      read_secs: Number(value("#probeReadSecs") || "3"),
    }),
  });
});

bootstrapLegacyPanel()
  .then(() => probeLegacyPanel(false))
  .catch((error) => {
    setText(legacyElements.statusBar, `加载连接面板失败: ${error}`);
    setText(legacyElements.connectionState, "加载失败");
    pushLog(`旧连接面板初始化: 失败`);
  });

render();
