| #  | Triple (rustup target)            | LLVM arch   | 位宽    | 备注/典型场景                     |
| -- | --------------------------------- | ----------- | ----- | --------------------------- |
| 1  | `x86_64-unknown-linux-gnu`        | x86\_64     | 64    | 桌面/服务器绝对主流                  |
| 2  | `x86_64-unknown-linux-musl`       | x86\_64     | 64    | 静态链接，容器/Docker 最爱           |
| 3  | `x86_64-pc-windows-msvc`          | x86\_64     | 64    | Windows 官方工具链               |
| 4  | `x86_64-pc-windows-gnu`           | x86\_64     | 64    | Windows MinGW 工具链           |
| 5  | `x86_64-apple-darwin`             | x86\_64     | 64    | Intel Mac                   |
| 6  | `aarch64-apple-darwin`            | aarch64     | 64    | Apple Silicon Mac / iOS Sim |
| 7  | `aarch64-unknown-linux-gnu`       | aarch64     | 64    | ARM 服务器、树莓派 3/4、Android     |
| 8  | `aarch64-unknown-linux-musl`      | aarch64     | 64    | 静态链接 ARM64                  |
| 9  | `aarch64-linux-android`           | aarch64     | 64    | Android NDK 官方              |
| 10 | `armv7-linux-androideabi`         | arm         | 32    | Android 32 位                |
| 11 | `i686-unknown-linux-gnu`          | x86         | 32    | 老 x86 32 位                  |
| 12 | `i686-pc-windows-msvc`            | x86         | 32    | Windows 32 位                |
| 13 | `riscv64gc-unknown-linux-gnu`     | riscv64     | 64    | RISC-V 64 通用                |
| 14 | `riscv32gc-unknown-linux-gnu`     | riscv32     | 32    | RISC-V 32 通用                |
| 15 | `riscv64imac-unknown-none-elf`    | riscv64     | 64    | **仅 no\_std**，裸机            |
| 16 | `riscv32imac-unknown-none-elf`    | riscv32     | 32    | **仅 no\_std**，裸机            |
| 17 | `mips-unknown-linux-gnu`          | mips        | 32    | 路由/存量龙芯                     |
| 18 | `mipsel-unknown-linux-gnu`        | mips        | 32    | 小端 MIPS                     |
| 19 | `mips64-unknown-linux-gnuabi64`   | mips64      | 64    | 大端 64                       |
| 20 | `mips64el-unknown-linux-gnuabi64` | mips64      | 64    | 小端 64                       |
| 21 | `powerpc-unknown-linux-gnu`       | powerpc     | 32    | NXP T 系列                    |
| 22 | `powerpc64-unknown-linux-gnu`     | powerpc64   | 64    | 大端 POWER                    |
| 23 | `powerpc64le-unknown-linux-gnu`   | powerpc64   | 64    | 小端 POWER、OpenPOWER          |
| 24 | `s390x-unknown-linux-gnu`         | s390x       | 64    | IBM z 大型机                   |
| 25 | `sparc64-unknown-linux-gnu`       | sparc64     | 64    | Leon/Oracle SPARC           |
| 26 | `sparcv9-sun-solaris`             | sparc64     | 64    | Solaris                     |
| 27 | `i586-pc-solaris`                 | x86         | 32    | Solaris x86                 |
| 28 | `x86_64-sun-solaris`              | x86\_64     | 64    | Solaris x86\_64             |
| 29 | `armv5te-unknown-linux-gnueabi`   | arm         | 32    | 老 ARMv5                     |
| 30 | `armv7-unknown-linux-gnueabihf`   | arm         | 32    | 树莓派 2、BeagleBone            |
| 31 | `thumbv6m-none-eabi`              | arm         | 16/32 | **仅 no\_std** Cortex-M0     |
| 32 | `thumbv7m-none-eabi`              | arm         | 16/32 | **仅 no\_std** Cortex-M3     |
| 33 | `thumbv7em-none-eabi`             | arm         | 16/32 | **仅 no\_std** Cortex-M4/M7  |
| 34 | `thumbv8m.main-none-eabi`         | arm         | 16/32 | **仅 no\_std** Cortex-M33    |
| 35 | `aarch64-unknown-none`            | aarch64     | 64    | **仅 no\_std** 裸机 ARM64      |
| 36 | `x86_64-unknown-none`             | x86\_64     | 64    | **仅 no\_std** 裸机 x86\_64    |
| 37 | `wasm32-unknown-unknown`          | wasm32      | 32    | WebAssembly 无宿主             |
| 38 | `wasm32-wasi`                     | wasm32      | 32    | WebAssembly + WASI          |
| 39 | `wasm64-unknown-unknown`          | wasm64      | 64    | WebAssembly 64 位实验          |
| 40 | `avr-unknown-gnu-atmega328`       | avr         | 8     | **仅 no\_std** Arduino UNO   |
| 41 | `msp430-none-elf`                 | msp430      | 16    | **仅 no\_std** TI MSP430     |
| 42 | `xtensa-esp32-none-elf`           | xtensa      | 32    | **仅 no\_std** ESP32 经典      |
| 43 | `xtensa-esp32s2-none-elf`         | xtensa      | 32    | **仅 no\_std** ESP32-S2      |
| 44 | `xtensa-esp32s3-none-elf`         | xtensa      | 32    | **仅 no\_std** ESP32-S3      |
| 45 | `loongarch64-unknown-linux-gnu`   | loongarch64 | 64    | 龙芯 3A5000/3C5000（已合并 1.82）  |
