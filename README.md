# Rust Jav

为 jav torrent 编写的一些小工具

## 功能

- 删除 jav torrent 中的无用文件，如 楼风最全资源\*
- 重命名 jav torrent 中的文件，如 hhd800.com@SSIS-001.mp4 -> SSIS-001.mp4
- 文件夹名重命名，如 ssis-001 -> SSIS-001
- 根据后缀整理文件，如 `-C` `ch` 结尾的文件放到 `CHINESE` 文件夹中，`-UC` 结尾的文件放到 `UNCENSORED` 文件夹中

## 使用

- `-d` 指定 jav torrent 文件夹
- `-o` 指定整理后输出的文件夹，`CHINESE` 和 `UNCENSORED` 文件夹会放在这个文件夹下，不指定则不尽兴整理
- `-a` 所有功能启用

## Build

编译

```shell
cargo build --release
```

跨平台编译

```shell
CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --target x86_64-unknown-linux-gnu -v
```

## 生成测试文件

```shell
cargo run --example create
```
