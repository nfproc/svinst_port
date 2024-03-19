BIN_NAME = svinst_port

.PHONY: release run clean

all: release

run:
	cargo run --release sample/sample.sv

clean:
	cargo clean

release:
	cargo build --release
