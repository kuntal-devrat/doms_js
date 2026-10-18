// Dense array allocation, indexing, and accumulator summation
let arr = [];
let i = 0;
while (i < 20000) {
    arr.push(i * 3 + 1);
    i = i + 1;
}

let sum = 0;
let j = 0;
while (j < arr.length) {
    sum = sum + arr[j];
    j = j + 1;
}
sum;
