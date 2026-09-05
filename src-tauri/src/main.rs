// 防止在 Windows 发布构建时弹出控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 命令注册、插件、Git 服务状态都在 core 的 app_builder() 里；
    // 这里仅负责在持有 tauri.conf.json 的 crate 内生成 context 并启动。
    devenv_core::app_builder()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
