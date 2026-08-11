// 防止在 Windows 发布构建时弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    dev_env_manager::run();
}
