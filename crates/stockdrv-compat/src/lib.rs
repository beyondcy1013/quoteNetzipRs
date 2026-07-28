#![allow(non_snake_case)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

mod gateway;

const ABI_VERSION: u32 = 0x7812_3456;
const PUSH_MAGIC: u32 = 0x3f00_1234;

struct PushRuntime {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

static PUSH_RUNTIME: OnceLock<Mutex<Option<PushRuntime>>> = OnceLock::new();

#[inline]
fn success() -> i32 {
    1
}

#[unsafe(no_mangle)]
pub extern "system" fn StockInit(_param: *mut c_void) -> i32 {
    success()
}

#[unsafe(no_mangle)]
pub extern "system" fn Stock_Init(
    hwnd: *mut c_void,
    message: *mut c_void,
    _reserved: *mut c_void,
) -> i32 {
    trace_abi(&format!("Stock_Init hwnd={:?} message={:?}", hwnd, message));
    #[cfg(target_os = "windows")]
    {
        start_push(hwnd as isize, message as usize as u32)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (hwnd, message);
        success()
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Stock_Quit(_param: *mut c_void) -> i32 {
    trace_abi("Stock_Quit");
    stop_push();
    success()
}

#[unsafe(no_mangle)]
pub extern "system" fn ReInitStockInfo(_param: *mut c_void) -> i32 {
    success()
}

#[unsafe(no_mangle)]
pub extern "system" fn SetCodeTable(_param: *mut c_void) -> i32 {
    success()
}

#[unsafe(no_mangle)]
pub extern "system" fn SetParam(_param: *mut c_void) -> i32 {
    success()
}

#[unsafe(no_mangle)]
pub extern "system" fn SetupReceiver(_param: *mut c_void) -> i32 {
    success()
}

#[unsafe(no_mangle)]
pub extern "system" fn GetTotalNumber() -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn GetStockDrvInfo(selector: i32, _out: *mut c_void) -> u32 {
    match selector {
        1 => ABI_VERSION,
        2 => 0x300,
        3 => 1,
        4 => 12,
        5 => 32,
        1..=32 => selector as u32,
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn GetStockByCode(code: *const c_void, out: *mut c_void, _flags: i32) -> i32 {
    trace_abi(&format!(
        "GetStockByCode code_ptr={:?} out_ptr={:?}",
        code, out
    ));
    fetch_report(code.cast(), out.cast())
}

#[unsafe(no_mangle)]
pub extern "system" fn GetStockByCodeEx(code: *const c_void, out: *mut c_void, flags: i32) -> i32 {
    GetStockByCode(code, out, flags)
}

#[unsafe(no_mangle)]
pub extern "system" fn GetStockByNo(_no: i32, _out: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn GetStockByNoEx(_no: i32, _out: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn GetTradeData(_param: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn SCAskData(_ask: i32, _out: *mut c_void) -> i32 {
    0
}

#[unsafe(no_mangle)]
pub extern "system" fn SCStockInit(_a: *mut c_void, _b: *mut c_void, _c: *mut c_void) -> i32 {
    success()
}

fn fetch_report(code: *const u16, out: *mut u8) -> i32 {
    if code.is_null() || out.is_null() {
        return 0;
    }
    let result = std::panic::catch_unwind(|| {
        let instrument = unsafe { read_wide_code(code) }?;
        let report = match gateway::fetch_report(&instrument) {
            Ok(report) => report,
            Err(error) => {
                log_error(&format!(
                    "GetStockByCode instrument={instrument} error={error}"
                ));
                return None;
            }
        };
        unsafe { std::ptr::copy_nonoverlapping(report.as_ptr(), out, report.len()) };
        Some(1)
    });
    result.ok().flatten().unwrap_or(0)
}

fn log_error(message: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("stockdrv-compat.log")
    {
        let _ = writeln!(file, "{message}");
    }
}

fn trace_abi(message: &str) {
    if std::env::var("TUWENCA_TRACE_ABI").as_deref() == Ok("1") {
        log_error(message);
    }
}

#[cfg(target_os = "windows")]
fn start_push(hwnd: isize, message: u32) -> i32 {
    if hwnd == 0 || message == 0 {
        return 0;
    }
    stop_push();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::spawn(move || push_loop(hwnd, message, thread_stop));
    *PUSH_RUNTIME
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(PushRuntime { stop, thread });
    1
}

fn stop_push() {
    let Some(runtime) = PUSH_RUNTIME.get() else {
        return;
    };
    let active = runtime.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(active) = active {
        active.stop.store(true, Ordering::Release);
        let _ = active.thread.join();
    }
}

#[cfg(target_os = "windows")]
fn push_loop(hwnd: isize, message: u32, stop: Arc<AtomicBool>) {
    unsafe extern "system" {
        fn SendMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    }
    let configured_codes = std::env::var("TUWENCA_PUSH_CODES").ok();
    let interval = std::env::var("TUWENCA_PUSH_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1_000)
        .max(100);
    while !stop.load(Ordering::Acquire) {
        let mut records = match gateway::fetch_worklist() {
            Ok(quotes) => quotes,
            Err(error) => {
                log_error(&format!("full push worklist error={error}"));
                Vec::new()
            }
        };
        if let Some(codes) = &configured_codes {
            let wanted: std::collections::HashSet<&str> = codes
                .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                .filter(|value| !value.is_empty())
                .collect();
            records.retain(|quote| wanted.contains(quote.instrument.as_str()));
        }
        for batch in records.chunks(2_000) {
            let mut packet = match gateway::encode_push_packet(batch) {
                Ok(packet) => packet,
                Err(error) => {
                    log_error(&format!("push packet encode error: {error}"));
                    continue;
                }
            };
            if let Err(error) = gateway::prepare_push_packet(&mut packet) {
                log_error(&format!("push packet prepare error: {error}"));
                continue;
            }
            trace_abi(&format!(
                "push batch count={} bytes={} hwnd=0x{:x} message=0x{:x}",
                batch.len(),
                packet.len(),
                hwnd,
                message
            ));
            unsafe { SendMessageW(hwnd, message, PUSH_MAGIC as usize, packet.as_ptr() as isize) };
        }
        let mut waited = 0;
        while waited < interval && !stop.load(Ordering::Acquire) {
            let step = (interval - waited).min(100);
            std::thread::sleep(Duration::from_millis(step));
            waited += step;
        }
    }
}

unsafe fn read_wide_code(code: *const u16) -> Option<String> {
    let mut words = Vec::with_capacity(12);
    for index in 0..12 {
        let word = unsafe { code.add(index).read() };
        if word == 0 {
            break;
        }
        words.push(word);
    }
    if words.is_empty() || words.len() == 12 {
        return None;
    }
    String::from_utf16(&words).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialization_is_successful_and_unimplemented_reads_are_explicitly_empty() {
        assert_eq!(StockInit(std::ptr::null_mut()), 1);
        assert_eq!(Stock_Quit(std::ptr::null_mut()), 1);
        assert_eq!(GetTotalNumber(), 0);
        assert_eq!(GetStockByNo(1, std::ptr::null_mut()), 0);
    }

    #[test]
    fn observed_driver_info_constants_are_preserved() {
        assert_eq!(GetStockDrvInfo(1, std::ptr::null_mut()), ABI_VERSION);
        assert_eq!(GetStockDrvInfo(2, std::ptr::null_mut()), 0x300);
    }

    #[test]
    fn full_push_packet_matches_the_observed_send_message_layout() {
        let packet = gateway::encode_push_packet(&[gateway::PushQuote {
            instrument: "SH600000".to_owned(),
            name: "浦发银行".to_owned(),
            timestamp: 0,
            price: 9.04,
            last_close: 9.05,
            volume: 10.0,
            amount: 20.0,
        }])
        .unwrap();
        assert_eq!(packet.len(), 0x124 + 0x9e);
        assert_eq!(
            u32::from_le_bytes(packet[0..4].try_into().unwrap()),
            PUSH_MAGIC
        );
        assert_eq!(u32::from_le_bytes(packet[4..8].try_into().unwrap()), 1);
        assert_eq!(
            &packet[0x14..0x2a],
            b"\xca\xb5\xca\xb1\xca\xfd\xbe\xdd RCV_REPORTV3\0"
        );
        assert_eq!(&packet[0x11c..0x120], &[0; 4]);
        assert_eq!(
            u16::from_le_bytes(packet[0x124..0x126].try_into().unwrap()),
            0x9e
        );
        assert_eq!(&packet[0x12a..0x132], b"SH600000");
        assert_eq!(
            f32::from_le_bytes(packet[0x166..0x16a].try_into().unwrap()),
            9.04
        );
        assert_eq!(
            f32::from_le_bytes(packet[0x156..0x15a].try_into().unwrap()),
            9.05
        );
        assert_eq!(
            f32::from_le_bytes(packet[0x16a..0x16e].try_into().unwrap()),
            10.0
        );
        assert_eq!(
            f32::from_le_bytes(packet[0x16e..0x172].try_into().unwrap()),
            20.0
        );
    }
}
