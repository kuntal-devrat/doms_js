// Recursive Fibonacci benchmark (measures function call overhead and stack frames)
function fib(n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

let result = fib(28);
result;
