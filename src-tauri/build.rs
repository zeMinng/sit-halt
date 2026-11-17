fn main() {
    // tauri-build 默认只监听 tauri.conf.json 与 capabilities，不会因替换 icons/ 下的
    // 图标文件而重新编译；这里显式把图标目录纳入增量监听，换图标后能自动重新嵌入。
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
