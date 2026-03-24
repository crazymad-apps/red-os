# Phase 0：项目基础设施 — 学习文档

> 里程碑：在 QEMU virt machine 上通过 PL011 UART 输出 `Hello, Red OS!`

---

## 目录

1. [这个阶段做了什么](#1-这个阶段做了什么)
2. [Rust 知识点](#2-rust-知识点)
   - 2.1 `no_std` 环境
   - 2.2 `no_main` 与自定义入口
   - 2.3 裸指针与 `unsafe`
   - 2.4 内联汇编 `core::arch::asm!`
   - 2.5 全局汇编 `global_asm!`
   - 2.6 `core::fmt::Write` trait
   - 2.7 `#[macro_export]` 宏
   - 2.8 `#[panic_handler]`
   - 2.9 Cargo workspace
3. [OS 开发知识](#3-os-开发知识)
   - 3.1 AArch64 启动流程
   - 3.2 PL011 UART 原理
   - 3.3 MMIO（内存映射 I/O）
   - 3.4 链接器脚本
   - 3.5 BSS 段清零
   - 3.6 build.rs 构建脚本
4. [关键代码逐行解析](#4-关键代码逐行解析)
   - 4.1 boot.S
   - 4.2 uart.rs
   - 4.3 main.rs
5. [延伸阅读](#5-延伸阅读)

---

## 1. 这个阶段做了什么

Phase 0 的核心目标是搭建"能跑起来的最小系统"，验证整个工具链（编译→链接→QEMU 加载→CPU 执行）通路没有问题。

**完成的工作：**

```
CPU 上电
  → QEMU 将 ELF 加载到内存 0x40080000
  → 跳转到 _start（汇编）
  → 初始化栈指针
  → 清零 BSS 段
  → 跳转到 Rust kernel_main()
  → kernel_main() 通过 UART 输出字符串
  → 串口打印 "Hello, Red OS!"
```

**文件清单：**

| 文件 | 职责 |
|------|------|
| `rust-toolchain.toml` | 固定 Rust nightly 工具链版本 |
| `Cargo.toml` (workspace) | 管理多个 crate（kernel, user） |
| `kernel/Cargo.toml` | 内核 crate 配置，禁用标准库 |
| `kernel/kernel.ld` | 链接器脚本，控制内存布局 |
| `kernel/build.rs` | 构建脚本，向链接器传递参数 |
| `kernel/src/arch/aarch64/boot.S` | 汇编启动代码 |
| `kernel/src/uart.rs` | PL011 UART 驱动 |
| `kernel/src/main.rs` | Rust 内核入口 |

---

## 2. Rust 知识点

### 2.1 `no_std` 环境

```rust
#![no_std]
```

Rust 的标准库（`std`）依赖操作系统提供的功能：堆分配、线程、文件系统、网络……
我们**正在写操作系统本身**，所以这些都不存在。`#![no_std]` 告诉编译器：不链接 std，只使用 `core`。

**`core` 库** 是 `std` 的无 OS 子集，包含：
- 基本类型（`u8`, `bool`, `Option`, `Result`……）
- 迭代器 trait
- `core::fmt`（格式化）
- `core::panic`（panic 机制）
- `core::arch`（内联汇编）

```rust
// std 中有，core 中没有：
use std::vec::Vec;       // ❌ 需要堆分配，core 没有
use std::string::String; // ❌ 需要堆分配，core 没有
use std::fs::File;       // ❌ 需要 OS，core 没有

// core 中有：
use core::fmt::Write;    // ✅ 格式化 trait
use core::panic::PanicInfo; // ✅ panic 信息
```

### 2.2 `no_main` 与自定义入口

```rust
#![no_main]
```

正常的 Rust 程序入口是 `fn main()`，背后由运行时（`crt0` + Rust runtime）初始化后调用。
裸机环境没有这些运行时，我们需要完全控制入口点。

`#![no_main]` 禁用默认入口，我们在汇编中手动设置入口：

```asm
bl  kernel_main   // 汇编跳转到 Rust 函数
```

```rust
// #[no_mangle] 禁止名称混淆（name mangling）
// Rust 默认会把函数名编译成类似 _ZN6kernel11kernel_main17h... 的形式
// no_mangle 保证汇编中能直接用 "kernel_main" 找到它
#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // ...
}
```

**`extern "C"`** 指定使用 C 调用约定（ABI），确保汇编和 Rust 之间参数传递方式一致。

**`-> !`** 表示这个函数永不返回（发散类型），因为内核没有"退出"这个概念。

### 2.3 裸指针与 `unsafe`

```rust
fn uart_reg(offset: usize) -> *mut u32 {
    (UART_BASE + offset) as *mut u32
}
```

这里把一个整数地址转换成裸指针（raw pointer）。裸指针有两种：
- `*const T`：只读裸指针
- `*mut T`：可读写裸指针

**为什么需要 `unsafe`？**
Rust 的核心保证是内存安全，而裸指针可以：
- 指向任意地址（包括不存在的地址）
- 在多线程中不受保护地访问
- 造成悬空指针（dangling pointer）

所以访问裸指针必须在 `unsafe` 块中，这是程序员对编译器说："我知道我在做什么，这是安全的"：

```rust
pub fn putc(c: u8) {
    unsafe {
        // 我们知道 0x09000000 是 UART 寄存器的真实地址
        // QEMU 保证这个地址可以安全读写
        while uart_reg(UART_FR).read_volatile() & UART_FR_TXFF != 0 {}
        uart_reg(UART_DR).write_volatile(c as u32);
    }
}
```

**`read_volatile()` / `write_volatile()`**：
告诉编译器"这个读写有副作用，不要优化掉"。如果用普通赋值，编译器可能认为"写了但没读，优化掉"——对 MMIO 寄存器来说是致命错误。

### 2.4 内联汇编 `core::arch::asm!`

```rust
// 在 Rust 代码中嵌入汇编指令
unsafe {
    core::arch::asm!("wfi");  // Wait For Interrupt — 让 CPU 休眠
}
```

`asm!` 宏语法：
```rust
// 基本用法
asm!("指令");

// 带输入/输出操作数
asm!(
    "mov {0}, {1}",   // {0} 是输出，{1} 是输入
    out(reg) result,  // 输出：将寄存器结果写入 result 变量
    in(reg) value,    // 输入：将 value 变量放入寄存器
);

// 多条指令
asm!(
    "mrs {0}, currentel",
    "lsr {0}, {0}, #2",
    out(reg) el,
);
```

### 2.5 全局汇编 `global_asm!`

```rust
// 在 arch/aarch64/mod.rs
use core::arch::global_asm;
global_asm!(include_str!("boot.S"));
```

`global_asm!` 将整个汇编文件嵌入到当前编译单元，等同于将 `.S` 文件直接链接进来。
`include_str!` 在编译时读取文件内容，返回 `&'static str`。

### 2.6 `core::fmt::Write` trait

```rust
pub struct UartWriter;

impl core::fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        puts(s);
        Ok(())
    }
}
```

这是 Rust 格式化输出的核心 trait。只需实现 `write_str`，就能自动获得格式化能力：

```rust
use core::fmt::Write;
let mut writer = UartWriter;
write!(writer, "x = {}", 42).unwrap();  // 自动调用 write_str 多次
```

`println!` 宏就是基于这个实现的：

```rust
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        let _ = write!($crate::uart::UartWriter, "{}\n", format_args!($($arg)*));
    });
}
```

**`format_args!`**：延迟格式化，不分配堆内存，直接生成格式化参数包。

### 2.7 `#[macro_export]` 宏

```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        // ...
    });
}
```

`macro_rules!` 定义声明式宏（Declarative Macros）：
- `$($arg:tt)*`：匹配零个或多个任意 token（`tt` = token tree）
- `$crate`：引用当前 crate，避免命名冲突

`#[macro_export]` 将宏导出到 crate 根，使其他模块可以 `use crate::println!` 使用。

### 2.8 `#[panic_handler]`

```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n[KERNEL PANIC]");
    // ...
    loop { unsafe { core::arch::asm!("wfi"); } }
}
```

`no_std` 环境下必须提供一个 panic handler。当代码中出现：
- `panic!("msg")`
- 数组越界
- `unwrap()` on `None`/`Err`
- 整数溢出（debug 模式）

…编译器会调用这个函数。
内核的 panic handler 不能退出，只能打印错误信息后停机。

### 2.9 Cargo workspace

```toml
# 根 Cargo.toml
[workspace]
members = ["kernel", "user"]
resolver = "2"

[profile.dev]
panic = "abort"    # panic 时直接终止，不展开栈（节省代码体积）
```

**Workspace** 让多个相关 crate 共享：
- 同一个 `target/` 目录（避免重复编译依赖）
- 同一个 `Cargo.lock`（保证版本一致）
- 根级 `[profile]` 配置

**`panic = "abort"`**：
- `unwind`（默认）：panic 时逐层清理栈帧，需要展开表（unwind tables）
- `abort`：panic 时直接终止，代码更小，适合内核

---

## 3. OS 开发知识

### 3.1 AArch64 启动流程

QEMU 使用 `-kernel` 参数加载 ELF 文件时，流程如下：

```
QEMU 固件（简化版）
  → 解析 ELF，将各段加载到指定物理地址
  → 设置 CPU 初始状态（PC = ELF Entry Point）
  → 释放 CPU 运行

CPU 开始执行（从 _start）
  → 此时处于 EL1（特权级 1）或 EL2（超管级）
  → 寄存器状态未初始化（除少数约定外）
  → 没有栈，不能调用函数
  → 没有 BSS 清零，全局变量值不可信
```

**为什么要过滤 CPU 0？**

多核系统（SMP）中，所有 CPU 核心会**同时**从 `_start` 开始执行。如果不加过滤，多个核心会同时初始化同一片内存，造成竞争条件（Race Condition）。

```asm
mrs  x0, mpidr_el1   // 读多处理器关联寄存器
and  x0, x0, #0xFF  // 取最低字节 = CPU 编号（Affinity Level 0）
cbnz x0, .park       // 非 0 号 CPU 跳转到 park（死循环）
```

`mpidr_el1`（Multiprocessor Affinity Register）在 AArch64 中标识每个核心的编号。

**为什么 CPU 1/2/3 需要 `wfe` 而不是普通死循环？**

```asm
.park:
    wfe        // Wait For Event — CPU 进入低功耗状态
    b .park    // 收到事件后继续等待
```

`wfe` 让 CPU 暂停，直到收到 SEV（Send Event）指令，此时功耗极低。后期实现 SMP 时，CPU 0 会用 `sev` 唤醒其他核心并分配任务。

### 3.2 PL011 UART 原理

**UART**（Universal Asynchronous Receiver/Transmitter）是最简单的串行通信接口。
**PL011** 是 ARM 设计的 UART IP 核，被广泛用于 ARM 开发板和 QEMU。

UART 通信原理：
```
发送方：将字节转换为时序信号，按位串行发出
接收方：按时序采样，还原字节
波特率：每秒传输的符号数（如 115200 baud = 每秒约 11520 字节）
```

**PL011 关键寄存器：**

| 偏移 | 名称 | 功能 |
|------|------|------|
| 0x00 | DR   | Data Register — 读/写数据 |
| 0x18 | FR   | Flag Register — 状态标志 |
| 0x24 | IBRD | 整数波特率除数 |
| 0x28 | FBRD | 小数波特率除数 |
| 0x2C | LCR_H | 线控制寄存器 |
| 0x30 | CR   | 控制寄存器 |

**FR（Flag Register）关键位：**
```
bit 3: RXFE — 接收 FIFO 为空
bit 4: TXFF — 发送 FIFO 已满 ← 我们用这个判断能否写入
bit 5: RXFF — 接收 FIFO 已满
bit 7: TXFE — 发送 FIFO 为空
```

**发送流程：**
```
while FR.TXFF == 1 { 等待 }  // FIFO 满了就等
DR = 字节值                   // 写入数据寄存器，硬件自动发送
```

**为什么 QEMU 不需要初始化波特率？**
QEMU 的 PL011 模拟器默认已配置好，不需要手动设置。真实硬件需要根据时钟频率计算并设置 IBRD/FBRD。

### 3.3 MMIO（内存映射 I/O）

普通 RAM 和硬件寄存器都通过内存地址访问，这就是 MMIO：

```
物理地址空间（QEMU virt machine）:
0x0000_0000 - 0x0800_0000  设备区域
  0x0900_0000              PL011 UART
  0x0800_0000              GIC 中断控制器
0x4000_0000 - 0x8000_0000  RAM（128MB）
  0x4008_0000              内核加载位置
```

**CPU 读写内存地址时：**
- 地址在 RAM 范围 → 访问实际内存
- 地址在设备范围 → 总线将访问转发到对应硬件

**为什么要用 `write_volatile`？**

编译器优化可能会：
1. 重排指令顺序
2. 删除"无效果"的写操作（写了但没读）
3. 合并多次读/写

对 MMIO 寄存器，每次读写都有硬件副作用，顺序和次数都不能改变。`volatile` 告诉编译器："每次都必须真实访问内存"。

### 3.4 链接器脚本

链接器脚本（`.ld` 文件）控制程序在内存中的布局。

```ld
OUTPUT_ARCH(aarch64)    /* 目标架构 */
ENTRY(_start)           /* 入口符号（ELF Entry Point） */

SECTIONS {
    . = 0x40080000;     /* 当前地址计数器，从这里开始放置内容 */

    .text : {
        KEEP(*(.text.boot))   /* 启动代码放最前面，KEEP 防止被 GC 删除 */
        *(.text .text.*)      /* 所有目标文件的 .text 段 */
    }

    . = ALIGN(4096);    /* 对齐到 4KB 页边界 */
    .rodata : { ... }   /* 只读数据段 */

    .bss (NOLOAD) : {   /* NOLOAD = 不占用文件空间（只记录大小） */
        __bss_start = .; /* 导出符号，供汇编使用 */
        *(.bss .bss.*)
        __bss_end = .;
    }
}
```

**为什么内核加载到 `0x4008_0000`？**
这是 QEMU virt machine 的约定：`-kernel` 参数加载的映像默认放在这个地址（实际上 QEMU 解析 ELF，按段放置，入口点在 ELF header 中指定）。

**段（Section）的概念：**

| 段 | 内容 | 属性 |
|----|------|------|
| `.text` | 机器码（指令） | 可读、可执行 |
| `.rodata` | 字符串常量、只读数据 | 只读 |
| `.data` | 有初始值的全局变量 | 可读写 |
| `.bss` | 无初始值的全局变量 | 可读写（运行时清零） |

### 3.5 BSS 段清零

**为什么需要手动清零 BSS？**

C/Rust 标准规定：全局变量和静态变量默认为零：
```rust
static COUNTER: u32 = 0;  // 保证为 0
```

但 `.bss` 段在 ELF 文件中**不存储实际内容**（因为全是零，不占空间）。操作系统加载 ELF 时负责清零，但现在没有操作系统，所以我们自己清：

```asm
adr  x0, __bss_start  // x0 = BSS 起始地址（链接器脚本导出的符号）
adr  x1, __bss_end    // x1 = BSS 结束地址
.clear_bss:
    cmp  x0, x1
    b.ge .bss_done     // x0 >= x1，清零完毕
    str  xzr, [x0], #8  // 将 xzr（零寄存器）写入 [x0]，x0 += 8
    b    .clear_bss
```

`xzr`：AArch64 的零寄存器，读取永远是 0，写入被丢弃。

### 3.6 build.rs 构建脚本

```rust
// kernel/build.rs
fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let ld_script = PathBuf::from(&manifest_dir).join("kernel.ld");

    println!("cargo:rustc-link-arg=-T{}", ld_script.display());
    println!("cargo:rustc-link-arg=-no-pie");
    println!("cargo:rerun-if-changed=kernel.ld");
}
```

`build.rs` 在编译主代码之前运行，通过标准输出向 cargo 传递指令：

| 指令 | 含义 |
|------|------|
| `cargo:rustc-link-arg=X` | 向链接器传递参数 X |
| `cargo:rerun-if-changed=F` | 文件 F 变化时重新运行 build.rs |
| `cargo:rustc-cfg=feature` | 设置条件编译 feature |

**为什么不直接在 `.cargo/config.toml` 里配置链接器参数？**
`.cargo/config.toml` 中的路径相对于 cargo 执行目录（workspace 根），但链接脚本在 kernel 子目录里。`build.rs` 可以用 `CARGO_MANIFEST_DIR` 获取 package 的绝对路径，解决路径问题。

---

## 4. 关键代码逐行解析

### 4.1 boot.S

```asm
.section ".text.boot"   // 放入 .text.boot 段（链接器脚本里排第一位）
.global _start          // 导出符号，使链接器能找到入口点

_start:
    // ── 多核过滤 ──────────────────────────────────────────
    mrs     x0, mpidr_el1   // 读取 CPU 关联寄存器（Multi-Processor ID）
    and     x0, x0, #0xFF   // 取 Aff0 字段（CPU 在集群中的编号）
    cbnz    x0, .park        // Compare and Branch if Not Zero
                             // 非 0 号 CPU 跳到 .park

    // ── 关闭中断 ──────────────────────────────────────────
    msr     daifset, #0xF   // DAIF = Debug/Abort/IRQ/FIQ 掩码
                             // 全部置 1 = 屏蔽所有中断和异常
                             // 防止启动期间的意外中断

    // ── 初始化栈指针 ──────────────────────────────────────
    adr     x0, _start      // x0 = _start 的运行时地址（PC 相对）
    mov     sp, x0           // 栈向低地址增长，从 _start 开始
                             // _start 之前是 0x4008_0000，有足够空间

    // ── 清零 BSS ──────────────────────────────────────────
    adr     x0, __bss_start  // 链接器脚本导出的符号
    adr     x1, __bss_end
.clear_bss:
    cmp     x0, x1
    b.ge    .bss_done        // Branch if Greater or Equal（地址比较）
    str     xzr, [x0], #8   // Store Zero Register，post-increment by 8
    b       .clear_bss
.bss_done:

    // ── 跳入 Rust ─────────────────────────────────────────
    bl      kernel_main      // Branch and Link（保存返回地址到 lr）
                             // 理论上不应返回

    // ── 停机 ──────────────────────────────────────────────
.park:
    wfe                      // Wait For Event（低功耗休眠）
    b       .park
```

**AArch64 寄存器约定：**
- `x0-x7`：函数参数/返回值（caller-saved）
- `x8`：系统调用号
- `x19-x28`：callee-saved（被调用者负责保存）
- `x29`：frame pointer（FP）
- `x30`：link register（LR），`bl` 指令自动保存返回地址
- `xzr`：零寄存器（Zero Register）
- `sp`：栈指针（Stack Pointer）
- `pc`：程序计数器（不能直接访问）

### 4.2 uart.rs

```rust
const UART_BASE: usize = 0x0900_0000;  // QEMU virt machine PL011 基址
const UART_DR: usize = 0x00;           // Data Register offset
const UART_FR: usize = 0x18;           // Flag Register offset
const UART_FR_TXFF: u32 = 1 << 5;     // bit 5 = TX FIFO Full

// 根据偏移量构造寄存器地址，返回裸指针
fn uart_reg(offset: usize) -> *mut u32 {
    (UART_BASE + offset) as *mut u32
    //         ^^^^^^^^ 加偏移量
    //                   ^^^^^^^^^^^^^^ 转换为裸指针
}

pub fn putc(c: u8) {
    unsafe {
        // 轮询等待：TX FIFO 不满才能写入
        while uart_reg(UART_FR).read_volatile() & UART_FR_TXFF != 0 {}
        //    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
        //    读 Flag Register，检查 bit 5（TXFF）是否为 1

        uart_reg(UART_DR).write_volatile(c as u32);
        //                ^^^^^^^^^^^^^^ 必须用 volatile，禁止编译器优化
    }
}

pub fn puts(s: &str) {
    for c in s.bytes() {    // 遍历字节（UTF-8）
        if c == b'\n' {
            putc(b'\r');    // \n → \r\n（终端需要回车+换行）
        }
        putc(c);
    }
}
```

### 4.3 main.rs

```rust
#![no_std]   // 不链接标准库
#![no_main]  // 不使用默认的 main 入口

mod arch;    // 引入 arch 模块（包含 boot.S）
mod uart;    // 引入 uart 模块

use core::panic::PanicInfo;

// #[no_mangle] 保持函数名不被混淆，汇编中 bl kernel_main 才能找到
// extern "C" 使用 C 调用约定
// -> ! 永不返回类型
#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    println!("Hello, Red OS!");
    println!("Architecture: AArch64");
    println!("Kernel loaded and running.");

    loop {
        unsafe {
            core::arch::asm!("wfi");  // 低功耗等待中断
        }
    }
}

// no_std 必须提供的 panic 处理函数
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n[KERNEL PANIC]");
    if let Some(loc) = info.location() {
        println!("  at {}:{}", loc.file(), loc.line());
    }
    if let Some(msg) = info.message().as_str() {
        println!("  {}", msg);
    }
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}
```

---

## 5. 延伸阅读

### Rust 相关

- [The Rust Reference — inline assembly](https://doc.rust-lang.org/reference/inline-assembly.html) — `asm!` 完整语法
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/) — unsafe Rust 深度指南
- [Rust Embedded Book](https://docs.rust-embedded.org/book/) — 嵌入式 Rust 开发指南
- [Writing an OS in Rust (phil-opp)](https://os.phil-opp.com/) — x86_64 但 Rust OS 开发最佳参考

### AArch64 架构

- [ARM Architecture Reference Manual (AArch64)](https://developer.arm.com/documentation/ddi0487/latest) — ARM 官方手册（权威但庞大）
- [AArch64 Instruction Set Reference](https://developer.arm.com/documentation/ddi0596/latest) — 指令参考
- [MPIDR_EL1 寄存器说明](https://developer.arm.com/documentation/ddi0595/2021-06/AArch64-Registers/MPIDR-EL1--Multiprocessor-Affinity-Register)

### PL011 UART

- [PL011 Technical Reference Manual](https://developer.arm.com/documentation/ddi0183/latest) — ARM 官方文档
- [PL011 驱动实战](https://krinkinmu.github.io/2020/11/29/PL011.html)

### QEMU virt machine

- [QEMU AArch64 virt machine 文档](https://qemu-project.gitlab.io/qemu/system/arm/virt.html)
- [QEMU 设备树（DTB）说明](https://qemu-project.gitlab.io/qemu/system/device-tree.html)

### 链接器脚本

- [GNU LD Linker Script Guide](https://sourceware.org/binutils/docs/ld/Scripts.html) — 完整参考
- [OSDev Wiki — Linker Scripts](https://wiki.osdev.org/Linker_Scripts) — 面向 OS 开发的实用指南

### OS 开发综合

- [OSDev Wiki](https://wiki.osdev.org/) — OS 开发百科全书
- [rCore-Tutorial Book](https://rcore-os.cn/rCore-Tutorial-Book-v3/) — 中文，从零构建 RISC-V OS（Rust）
- [Writing a Simple OS from Scratch](https://www.cs.bham.ac.uk/~exr/lectures/opsys/10_11/lectures/os-dev.pdf)

---

*下一阶段：[Phase 1 — AArch64 完整启动序列与 MMU 开启](phase1.md)*
