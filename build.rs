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
