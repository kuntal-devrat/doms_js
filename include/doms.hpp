/*
 * DOMS JavaScript Engine — Modern C++20 Embedding Wrapper
 *
 * Copyright (c) 2026 DOMS Contributors
 * Licensed under MIT OR Apache-2.0
 */

#ifndef DOMS_HPP
#define DOMS_HPP

#include "doms.h"

#include <concepts>
#include <exception>
#include <functional>
#include <memory>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace doms {

// -----------------------------------------------------------------------------
// Exceptions
// -----------------------------------------------------------------------------

class Exception : public std::runtime_error {
public:
    explicit Exception(const std::string& message, doms_status_t status = DOMS_ERROR_RUNTIME)
        : std::runtime_error(message), status_(status) {}

    [[nodiscard]] doms_status_t status() const noexcept {
        return status_;
    }

private:
    doms_status_t status_;
};

class CompileError : public Exception {
public:
    explicit CompileError(const std::string& message)
        : Exception(message, DOMS_ERROR_COMPILE) {}
};

class RuntimeError : public Exception {
public:
    explicit RuntimeError(const std::string& message)
        : Exception(message, DOMS_ERROR_RUNTIME) {}
};

class TypeError : public Exception {
public:
    explicit TypeError(const std::string& message)
        : Exception(message, DOMS_ERROR_TYPE) {}
};

inline void check_status(doms_status_t status, doms_context_t* ctx = nullptr) {
    if (status == DOMS_OK) return;

    std::string err_msg;
    if (ctx) {
        const char* msg = doms_get_error_message(ctx);
        if (msg) err_msg = msg;
    }
    if (err_msg.empty()) {
        switch (status) {
            case DOMS_ERROR_COMPILE: err_msg = "DOMS compilation error"; break;
            case DOMS_ERROR_RUNTIME: err_msg = "DOMS runtime execution error"; break;
            case DOMS_ERROR_OOM: err_msg = "DOMS out of memory error"; break;
            case DOMS_ERROR_TYPE: err_msg = "DOMS type error"; break;
            case DOMS_ERROR_INVALID_ARG: err_msg = "DOMS invalid argument error"; break;
            default: err_msg = "DOMS unknown error"; break;
        }
    }

    switch (status) {
        case DOMS_ERROR_COMPILE: throw CompileError(err_msg);
        case DOMS_ERROR_TYPE: throw TypeError(err_msg);
        default: throw RuntimeError(err_msg);
    }
}

// Forward declarations
class Context;
class Value;

// -----------------------------------------------------------------------------
// Value: RAII Managed JavaScript Value Handle
// -----------------------------------------------------------------------------

class Value {
public:
    Value() noexcept : raw_(doms_value_new_undefined()) {}

    explicit Value(doms_value_t* raw, bool take_ownership = true) noexcept : raw_(raw) {
        if (!take_ownership && raw_) {
            doms_value_ref(raw_);
        }
    }

    ~Value() noexcept {
        if (raw_) {
            doms_value_unref(raw_);
            raw_ = nullptr;
        }
    }

    Value(const Value& other) noexcept : raw_(other.raw_) {
        if (raw_) {
            doms_value_ref(raw_);
        }
    }

    Value& operator=(const Value& other) noexcept {
        if (this != &other) {
            if (raw_) doms_value_unref(raw_);
            raw_ = other.raw_;
            if (raw_) doms_value_ref(raw_);
        }
        return *this;
    }

    Value(Value&& other) noexcept : raw_(other.raw_) {
        other.raw_ = nullptr;
    }

    Value& operator=(Value&& other) noexcept {
        if (this != &other) {
            if (raw_) doms_value_unref(raw_);
            raw_ = other.raw_;
            other.raw_ = nullptr;
        }
        return *this;
    }

    [[nodiscard]] doms_value_t* raw() const noexcept {
        return raw_;
    }

    // Predicates
    [[nodiscard]] bool is_null() const noexcept {
        return raw_ && doms_value_is_null(raw_) != 0;
    }

    [[nodiscard]] bool is_undefined() const noexcept {
        return !raw_ || doms_value_is_undefined(raw_) != 0;
    }

    [[nodiscard]] bool is_boolean() const noexcept {
        return raw_ && doms_value_is_boolean(raw_) != 0;
    }

    [[nodiscard]] bool is_number() const noexcept {
        return raw_ && doms_value_is_number(raw_) != 0;
    }

    [[nodiscard]] bool is_string() const noexcept {
        return raw_ && doms_value_is_string(raw_) != 0;
    }

    [[nodiscard]] bool is_object() const noexcept {
        return raw_ && doms_value_is_object(raw_) != 0;
    }

    [[nodiscard]] bool is_function() const noexcept {
        return raw_ && doms_value_is_function(raw_) != 0;
    }

    // Conversions
    [[nodiscard]] double to_number(doms_context_t* ctx = nullptr) const {
        if (!raw_) throw TypeError("Null value handle");
        double out = 0.0;
        doms_status_t st = doms_value_to_number(ctx, raw_, &out);
        check_status(st, ctx);
        return out;
    }

    [[nodiscard]] std::string to_string(doms_context_t* ctx = nullptr) const {
        if (!raw_) return "undefined";
        const char* out = nullptr;
        size_t len = 0;
        doms_status_t st = doms_value_to_string(ctx, raw_, &out, &len);
        check_status(st, ctx);
        std::string res(out ? out : "", len);
        if (out) doms_free_string(ctx, out);
        return res;
    }

    // Static constructors
    static Value make_undefined() noexcept {
        return Value(doms_value_new_undefined());
    }

    static Value make_null() noexcept {
        return Value(doms_value_new_null());
    }

    static Value make_boolean(bool b) noexcept {
        return Value(doms_value_new_boolean(b ? 1 : 0));
    }

    static Value make_number(double n) noexcept {
        return Value(doms_value_new_number(n));
    }

    static Value make_string(doms_context_t* ctx, std::string_view str) {
        doms_value_t* val = nullptr;
        doms_status_t st = doms_value_new_string(ctx, str.data(), str.size(), &val);
        check_status(st, ctx);
        return Value(val);
    }

    static Value make_object(doms_context_t* ctx) {
        doms_value_t* val = nullptr;
        doms_status_t st = doms_object_new(ctx, &val);
        check_status(st, ctx);
        return Value(val);
    }

    static Value make_array(doms_context_t* ctx) {
        doms_value_t* val = nullptr;
        doms_status_t st = doms_array_new(ctx, &val);
        check_status(st, ctx);
        return Value(val);
    }

    // Object property operations
    [[nodiscard]] Value get(doms_context_t* ctx, std::string_view key) const {
        doms_value_t* res = nullptr;
        doms_status_t st = doms_object_get(ctx, raw_, key.data(), key.size(), &res);
        check_status(st, ctx);
        return Value(res);
    }

    void set(doms_context_t* ctx, std::string_view key, const Value& val) const {
        doms_status_t st = doms_object_set(ctx, raw_, key.data(), key.size(), val.raw());
        check_status(st, ctx);
    }

    // Array operations
    void push(doms_context_t* ctx, const Value& val) const {
        doms_status_t st = doms_array_push(ctx, raw_, val.raw());
        check_status(st, ctx);
    }

    [[nodiscard]] size_t length(doms_context_t* ctx) const {
        size_t len = 0;
        doms_status_t st = doms_array_length(ctx, raw_, &len);
        check_status(st, ctx);
        return len;
    }

    [[nodiscard]] Value get_at(doms_context_t* ctx, size_t index) const {
        doms_value_t* res = nullptr;
        doms_status_t st = doms_array_get_at(ctx, raw_, index, &res);
        check_status(st, ctx);
        return Value(res);
    }

    // Function invocation
    [[nodiscard]] Value call(doms_context_t* ctx, const Value& this_val, std::span<const Value> args) const {
        std::vector<doms_value_t*> raw_args;
        raw_args.reserve(args.size());
        for (const auto& a : args) {
            raw_args.push_back(a.raw());
        }
        doms_value_t* res = nullptr;
        doms_status_t st = doms_call(
            ctx,
            raw_,
            this_val.raw(),
            raw_args.size(),
            raw_args.empty() ? nullptr : raw_args.data(),
            &res
        );
        check_status(st, ctx);
        return Value(res);
    }

    [[nodiscard]] Value operator()(doms_context_t* ctx, std::span<const Value> args) const {
        return call(ctx, Value::make_undefined(), args);
    }

private:
    doms_value_t* raw_{nullptr};
};

// -----------------------------------------------------------------------------
// Context: Execution Context with Script Evaluation and C++ Callback Bindings
// -----------------------------------------------------------------------------

class Context {
public:
    using NativeCallback = std::function<Value(Context&, const Value& /* this */, std::span<const Value> /* args */)>;

    explicit Context(doms_context_t* ctx, bool owned = true) noexcept
        : ctx_(ctx), owned_(owned) {}

    ~Context() noexcept {
        if (owned_ && ctx_) {
            doms_context_destroy(ctx_);
            ctx_ = nullptr;
        }
    }

    Context(const Context&) = delete;
    Context& operator=(const Context&) = delete;

    Context(Context&& other) noexcept : ctx_(other.ctx_), owned_(other.owned_), callbacks_(std::move(other.callbacks_)) {
        other.ctx_ = nullptr;
        other.owned_ = false;
    }

    Context& operator=(Context&& other) noexcept {
        if (this != &other) {
            if (owned_ && ctx_) doms_context_destroy(ctx_);
            ctx_ = other.ctx_;
            owned_ = other.owned_;
            callbacks_ = std::move(other.callbacks_);
            other.ctx_ = nullptr;
            other.owned_ = false;
        }
        return *this;
    }

    [[nodiscard]] doms_context_t* raw() const noexcept {
        return ctx_;
    }

    // Script evaluation
    [[nodiscard]] Value eval(std::string_view source) const {
        doms_value_t* res = nullptr;
        doms_status_t st = doms_eval(ctx_, source.data(), source.size(), &res);
        check_status(st, ctx_);
        return Value(res);
    }

    // Global object
    [[nodiscard]] Value get_global() const {
        doms_value_t* res = nullptr;
        doms_status_t st = doms_get_global(ctx_, &res);
        check_status(st, ctx_);
        return Value(res);
    }

    void set_global(std::string_view name, const Value& val) const {
        doms_status_t st = doms_set_global(ctx_, name.data(), name.size(), val.raw());
        check_status(st, ctx_);
    }

    // Value construction helpers
    [[nodiscard]] Value new_string(std::string_view s) const {
        return Value::make_string(ctx_, s);
    }

    [[nodiscard]] Value new_number(double n) const noexcept {
        return Value::make_number(n);
    }

    [[nodiscard]] Value new_boolean(bool b) const noexcept {
        return Value::make_boolean(b);
    }

    [[nodiscard]] Value new_object() const {
        return Value::make_object(ctx_);
    }

    [[nodiscard]] Value new_array() const {
        return Value::make_array(ctx_);
    }

    // Native C++ function binding (arbitrary lambdas / std::function)
    void set_global_function(std::string_view name, NativeCallback fn) {
        auto cb_ptr = std::make_unique<NativeCallback>(std::move(fn));
        NativeCallback* raw_cb = cb_ptr.get();
        callbacks_.push_back(std::move(cb_ptr));

        auto c_thunk = [](doms_context_t* c, doms_value_t* this_val, size_t argc,
                          doms_value_t** argv, doms_value_t** result, void* userdata) -> doms_status_t {
            auto* target_fn = static_cast<NativeCallback*>(userdata);
            if (!target_fn) return DOMS_ERROR_INVALID_ARG;

            try {
                Context non_owning_ctx(c, false);
                Value this_v(this_val, false);
                std::vector<Value> args;
                args.reserve(argc);
                for (size_t i = 0; i < argc; ++i) {
                    args.emplace_back(argv[i], false);
                }

                Value ret = (*target_fn)(non_owning_ctx, this_v, std::span<const Value>(args.data(), args.size()));
                if (result) {
                    *result = ret.raw();
                    if (*result) {
                        doms_value_ref(*result);
                    }
                }
                return DOMS_OK;
            } catch (const Exception& ex) {
                return ex.status();
            } catch (...) {
                return DOMS_ERROR_RUNTIME;
            }
        };

        doms_status_t st = doms_set_global_function(
            ctx_,
            name.data(),
            name.size(),
            c_thunk,
            raw_cb
        );
        check_status(st, ctx_);
    }

    // Simplified lambda binding: [](Context&, std::span<const Value>) -> Value
    template <typename F>
        requires std::invocable<F, Context&, std::span<const Value>>
    void set_global_function(std::string_view name, F&& fn) {
        set_global_function(name, [f = std::forward<F>(fn)](Context& ctx, const Value&, std::span<const Value> args) mutable -> Value {
            return f(ctx, args);
        });
    }

private:
    doms_context_t* ctx_{nullptr};
    bool owned_{true};
    std::vector<std::unique_ptr<NativeCallback>> callbacks_;
};

// -----------------------------------------------------------------------------
// Isolate: Independent JS Heap and World
// -----------------------------------------------------------------------------

class Isolate {
public:
    explicit Isolate(doms_isolate_t* isolate, bool owned = true) noexcept
        : isolate_(isolate), owned_(owned) {}

    ~Isolate() noexcept {
        if (owned_ && isolate_) {
            doms_isolate_destroy(isolate_);
            isolate_ = nullptr;
        }
    }

    Isolate(const Isolate&) = delete;
    Isolate& operator=(const Isolate&) = delete;

    Isolate(Isolate&& other) noexcept : isolate_(other.isolate_), owned_(other.owned_) {
        other.isolate_ = nullptr;
        other.owned_ = false;
    }

    Isolate& operator=(Isolate&& other) noexcept {
        if (this != &other) {
            if (owned_ && isolate_) doms_isolate_destroy(isolate_);
            isolate_ = other.isolate_;
            owned_ = other.owned_;
            other.isolate_ = nullptr;
            other.owned_ = false;
        }
        return *this;
    }

    [[nodiscard]] doms_isolate_t* raw() const noexcept {
        return isolate_;
    }

    [[nodiscard]] Context create_context() const {
        doms_context_t* ctx = doms_context_new(isolate_);
        if (!ctx) throw RuntimeError("Failed to create DOMS context");
        return Context(ctx, true);
    }

    void post_message(std::span<const uint8_t> data) const {
        doms_status_t st = doms_isolate_post_message(isolate_, data.data(), data.size());
        check_status(st);
    }

private:
    doms_isolate_t* isolate_{nullptr};
    bool owned_{true};
};

// -----------------------------------------------------------------------------
// Engine: Process-Global Runtime Host
// -----------------------------------------------------------------------------

class Engine {
public:
    Engine() {
        engine_ = doms_engine_new();
        if (!engine_) throw RuntimeError("Failed to initialize DOMS engine");
    }

    ~Engine() noexcept {
        if (engine_) {
            doms_engine_destroy(engine_);
            engine_ = nullptr;
        }
    }

    Engine(const Engine&) = delete;
    Engine& operator=(const Engine&) = delete;

    Engine(Engine&& other) noexcept : engine_(other.engine_) {
        other.engine_ = nullptr;
    }

    Engine& operator=(Engine&& other) noexcept {
        if (this != &other) {
            if (engine_) doms_engine_destroy(engine_);
            engine_ = other.engine_;
            other.engine_ = nullptr;
        }
        return *this;
    }

    [[nodiscard]] doms_engine_t* raw() const noexcept {
        return engine_;
    }

    [[nodiscard]] static uint32_t version() noexcept {
        return doms_version();
    }

    [[nodiscard]] Isolate create_isolate() const {
        doms_isolate_t* iso = doms_isolate_new(engine_);
        if (!iso) throw RuntimeError("Failed to create DOMS isolate");
        return Isolate(iso, true);
    }

    // Convenience: create isolate and context in one step
    [[nodiscard]] std::pair<Isolate, Context> create_runtime() const {
        Isolate iso = create_isolate();
        Context ctx = iso.create_context();
        return {std::move(iso), std::move(ctx)};
    }

private:
    doms_engine_t* engine_{nullptr};
};

} // namespace doms

#endif // DOMS_HPP
