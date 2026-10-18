// Object creation and property access inline cache benchmark
function Point(x, y) {
    this.x = x;
    this.y = y;
}

let sum = 0;
let i = 0;
while (i < 50000) {
    let p = new Point(i, i * 2);
    sum = sum + p.x + p.y;
    i = i + 1;
}
sum;
