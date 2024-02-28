# Rust Jav

为 jav torrent 编写的一些小工具

## 功能

- 使用 rust 编写，tokio 异步，速度快
- 删除 jav torrent 中的无用文件，如 楼风最全资源\*
- 重命名 jav torrent 中的文件，如 hhd800.com@SSIS-001.mp4 -> SSIS-001.mp4
- 文件夹名重命名，如 ssis-001 -> SSIS-001
- 根据后缀整理文件，如 `-C` `ch` 结尾的文件放到 `CHINESE` 文件夹中，`-UC` 结尾的文件放到 `UNCENSORED` 文件夹中
- 使用 dialoguer 交互式操作，方便使用

## 使用

```
cargo run  -- -d ./examples/test
```

| 参数                       | 说明                                                                                       |
| -------------------------- | ------------------------------------------------------------------------------------------ |
| -d                         | jav torrent 文件夹                                                                         |
| -o                         | 整理后输出的文件夹，`CHINESE` 和 `UNCENSORED` 文件夹会放在这个文件夹下，不指定则不进行整理 |
| -a                         | 所有功能启用                                                                               |
| -l                         | 日志级别，可选 `trace` `debug` `info` `warn` `error`                                       |
| -v                         | 版本                                                                                       |
| --delete-ad                | 删除 jav torrent 中的无用文件, 如广告视频                                                  |
| --delete-dir-with-no-video | 删除 jav torrent 中的没有视频的文件夹 ｜                                                   |
| --move-chinese             | 将 `-C` `ch` 结尾的文件放到 `CHINESE` 文件夹中                                             |
| --move-uncensored          | 将 `-UC` 结尾的文件放到 `UNCENSORED` 文件夹中                                              |
| --rename-upper-case        | 文件夹名重命名为大写                                                                       |
| --remove-prefixes          | 删除文件名中的前缀，如 hhd800.com@                                                         |
| -h                         | 帮助                                                                                       |

## Build

编译

```shell
cargo build --release
```

跨平台编译

先安装 [cross](https://github.com/cross-rs/cross)

```shell
cargo install cross --git https://github.com/cross-rs/cross

```

```shell
CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --target x86_64-unknown-linux-gnu -v
```

## 测试命令

### 生成测试文件

```shell
cargo run --example create
```

### 全部操作

```shell
cargo run  -- -d ./examples/test -o . -a -l trace
```
