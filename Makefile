.PHONY: build clean

build:
	cargo build --release
	elf2uf2-rs target/thumbv6m-none-eabi/release/rp2040_temp_fan_ctrl rp2040_fan_ctrl.uf2

clean:
	rm -f rp2040_fan_ctrl.uf2
	rm -rf target
