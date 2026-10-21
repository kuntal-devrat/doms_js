CC ?= gcc
CFLAGS ?= -Wall -Wextra -O2 -I../../crates/doms_ffi/include
LDFLAGS ?= -L../../target/release -ldoms_ffi -Wl,-rpath,../../target/release -lpthread -ldl -lm

TARGET = doms_c_demo

all: $(TARGET)

$(TARGET): main.c
	$(CC) $(CFLAGS) main.c -o $(TARGET) $(LDFLAGS)

run: $(TARGET)
	./$(TARGET)

clean:
	rm -f $(TARGET)

.PHONY: all run clean
