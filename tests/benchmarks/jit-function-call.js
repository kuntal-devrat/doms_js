// Function calling and JIT optimization benchmark
function compute(a, b) {
    return (a * 3 + b * 2) & 65535;
}

let acc = 0;
let i = 0;
while (i < 100000) {
    acc = compute(acc, i);
    i = i + 1;
}
acc;
