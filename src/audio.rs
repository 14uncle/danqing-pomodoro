//! @author 十四叔
//! @date 2026/07/25

//! 完成反馈音效：Windows 走 `MessageBeep(MB_ICONASTERISK)`, 其它平台 stub。
//!
//! 设计取舍：无 WAV 资产 + 无音频管线负担; 用户后续若提供 WAV,
//! 替换 `beep()` 实现即可。

/// 播放一次系统提示音 (番茄阶段流转用)。
///
/// Windows: `MessageBeep(0x00000040)` (`MB_ICONASTERISK`)。
/// Mac / Linux: 无操作 (任务范围 Windows 优先)。
pub fn beep() {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: MessageBeep 是纯系统调用，无内存 / 句柄参数，调用安全。
        unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(0x00000040);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::any::type_name::<()>(); // 占位，避免空 fn 警告
    }
}
