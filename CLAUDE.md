# Red OS — CLAUDE.md

## 项目概述

AArch64 架构的 POSIX 兼容操作系统，Rust 语言开发。目标是能运行 BusyBox/ash shell 级别的程序。

## 技术栈

- 语言: Rust (nightly) + AArch64 汇编
- 目标架构: `aarch64-unknown-none-softfloat`
- 运行平台: QEMU virt machine（主力开发）、VirtualBox/VMware、真实物理硬件
- POSIX 目标: 核心子集（进程、文件、信号、管道），能运行 BusyBox/ash

## 项目结构

```
red-os/
├── Cargo.toml              # Cargo workspace 根
├── Makefile                # 顶层构建与运行脚本
├── rust-toolchain.toml     # 固定 nightly 工具链版本
├── CLAUDE.md               # 本文件，Claude 项目配置
├── kernel/                 # 内核 crate
│   ├── Cargo.toml
│   ├── kernel.ld           # 链接器脚本
│   └── src/
│       ├── main.rs         # 内核入口
│       └── arch/aarch64/
│           ├── boot.S      # 汇编启动代码
│           └── ...
├── user/                   # 用户空间程序 crate
└── tools/                  # 辅助脚本
```

## 构建命令

```bash
make run       # 构建并在 QEMU 中运行（串口输出到终端）
make debug     # 启动 QEMU + 等待 GDB 连接（端口 1234）
make build     # 仅构建，不运行
make clean     # 清理构建产物
make initramfs # 打包 BusyBox initramfs（Phase 7 后可用）
make disk      # 创建 ext2 磁盘镜像（Phase 8 后可用）
```

## QEMU 运行参数

```
qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a57 \
  -nographic \
  -kernel kernel/target/aarch64-unknown-none-softfloat/release/kernel \
  -m 128M
```

## 代码规范

- 内核代码全部 `#![no_std]`，除明确标注处外不使用 std
- `unsafe` 代码块必须加注释说明为何安全
- 每个 Phase 完成并验证后执行 `git commit`
- 不引入不必要的抽象，保持代码简洁

## 提交规范

格式: `feat(phaseN): 简要描述`

每次提交前确保 `make run` 能运行到当前里程碑，不引入回归。

示例：
```
feat(phase0): 项目基础设施，QEMU 串口输出 Hello World
feat(phase1): AArch64 启动序列完整，MMU 开启
feat(phase2): 物理内存管理与内核堆分配器
```

## 阶段文档规范

**每个 Phase 完成并 git commit 后，必须生成对应的学习文档** `docs/phaseN.md`，内容包括：

1. **该阶段概述** — 做了什么、达成了什么里程碑
2. **Rust 知识点** — 涉及的语言特性、语法、惯用法（附代码示例）
3. **OS 开发知识** — 硬件原理、设计思路、权衡取舍
4. **关键代码解析** — 核心代码逐行注解
5. **延伸阅读** — 相关规范文档、参考资料链接

文档提交格式: `docs(phaseN): 添加 PhaseN 学习文档`

## 里程碑

| 阶段 | 验证标志 |
|------|---------|
| Phase 0 | QEMU 串口输出 "Hello, Red OS!" |
| Phase 1 | EL1 启动，MMU 开启，异常不死机 |
| Phase 2 | `Box::new(42)` 不 panic |
| Phase 3 | 定时器每 10ms 触发中断 |
| Phase 4 | 两个进程交替打印 A/B |
| Phase 5 | 运行静态编译 Hello World C 程序 |
| Phase 6 | VFS 文件读写，/dev /proc 正常 |
| Phase 7 | BusyBox ash shell 可用 |
| Phase 8 | ext2 持久化文件系统 |
| Phase 9 | TCP 连接成功 |
| Phase 10 | RPi4 真机启动到 shell |

## 参考资料

- [rCore-Tutorial-v3-arm64](https://github.com/rcore-os/rCore-Tutorial-v3-arm64)
- [ArceOS](https://github.com/arceos-org/arceos)
- [rust-raspberrypi-OS-tutorials](https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials)
- [QEMU virt machine 文档](https://qemu-project.gitlab.io/qemu/system/arm/virt.html)
- [AArch64 MMU 编程](https://lowenware.com/blog/aarch64-mmu-programming/)
- [PL011 UART 驱动](https://krinkinmu.github.io/2020/11/29/PL011.html)
