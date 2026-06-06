# 仓库规范

## 项目结构与模块组织

这是一个包含三个 crate 的 Rust 工作空间：

- `mfprog-esp-lib/`：核心库，负责解析 flash 条目、读写 JSON、压缩固件以及计算元数据。
- `mfprog-esp-cli/`：命令行前端。其二进制文件名为 `MFProg-ESP-fw-tool`。
- `mfprog-esp-gui/`：基于 Slint 的图形界面前端。UI 定义位于 `mfprog-esp-gui/ui/`，二进制文件名为 `MFProg-ESP-fw-tool-gui`。

共享功能通常应放在 `mfprog-esp-lib` 中，以确保 CLI 和 GUI 保持一致。构建输出位于 `target/` 目录下，不应提交到版本控制。

## 构建、测试与开发命令

- `cargo build`：以调试模式构建所有工作空间 crate。
- `cargo build --release`：在 `target/release/` 下构建优化的 CLI 和 GUI 二进制文件。
- `cargo build --release -p mfprog-esp-cli`：仅构建 CLI 的 release 二进制文件。
- `cargo build --release -p mfprog-esp-gui`：仅构建 GUI 的 release 二进制文件。
- `cargo test`：运行工作空间测试套件；CI 也会使用此命令。
- `cargo fmt`：在提交前格式化 Rust 代码。
- `cargo clippy --workspace --all-targets`：在本地可用时运行 lint 检查。

使用 `cargo run -p mfprog-esp-cli -- -i <输入> -o <输出>` 运行 CLI。

## 代码风格与命名规范

使用 Rust 2021 规范和 `rustfmt` 默认配置：四空格缩进，函数/模块使用 `snake_case`，类型使用 `PascalCase`，常量使用 `SCREAMING_SNAKE_CASE`。用户可见的二进制文件名保持与各自 crate 的 `Cargo.toml` 中定义的一致。

建议从可能失败的代码中返回 `Result`，并将解析、压缩和 JSON 转换逻辑保留在库 crate 中。GUI 代码应负责展示和文件选择，不要重复业务逻辑。

## 测试规范

将单元测试放在被测代码附近，使用 `#[cfg(test)] mod tests`。当测试类似 CLI 的工作流或文件输出行为时，在 crate 级别的 `tests/` 目录下添加集成测试。测试命名应体现行为，例如 `parses_tuple_file_list` 或 `writes_compressed_flash_map`。

在提交 PR 前，运行 `cargo test`；对于 UI 或 release 相关的变更，还需对受影响的 crate 进行针对性的 release 构建。

## 提交与 Pull Request 规范

近期历史提交使用简短的祈使句主题，并偶尔使用 Conventional Commit 前缀，如 `feat:` 和 `fix:`。请遵循此风格，例如 `feat: flatten flash file paths` 或 `fix: validate missing flash entries`。

PR 应包含简洁的摘要、用于验证的命令，以及 GUI 变更的相关截图。在适用时关联相关 issue。对于发布工作流的变更，需提及平台影响，因为 CI 会为多个操作系统构建 release 产物。
