#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("stockdrv_probe is a Windows-only acceptance tool");
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = run() {
        eprintln!("probe_error={error}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use std::mem::MaybeUninit;
    use tuwenca_codec::OemReport;

    type GetStockByCode = unsafe extern "system" fn(*const u16, *mut OemReport, i32) -> i32;
    let instrument = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "SH600000".to_owned());
    if instrument == "--push" {
        return run_push_probe();
    }
    let wide: Vec<u16> = instrument
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let library = unsafe { libloading::Library::new("Stockdrv.dll") }?;
    let get: libloading::Symbol<GetStockByCode> = unsafe { library.get(b"GetStockByCode") }?;
    let mut report = MaybeUninit::<OemReport>::zeroed();
    let result = unsafe { get(wide.as_ptr(), report.as_mut_ptr(), 0) };
    if result != 1 {
        return Err(format!("GetStockByCode returned {result}").into());
    }
    let report = unsafe { report.assume_init() };
    let label = utf16_string(unsafe { std::ptr::addr_of!(report.label).read_unaligned() });
    let name = utf16_string(unsafe { std::ptr::addr_of!(report.name).read_unaligned() });
    let time = unsafe { std::ptr::addr_of!(report.time).read_unaligned() };
    let close = unsafe { std::ptr::addr_of!(report.close).read_unaligned() };
    let volume = unsafe { std::ptr::addr_of!(report.volume).read_unaligned() };
    let amount = unsafe { std::ptr::addr_of!(report.amount).read_unaligned() };
    println!("result=1");
    println!("label={label}");
    println!("name={name}");
    println!("time={time}");
    println!("close={close}");
    println!("volume={volume}");
    println!("amount={amount}");
    Ok(())
}

#[cfg(target_os = "windows")]
fn run_push_probe() -> Result<(), Box<dyn std::error::Error>> {
    use std::ffi::c_void;
    use std::mem::zeroed;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    const PUSH_MESSAGE: u32 = 0x8123;
    const WM_CLOSE: u32 = 0x0010;
    static RECEIVED: AtomicBool = AtomicBool::new(false);
    static TOTAL: AtomicUsize = AtomicUsize::new(0);
    static BATCHES: AtomicUsize = AtomicUsize::new(0);

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        point: Point,
        private: u32,
    }
    #[repr(C)]
    struct WndClass {
        style: u32,
        wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu_name: *const u16,
        class_name: *const u16,
    }
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn RegisterClassW(class: *const WndClass) -> u16;
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: *mut c_void,
        ) -> isize;
        fn GetMessageW(message: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        fn TranslateMessage(message: *const Msg) -> i32;
        fn DispatchMessageW(message: *const Msg) -> isize;
        fn DefWindowProcW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
        fn PostMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> i32;
        fn PostQuitMessage(exit_code: i32);
    }
    unsafe extern "system" fn window_proc(
        hwnd: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        if message == PUSH_MESSAGE && lparam != 0 {
            let bytes = unsafe { std::slice::from_raw_parts(lparam as *const u8, 6 + 158) };
            let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
            let count = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
            let label_end = bytes[6..18]
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(12);
            let label = String::from_utf8_lossy(&bytes[6..6 + label_end]);
            let close = f32::from_le_bytes(bytes[56..60].try_into().unwrap());
            let total = TOTAL.fetch_add(count as usize, Ordering::AcqRel) + count as usize;
            let batches = BATCHES.fetch_add(1, Ordering::AcqRel) + 1;
            println!("push_received=1");
            println!("message=0x{message:04x}");
            println!("wparam=0x{wparam:08x}");
            println!("magic=0x{magic:08x}");
            println!("count={count}");
            println!("label={label}");
            println!("close={close}");
            println!("batch={batches}");
            println!("total={total}");
            RECEIVED.store(true, Ordering::Release);
            if count < 2_000 {
                unsafe { PostQuitMessage(0) };
            }
            return 1;
        }
        if message == WM_CLOSE {
            unsafe { PostQuitMessage(2) };
            return 0;
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    type StockInit = unsafe extern "system" fn(isize, u32, isize) -> i32;
    type StockQuit = unsafe extern "system" fn(*mut c_void) -> i32;
    let library = unsafe { libloading::Library::new("Stockdrv.dll") }?;
    let init: libloading::Symbol<StockInit> = unsafe { library.get(b"Stock_Init") }?;
    let quit: libloading::Symbol<StockQuit> = unsafe { library.get(b"Stock_Quit") }?;
    let class_name: Vec<u16> = "StockdrvProbeWindow\0".encode_utf16().collect();
    let instance = unsafe { GetModuleHandleW(null()) };
    let class = WndClass {
        style: 0,
        wnd_proc: Some(window_proc),
        cls_extra: 0,
        wnd_extra: 0,
        instance,
        icon: 0,
        cursor: 0,
        background: 0,
        menu_name: null(),
        class_name: class_name.as_ptr(),
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err("RegisterClassW failed".into());
    }
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            instance,
            null_mut(),
        )
    };
    if hwnd == 0 {
        return Err("CreateWindowExW failed".into());
    }
    if unsafe { init(hwnd, PUSH_MESSAGE, 0) } != 1 {
        return Err("Stock_Init failed".into());
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        if !RECEIVED.load(Ordering::Acquire) {
            unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
        }
    });
    let mut message: Msg = unsafe { zeroed() };
    while unsafe { GetMessageW(&mut message, 0, 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    unsafe { quit(null_mut()) };
    if !RECEIVED.load(Ordering::Acquire) {
        return Err("push timeout".into());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn utf16_string<const N: usize>(words: [u16; N]) -> String {
    let end = words.iter().position(|word| *word == 0).unwrap_or(N);
    String::from_utf16_lossy(&words[..end])
}
