// Sieve of Eratosthenes prime generation benchmark
let max = 20000;
let isPrime = [];
let i = 0;
while (i <= max) {
    isPrime.push(true);
    i = i + 1;
}

isPrime[0] = false;
isPrime[1] = false;

let p = 2;
while (p * p <= max) {
    if (isPrime[p]) {
        let m = p * p;
        while (m <= max) {
            isPrime[m] = false;
            m = m + p;
        }
    }
    p = p + 1;
}

let count = 0;
let k = 0;
while (k <= max) {
    if (isPrime[k]) {
        count = count + 1;
    }
    k = k + 1;
}
count;
