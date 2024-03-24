LOCAL_DIR := $(GET_LOCAL_DIR)

librust_app:
	cd $(LK_TOP_DIR)/app/rust_test && cargo build --target armv7a-none-eabi
	cp app/rust_test/target/armv7a-none-eabi/debug/librust_app.a $(BUILDDIR)/librust_app.a

#OBJS += librust_app.a
