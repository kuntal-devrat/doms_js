// Control flow and loop benchmark
let sum = 0;
let i = 0;
while (i < 500000) {
    if (i % 2 === 0) {
        sum = sum + i;
    } else {
        sum = sum - (i / 2);
    }
    i = i + 1;
}
sum;
