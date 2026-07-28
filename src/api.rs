#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::ffi::c_void;
use std::path::Path;

use libloading::{Library, Symbol};

use crate::{StockMessage, interpret_sync_answer};

pub type StockAnswer =
    unsafe extern "system" fn(form: *const u16, data: *mut c_void, ask_id: i32) -> i32;
type StartFn = unsafe extern "system" fn(callback: StockAnswer) -> i32;
type AskFn = unsafe extern "system" fn(ask: *const u16, out: *mut c_void, max_len: i32) -> i32;
type StopFn = unsafe extern "system" fn() -> i32;

pub struct StockApi {
    _library: Library,
    start: StartFn,
    ask: AskFn,
    stop: StopFn,
}

impl StockApi {
    pub unsafe fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let library = unsafe { Library::new(path) }?;
        let start: StartFn = {
            let symbol: Symbol<'_, StartFn> = unsafe { library.get(b"Start\0") }?;
            *symbol
        };
        let ask: AskFn = {
            let symbol: Symbol<'_, AskFn> = unsafe { library.get(b"Ask\0") }?;
            *symbol
        };
        let stop: StopFn = {
            let symbol: Symbol<'_, StopFn> = unsafe { library.get(b"Stop\0") }?;
            *symbol
        };

        Ok(Self {
            _library: library,
            start,
            ask,
            stop,
        })
    }

    pub fn start(&self, callback: StockAnswer) -> i32 {
        unsafe { (self.start)(callback) }
    }

    pub fn ask(&self, ask: &[u16], out: &mut [u8]) -> Result<i32, Box<dyn std::error::Error>> {
        let max_len = i32::try_from(out.len())?;
        let ret = unsafe { (self.ask)(ask.as_ptr(), out.as_mut_ptr().cast::<c_void>(), max_len) };
        Ok(ret)
    }

    pub fn ask_message(
        &self,
        ask: &[u16],
        out: &mut [u8],
    ) -> Result<StockMessage, Box<dyn std::error::Error>> {
        let ret = self.ask(ask, out)?;
        Ok(interpret_sync_answer(ret, out))
    }

    pub fn stop(&self) -> i32 {
        unsafe { (self.stop)() }
    }
}
