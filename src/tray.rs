//! @author 十四叔
//! @date 2026/07/26

//! 番茄钟系统托盘菜单构建。
//!
//! 菜单项 ID 用 ASCII 数字字符串 ("1"/"2"/"3"), 与 [`crate::window::tray_action_ids`]
//! 的 u8 常量值一一对应; 框架层 `Handler::about_to_wait` 解析字符串为 u8
//! 后转交 `app.tray_action`。
//!
//! 快捷键 label 由 [`crate::window::shortcut_for_id`] 给出, 与全局热键共用字符串。

use danqing::shortcut_for_id;
use danqing::tray_action_ids;
use danqing::tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

/// 构建托盘菜单: 三条目 (暂停/开始 / 显示/隐藏 / 退出) + 中间分隔符。
///
/// label 格式: `<中文动作>  <快捷键>`(右侧快捷键与首次启动 hint 一致,
/// 单一来源 [`shortcut_for_id`])。
pub fn build_menu() -> Menu {
    let menu = Menu::new();

    let item_pause = MenuItem::with_id(
        tray_action_ids::START_PAUSE.to_string(),
        format!(
            "暂停/开始  {}",
            shortcut_for_id(tray_action_ids::START_PAUSE)
        ),
        true,
        None,
    );
    let item_toggle = MenuItem::with_id(
        tray_action_ids::TOGGLE_VISIBLE.to_string(),
        format!(
            "显示/隐藏  {}",
            shortcut_for_id(tray_action_ids::TOGGLE_VISIBLE)
        ),
        true,
        None,
    );
    let separator = PredefinedMenuItem::separator();
    let item_quit = MenuItem::with_id(
        tray_action_ids::QUIT.to_string(),
        format!("退出  {}", shortcut_for_id(tray_action_ids::QUIT)),
        true,
        None,
    );

    // append 失败时 (理论上平台实现 bug) 记录并继续; 至少保证能显示。
    if let Err(err) = menu.append(&item_pause) {
        log::warn!("添加托盘菜单项 START_PAUSE 失败: {err}");
    }
    if let Err(err) = menu.append(&item_toggle) {
        log::warn!("添加托盘菜单项 TOGGLE_VISIBLE 失败: {err}");
    }
    if let Err(err) = menu.append(&separator) {
        log::warn!("添加托盘分隔符失败: {err}");
    }
    if let Err(err) = menu.append(&item_quit) {
        log::warn!("添加托盘菜单项 QUIT 失败: {err}");
    }

    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_menu_has_three_items() {
        // 烟雾测试: 菜单构造成功, 含 4 个 child (3 项 + 1 分隔符)。
        let menu = build_menu();
        let n = menu.items().len();
        assert!(n >= 3, "菜单应至少含 3 条目, 实际 {n}");
    }
}
