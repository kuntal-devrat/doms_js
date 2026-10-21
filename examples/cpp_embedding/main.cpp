#include "doms.hpp"

#include <cassert>
#include <cmath>
#include <iostream>
#include <string>

int main() {
    std::cout << "====================================================\n";
    std::cout << " DOMS C++20 Embedding Verification Suite\n";
    std::cout << "====================================================\n";

    // 1. Initialize Engine
    doms::Engine engine;
    uint32_t version = doms::Engine::version();
    std::cout << "[1] DOMS Engine initialized (ABI version: " << version << ")\n";
    assert(version >= 1);

    // 2. Create Isolate and Context
    auto [isolate, ctx] = engine.create_runtime();
    std::cout << "[2] Isolate and Context created successfully\n";

    // 3. Basic arithmetic evaluation
    {
        doms::Value res = ctx.eval("10 + 20 * 2");
        assert(res.is_number());
        double val = res.to_number(ctx.raw());
        std::cout << "[3] JS arithmetic eval: 10 + 20 * 2 = " << val << "\n";
        assert(std::fabs(val - 50.0) < 1e-6);
    }

    // 4. String evaluation & conversion
    {
        doms::Value res = ctx.eval("'Hello from ' + 'DOMS C++20!'");
        assert(res.is_string());
        std::string text = res.to_string(ctx.raw());
        std::cout << "[4] JS string eval: " << text << "\n";
        assert(text == "Hello from DOMS C++20!");
    }

    // 5. C++20 Lambda binding: register host function
    {
        ctx.set_global_function("cpp_multiply", [](doms::Context& c, std::span<const doms::Value> args) -> doms::Value {
            assert(args.size() >= 2);
            double a = args[0].to_number(c.raw());
            double b = args[1].to_number(c.raw());
            return c.new_number(a * b);
        });

        doms::Value res = ctx.eval("cpp_multiply(6, 7)");
        assert(res.is_number());
        double num = res.to_number(ctx.raw());
        std::cout << "[5] Host C++ lambda invoked from JS: cpp_multiply(6, 7) = " << num << "\n";
        assert(std::fabs(num - 42.0) < 1e-6);
    }

    // 6. Object creation and property manipulation
    {
        doms::Value obj = ctx.new_object();
        obj.set(ctx.raw(), "name", ctx.new_string("DOMS JS Engine"));
        obj.set(ctx.raw(), "speed", ctx.new_number(9000.0));
        ctx.set_global("config", obj);

        doms::Value summary = ctx.eval("config.name + ' speed level: ' + config.speed");
        std::string s = summary.to_string(ctx.raw());
        std::cout << "[6] Object property manipulation: " << s << "\n";
        assert(s.find("DOMS JS Engine") != std::string::npos);
        assert(s.find("9000") != std::string::npos);
    }

    // 7. Array creation and manipulation
    {
        doms::Value arr = ctx.new_array();
        arr.push(ctx.raw(), ctx.new_number(100));
        arr.push(ctx.raw(), ctx.new_number(200));
        arr.push(ctx.raw(), ctx.new_number(300));

        size_t len = arr.length(ctx.raw());
        assert(len == 3);
        ctx.set_global("items", arr);

        doms::Value sum = ctx.eval("items[0] + items[1] + items[2]");
        double sum_val = sum.to_number(ctx.raw());
        std::cout << "[7] Array length (" << len << ") sum: " << sum_val << "\n";
        assert(std::fabs(sum_val - 600.0) < 1e-6);
    }

    // 8. Exception handling verification
    {
        bool caught = false;
        try {
            (void)ctx.eval("throw new Error('C++ Catch Test!');");
        } catch (const doms::Exception& ex) {
            caught = true;
            std::cout << "[8] Successfully caught JS exception in C++: " << ex.what() << "\n";
        }
        assert(caught);
    }

    std::cout << "====================================================\n";
    std::cout << " ALL C++20 EMBEDDING TESTS PASSED (100% Zero Mocks)!\n";
    std::cout << "====================================================\n";
    return 0;
}
