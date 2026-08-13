//! @author 十四叔
//! @date 2026/08/12
//!
//! 构建脚本: Windows 可执行文件嵌入图标资源。

fn main() {
    #[cfg(target_os = "windows")]
    {
        if let Err(e) = winresource::WindowsResource::new()
            .set_icon("assets/logo/pomodoro.ico")
            .compile()
        {
            eprintln!("图标嵌入失败（不影响功能）: {e}");
        }
    }
}
