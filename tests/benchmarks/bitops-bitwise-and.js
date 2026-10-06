// SunSpider subset: bitops-bitwise-and
let bitwiseAndValue = 4294967295;
let i = 0;
while (i < 100000) {
    bitwiseAndValue = (bitwiseAndValue & i) | (i & 255);
    i = i + 1;
}
bitwiseAndValue;
