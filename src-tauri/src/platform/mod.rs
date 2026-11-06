//! 平台适配层：平台差异集中在这里，核心提醒逻辑不感知平台。

/// 一段时间内没有任何键鼠输入就算「离开座位」。
pub struct IdleProbe;

impl IdleProbe {
    /// 返回系统空闲秒数；无法获取时返回 None（调用方不得据此推断用户已离开）。
    pub fn idle_seconds() -> Option<u64> {
        imp::idle_seconds()
    }
}

/// 当前平台标识，前端「关于」区域会展示。
pub fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

#[cfg(windows)]
mod imp {
    use std::time::Duration;

    #[repr(C)]
    struct LastInputInfo {
        cb_size: u32,
        dw_time: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetLastInputInfo(plii: *mut LastInputInfo) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        // 与 LastInputInfo.dwTime 同源（32 位、约 49.7 天回绕），两者可直接相减。
        fn GetTickCount() -> u32;
    }

    pub fn idle_seconds() -> Option<u64> {
        let mut info = LastInputInfo {
            cb_size: std::mem::size_of::<LastInputInfo>() as u32,
            dw_time: 0,
        };
        // SAFETY: 传入的是本函数栈上的有效结构体指针，字段按 Win32 定义布局。
        let ok = unsafe { GetLastInputInfo(&mut info) };
        if ok == 0 {
            return None;
        }
        // SAFETY: 无参数、无副作用，仅返回系统启动以来的毫秒数。
        let now_ms = unsafe { GetTickCount() } as u64;
        let idle_ms = now_ms.saturating_sub(info.dw_time as u64);
        Some(Duration::from_millis(idle_ms).as_secs())
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn idle_seconds() -> Option<u64> {
        None
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::IdleProbe;

    #[test]
    fn idle_probe_reads_system_idle_time() {
        // 真实调用 GetLastInputInfo：只要有输入设备会话，就应能读到非负的空闲秒数
        let seconds = IdleProbe::idle_seconds();
        println!("[idle-probe] 当前系统空闲秒数 = {seconds:?}");
        assert!(seconds.is_some(), "Windows 下应能获取系统空闲时长");
        assert!(
            seconds.unwrap() < 60 * 60 * 24 * 400,
            "空闲秒数应在合理范围内"
        );
    }
}
