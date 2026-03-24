KERNEL_ELF := target/aarch64-unknown-none-softfloat/debug/kernel
KERNEL_ELF_RELEASE := target/aarch64-unknown-none-softfloat/release/kernel

QEMU := qemu-system-aarch64
QEMU_ARGS := \
	-machine virt \
	-cpu cortex-a57 \
	-nographic \
	-m 128M

.PHONY: build run debug clean

## Build the kernel (debug mode)
build:
	cd kernel && cargo build

## Build the kernel (release mode)
build-release:
	cd kernel && cargo build --release

## Run the kernel in QEMU
run: build
	$(QEMU) $(QEMU_ARGS) -kernel $(KERNEL_ELF)

## Run release build in QEMU
run-release: build-release
	$(QEMU) $(QEMU_ARGS) -kernel $(KERNEL_ELF_RELEASE)

## Start QEMU and wait for GDB on port 1234
debug: build
	$(QEMU) $(QEMU_ARGS) -kernel $(KERNEL_ELF) -s -S

## Clean all build artifacts
clean:
	cargo clean
