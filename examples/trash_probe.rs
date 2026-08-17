//! 复现并验证 trash 在 GPUI 后台线程（已初始化为 MTA）上的删除行为。

use std::thread;

// ole32!CoInitializeEx：HRESULT CoInitializeEx(LPVOID pvReserved, DWORD dwCoInit)
#[link(name = "ole32")]
unsafe extern "system" {
    fn CoInitializeEx(pv_reserved: *const core::ffi::c_void, dw_co_init: u32) -> i32;
}

const COINIT_MULTITHREADED: u32 = 0x0;
const COINIT_APARTMENTTHREADED: u32 = 0x2;

fn main() {
    let tmp = "D:\\Projects\\gui_for_yazi\\_probe_trash_test.txt";
    std::fs::write(tmp, "delete me via trash").unwrap();

    let path = tmp.to_string();
    let r = thread::spawn(move || {
        // 模拟 GPUI 后台执行器线程：先被初始化为 MTA
        let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED) };
        eprintln!(
            "[trash-probe] CoInitializeEx(MTA) -> hr={:#010x} ({} = RPC_E_CHANGED_MODE)",
            hr as u32,
            if hr as u32 == 0x8001_0106 { "YES" } else { "no" }
        );

        // 关键：trash 内部会再次 CoInitializeEx，用 coinit_multithreaded 时 MTA 上返回 S_FALSE 不 panic
        match trash::delete(&path) {
            Ok(()) => eprintln!("[trash-probe] trash::delete OK"),
            Err(e) => eprintln!("[trash-probe] trash::delete ERR: {:?}", e),
        }
    });

    r.join().unwrap();

    if std::path::Path::new(tmp).exists() {
        eprintln!("[trash-probe] file still exists (delete failed)");
    } else {
        eprintln!("[trash-probe] file deleted (moved to recycle bin)");
    }
}
