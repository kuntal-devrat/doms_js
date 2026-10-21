CXX ?= g++
CXXFLAGS ?= -std=c++20 -Wall -Wextra -O2
INCLUDES = -I../../include
LDFLAGS = -L../../target/release -ldoms_ffi -Wl,-rpath,'$$ORIGIN/../../target/release'

TARGET = doms_cpp_example

all: $(TARGET)

$(TARGET): main.cpp ../../include/doms.hpp ../../include/doms.h
	$(CXX) $(CXXFLAGS) $(INCLUDES) main.cpp $(LDFLAGS) -o $(TARGET)

run: $(TARGET)
	./$(TARGET)

clean:
	rm -f $(TARGET)

.PHONY: all run clean
