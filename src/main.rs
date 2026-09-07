#[cfg(target_os = "windows")]
use std::error::Error;
#[cfg(target_os = "windows")]
use std::io::{self, Write};
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "windows")]
use netzipapi_rust_demo::{StockAnswer, StockApi};
#[cfg(target_os = "windows")]
use netzipapi_rust_demo::{StockMessageKind, interpret_callback_ptr, to_wide_null};

#[cfg(target_os = "windows")]
const MAX_ANSWER_LEN: usize = 20 * 1000 * 1024;
#[cfg(target_os = "windows")]
const DEFAULT_DLL: &str = "Stock64.dll";
#[cfg(target_os = "windows")]
const INIT_QUERIES: [&str; 3] = [
    "股票数据?请求=登录&模块=认证&账号=&密码=&自动升级=稳定版&版本=20221120&等待=10000&编号=0",
    "股票数据?请求=登录&模块=股票备用&账号=&密码=&等待=10000&编号=20",
    "股票数据?请求=初始化&分析软件=自定义&等待=10000&编号=-1",
];

#[cfg(target_os = "windows")]
static DLL_RELOAD_NEEDED: AtomicBool = AtomicBool::new(false);

#[cfg(not(target_os = "windows"))]
fn main() {
    println!(
        "这个 binary 是 Windows DLL 示例；真实 DLL 调用仍依赖 Windows 下的 Stock.dll/Stock64.dll。"
    );
    println!(
        "如果当前优先走纯 Rust + Linux，请直接使用 `cargo run --bin netzip_linux -- --help` 或 `cargo run --bin quoteNetzipRs`。"
    );
    println!(
        "按 2026-03-29/30 现场取证，Windows 现场主链仍是 x86 Stock.dll + 网际风.exe + 127.0.0.1:2000 本地桥。"
    );
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
fn run() -> Result<(), Box<dyn Error>> {
    let dll_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DLL));

    println!("加载 DLL: {}", dll_path.display());
    let api = unsafe { StockApi::load(&dll_path) }?;

    let started = api.start(stock_callback as StockAnswer);
    if started == 0 {
        return Err("Start 回调注册失败".into());
    }

    println!("初始化成功。");
    let mut answer = vec![0u8; MAX_ANSWER_LEN];

    // for query in INIT_QUERIES {
    //     run_query(&api, query, &mut answer)?;
    //     if DLL_RELOAD_NEEDED.load(Ordering::Relaxed) {
    //         println!("检测到 DLL 需要重新载入，请先重启程序。");
    //         break;
    //     }
    // }

    println!("进入交互模式。直接粘贴完整调用串，输入 q 退出。");
    let stdin = io::stdin();
    loop {
        print!("netzip> ");
        io::stdout().flush()?;

        let mut line = String::new();
        stdin.read_line(&mut line)?;
        let line = line.trim();

        if line.is_empty() || line.eq_ignore_ascii_case("q") {
            break;
        }

        run_query(&api, line, &mut answer)?;
        if DLL_RELOAD_NEEDED.load(Ordering::Relaxed) {
            println!("检测到 DLL 需要重新载入，请先重启程序。");
            break;
        }
    }

    let stop_ret = api.stop();
    println!("Stop 返回值: {stop_ret}");
    if stop_ret >= 1000 {
        eprintln!("官方示例在 Stop >= 1000 时会直接结束进程。");
        std::process::exit(0);
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn run_query(api: &StockApi, query: &str, answer: &mut [u8]) -> Result<(), Box<dyn Error>> {
    answer.fill(0);
    let wide = to_wide_null(query);
    let message = api.ask_message(&wide, answer)?;
    println!("Ask 返回值: {}", message.return_code.unwrap_or_default());

    if message.accepted_async {
        println!("[sync/accepted_async] {}", message.summary);
    } else if message.kind != StockMessageKind::Empty {
        println!("[sync/{:?}] {}", message.kind, message.summary);
    }

    Ok(())
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn stock_callback(
    form: *const u16,
    data: *mut std::ffi::c_void,
    ask_id: i32,
) -> i32 {
    let result = std::panic::catch_unwind(|| unsafe {
        let message = interpret_callback_ptr(form, data, ask_id);

        if message.form.as_deref() == Some("错误")
            && message.summary.contains("dll升级后需要重新载入")
        {
            DLL_RELOAD_NEEDED.store(true, Ordering::Relaxed);
            eprintln!("{}", message.summary);
            return 0;
        }

        let form = message.form.as_deref().unwrap_or("(unknown)");
        println!("[callback/{form}/{:?}] {}", message.kind, message.summary);
        1
    });

    result.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use netzipapi_rust_demo::{StockMessageKind, StockTextFormat, interpret_sync_answer};

    #[test]
    fn interpret_sync_answer_decodes_utf16_json() {
        let utf16: Vec<u16> = "{\"请求\":\"提示信息\",\"数据\":\"初始化完成\"}"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut bytes = Vec::with_capacity(utf16.len() * 2);
        for word in utf16 {
            bytes.extend_from_slice(&word.to_le_bytes());
        }

        let message = interpret_sync_answer(bytes.len() as i32, &bytes);
        assert_eq!(message.kind, StockMessageKind::Text);
        assert_eq!(message.text_format, Some(StockTextFormat::Json));
        assert!(message.summary.contains("初始化完成"));
    }
}
