# MFProg-ESP-fw-tool

基于 [compress_flash](https://github.com/OfflineProgrammerForEsp) 的 ESP 固件压缩打包工具，提供 CLI 和 GUI 两种使用方式。

## 功能

- 读取 ESP 项目文件夹中的 `flasher_args.json`，自动识别需要压缩的固件文件
- 对每个文件进行 **zlib deflate** 压缩（4 字节对齐），生成 `.zl` 文件
- 计算原始文件的 **MD5** 校验值
- 输出新的 `flasher_args.json`，包含 `map` 字段记录压缩元数据
- GUI 支持手动添加/删除文件条目，或导入文件夹自动识别

## 项目结构

```
MFProg-ESP-fw-tool/
├── mfprog-esp-lib/     # 核心库（压缩、JSON、解析）
├── mfprog-esp-cli/     # CLI 工具
└── mfprog-esp-gui/     # GUI 工具（Slint）
```

## 构建

```bash
cargo build --release
```

构建产物位于 `target/release/`：
- `MFProg-ESP-fw-tool` — CLI 工具
- `MFProg-ESP-fw-tool-gui` — GUI 工具

## CLI 使用

### 文件夹模式

输入一个包含 `flasher_args.json` 的文件夹：

```bash
MFProg-ESP-fw-tool -i <input_folder> -o <output_folder>
```

### 文件列表模式

直接传入地址和文件对：

```bash
MFProg-ESP-fw-tool -i "[(0x0, bootloader.bin), (0x10000, app.bin)]" -o <output_folder>
```

## GUI 使用

启动 GUI：

```bash
MFProg-ESP-fw-tool-gui
```

界面说明：
- **导入文件夹**：选择含 `flasher_args.json` 的文件夹，自动填充文件列表
- **添加行 / 删除选中**：手动管理条目
- **文件路径 + 地址**：每行对应一个固件文件及其烧录地址
- **输出文件夹**：选择压缩结果保存位置
- **开始压缩**：执行压缩并显示状态

## 依赖

- Rust 1.70+
- [Slint](https://slint.dev/)（GUI）
- [clap](https://github.com/clap-rs/clap)（CLI）
- [flate2](https://github.com/rust-lang/flate2-rs)、[serde_json](https://github.com/serde-rs/json)、[md5](https://github.com/stainless-steel/md5)

## License

MIT
