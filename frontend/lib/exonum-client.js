(function e(t,n,r){function s(o,u){if(!n[o]){if(!t[o]){var a=typeof require=="function"&&require;if(!u&&a)return a(o,!0);if(i)return i(o,!0);var f=new Error("Cannot find module '"+o+"'");throw f.code="MODULE_NOT_FOUND",f}var l=n[o]={exports:{}};t[o][0].call(l.exports,function(e){var n=t[o][1][e];return s(n?n:e)},l,l.exports,e,t,n,r)}return n[o].exports}var i=typeof require=="function"&&require;for(var o=0;o<r.length;o++)s(r[o]);return s})({1:[function(require,module,exports){
'use strict'

exports.byteLength = byteLength
exports.toByteArray = toByteArray
exports.fromByteArray = fromByteArray

var lookup = []
var revLookup = []
var Arr = typeof Uint8Array !== 'undefined' ? Uint8Array : Array

var code = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
for (var i = 0, len = code.length; i < len; ++i) {
  lookup[i] = code[i]
  revLookup[code.charCodeAt(i)] = i
}

revLookup['-'.charCodeAt(0)] = 62
revLookup['_'.charCodeAt(0)] = 63

function placeHoldersCount (b64) {
  var len = b64.length
  if (len % 4 > 0) {
    throw new Error('Invalid string. Length must be a multiple of 4')
  }

  // the number of equal signs (place holders)
  // if there are two placeholders, than the two characters before it
  // represent one byte
  // if there is only one, then the three characters before it represent 2 bytes
  // this is just a cheap hack to not do indexOf twice
  return b64[len - 2] === '=' ? 2 : b64[len - 1] === '=' ? 1 : 0
}

function byteLength (b64) {
  // base64 is 4/3 + up to two characters of the original data
  return b64.length * 3 / 4 - placeHoldersCount(b64)
}

function toByteArray (b64) {
  var i, j, l, tmp, placeHolders, arr
  var len = b64.length
  placeHolders = placeHoldersCount(b64)

  arr = new Arr(len * 3 / 4 - placeHolders)

  // if there are placeholders, only get up to the last complete 4 chars
  l = placeHolders > 0 ? len - 4 : len

  var L = 0

  for (i = 0, j = 0; i < l; i += 4, j += 3) {
    tmp = (revLookup[b64.charCodeAt(i)] << 18) | (revLookup[b64.charCodeAt(i + 1)] << 12) | (revLookup[b64.charCodeAt(i + 2)] << 6) | revLookup[b64.charCodeAt(i + 3)]
    arr[L++] = (tmp >> 16) & 0xFF
    arr[L++] = (tmp >> 8) & 0xFF
    arr[L++] = tmp & 0xFF
  }

  if (placeHolders === 2) {
    tmp = (revLookup[b64.charCodeAt(i)] << 2) | (revLookup[b64.charCodeAt(i + 1)] >> 4)
    arr[L++] = tmp & 0xFF
  } else if (placeHolders === 1) {
    tmp = (revLookup[b64.charCodeAt(i)] << 10) | (revLookup[b64.charCodeAt(i + 1)] << 4) | (revLookup[b64.charCodeAt(i + 2)] >> 2)
    arr[L++] = (tmp >> 8) & 0xFF
    arr[L++] = tmp & 0xFF
  }

  return arr
}

function tripletToBase64 (num) {
  return lookup[num >> 18 & 0x3F] + lookup[num >> 12 & 0x3F] + lookup[num >> 6 & 0x3F] + lookup[num & 0x3F]
}

function encodeChunk (uint8, start, end) {
  var tmp
  var output = []
  for (var i = start; i < end; i += 3) {
    tmp = (uint8[i] << 16) + (uint8[i + 1] << 8) + (uint8[i + 2])
    output.push(tripletToBase64(tmp))
  }
  return output.join('')
}

function fromByteArray (uint8) {
  var tmp
  var len = uint8.length
  var extraBytes = len % 3 // if we have 1 byte left, pad 2 bytes
  var output = ''
  var parts = []
  var maxChunkLength = 16383 // must be multiple of 3

  // go through the array every three bytes, we'll deal with trailing stuff later
  for (var i = 0, len2 = len - extraBytes; i < len2; i += maxChunkLength) {
    parts.push(encodeChunk(uint8, i, (i + maxChunkLength) > len2 ? len2 : (i + maxChunkLength)))
  }

  // pad the end with zeros, but make sure to not forget the extra bytes
  if (extraBytes === 1) {
    tmp = uint8[len - 1]
    output += lookup[tmp >> 2]
    output += lookup[(tmp << 4) & 0x3F]
    output += '=='
  } else if (extraBytes === 2) {
    tmp = (uint8[len - 2] << 8) + (uint8[len - 1])
    output += lookup[tmp >> 10]
    output += lookup[(tmp >> 4) & 0x3F]
    output += lookup[(tmp << 2) & 0x3F]
    output += '='
  }

  parts.push(output)

  return parts.join('')
}

},{}],2:[function(require,module,exports){
var bigInt = (function (undefined) {
    "use strict";

    var BASE = 1e7,
        LOG_BASE = 7,
        MAX_INT = 9007199254740992,
        MAX_INT_ARR = smallToArray(MAX_INT),
        LOG_MAX_INT = Math.log(MAX_INT);

    function Integer(v, radix) {
        if (typeof v === "undefined") return Integer[0];
        if (typeof radix !== "undefined") return +radix === 10 ? parseValue(v) : parseBase(v, radix);
        return parseValue(v);
    }

    function BigInteger(value, sign) {
        this.value = value;
        this.sign = sign;
        this.isSmall = false;
    }
    BigInteger.prototype = Object.create(Integer.prototype);

    function SmallInteger(value) {
        this.value = value;
        this.sign = value < 0;
        this.isSmall = true;
    }
    SmallInteger.prototype = Object.create(Integer.prototype);

    function isPrecise(n) {
        return -MAX_INT < n && n < MAX_INT;
    }

    function smallToArray(n) { // For performance reasons doesn't reference BASE, need to change this function if BASE changes
        if (n < 1e7)
            return [n];
        if (n < 1e14)
            return [n % 1e7, Math.floor(n / 1e7)];
        return [n % 1e7, Math.floor(n / 1e7) % 1e7, Math.floor(n / 1e14)];
    }

    function arrayToSmall(arr) { // If BASE changes this function may need to change
        trim(arr);
        var length = arr.length;
        if (length < 4 && compareAbs(arr, MAX_INT_ARR) < 0) {
            switch (length) {
                case 0: return 0;
                case 1: return arr[0];
                case 2: return arr[0] + arr[1] * BASE;
                default: return arr[0] + (arr[1] + arr[2] * BASE) * BASE;
            }
        }
        return arr;
    }

    function trim(v) {
        var i = v.length;
        while (v[--i] === 0);
        v.length = i + 1;
    }

    function createArray(length) { // function shamelessly stolen from Yaffle's library https://github.com/Yaffle/BigInteger
        var x = new Array(length);
        var i = -1;
        while (++i < length) {
            x[i] = 0;
        }
        return x;
    }

    function truncate(n) {
        if (n > 0) return Math.floor(n);
        return Math.ceil(n);
    }

    function add(a, b) { // assumes a and b are arrays with a.length >= b.length
        var l_a = a.length,
            l_b = b.length,
            r = new Array(l_a),
            carry = 0,
            base = BASE,
            sum, i;
        for (i = 0; i < l_b; i++) {
            sum = a[i] + b[i] + carry;
            carry = sum >= base ? 1 : 0;
            r[i] = sum - carry * base;
        }
        while (i < l_a) {
            sum = a[i] + carry;
            carry = sum === base ? 1 : 0;
            r[i++] = sum - carry * base;
        }
        if (carry > 0) r.push(carry);
        return r;
    }

    function addAny(a, b) {
        if (a.length >= b.length) return add(a, b);
        return add(b, a);
    }

    function addSmall(a, carry) { // assumes a is array, carry is number with 0 <= carry < MAX_INT
        var l = a.length,
            r = new Array(l),
            base = BASE,
            sum, i;
        for (i = 0; i < l; i++) {
            sum = a[i] - base + carry;
            carry = Math.floor(sum / base);
            r[i] = sum - carry * base;
            carry += 1;
        }
        while (carry > 0) {
            r[i++] = carry % base;
            carry = Math.floor(carry / base);
        }
        return r;
    }

    BigInteger.prototype.add = function (v) {
        var value, n = parseValue(v);
        if (this.sign !== n.sign) {
            return this.subtract(n.negate());
        }
        var a = this.value, b = n.value;
        if (n.isSmall) {
            return new BigInteger(addSmall(a, Math.abs(b)), this.sign);
        }
        return new BigInteger(addAny(a, b), this.sign);
    };
    BigInteger.prototype.plus = BigInteger.prototype.add;

    SmallInteger.prototype.add = function (v) {
        var n = parseValue(v);
        var a = this.value;
        if (a < 0 !== n.sign) {
            return this.subtract(n.negate());
        }
        var b = n.value;
        if (n.isSmall) {
            if (isPrecise(a + b)) return new SmallInteger(a + b);
            b = smallToArray(Math.abs(b));
        }
        return new BigInteger(addSmall(b, Math.abs(a)), a < 0);
    };
    SmallInteger.prototype.plus = SmallInteger.prototype.add;

    function subtract(a, b) { // assumes a and b are arrays with a >= b
        var a_l = a.length,
            b_l = b.length,
            r = new Array(a_l),
            borrow = 0,
            base = BASE,
            i, difference;
        for (i = 0; i < b_l; i++) {
            difference = a[i] - borrow - b[i];
            if (difference < 0) {
                difference += base;
                borrow = 1;
            } else borrow = 0;
            r[i] = difference;
        }
        for (i = b_l; i < a_l; i++) {
            difference = a[i] - borrow;
            if (difference < 0) difference += base;
            else {
                r[i++] = difference;
                break;
            }
            r[i] = difference;
        }
        for (; i < a_l; i++) {
            r[i] = a[i];
        }
        trim(r);
        return r;
    }

    function subtractAny(a, b, sign) {
        var value, isSmall;
        if (compareAbs(a, b) >= 0) {
            value = subtract(a,b);
        } else {
            value = subtract(b, a);
            sign = !sign;
        }
        value = arrayToSmall(value);
        if (typeof value === "number") {
            if (sign) value = -value;
            return new SmallInteger(value);
        }
        return new BigInteger(value, sign);
    }

    function subtractSmall(a, b, sign) { // assumes a is array, b is number with 0 <= b < MAX_INT
        var l = a.length,
            r = new Array(l),
            carry = -b,
            base = BASE,
            i, difference;
        for (i = 0; i < l; i++) {
            difference = a[i] + carry;
            carry = Math.floor(difference / base);
            difference %= base;
            r[i] = difference < 0 ? difference + base : difference;
        }
        r = arrayToSmall(r);
        if (typeof r === "number") {
            if (sign) r = -r;
            return new SmallInteger(r);
        } return new BigInteger(r, sign);
    }

    BigInteger.prototype.subtract = function (v) {
        var n = parseValue(v);
        if (this.sign !== n.sign) {
            return this.add(n.negate());
        }
        var a = this.value, b = n.value;
        if (n.isSmall)
            return subtractSmall(a, Math.abs(b), this.sign);
        return subtractAny(a, b, this.sign);
    };
    BigInteger.prototype.minus = BigInteger.prototype.subtract;

    SmallInteger.prototype.subtract = function (v) {
        var n = parseValue(v);
        var a = this.value;
        if (a < 0 !== n.sign) {
            return this.add(n.negate());
        }
        var b = n.value;
        if (n.isSmall) {
            return new SmallInteger(a - b);
        }
        return subtractSmall(b, Math.abs(a), a >= 0);
    };
    SmallInteger.prototype.minus = SmallInteger.prototype.subtract;

    BigInteger.prototype.negate = function () {
        return new BigInteger(this.value, !this.sign);
    };
    SmallInteger.prototype.negate = function () {
        var sign = this.sign;
        var small = new SmallInteger(-this.value);
        small.sign = !sign;
        return small;
    };

    BigInteger.prototype.abs = function () {
        return new BigInteger(this.value, false);
    };
    SmallInteger.prototype.abs = function () {
        return new SmallInteger(Math.abs(this.value));
    };

    function multiplyLong(a, b) {
        var a_l = a.length,
            b_l = b.length,
            l = a_l + b_l,
            r = createArray(l),
            base = BASE,
            product, carry, i, a_i, b_j;
        for (i = 0; i < a_l; ++i) {
            a_i = a[i];
            for (var j = 0; j < b_l; ++j) {
                b_j = b[j];
                product = a_i * b_j + r[i + j];
                carry = Math.floor(product / base);
                r[i + j] = product - carry * base;
                r[i + j + 1] += carry;
            }
        }
        trim(r);
        return r;
    }

    function multiplySmall(a, b) { // assumes a is array, b is number with |b| < BASE
        var l = a.length,
            r = new Array(l),
            base = BASE,
            carry = 0,
            product, i;
        for (i = 0; i < l; i++) {
            product = a[i] * b + carry;
            carry = Math.floor(product / base);
            r[i] = product - carry * base;
        }
        while (carry > 0) {
            r[i++] = carry % base;
            carry = Math.floor(carry / base);
        }
        return r;
    }

    function shiftLeft(x, n) {
        var r = [];
        while (n-- > 0) r.push(0);
        return r.concat(x);
    }

    function multiplyKaratsuba(x, y) {
        var n = Math.max(x.length, y.length);

        if (n <= 30) return multiplyLong(x, y);
        n = Math.ceil(n / 2);

        var b = x.slice(n),
            a = x.slice(0, n),
            d = y.slice(n),
            c = y.slice(0, n);

        var ac = multiplyKaratsuba(a, c),
            bd = multiplyKaratsuba(b, d),
            abcd = multiplyKaratsuba(addAny(a, b), addAny(c, d));

        var product = addAny(addAny(ac, shiftLeft(subtract(subtract(abcd, ac), bd), n)), shiftLeft(bd, 2 * n));
        trim(product);
        return product;
    }

    // The following function is derived from a surface fit of a graph plotting the performance difference
    // between long multiplication and karatsuba multiplication versus the lengths of the two arrays.
    function useKaratsuba(l1, l2) {
        return -0.012 * l1 - 0.012 * l2 + 0.000015 * l1 * l2 > 0;
    }

    BigInteger.prototype.multiply = function (v) {
        var value, n = parseValue(v),
            a = this.value, b = n.value,
            sign = this.sign !== n.sign,
            abs;
        if (n.isSmall) {
            if (b === 0) return Integer[0];
            if (b === 1) return this;
            if (b === -1) return this.negate();
            abs = Math.abs(b);
            if (abs < BASE) {
                return new BigInteger(multiplySmall(a, abs), sign);
            }
            b = smallToArray(abs);
        }
        if (useKaratsuba(a.length, b.length)) // Karatsuba is only faster for certain array sizes
            return new BigInteger(multiplyKaratsuba(a, b), sign);
        return new BigInteger(multiplyLong(a, b), sign);
    };

    BigInteger.prototype.times = BigInteger.prototype.multiply;

    function multiplySmallAndArray(a, b, sign) { // a >= 0
        if (a < BASE) {
            return new BigInteger(multiplySmall(b, a), sign);
        }
        return new BigInteger(multiplyLong(b, smallToArray(a)), sign);
    }
    SmallInteger.prototype._multiplyBySmall = function (a) {
            if (isPrecise(a.value * this.value)) {
                return new SmallInteger(a.value * this.value);
            }
            return multiplySmallAndArray(Math.abs(a.value), smallToArray(Math.abs(this.value)), this.sign !== a.sign);
    };
    BigInteger.prototype._multiplyBySmall = function (a) {
            if (a.value === 0) return Integer[0];
            if (a.value === 1) return this;
            if (a.value === -1) return this.negate();
            return multiplySmallAndArray(Math.abs(a.value), this.value, this.sign !== a.sign);
    };
    SmallInteger.prototype.multiply = function (v) {
        return parseValue(v)._multiplyBySmall(this);
    };
    SmallInteger.prototype.times = SmallInteger.prototype.multiply;

    function square(a) {
        var l = a.length,
            r = createArray(l + l),
            base = BASE,
            product, carry, i, a_i, a_j;
        for (i = 0; i < l; i++) {
            a_i = a[i];
            for (var j = 0; j < l; j++) {
                a_j = a[j];
                product = a_i * a_j + r[i + j];
                carry = Math.floor(product / base);
                r[i + j] = product - carry * base;
                r[i + j + 1] += carry;
            }
        }
        trim(r);
        return r;
    }

    BigInteger.prototype.square = function () {
        return new BigInteger(square(this.value), false);
    };

    SmallInteger.prototype.square = function () {
        var value = this.value * this.value;
        if (isPrecise(value)) return new SmallInteger(value);
        return new BigInteger(square(smallToArray(Math.abs(this.value))), false);
    };

    function divMod1(a, b) { // Left over from previous version. Performs faster than divMod2 on smaller input sizes.
        var a_l = a.length,
            b_l = b.length,
            base = BASE,
            result = createArray(b.length),
            divisorMostSignificantDigit = b[b_l - 1],
            // normalization
            lambda = Math.ceil(base / (2 * divisorMostSignificantDigit)),
            remainder = multiplySmall(a, lambda),
            divisor = multiplySmall(b, lambda),
            quotientDigit, shift, carry, borrow, i, l, q;
        if (remainder.length <= a_l) remainder.push(0);
        divisor.push(0);
        divisorMostSignificantDigit = divisor[b_l - 1];
        for (shift = a_l - b_l; shift >= 0; shift--) {
            quotientDigit = base - 1;
            if (remainder[shift + b_l] !== divisorMostSignificantDigit) {
              quotientDigit = Math.floor((remainder[shift + b_l] * base + remainder[shift + b_l - 1]) / divisorMostSignificantDigit);
            }
            // quotientDigit <= base - 1
            carry = 0;
            borrow = 0;
            l = divisor.length;
            for (i = 0; i < l; i++) {
                carry += quotientDigit * divisor[i];
                q = Math.floor(carry / base);
                borrow += remainder[shift + i] - (carry - q * base);
                carry = q;
                if (borrow < 0) {
                    remainder[shift + i] = borrow + base;
                    borrow = -1;
                } else {
                    remainder[shift + i] = borrow;
                    borrow = 0;
                }
            }
            while (borrow !== 0) {
                quotientDigit -= 1;
                carry = 0;
                for (i = 0; i < l; i++) {
                    carry += remainder[shift + i] - base + divisor[i];
                    if (carry < 0) {
                        remainder[shift + i] = carry + base;
                        carry = 0;
                    } else {
                        remainder[shift + i] = carry;
                        carry = 1;
                    }
                }
                borrow += carry;
            }
            result[shift] = quotientDigit;
        }
        // denormalization
        remainder = divModSmall(remainder, lambda)[0];
        return [arrayToSmall(result), arrayToSmall(remainder)];
    }

    function divMod2(a, b) { // Implementation idea shamelessly stolen from Silent Matt's library http://silentmatt.com/biginteger/
        // Performs faster than divMod1 on larger input sizes.
        var a_l = a.length,
            b_l = b.length,
            result = [],
            part = [],
            base = BASE,
            guess, xlen, highx, highy, check;
        while (a_l) {
            part.unshift(a[--a_l]);
            if (compareAbs(part, b) < 0) {
                result.push(0);
                continue;
            }
            xlen = part.length;
            highx = part[xlen - 1] * base + part[xlen - 2];
            highy = b[b_l - 1] * base + b[b_l - 2];
            if (xlen > b_l) {
                highx = (highx + 1) * base;
            }
            guess = Math.ceil(highx / highy);
            do {
                check = multiplySmall(b, guess);
                if (compareAbs(check, part) <= 0) break;
                guess--;
            } while (guess);
            result.push(guess);
            part = subtract(part, check);
        }
        result.reverse();
        return [arrayToSmall(result), arrayToSmall(part)];
    }

    function divModSmall(value, lambda) {
        var length = value.length,
            quotient = createArray(length),
            base = BASE,
            i, q, remainder, divisor;
        remainder = 0;
        for (i = length - 1; i >= 0; --i) {
            divisor = remainder * base + value[i];
            q = truncate(divisor / lambda);
            remainder = divisor - q * lambda;
            quotient[i] = q | 0;
        }
        return [quotient, remainder | 0];
    }

    function divModAny(self, v) {
        var value, n = parseValue(v);
        var a = self.value, b = n.value;
        var quotient;
        if (b === 0) throw new Error("Cannot divide by zero");
        if (self.isSmall) {
            if (n.isSmall) {
                return [new SmallInteger(truncate(a / b)), new SmallInteger(a % b)];
            }
            return [Integer[0], self];
        }
        if (n.isSmall) {
            if (b === 1) return [self, Integer[0]];
            if (b == -1) return [self.negate(), Integer[0]];
            var abs = Math.abs(b);
            if (abs < BASE) {
                value = divModSmall(a, abs);
                quotient = arrayToSmall(value[0]);
                var remainder = value[1];
                if (self.sign) remainder = -remainder;
                if (typeof quotient === "number") {
                    if (self.sign !== n.sign) quotient = -quotient;
                    return [new SmallInteger(quotient), new SmallInteger(remainder)];
                }
                return [new BigInteger(quotient, self.sign !== n.sign), new SmallInteger(remainder)];
            }
            b = smallToArray(abs);
        }
        var comparison = compareAbs(a, b);
        if (comparison === -1) return [Integer[0], self];
        if (comparison === 0) return [Integer[self.sign === n.sign ? 1 : -1], Integer[0]];

        // divMod1 is faster on smaller input sizes
        if (a.length + b.length <= 200)
            value = divMod1(a, b);
        else value = divMod2(a, b);

        quotient = value[0];
        var qSign = self.sign !== n.sign,
            mod = value[1],
            mSign = self.sign;
        if (typeof quotient === "number") {
            if (qSign) quotient = -quotient;
            quotient = new SmallInteger(quotient);
        } else quotient = new BigInteger(quotient, qSign);
        if (typeof mod === "number") {
            if (mSign) mod = -mod;
            mod = new SmallInteger(mod);
        } else mod = new BigInteger(mod, mSign);
        return [quotient, mod];
    }

    BigInteger.prototype.divmod = function (v) {
        var result = divModAny(this, v);
        return {
            quotient: result[0],
            remainder: result[1]
        };
    };
    SmallInteger.prototype.divmod = BigInteger.prototype.divmod;

    BigInteger.prototype.divide = function (v) {
        return divModAny(this, v)[0];
    };
    SmallInteger.prototype.over = SmallInteger.prototype.divide = BigInteger.prototype.over = BigInteger.prototype.divide;

    BigInteger.prototype.mod = function (v) {
        return divModAny(this, v)[1];
    };
    SmallInteger.prototype.remainder = SmallInteger.prototype.mod = BigInteger.prototype.remainder = BigInteger.prototype.mod;

    BigInteger.prototype.pow = function (v) {
        var n = parseValue(v),
            a = this.value,
            b = n.value,
            value, x, y;
        if (b === 0) return Integer[1];
        if (a === 0) return Integer[0];
        if (a === 1) return Integer[1];
        if (a === -1) return n.isEven() ? Integer[1] : Integer[-1];
        if (n.sign) {
            return Integer[0];
        }
        if (!n.isSmall) throw new Error("The exponent " + n.toString() + " is too large.");
        if (this.isSmall) {
            if (isPrecise(value = Math.pow(a, b)))
                return new SmallInteger(truncate(value));
        }
        x = this;
        y = Integer[1];
        while (true) {
            if (b & 1 === 1) {
                y = y.times(x);
                --b;
            }
            if (b === 0) break;
            b /= 2;
            x = x.square();
        }
        return y;
    };
    SmallInteger.prototype.pow = BigInteger.prototype.pow;

    BigInteger.prototype.modPow = function (exp, mod) {
        exp = parseValue(exp);
        mod = parseValue(mod);
        if (mod.isZero()) throw new Error("Cannot take modPow with modulus 0");
        var r = Integer[1],
            base = this.mod(mod);
        while (exp.isPositive()) {
            if (base.isZero()) return Integer[0];
            if (exp.isOdd()) r = r.multiply(base).mod(mod);
            exp = exp.divide(2);
            base = base.square().mod(mod);
        }
        return r;
    };
    SmallInteger.prototype.modPow = BigInteger.prototype.modPow;

    function compareAbs(a, b) {
        if (a.length !== b.length) {
            return a.length > b.length ? 1 : -1;
        }
        for (var i = a.length - 1; i >= 0; i--) {
            if (a[i] !== b[i]) return a[i] > b[i] ? 1 : -1;
        }
        return 0;
    }

    BigInteger.prototype.compareAbs = function (v) {
        var n = parseValue(v),
            a = this.value,
            b = n.value;
        if (n.isSmall) return 1;
        return compareAbs(a, b);
    };
    SmallInteger.prototype.compareAbs = function (v) {
        var n = parseValue(v),
            a = Math.abs(this.value),
            b = n.value;
        if (n.isSmall) {
            b = Math.abs(b);
            return a === b ? 0 : a > b ? 1 : -1;
        }
        return -1;
    };

    BigInteger.prototype.compare = function (v) {
        // See discussion about comparison with Infinity:
        // https://github.com/peterolson/BigInteger.js/issues/61
        if (v === Infinity) {
            return -1;
        }
        if (v === -Infinity) {
            return 1;
        }

        var n = parseValue(v),
            a = this.value,
            b = n.value;
        if (this.sign !== n.sign) {
            return n.sign ? 1 : -1;
        }
        if (n.isSmall) {
            return this.sign ? -1 : 1;
        }
        return compareAbs(a, b) * (this.sign ? -1 : 1);
    };
    BigInteger.prototype.compareTo = BigInteger.prototype.compare;

    SmallInteger.prototype.compare = function (v) {
        if (v === Infinity) {
            return -1;
        }
        if (v === -Infinity) {
            return 1;
        }

        var n = parseValue(v),
            a = this.value,
            b = n.value;
        if (n.isSmall) {
            return a == b ? 0 : a > b ? 1 : -1;
        }
        if (a < 0 !== n.sign) {
            return a < 0 ? -1 : 1;
        }
        return a < 0 ? 1 : -1;
    };
    SmallInteger.prototype.compareTo = SmallInteger.prototype.compare;

    BigInteger.prototype.equals = function (v) {
        return this.compare(v) === 0;
    };
    SmallInteger.prototype.eq = SmallInteger.prototype.equals = BigInteger.prototype.eq = BigInteger.prototype.equals;

    BigInteger.prototype.notEquals = function (v) {
        return this.compare(v) !== 0;
    };
    SmallInteger.prototype.neq = SmallInteger.prototype.notEquals = BigInteger.prototype.neq = BigInteger.prototype.notEquals;

    BigInteger.prototype.greater = function (v) {
        return this.compare(v) > 0;
    };
    SmallInteger.prototype.gt = SmallInteger.prototype.greater = BigInteger.prototype.gt = BigInteger.prototype.greater;

    BigInteger.prototype.lesser = function (v) {
        return this.compare(v) < 0;
    };
    SmallInteger.prototype.lt = SmallInteger.prototype.lesser = BigInteger.prototype.lt = BigInteger.prototype.lesser;

    BigInteger.prototype.greaterOrEquals = function (v) {
        return this.compare(v) >= 0;
    };
    SmallInteger.prototype.geq = SmallInteger.prototype.greaterOrEquals = BigInteger.prototype.geq = BigInteger.prototype.greaterOrEquals;

    BigInteger.prototype.lesserOrEquals = function (v) {
        return this.compare(v) <= 0;
    };
    SmallInteger.prototype.leq = SmallInteger.prototype.lesserOrEquals = BigInteger.prototype.leq = BigInteger.prototype.lesserOrEquals;

    BigInteger.prototype.isEven = function () {
        return (this.value[0] & 1) === 0;
    };
    SmallInteger.prototype.isEven = function () {
        return (this.value & 1) === 0;
    };

    BigInteger.prototype.isOdd = function () {
        return (this.value[0] & 1) === 1;
    };
    SmallInteger.prototype.isOdd = function () {
        return (this.value & 1) === 1;
    };

    BigInteger.prototype.isPositive = function () {
        return !this.sign;
    };
    SmallInteger.prototype.isPositive = function () {
        return this.value > 0;
    };

    BigInteger.prototype.isNegative = function () {
        return this.sign;
    };
    SmallInteger.prototype.isNegative = function () {
        return this.value < 0;
    };

    BigInteger.prototype.isUnit = function () {
        return false;
    };
    SmallInteger.prototype.isUnit = function () {
        return Math.abs(this.value) === 1;
    };

    BigInteger.prototype.isZero = function () {
        return false;
    };
    SmallInteger.prototype.isZero = function () {
        return this.value === 0;
    };
    BigInteger.prototype.isDivisibleBy = function (v) {
        var n = parseValue(v);
        var value = n.value;
        if (value === 0) return false;
        if (value === 1) return true;
        if (value === 2) return this.isEven();
        return this.mod(n).equals(Integer[0]);
    };
    SmallInteger.prototype.isDivisibleBy = BigInteger.prototype.isDivisibleBy;

    function isBasicPrime(v) {
        var n = v.abs();
        if (n.isUnit()) return false;
        if (n.equals(2) || n.equals(3) || n.equals(5)) return true;
        if (n.isEven() || n.isDivisibleBy(3) || n.isDivisibleBy(5)) return false;
        if (n.lesser(25)) return true;
        // we don't know if it's prime: let the other functions figure it out
    }

    BigInteger.prototype.isPrime = function () {
        var isPrime = isBasicPrime(this);
        if (isPrime !== undefined) return isPrime;
        var n = this.abs(),
            nPrev = n.prev();
        var a = [2, 3, 5, 7, 11, 13, 17, 19],
            b = nPrev,
            d, t, i, x;
        while (b.isEven()) b = b.divide(2);
        for (i = 0; i < a.length; i++) {
            x = bigInt(a[i]).modPow(b, n);
            if (x.equals(Integer[1]) || x.equals(nPrev)) continue;
            for (t = true, d = b; t && d.lesser(nPrev) ; d = d.multiply(2)) {
                x = x.square().mod(n);
                if (x.equals(nPrev)) t = false;
            }
            if (t) return false;
        }
        return true;
    };
    SmallInteger.prototype.isPrime = BigInteger.prototype.isPrime;

    BigInteger.prototype.isProbablePrime = function (iterations) {
        var isPrime = isBasicPrime(this);
        if (isPrime !== undefined) return isPrime;
        var n = this.abs();
        var t = iterations === undefined ? 5 : iterations;
        // use the Fermat primality test
        for (var i = 0; i < t; i++) {
            var a = bigInt.randBetween(2, n.minus(2));
            if (!a.modPow(n.prev(), n).isUnit()) return false; // definitely composite
        }
        return true; // large chance of being prime
    };
    SmallInteger.prototype.isProbablePrime = BigInteger.prototype.isProbablePrime;

    BigInteger.prototype.modInv = function (n) {
        var t = bigInt.zero, newT = bigInt.one, r = parseValue(n), newR = this.abs(), q, lastT, lastR;
        while (!newR.equals(bigInt.zero)) {
        	q = r.divide(newR);
          lastT = t;
          lastR = r;
          t = newT;
          r = newR;
          newT = lastT.subtract(q.multiply(newT));
          newR = lastR.subtract(q.multiply(newR));
        }
        if (!r.equals(1)) throw new Error(this.toString() + " and " + n.toString() + " are not co-prime");
        if (t.compare(0) === -1) {
        	t = t.add(n);
        }
        return t;
    }
    SmallInteger.prototype.modInv = BigInteger.prototype.modInv;

    BigInteger.prototype.next = function () {
        var value = this.value;
        if (this.sign) {
            return subtractSmall(value, 1, this.sign);
        }
        return new BigInteger(addSmall(value, 1), this.sign);
    };
    SmallInteger.prototype.next = function () {
        var value = this.value;
        if (value + 1 < MAX_INT) return new SmallInteger(value + 1);
        return new BigInteger(MAX_INT_ARR, false);
    };

    BigInteger.prototype.prev = function () {
        var value = this.value;
        if (this.sign) {
            return new BigInteger(addSmall(value, 1), true);
        }
        return subtractSmall(value, 1, this.sign);
    };
    SmallInteger.prototype.prev = function () {
        var value = this.value;
        if (value - 1 > -MAX_INT) return new SmallInteger(value - 1);
        return new BigInteger(MAX_INT_ARR, true);
    };

    var powersOfTwo = [1];
    while (powersOfTwo[powersOfTwo.length - 1] <= BASE) powersOfTwo.push(2 * powersOfTwo[powersOfTwo.length - 1]);
    var powers2Length = powersOfTwo.length, highestPower2 = powersOfTwo[powers2Length - 1];

    function shift_isSmall(n) {
        return ((typeof n === "number" || typeof n === "string") && +Math.abs(n) <= BASE) ||
            (n instanceof BigInteger && n.value.length <= 1);
    }

    BigInteger.prototype.shiftLeft = function (n) {
        if (!shift_isSmall(n)) {
            throw new Error(String(n) + " is too large for shifting.");
        }
        n = +n;
        if (n < 0) return this.shiftRight(-n);
        var result = this;
        while (n >= powers2Length) {
            result = result.multiply(highestPower2);
            n -= powers2Length - 1;
        }
        return result.multiply(powersOfTwo[n]);
    };
    SmallInteger.prototype.shiftLeft = BigInteger.prototype.shiftLeft;

    BigInteger.prototype.shiftRight = function (n) {
        var remQuo;
        if (!shift_isSmall(n)) {
            throw new Error(String(n) + " is too large for shifting.");
        }
        n = +n;
        if (n < 0) return this.shiftLeft(-n);
        var result = this;
        while (n >= powers2Length) {
            if (result.isZero()) return result;
            remQuo = divModAny(result, highestPower2);
            result = remQuo[1].isNegative() ? remQuo[0].prev() : remQuo[0];
            n -= powers2Length - 1;
        }
        remQuo = divModAny(result, powersOfTwo[n]);
        return remQuo[1].isNegative() ? remQuo[0].prev() : remQuo[0];
    };
    SmallInteger.prototype.shiftRight = BigInteger.prototype.shiftRight;

    function bitwise(x, y, fn) {
        y = parseValue(y);
        var xSign = x.isNegative(), ySign = y.isNegative();
        var xRem = xSign ? x.not() : x,
            yRem = ySign ? y.not() : y;
        var xBits = [], yBits = [];
        var xStop = false, yStop = false;
        while (!xStop || !yStop) {
            if (xRem.isZero()) { // virtual sign extension for simulating two's complement
                xStop = true;
                xBits.push(xSign ? 1 : 0);
            }
            else if (xSign) xBits.push(xRem.isEven() ? 1 : 0); // two's complement for negative numbers
            else xBits.push(xRem.isEven() ? 0 : 1);

            if (yRem.isZero()) {
                yStop = true;
                yBits.push(ySign ? 1 : 0);
            }
            else if (ySign) yBits.push(yRem.isEven() ? 1 : 0);
            else yBits.push(yRem.isEven() ? 0 : 1);

            xRem = xRem.over(2);
            yRem = yRem.over(2);
        }
        var result = [];
        for (var i = 0; i < xBits.length; i++) result.push(fn(xBits[i], yBits[i]));
        var sum = bigInt(result.pop()).negate().times(bigInt(2).pow(result.length));
        while (result.length) {
            sum = sum.add(bigInt(result.pop()).times(bigInt(2).pow(result.length)));
        }
        return sum;
    }

    BigInteger.prototype.not = function () {
        return this.negate().prev();
    };
    SmallInteger.prototype.not = BigInteger.prototype.not;

    BigInteger.prototype.and = function (n) {
        return bitwise(this, n, function (a, b) { return a & b; });
    };
    SmallInteger.prototype.and = BigInteger.prototype.and;

    BigInteger.prototype.or = function (n) {
        return bitwise(this, n, function (a, b) { return a | b; });
    };
    SmallInteger.prototype.or = BigInteger.prototype.or;

    BigInteger.prototype.xor = function (n) {
        return bitwise(this, n, function (a, b) { return a ^ b; });
    };
    SmallInteger.prototype.xor = BigInteger.prototype.xor;

    var LOBMASK_I = 1 << 30, LOBMASK_BI = (BASE & -BASE) * (BASE & -BASE) | LOBMASK_I;
    function roughLOB(n) { // get lowestOneBit (rough)
        // SmallInteger: return Min(lowestOneBit(n), 1 << 30)
        // BigInteger: return Min(lowestOneBit(n), 1 << 14) [BASE=1e7]
        var v = n.value, x = typeof v === "number" ? v | LOBMASK_I : v[0] + v[1] * BASE | LOBMASK_BI;
        return x & -x;
    }

    function max(a, b) {
        a = parseValue(a);
        b = parseValue(b);
        return a.greater(b) ? a : b;
    }
    function min(a,b) {
        a = parseValue(a);
        b = parseValue(b);
        return a.lesser(b) ? a : b;
    }
    function gcd(a, b) {
        a = parseValue(a).abs();
        b = parseValue(b).abs();
        if (a.equals(b)) return a;
        if (a.isZero()) return b;
        if (b.isZero()) return a;
        var c = Integer[1], d, t;
        while (a.isEven() && b.isEven()) {
            d = Math.min(roughLOB(a), roughLOB(b));
            a = a.divide(d);
            b = b.divide(d);
            c = c.multiply(d);
        }
        while (a.isEven()) {
            a = a.divide(roughLOB(a));
        }
        do {
            while (b.isEven()) {
                b = b.divide(roughLOB(b));
            }
            if (a.greater(b)) {
                t = b; b = a; a = t;
            }
            b = b.subtract(a);
        } while (!b.isZero());
        return c.isUnit() ? a : a.multiply(c);
    }
    function lcm(a, b) {
        a = parseValue(a).abs();
        b = parseValue(b).abs();
        return a.divide(gcd(a, b)).multiply(b);
    }
    function randBetween(a, b) {
        a = parseValue(a);
        b = parseValue(b);
        var low = min(a, b), high = max(a, b);
        var range = high.subtract(low);
        if (range.isSmall) return low.add(Math.round(Math.random() * range));
        var length = range.value.length - 1;
        var result = [], restricted = true;
        for (var i = length; i >= 0; i--) {
            var top = restricted ? range.value[i] : BASE;
            var digit = truncate(Math.random() * top);
            result.unshift(digit);
            if (digit < top) restricted = false;
        }
        result = arrayToSmall(result);
        return low.add(typeof result === "number" ? new SmallInteger(result) : new BigInteger(result, false));
    }
    var parseBase = function (text, base) {
        var val = Integer[0], pow = Integer[1],
            length = text.length;
        if (2 <= base && base <= 36) {
            if (length <= LOG_MAX_INT / Math.log(base)) {
                return new SmallInteger(parseInt(text, base));
            }
        }
        base = parseValue(base);
        var digits = [];
        var i;
        var isNegative = text[0] === "-";
        for (i = isNegative ? 1 : 0; i < text.length; i++) {
            var c = text[i].toLowerCase(),
                charCode = c.charCodeAt(0);
            if (48 <= charCode && charCode <= 57) digits.push(parseValue(c));
            else if (97 <= charCode && charCode <= 122) digits.push(parseValue(c.charCodeAt(0) - 87));
            else if (c === "<") {
                var start = i;
                do { i++; } while (text[i] !== ">");
                digits.push(parseValue(text.slice(start + 1, i)));
            }
            else throw new Error(c + " is not a valid character");
        }
        digits.reverse();
        for (i = 0; i < digits.length; i++) {
            val = val.add(digits[i].times(pow));
            pow = pow.times(base);
        }
        return isNegative ? val.negate() : val;
    };

    function stringify(digit) {
        var v = digit.value;
        if (typeof v === "number") v = [v];
        if (v.length === 1 && v[0] <= 35) {
            return "0123456789abcdefghijklmnopqrstuvwxyz".charAt(v[0]);
        }
        return "<" + v + ">";
    }
    function toBase(n, base) {
        base = bigInt(base);
        if (base.isZero()) {
            if (n.isZero()) return "0";
            throw new Error("Cannot convert nonzero numbers to base 0.");
        }
        if (base.equals(-1)) {
            if (n.isZero()) return "0";
            if (n.isNegative()) return new Array(1 - n).join("10");
            return "1" + new Array(+n).join("01");
        }
        var minusSign = "";
        if (n.isNegative() && base.isPositive()) {
            minusSign = "-";
            n = n.abs();
        }
        if (base.equals(1)) {
            if (n.isZero()) return "0";
            return minusSign + new Array(+n + 1).join(1);
        }
        var out = [];
        var left = n, divmod;
        while (left.isNegative() || left.compareAbs(base) >= 0) {
            divmod = left.divmod(base);
            left = divmod.quotient;
            var digit = divmod.remainder;
            if (digit.isNegative()) {
                digit = base.minus(digit).abs();
                left = left.next();
            }
            out.push(stringify(digit));
        }
        out.push(stringify(left));
        return minusSign + out.reverse().join("");
    }

    BigInteger.prototype.toString = function (radix) {
        if (radix === undefined) radix = 10;
        if (radix !== 10) return toBase(this, radix);
        var v = this.value, l = v.length, str = String(v[--l]), zeros = "0000000", digit;
        while (--l >= 0) {
            digit = String(v[l]);
            str += zeros.slice(digit.length) + digit;
        }
        var sign = this.sign ? "-" : "";
        return sign + str;
    };
    SmallInteger.prototype.toString = function (radix) {
        if (radix === undefined) radix = 10;
        if (radix != 10) return toBase(this, radix);
        return String(this.value);
    };

    BigInteger.prototype.valueOf = function () {
        return +this.toString();
    };
    BigInteger.prototype.toJSNumber = BigInteger.prototype.valueOf;

    SmallInteger.prototype.valueOf = function () {
        return this.value;
    };
    SmallInteger.prototype.toJSNumber = SmallInteger.prototype.valueOf;

    function parseStringValue(v) {
            if (isPrecise(+v)) {
                var x = +v;
                if (x === truncate(x))
                    return new SmallInteger(x);
                throw "Invalid integer: " + v;
            }
            var sign = v[0] === "-";
            if (sign) v = v.slice(1);
            var split = v.split(/e/i);
            if (split.length > 2) throw new Error("Invalid integer: " + split.join("e"));
            if (split.length === 2) {
                var exp = split[1];
                if (exp[0] === "+") exp = exp.slice(1);
                exp = +exp;
                if (exp !== truncate(exp) || !isPrecise(exp)) throw new Error("Invalid integer: " + exp + " is not a valid exponent.");
                var text = split[0];
                var decimalPlace = text.indexOf(".");
                if (decimalPlace >= 0) {
                    exp -= text.length - decimalPlace - 1;
                    text = text.slice(0, decimalPlace) + text.slice(decimalPlace + 1);
                }
                if (exp < 0) throw new Error("Cannot include negative exponent part for integers");
                text += (new Array(exp + 1)).join("0");
                v = text;
            }
            var isValid = /^([0-9][0-9]*)$/.test(v);
            if (!isValid) throw new Error("Invalid integer: " + v);
            var r = [], max = v.length, l = LOG_BASE, min = max - l;
            while (max > 0) {
                r.push(+v.slice(min, max));
                min -= l;
                if (min < 0) min = 0;
                max -= l;
            }
            trim(r);
            return new BigInteger(r, sign);
    }

    function parseNumberValue(v) {
        if (isPrecise(v)) {
            if (v !== truncate(v)) throw new Error(v + " is not an integer.");
            return new SmallInteger(v);
        }
        return parseStringValue(v.toString());
    }

    function parseValue(v) {
        if (typeof v === "number") {
            return parseNumberValue(v);
        }
        if (typeof v === "string") {
            return parseStringValue(v);
        }
        return v;
    }
    // Pre-define numbers in range [-999,999]
    for (var i = 0; i < 1000; i++) {
        Integer[i] = new SmallInteger(i);
        if (i > 0) Integer[-i] = new SmallInteger(-i);
    }
    // Backwards compatibility
    Integer.one = Integer[1];
    Integer.zero = Integer[0];
    Integer.minusOne = Integer[-1];
    Integer.max = max;
    Integer.min = min;
    Integer.gcd = gcd;
    Integer.lcm = lcm;
    Integer.isInstance = function (x) { return x instanceof BigInteger || x instanceof SmallInteger; };
    Integer.randBetween = randBetween;
    return Integer;
})();

// Node.js check
if (typeof module !== "undefined" && module.hasOwnProperty("exports")) {
    module.exports = bigInt;
}

},{}],3:[function(require,module,exports){

},{}],4:[function(require,module,exports){
arguments[4][3][0].apply(exports,arguments)
},{"dup":3}],5:[function(require,module,exports){
(function (global){
/*!
 * The buffer module from node.js, for the browser.
 *
 * @author   Feross Aboukhadijeh <feross@feross.org> <http://feross.org>
 * @license  MIT
 */
/* eslint-disable no-proto */

'use strict'

var base64 = require('base64-js')
var ieee754 = require('ieee754')
var isArray = require('isarray')

exports.Buffer = Buffer
exports.SlowBuffer = SlowBuffer
exports.INSPECT_MAX_BYTES = 50

/**
 * If `Buffer.TYPED_ARRAY_SUPPORT`:
 *   === true    Use Uint8Array implementation (fastest)
 *   === false   Use Object implementation (most compatible, even IE6)
 *
 * Browsers that support typed arrays are IE 10+, Firefox 4+, Chrome 7+, Safari 5.1+,
 * Opera 11.6+, iOS 4.2+.
 *
 * Due to various browser bugs, sometimes the Object implementation will be used even
 * when the browser supports typed arrays.
 *
 * Note:
 *
 *   - Firefox 4-29 lacks support for adding new properties to `Uint8Array` instances,
 *     See: https://bugzilla.mozilla.org/show_bug.cgi?id=695438.
 *
 *   - Chrome 9-10 is missing the `TypedArray.prototype.subarray` function.
 *
 *   - IE10 has a broken `TypedArray.prototype.subarray` function which returns arrays of
 *     incorrect length in some situations.

 * We detect these buggy browsers and set `Buffer.TYPED_ARRAY_SUPPORT` to `false` so they
 * get the Object implementation, which is slower but behaves correctly.
 */
Buffer.TYPED_ARRAY_SUPPORT = global.TYPED_ARRAY_SUPPORT !== undefined
  ? global.TYPED_ARRAY_SUPPORT
  : typedArraySupport()

/*
 * Export kMaxLength after typed array support is determined.
 */
exports.kMaxLength = kMaxLength()

function typedArraySupport () {
  try {
    var arr = new Uint8Array(1)
    arr.__proto__ = {__proto__: Uint8Array.prototype, foo: function () { return 42 }}
    return arr.foo() === 42 && // typed array instances can be augmented
        typeof arr.subarray === 'function' && // chrome 9-10 lack `subarray`
        arr.subarray(1, 1).byteLength === 0 // ie10 has broken `subarray`
  } catch (e) {
    return false
  }
}

function kMaxLength () {
  return Buffer.TYPED_ARRAY_SUPPORT
    ? 0x7fffffff
    : 0x3fffffff
}

function createBuffer (that, length) {
  if (kMaxLength() < length) {
    throw new RangeError('Invalid typed array length')
  }
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    // Return an augmented `Uint8Array` instance, for best performance
    that = new Uint8Array(length)
    that.__proto__ = Buffer.prototype
  } else {
    // Fallback: Return an object instance of the Buffer class
    if (that === null) {
      that = new Buffer(length)
    }
    that.length = length
  }

  return that
}

/**
 * The Buffer constructor returns instances of `Uint8Array` that have their
 * prototype changed to `Buffer.prototype`. Furthermore, `Buffer` is a subclass of
 * `Uint8Array`, so the returned instances will have all the node `Buffer` methods
 * and the `Uint8Array` methods. Square bracket notation works as expected -- it
 * returns a single octet.
 *
 * The `Uint8Array` prototype remains unmodified.
 */

function Buffer (arg, encodingOrOffset, length) {
  if (!Buffer.TYPED_ARRAY_SUPPORT && !(this instanceof Buffer)) {
    return new Buffer(arg, encodingOrOffset, length)
  }

  // Common case.
  if (typeof arg === 'number') {
    if (typeof encodingOrOffset === 'string') {
      throw new Error(
        'If encoding is specified then the first argument must be a string'
      )
    }
    return allocUnsafe(this, arg)
  }
  return from(this, arg, encodingOrOffset, length)
}

Buffer.poolSize = 8192 // not used by this implementation

// TODO: Legacy, not needed anymore. Remove in next major version.
Buffer._augment = function (arr) {
  arr.__proto__ = Buffer.prototype
  return arr
}

function from (that, value, encodingOrOffset, length) {
  if (typeof value === 'number') {
    throw new TypeError('"value" argument must not be a number')
  }

  if (typeof ArrayBuffer !== 'undefined' && value instanceof ArrayBuffer) {
    return fromArrayBuffer(that, value, encodingOrOffset, length)
  }

  if (typeof value === 'string') {
    return fromString(that, value, encodingOrOffset)
  }

  return fromObject(that, value)
}

/**
 * Functionally equivalent to Buffer(arg, encoding) but throws a TypeError
 * if value is a number.
 * Buffer.from(str[, encoding])
 * Buffer.from(array)
 * Buffer.from(buffer)
 * Buffer.from(arrayBuffer[, byteOffset[, length]])
 **/
Buffer.from = function (value, encodingOrOffset, length) {
  return from(null, value, encodingOrOffset, length)
}

if (Buffer.TYPED_ARRAY_SUPPORT) {
  Buffer.prototype.__proto__ = Uint8Array.prototype
  Buffer.__proto__ = Uint8Array
  if (typeof Symbol !== 'undefined' && Symbol.species &&
      Buffer[Symbol.species] === Buffer) {
    // Fix subarray() in ES2016. See: https://github.com/feross/buffer/pull/97
    Object.defineProperty(Buffer, Symbol.species, {
      value: null,
      configurable: true
    })
  }
}

function assertSize (size) {
  if (typeof size !== 'number') {
    throw new TypeError('"size" argument must be a number')
  } else if (size < 0) {
    throw new RangeError('"size" argument must not be negative')
  }
}

function alloc (that, size, fill, encoding) {
  assertSize(size)
  if (size <= 0) {
    return createBuffer(that, size)
  }
  if (fill !== undefined) {
    // Only pay attention to encoding if it's a string. This
    // prevents accidentally sending in a number that would
    // be interpretted as a start offset.
    return typeof encoding === 'string'
      ? createBuffer(that, size).fill(fill, encoding)
      : createBuffer(that, size).fill(fill)
  }
  return createBuffer(that, size)
}

/**
 * Creates a new filled Buffer instance.
 * alloc(size[, fill[, encoding]])
 **/
Buffer.alloc = function (size, fill, encoding) {
  return alloc(null, size, fill, encoding)
}

function allocUnsafe (that, size) {
  assertSize(size)
  that = createBuffer(that, size < 0 ? 0 : checked(size) | 0)
  if (!Buffer.TYPED_ARRAY_SUPPORT) {
    for (var i = 0; i < size; ++i) {
      that[i] = 0
    }
  }
  return that
}

/**
 * Equivalent to Buffer(num), by default creates a non-zero-filled Buffer instance.
 * */
Buffer.allocUnsafe = function (size) {
  return allocUnsafe(null, size)
}
/**
 * Equivalent to SlowBuffer(num), by default creates a non-zero-filled Buffer instance.
 */
Buffer.allocUnsafeSlow = function (size) {
  return allocUnsafe(null, size)
}

function fromString (that, string, encoding) {
  if (typeof encoding !== 'string' || encoding === '') {
    encoding = 'utf8'
  }

  if (!Buffer.isEncoding(encoding)) {
    throw new TypeError('"encoding" must be a valid string encoding')
  }

  var length = byteLength(string, encoding) | 0
  that = createBuffer(that, length)

  var actual = that.write(string, encoding)

  if (actual !== length) {
    // Writing a hex string, for example, that contains invalid characters will
    // cause everything after the first invalid character to be ignored. (e.g.
    // 'abxxcd' will be treated as 'ab')
    that = that.slice(0, actual)
  }

  return that
}

function fromArrayLike (that, array) {
  var length = array.length < 0 ? 0 : checked(array.length) | 0
  that = createBuffer(that, length)
  for (var i = 0; i < length; i += 1) {
    that[i] = array[i] & 255
  }
  return that
}

function fromArrayBuffer (that, array, byteOffset, length) {
  array.byteLength // this throws if `array` is not a valid ArrayBuffer

  if (byteOffset < 0 || array.byteLength < byteOffset) {
    throw new RangeError('\'offset\' is out of bounds')
  }

  if (array.byteLength < byteOffset + (length || 0)) {
    throw new RangeError('\'length\' is out of bounds')
  }

  if (byteOffset === undefined && length === undefined) {
    array = new Uint8Array(array)
  } else if (length === undefined) {
    array = new Uint8Array(array, byteOffset)
  } else {
    array = new Uint8Array(array, byteOffset, length)
  }

  if (Buffer.TYPED_ARRAY_SUPPORT) {
    // Return an augmented `Uint8Array` instance, for best performance
    that = array
    that.__proto__ = Buffer.prototype
  } else {
    // Fallback: Return an object instance of the Buffer class
    that = fromArrayLike(that, array)
  }
  return that
}

function fromObject (that, obj) {
  if (Buffer.isBuffer(obj)) {
    var len = checked(obj.length) | 0
    that = createBuffer(that, len)

    if (that.length === 0) {
      return that
    }

    obj.copy(that, 0, 0, len)
    return that
  }

  if (obj) {
    if ((typeof ArrayBuffer !== 'undefined' &&
        obj.buffer instanceof ArrayBuffer) || 'length' in obj) {
      if (typeof obj.length !== 'number' || isnan(obj.length)) {
        return createBuffer(that, 0)
      }
      return fromArrayLike(that, obj)
    }

    if (obj.type === 'Buffer' && isArray(obj.data)) {
      return fromArrayLike(that, obj.data)
    }
  }

  throw new TypeError('First argument must be a string, Buffer, ArrayBuffer, Array, or array-like object.')
}

function checked (length) {
  // Note: cannot use `length < kMaxLength()` here because that fails when
  // length is NaN (which is otherwise coerced to zero.)
  if (length >= kMaxLength()) {
    throw new RangeError('Attempt to allocate Buffer larger than maximum ' +
                         'size: 0x' + kMaxLength().toString(16) + ' bytes')
  }
  return length | 0
}

function SlowBuffer (length) {
  if (+length != length) { // eslint-disable-line eqeqeq
    length = 0
  }
  return Buffer.alloc(+length)
}

Buffer.isBuffer = function isBuffer (b) {
  return !!(b != null && b._isBuffer)
}

Buffer.compare = function compare (a, b) {
  if (!Buffer.isBuffer(a) || !Buffer.isBuffer(b)) {
    throw new TypeError('Arguments must be Buffers')
  }

  if (a === b) return 0

  var x = a.length
  var y = b.length

  for (var i = 0, len = Math.min(x, y); i < len; ++i) {
    if (a[i] !== b[i]) {
      x = a[i]
      y = b[i]
      break
    }
  }

  if (x < y) return -1
  if (y < x) return 1
  return 0
}

Buffer.isEncoding = function isEncoding (encoding) {
  switch (String(encoding).toLowerCase()) {
    case 'hex':
    case 'utf8':
    case 'utf-8':
    case 'ascii':
    case 'latin1':
    case 'binary':
    case 'base64':
    case 'ucs2':
    case 'ucs-2':
    case 'utf16le':
    case 'utf-16le':
      return true
    default:
      return false
  }
}

Buffer.concat = function concat (list, length) {
  if (!isArray(list)) {
    throw new TypeError('"list" argument must be an Array of Buffers')
  }

  if (list.length === 0) {
    return Buffer.alloc(0)
  }

  var i
  if (length === undefined) {
    length = 0
    for (i = 0; i < list.length; ++i) {
      length += list[i].length
    }
  }

  var buffer = Buffer.allocUnsafe(length)
  var pos = 0
  for (i = 0; i < list.length; ++i) {
    var buf = list[i]
    if (!Buffer.isBuffer(buf)) {
      throw new TypeError('"list" argument must be an Array of Buffers')
    }
    buf.copy(buffer, pos)
    pos += buf.length
  }
  return buffer
}

function byteLength (string, encoding) {
  if (Buffer.isBuffer(string)) {
    return string.length
  }
  if (typeof ArrayBuffer !== 'undefined' && typeof ArrayBuffer.isView === 'function' &&
      (ArrayBuffer.isView(string) || string instanceof ArrayBuffer)) {
    return string.byteLength
  }
  if (typeof string !== 'string') {
    string = '' + string
  }

  var len = string.length
  if (len === 0) return 0

  // Use a for loop to avoid recursion
  var loweredCase = false
  for (;;) {
    switch (encoding) {
      case 'ascii':
      case 'latin1':
      case 'binary':
        return len
      case 'utf8':
      case 'utf-8':
      case undefined:
        return utf8ToBytes(string).length
      case 'ucs2':
      case 'ucs-2':
      case 'utf16le':
      case 'utf-16le':
        return len * 2
      case 'hex':
        return len >>> 1
      case 'base64':
        return base64ToBytes(string).length
      default:
        if (loweredCase) return utf8ToBytes(string).length // assume utf8
        encoding = ('' + encoding).toLowerCase()
        loweredCase = true
    }
  }
}
Buffer.byteLength = byteLength

function slowToString (encoding, start, end) {
  var loweredCase = false

  // No need to verify that "this.length <= MAX_UINT32" since it's a read-only
  // property of a typed array.

  // This behaves neither like String nor Uint8Array in that we set start/end
  // to their upper/lower bounds if the value passed is out of range.
  // undefined is handled specially as per ECMA-262 6th Edition,
  // Section 13.3.3.7 Runtime Semantics: KeyedBindingInitialization.
  if (start === undefined || start < 0) {
    start = 0
  }
  // Return early if start > this.length. Done here to prevent potential uint32
  // coercion fail below.
  if (start > this.length) {
    return ''
  }

  if (end === undefined || end > this.length) {
    end = this.length
  }

  if (end <= 0) {
    return ''
  }

  // Force coersion to uint32. This will also coerce falsey/NaN values to 0.
  end >>>= 0
  start >>>= 0

  if (end <= start) {
    return ''
  }

  if (!encoding) encoding = 'utf8'

  while (true) {
    switch (encoding) {
      case 'hex':
        return hexSlice(this, start, end)

      case 'utf8':
      case 'utf-8':
        return utf8Slice(this, start, end)

      case 'ascii':
        return asciiSlice(this, start, end)

      case 'latin1':
      case 'binary':
        return latin1Slice(this, start, end)

      case 'base64':
        return base64Slice(this, start, end)

      case 'ucs2':
      case 'ucs-2':
      case 'utf16le':
      case 'utf-16le':
        return utf16leSlice(this, start, end)

      default:
        if (loweredCase) throw new TypeError('Unknown encoding: ' + encoding)
        encoding = (encoding + '').toLowerCase()
        loweredCase = true
    }
  }
}

// The property is used by `Buffer.isBuffer` and `is-buffer` (in Safari 5-7) to detect
// Buffer instances.
Buffer.prototype._isBuffer = true

function swap (b, n, m) {
  var i = b[n]
  b[n] = b[m]
  b[m] = i
}

Buffer.prototype.swap16 = function swap16 () {
  var len = this.length
  if (len % 2 !== 0) {
    throw new RangeError('Buffer size must be a multiple of 16-bits')
  }
  for (var i = 0; i < len; i += 2) {
    swap(this, i, i + 1)
  }
  return this
}

Buffer.prototype.swap32 = function swap32 () {
  var len = this.length
  if (len % 4 !== 0) {
    throw new RangeError('Buffer size must be a multiple of 32-bits')
  }
  for (var i = 0; i < len; i += 4) {
    swap(this, i, i + 3)
    swap(this, i + 1, i + 2)
  }
  return this
}

Buffer.prototype.swap64 = function swap64 () {
  var len = this.length
  if (len % 8 !== 0) {
    throw new RangeError('Buffer size must be a multiple of 64-bits')
  }
  for (var i = 0; i < len; i += 8) {
    swap(this, i, i + 7)
    swap(this, i + 1, i + 6)
    swap(this, i + 2, i + 5)
    swap(this, i + 3, i + 4)
  }
  return this
}

Buffer.prototype.toString = function toString () {
  var length = this.length | 0
  if (length === 0) return ''
  if (arguments.length === 0) return utf8Slice(this, 0, length)
  return slowToString.apply(this, arguments)
}

Buffer.prototype.equals = function equals (b) {
  if (!Buffer.isBuffer(b)) throw new TypeError('Argument must be a Buffer')
  if (this === b) return true
  return Buffer.compare(this, b) === 0
}

Buffer.prototype.inspect = function inspect () {
  var str = ''
  var max = exports.INSPECT_MAX_BYTES
  if (this.length > 0) {
    str = this.toString('hex', 0, max).match(/.{2}/g).join(' ')
    if (this.length > max) str += ' ... '
  }
  return '<Buffer ' + str + '>'
}

Buffer.prototype.compare = function compare (target, start, end, thisStart, thisEnd) {
  if (!Buffer.isBuffer(target)) {
    throw new TypeError('Argument must be a Buffer')
  }

  if (start === undefined) {
    start = 0
  }
  if (end === undefined) {
    end = target ? target.length : 0
  }
  if (thisStart === undefined) {
    thisStart = 0
  }
  if (thisEnd === undefined) {
    thisEnd = this.length
  }

  if (start < 0 || end > target.length || thisStart < 0 || thisEnd > this.length) {
    throw new RangeError('out of range index')
  }

  if (thisStart >= thisEnd && start >= end) {
    return 0
  }
  if (thisStart >= thisEnd) {
    return -1
  }
  if (start >= end) {
    return 1
  }

  start >>>= 0
  end >>>= 0
  thisStart >>>= 0
  thisEnd >>>= 0

  if (this === target) return 0

  var x = thisEnd - thisStart
  var y = end - start
  var len = Math.min(x, y)

  var thisCopy = this.slice(thisStart, thisEnd)
  var targetCopy = target.slice(start, end)

  for (var i = 0; i < len; ++i) {
    if (thisCopy[i] !== targetCopy[i]) {
      x = thisCopy[i]
      y = targetCopy[i]
      break
    }
  }

  if (x < y) return -1
  if (y < x) return 1
  return 0
}

// Finds either the first index of `val` in `buffer` at offset >= `byteOffset`,
// OR the last index of `val` in `buffer` at offset <= `byteOffset`.
//
// Arguments:
// - buffer - a Buffer to search
// - val - a string, Buffer, or number
// - byteOffset - an index into `buffer`; will be clamped to an int32
// - encoding - an optional encoding, relevant is val is a string
// - dir - true for indexOf, false for lastIndexOf
function bidirectionalIndexOf (buffer, val, byteOffset, encoding, dir) {
  // Empty buffer means no match
  if (buffer.length === 0) return -1

  // Normalize byteOffset
  if (typeof byteOffset === 'string') {
    encoding = byteOffset
    byteOffset = 0
  } else if (byteOffset > 0x7fffffff) {
    byteOffset = 0x7fffffff
  } else if (byteOffset < -0x80000000) {
    byteOffset = -0x80000000
  }
  byteOffset = +byteOffset  // Coerce to Number.
  if (isNaN(byteOffset)) {
    // byteOffset: it it's undefined, null, NaN, "foo", etc, search whole buffer
    byteOffset = dir ? 0 : (buffer.length - 1)
  }

  // Normalize byteOffset: negative offsets start from the end of the buffer
  if (byteOffset < 0) byteOffset = buffer.length + byteOffset
  if (byteOffset >= buffer.length) {
    if (dir) return -1
    else byteOffset = buffer.length - 1
  } else if (byteOffset < 0) {
    if (dir) byteOffset = 0
    else return -1
  }

  // Normalize val
  if (typeof val === 'string') {
    val = Buffer.from(val, encoding)
  }

  // Finally, search either indexOf (if dir is true) or lastIndexOf
  if (Buffer.isBuffer(val)) {
    // Special case: looking for empty string/buffer always fails
    if (val.length === 0) {
      return -1
    }
    return arrayIndexOf(buffer, val, byteOffset, encoding, dir)
  } else if (typeof val === 'number') {
    val = val & 0xFF // Search for a byte value [0-255]
    if (Buffer.TYPED_ARRAY_SUPPORT &&
        typeof Uint8Array.prototype.indexOf === 'function') {
      if (dir) {
        return Uint8Array.prototype.indexOf.call(buffer, val, byteOffset)
      } else {
        return Uint8Array.prototype.lastIndexOf.call(buffer, val, byteOffset)
      }
    }
    return arrayIndexOf(buffer, [ val ], byteOffset, encoding, dir)
  }

  throw new TypeError('val must be string, number or Buffer')
}

function arrayIndexOf (arr, val, byteOffset, encoding, dir) {
  var indexSize = 1
  var arrLength = arr.length
  var valLength = val.length

  if (encoding !== undefined) {
    encoding = String(encoding).toLowerCase()
    if (encoding === 'ucs2' || encoding === 'ucs-2' ||
        encoding === 'utf16le' || encoding === 'utf-16le') {
      if (arr.length < 2 || val.length < 2) {
        return -1
      }
      indexSize = 2
      arrLength /= 2
      valLength /= 2
      byteOffset /= 2
    }
  }

  function read (buf, i) {
    if (indexSize === 1) {
      return buf[i]
    } else {
      return buf.readUInt16BE(i * indexSize)
    }
  }

  var i
  if (dir) {
    var foundIndex = -1
    for (i = byteOffset; i < arrLength; i++) {
      if (read(arr, i) === read(val, foundIndex === -1 ? 0 : i - foundIndex)) {
        if (foundIndex === -1) foundIndex = i
        if (i - foundIndex + 1 === valLength) return foundIndex * indexSize
      } else {
        if (foundIndex !== -1) i -= i - foundIndex
        foundIndex = -1
      }
    }
  } else {
    if (byteOffset + valLength > arrLength) byteOffset = arrLength - valLength
    for (i = byteOffset; i >= 0; i--) {
      var found = true
      for (var j = 0; j < valLength; j++) {
        if (read(arr, i + j) !== read(val, j)) {
          found = false
          break
        }
      }
      if (found) return i
    }
  }

  return -1
}

Buffer.prototype.includes = function includes (val, byteOffset, encoding) {
  return this.indexOf(val, byteOffset, encoding) !== -1
}

Buffer.prototype.indexOf = function indexOf (val, byteOffset, encoding) {
  return bidirectionalIndexOf(this, val, byteOffset, encoding, true)
}

Buffer.prototype.lastIndexOf = function lastIndexOf (val, byteOffset, encoding) {
  return bidirectionalIndexOf(this, val, byteOffset, encoding, false)
}

function hexWrite (buf, string, offset, length) {
  offset = Number(offset) || 0
  var remaining = buf.length - offset
  if (!length) {
    length = remaining
  } else {
    length = Number(length)
    if (length > remaining) {
      length = remaining
    }
  }

  // must be an even number of digits
  var strLen = string.length
  if (strLen % 2 !== 0) throw new TypeError('Invalid hex string')

  if (length > strLen / 2) {
    length = strLen / 2
  }
  for (var i = 0; i < length; ++i) {
    var parsed = parseInt(string.substr(i * 2, 2), 16)
    if (isNaN(parsed)) return i
    buf[offset + i] = parsed
  }
  return i
}

function utf8Write (buf, string, offset, length) {
  return blitBuffer(utf8ToBytes(string, buf.length - offset), buf, offset, length)
}

function asciiWrite (buf, string, offset, length) {
  return blitBuffer(asciiToBytes(string), buf, offset, length)
}

function latin1Write (buf, string, offset, length) {
  return asciiWrite(buf, string, offset, length)
}

function base64Write (buf, string, offset, length) {
  return blitBuffer(base64ToBytes(string), buf, offset, length)
}

function ucs2Write (buf, string, offset, length) {
  return blitBuffer(utf16leToBytes(string, buf.length - offset), buf, offset, length)
}

Buffer.prototype.write = function write (string, offset, length, encoding) {
  // Buffer#write(string)
  if (offset === undefined) {
    encoding = 'utf8'
    length = this.length
    offset = 0
  // Buffer#write(string, encoding)
  } else if (length === undefined && typeof offset === 'string') {
    encoding = offset
    length = this.length
    offset = 0
  // Buffer#write(string, offset[, length][, encoding])
  } else if (isFinite(offset)) {
    offset = offset | 0
    if (isFinite(length)) {
      length = length | 0
      if (encoding === undefined) encoding = 'utf8'
    } else {
      encoding = length
      length = undefined
    }
  // legacy write(string, encoding, offset, length) - remove in v0.13
  } else {
    throw new Error(
      'Buffer.write(string, encoding, offset[, length]) is no longer supported'
    )
  }

  var remaining = this.length - offset
  if (length === undefined || length > remaining) length = remaining

  if ((string.length > 0 && (length < 0 || offset < 0)) || offset > this.length) {
    throw new RangeError('Attempt to write outside buffer bounds')
  }

  if (!encoding) encoding = 'utf8'

  var loweredCase = false
  for (;;) {
    switch (encoding) {
      case 'hex':
        return hexWrite(this, string, offset, length)

      case 'utf8':
      case 'utf-8':
        return utf8Write(this, string, offset, length)

      case 'ascii':
        return asciiWrite(this, string, offset, length)

      case 'latin1':
      case 'binary':
        return latin1Write(this, string, offset, length)

      case 'base64':
        // Warning: maxLength not taken into account in base64Write
        return base64Write(this, string, offset, length)

      case 'ucs2':
      case 'ucs-2':
      case 'utf16le':
      case 'utf-16le':
        return ucs2Write(this, string, offset, length)

      default:
        if (loweredCase) throw new TypeError('Unknown encoding: ' + encoding)
        encoding = ('' + encoding).toLowerCase()
        loweredCase = true
    }
  }
}

Buffer.prototype.toJSON = function toJSON () {
  return {
    type: 'Buffer',
    data: Array.prototype.slice.call(this._arr || this, 0)
  }
}

function base64Slice (buf, start, end) {
  if (start === 0 && end === buf.length) {
    return base64.fromByteArray(buf)
  } else {
    return base64.fromByteArray(buf.slice(start, end))
  }
}

function utf8Slice (buf, start, end) {
  end = Math.min(buf.length, end)
  var res = []

  var i = start
  while (i < end) {
    var firstByte = buf[i]
    var codePoint = null
    var bytesPerSequence = (firstByte > 0xEF) ? 4
      : (firstByte > 0xDF) ? 3
      : (firstByte > 0xBF) ? 2
      : 1

    if (i + bytesPerSequence <= end) {
      var secondByte, thirdByte, fourthByte, tempCodePoint

      switch (bytesPerSequence) {
        case 1:
          if (firstByte < 0x80) {
            codePoint = firstByte
          }
          break
        case 2:
          secondByte = buf[i + 1]
          if ((secondByte & 0xC0) === 0x80) {
            tempCodePoint = (firstByte & 0x1F) << 0x6 | (secondByte & 0x3F)
            if (tempCodePoint > 0x7F) {
              codePoint = tempCodePoint
            }
          }
          break
        case 3:
          secondByte = buf[i + 1]
          thirdByte = buf[i + 2]
          if ((secondByte & 0xC0) === 0x80 && (thirdByte & 0xC0) === 0x80) {
            tempCodePoint = (firstByte & 0xF) << 0xC | (secondByte & 0x3F) << 0x6 | (thirdByte & 0x3F)
            if (tempCodePoint > 0x7FF && (tempCodePoint < 0xD800 || tempCodePoint > 0xDFFF)) {
              codePoint = tempCodePoint
            }
          }
          break
        case 4:
          secondByte = buf[i + 1]
          thirdByte = buf[i + 2]
          fourthByte = buf[i + 3]
          if ((secondByte & 0xC0) === 0x80 && (thirdByte & 0xC0) === 0x80 && (fourthByte & 0xC0) === 0x80) {
            tempCodePoint = (firstByte & 0xF) << 0x12 | (secondByte & 0x3F) << 0xC | (thirdByte & 0x3F) << 0x6 | (fourthByte & 0x3F)
            if (tempCodePoint > 0xFFFF && tempCodePoint < 0x110000) {
              codePoint = tempCodePoint
            }
          }
      }
    }

    if (codePoint === null) {
      // we did not generate a valid codePoint so insert a
      // replacement char (U+FFFD) and advance only 1 byte
      codePoint = 0xFFFD
      bytesPerSequence = 1
    } else if (codePoint > 0xFFFF) {
      // encode to utf16 (surrogate pair dance)
      codePoint -= 0x10000
      res.push(codePoint >>> 10 & 0x3FF | 0xD800)
      codePoint = 0xDC00 | codePoint & 0x3FF
    }

    res.push(codePoint)
    i += bytesPerSequence
  }

  return decodeCodePointsArray(res)
}

// Based on http://stackoverflow.com/a/22747272/680742, the browser with
// the lowest limit is Chrome, with 0x10000 args.
// We go 1 magnitude less, for safety
var MAX_ARGUMENTS_LENGTH = 0x1000

function decodeCodePointsArray (codePoints) {
  var len = codePoints.length
  if (len <= MAX_ARGUMENTS_LENGTH) {
    return String.fromCharCode.apply(String, codePoints) // avoid extra slice()
  }

  // Decode in chunks to avoid "call stack size exceeded".
  var res = ''
  var i = 0
  while (i < len) {
    res += String.fromCharCode.apply(
      String,
      codePoints.slice(i, i += MAX_ARGUMENTS_LENGTH)
    )
  }
  return res
}

function asciiSlice (buf, start, end) {
  var ret = ''
  end = Math.min(buf.length, end)

  for (var i = start; i < end; ++i) {
    ret += String.fromCharCode(buf[i] & 0x7F)
  }
  return ret
}

function latin1Slice (buf, start, end) {
  var ret = ''
  end = Math.min(buf.length, end)

  for (var i = start; i < end; ++i) {
    ret += String.fromCharCode(buf[i])
  }
  return ret
}

function hexSlice (buf, start, end) {
  var len = buf.length

  if (!start || start < 0) start = 0
  if (!end || end < 0 || end > len) end = len

  var out = ''
  for (var i = start; i < end; ++i) {
    out += toHex(buf[i])
  }
  return out
}

function utf16leSlice (buf, start, end) {
  var bytes = buf.slice(start, end)
  var res = ''
  for (var i = 0; i < bytes.length; i += 2) {
    res += String.fromCharCode(bytes[i] + bytes[i + 1] * 256)
  }
  return res
}

Buffer.prototype.slice = function slice (start, end) {
  var len = this.length
  start = ~~start
  end = end === undefined ? len : ~~end

  if (start < 0) {
    start += len
    if (start < 0) start = 0
  } else if (start > len) {
    start = len
  }

  if (end < 0) {
    end += len
    if (end < 0) end = 0
  } else if (end > len) {
    end = len
  }

  if (end < start) end = start

  var newBuf
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    newBuf = this.subarray(start, end)
    newBuf.__proto__ = Buffer.prototype
  } else {
    var sliceLen = end - start
    newBuf = new Buffer(sliceLen, undefined)
    for (var i = 0; i < sliceLen; ++i) {
      newBuf[i] = this[i + start]
    }
  }

  return newBuf
}

/*
 * Need to make sure that buffer isn't trying to write out of bounds.
 */
function checkOffset (offset, ext, length) {
  if ((offset % 1) !== 0 || offset < 0) throw new RangeError('offset is not uint')
  if (offset + ext > length) throw new RangeError('Trying to access beyond buffer length')
}

Buffer.prototype.readUIntLE = function readUIntLE (offset, byteLength, noAssert) {
  offset = offset | 0
  byteLength = byteLength | 0
  if (!noAssert) checkOffset(offset, byteLength, this.length)

  var val = this[offset]
  var mul = 1
  var i = 0
  while (++i < byteLength && (mul *= 0x100)) {
    val += this[offset + i] * mul
  }

  return val
}

Buffer.prototype.readUIntBE = function readUIntBE (offset, byteLength, noAssert) {
  offset = offset | 0
  byteLength = byteLength | 0
  if (!noAssert) {
    checkOffset(offset, byteLength, this.length)
  }

  var val = this[offset + --byteLength]
  var mul = 1
  while (byteLength > 0 && (mul *= 0x100)) {
    val += this[offset + --byteLength] * mul
  }

  return val
}

Buffer.prototype.readUInt8 = function readUInt8 (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 1, this.length)
  return this[offset]
}

Buffer.prototype.readUInt16LE = function readUInt16LE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 2, this.length)
  return this[offset] | (this[offset + 1] << 8)
}

Buffer.prototype.readUInt16BE = function readUInt16BE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 2, this.length)
  return (this[offset] << 8) | this[offset + 1]
}

Buffer.prototype.readUInt32LE = function readUInt32LE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 4, this.length)

  return ((this[offset]) |
      (this[offset + 1] << 8) |
      (this[offset + 2] << 16)) +
      (this[offset + 3] * 0x1000000)
}

Buffer.prototype.readUInt32BE = function readUInt32BE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 4, this.length)

  return (this[offset] * 0x1000000) +
    ((this[offset + 1] << 16) |
    (this[offset + 2] << 8) |
    this[offset + 3])
}

Buffer.prototype.readIntLE = function readIntLE (offset, byteLength, noAssert) {
  offset = offset | 0
  byteLength = byteLength | 0
  if (!noAssert) checkOffset(offset, byteLength, this.length)

  var val = this[offset]
  var mul = 1
  var i = 0
  while (++i < byteLength && (mul *= 0x100)) {
    val += this[offset + i] * mul
  }
  mul *= 0x80

  if (val >= mul) val -= Math.pow(2, 8 * byteLength)

  return val
}

Buffer.prototype.readIntBE = function readIntBE (offset, byteLength, noAssert) {
  offset = offset | 0
  byteLength = byteLength | 0
  if (!noAssert) checkOffset(offset, byteLength, this.length)

  var i = byteLength
  var mul = 1
  var val = this[offset + --i]
  while (i > 0 && (mul *= 0x100)) {
    val += this[offset + --i] * mul
  }
  mul *= 0x80

  if (val >= mul) val -= Math.pow(2, 8 * byteLength)

  return val
}

Buffer.prototype.readInt8 = function readInt8 (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 1, this.length)
  if (!(this[offset] & 0x80)) return (this[offset])
  return ((0xff - this[offset] + 1) * -1)
}

Buffer.prototype.readInt16LE = function readInt16LE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 2, this.length)
  var val = this[offset] | (this[offset + 1] << 8)
  return (val & 0x8000) ? val | 0xFFFF0000 : val
}

Buffer.prototype.readInt16BE = function readInt16BE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 2, this.length)
  var val = this[offset + 1] | (this[offset] << 8)
  return (val & 0x8000) ? val | 0xFFFF0000 : val
}

Buffer.prototype.readInt32LE = function readInt32LE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 4, this.length)

  return (this[offset]) |
    (this[offset + 1] << 8) |
    (this[offset + 2] << 16) |
    (this[offset + 3] << 24)
}

Buffer.prototype.readInt32BE = function readInt32BE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 4, this.length)

  return (this[offset] << 24) |
    (this[offset + 1] << 16) |
    (this[offset + 2] << 8) |
    (this[offset + 3])
}

Buffer.prototype.readFloatLE = function readFloatLE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 4, this.length)
  return ieee754.read(this, offset, true, 23, 4)
}

Buffer.prototype.readFloatBE = function readFloatBE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 4, this.length)
  return ieee754.read(this, offset, false, 23, 4)
}

Buffer.prototype.readDoubleLE = function readDoubleLE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 8, this.length)
  return ieee754.read(this, offset, true, 52, 8)
}

Buffer.prototype.readDoubleBE = function readDoubleBE (offset, noAssert) {
  if (!noAssert) checkOffset(offset, 8, this.length)
  return ieee754.read(this, offset, false, 52, 8)
}

function checkInt (buf, value, offset, ext, max, min) {
  if (!Buffer.isBuffer(buf)) throw new TypeError('"buffer" argument must be a Buffer instance')
  if (value > max || value < min) throw new RangeError('"value" argument is out of bounds')
  if (offset + ext > buf.length) throw new RangeError('Index out of range')
}

Buffer.prototype.writeUIntLE = function writeUIntLE (value, offset, byteLength, noAssert) {
  value = +value
  offset = offset | 0
  byteLength = byteLength | 0
  if (!noAssert) {
    var maxBytes = Math.pow(2, 8 * byteLength) - 1
    checkInt(this, value, offset, byteLength, maxBytes, 0)
  }

  var mul = 1
  var i = 0
  this[offset] = value & 0xFF
  while (++i < byteLength && (mul *= 0x100)) {
    this[offset + i] = (value / mul) & 0xFF
  }

  return offset + byteLength
}

Buffer.prototype.writeUIntBE = function writeUIntBE (value, offset, byteLength, noAssert) {
  value = +value
  offset = offset | 0
  byteLength = byteLength | 0
  if (!noAssert) {
    var maxBytes = Math.pow(2, 8 * byteLength) - 1
    checkInt(this, value, offset, byteLength, maxBytes, 0)
  }

  var i = byteLength - 1
  var mul = 1
  this[offset + i] = value & 0xFF
  while (--i >= 0 && (mul *= 0x100)) {
    this[offset + i] = (value / mul) & 0xFF
  }

  return offset + byteLength
}

Buffer.prototype.writeUInt8 = function writeUInt8 (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 1, 0xff, 0)
  if (!Buffer.TYPED_ARRAY_SUPPORT) value = Math.floor(value)
  this[offset] = (value & 0xff)
  return offset + 1
}

function objectWriteUInt16 (buf, value, offset, littleEndian) {
  if (value < 0) value = 0xffff + value + 1
  for (var i = 0, j = Math.min(buf.length - offset, 2); i < j; ++i) {
    buf[offset + i] = (value & (0xff << (8 * (littleEndian ? i : 1 - i)))) >>>
      (littleEndian ? i : 1 - i) * 8
  }
}

Buffer.prototype.writeUInt16LE = function writeUInt16LE (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 2, 0xffff, 0)
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    this[offset] = (value & 0xff)
    this[offset + 1] = (value >>> 8)
  } else {
    objectWriteUInt16(this, value, offset, true)
  }
  return offset + 2
}

Buffer.prototype.writeUInt16BE = function writeUInt16BE (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 2, 0xffff, 0)
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    this[offset] = (value >>> 8)
    this[offset + 1] = (value & 0xff)
  } else {
    objectWriteUInt16(this, value, offset, false)
  }
  return offset + 2
}

function objectWriteUInt32 (buf, value, offset, littleEndian) {
  if (value < 0) value = 0xffffffff + value + 1
  for (var i = 0, j = Math.min(buf.length - offset, 4); i < j; ++i) {
    buf[offset + i] = (value >>> (littleEndian ? i : 3 - i) * 8) & 0xff
  }
}

Buffer.prototype.writeUInt32LE = function writeUInt32LE (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 4, 0xffffffff, 0)
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    this[offset + 3] = (value >>> 24)
    this[offset + 2] = (value >>> 16)
    this[offset + 1] = (value >>> 8)
    this[offset] = (value & 0xff)
  } else {
    objectWriteUInt32(this, value, offset, true)
  }
  return offset + 4
}

Buffer.prototype.writeUInt32BE = function writeUInt32BE (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 4, 0xffffffff, 0)
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    this[offset] = (value >>> 24)
    this[offset + 1] = (value >>> 16)
    this[offset + 2] = (value >>> 8)
    this[offset + 3] = (value & 0xff)
  } else {
    objectWriteUInt32(this, value, offset, false)
  }
  return offset + 4
}

Buffer.prototype.writeIntLE = function writeIntLE (value, offset, byteLength, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) {
    var limit = Math.pow(2, 8 * byteLength - 1)

    checkInt(this, value, offset, byteLength, limit - 1, -limit)
  }

  var i = 0
  var mul = 1
  var sub = 0
  this[offset] = value & 0xFF
  while (++i < byteLength && (mul *= 0x100)) {
    if (value < 0 && sub === 0 && this[offset + i - 1] !== 0) {
      sub = 1
    }
    this[offset + i] = ((value / mul) >> 0) - sub & 0xFF
  }

  return offset + byteLength
}

Buffer.prototype.writeIntBE = function writeIntBE (value, offset, byteLength, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) {
    var limit = Math.pow(2, 8 * byteLength - 1)

    checkInt(this, value, offset, byteLength, limit - 1, -limit)
  }

  var i = byteLength - 1
  var mul = 1
  var sub = 0
  this[offset + i] = value & 0xFF
  while (--i >= 0 && (mul *= 0x100)) {
    if (value < 0 && sub === 0 && this[offset + i + 1] !== 0) {
      sub = 1
    }
    this[offset + i] = ((value / mul) >> 0) - sub & 0xFF
  }

  return offset + byteLength
}

Buffer.prototype.writeInt8 = function writeInt8 (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 1, 0x7f, -0x80)
  if (!Buffer.TYPED_ARRAY_SUPPORT) value = Math.floor(value)
  if (value < 0) value = 0xff + value + 1
  this[offset] = (value & 0xff)
  return offset + 1
}

Buffer.prototype.writeInt16LE = function writeInt16LE (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 2, 0x7fff, -0x8000)
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    this[offset] = (value & 0xff)
    this[offset + 1] = (value >>> 8)
  } else {
    objectWriteUInt16(this, value, offset, true)
  }
  return offset + 2
}

Buffer.prototype.writeInt16BE = function writeInt16BE (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 2, 0x7fff, -0x8000)
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    this[offset] = (value >>> 8)
    this[offset + 1] = (value & 0xff)
  } else {
    objectWriteUInt16(this, value, offset, false)
  }
  return offset + 2
}

Buffer.prototype.writeInt32LE = function writeInt32LE (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 4, 0x7fffffff, -0x80000000)
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    this[offset] = (value & 0xff)
    this[offset + 1] = (value >>> 8)
    this[offset + 2] = (value >>> 16)
    this[offset + 3] = (value >>> 24)
  } else {
    objectWriteUInt32(this, value, offset, true)
  }
  return offset + 4
}

Buffer.prototype.writeInt32BE = function writeInt32BE (value, offset, noAssert) {
  value = +value
  offset = offset | 0
  if (!noAssert) checkInt(this, value, offset, 4, 0x7fffffff, -0x80000000)
  if (value < 0) value = 0xffffffff + value + 1
  if (Buffer.TYPED_ARRAY_SUPPORT) {
    this[offset] = (value >>> 24)
    this[offset + 1] = (value >>> 16)
    this[offset + 2] = (value >>> 8)
    this[offset + 3] = (value & 0xff)
  } else {
    objectWriteUInt32(this, value, offset, false)
  }
  return offset + 4
}

function checkIEEE754 (buf, value, offset, ext, max, min) {
  if (offset + ext > buf.length) throw new RangeError('Index out of range')
  if (offset < 0) throw new RangeError('Index out of range')
}

function writeFloat (buf, value, offset, littleEndian, noAssert) {
  if (!noAssert) {
    checkIEEE754(buf, value, offset, 4, 3.4028234663852886e+38, -3.4028234663852886e+38)
  }
  ieee754.write(buf, value, offset, littleEndian, 23, 4)
  return offset + 4
}

Buffer.prototype.writeFloatLE = function writeFloatLE (value, offset, noAssert) {
  return writeFloat(this, value, offset, true, noAssert)
}

Buffer.prototype.writeFloatBE = function writeFloatBE (value, offset, noAssert) {
  return writeFloat(this, value, offset, false, noAssert)
}

function writeDouble (buf, value, offset, littleEndian, noAssert) {
  if (!noAssert) {
    checkIEEE754(buf, value, offset, 8, 1.7976931348623157E+308, -1.7976931348623157E+308)
  }
  ieee754.write(buf, value, offset, littleEndian, 52, 8)
  return offset + 8
}

Buffer.prototype.writeDoubleLE = function writeDoubleLE (value, offset, noAssert) {
  return writeDouble(this, value, offset, true, noAssert)
}

Buffer.prototype.writeDoubleBE = function writeDoubleBE (value, offset, noAssert) {
  return writeDouble(this, value, offset, false, noAssert)
}

// copy(targetBuffer, targetStart=0, sourceStart=0, sourceEnd=buffer.length)
Buffer.prototype.copy = function copy (target, targetStart, start, end) {
  if (!start) start = 0
  if (!end && end !== 0) end = this.length
  if (targetStart >= target.length) targetStart = target.length
  if (!targetStart) targetStart = 0
  if (end > 0 && end < start) end = start

  // Copy 0 bytes; we're done
  if (end === start) return 0
  if (target.length === 0 || this.length === 0) return 0

  // Fatal error conditions
  if (targetStart < 0) {
    throw new RangeError('targetStart out of bounds')
  }
  if (start < 0 || start >= this.length) throw new RangeError('sourceStart out of bounds')
  if (end < 0) throw new RangeError('sourceEnd out of bounds')

  // Are we oob?
  if (end > this.length) end = this.length
  if (target.length - targetStart < end - start) {
    end = target.length - targetStart + start
  }

  var len = end - start
  var i

  if (this === target && start < targetStart && targetStart < end) {
    // descending copy from end
    for (i = len - 1; i >= 0; --i) {
      target[i + targetStart] = this[i + start]
    }
  } else if (len < 1000 || !Buffer.TYPED_ARRAY_SUPPORT) {
    // ascending copy from start
    for (i = 0; i < len; ++i) {
      target[i + targetStart] = this[i + start]
    }
  } else {
    Uint8Array.prototype.set.call(
      target,
      this.subarray(start, start + len),
      targetStart
    )
  }

  return len
}

// Usage:
//    buffer.fill(number[, offset[, end]])
//    buffer.fill(buffer[, offset[, end]])
//    buffer.fill(string[, offset[, end]][, encoding])
Buffer.prototype.fill = function fill (val, start, end, encoding) {
  // Handle string cases:
  if (typeof val === 'string') {
    if (typeof start === 'string') {
      encoding = start
      start = 0
      end = this.length
    } else if (typeof end === 'string') {
      encoding = end
      end = this.length
    }
    if (val.length === 1) {
      var code = val.charCodeAt(0)
      if (code < 256) {
        val = code
      }
    }
    if (encoding !== undefined && typeof encoding !== 'string') {
      throw new TypeError('encoding must be a string')
    }
    if (typeof encoding === 'string' && !Buffer.isEncoding(encoding)) {
      throw new TypeError('Unknown encoding: ' + encoding)
    }
  } else if (typeof val === 'number') {
    val = val & 255
  }

  // Invalid ranges are not set to a default, so can range check early.
  if (start < 0 || this.length < start || this.length < end) {
    throw new RangeError('Out of range index')
  }

  if (end <= start) {
    return this
  }

  start = start >>> 0
  end = end === undefined ? this.length : end >>> 0

  if (!val) val = 0

  var i
  if (typeof val === 'number') {
    for (i = start; i < end; ++i) {
      this[i] = val
    }
  } else {
    var bytes = Buffer.isBuffer(val)
      ? val
      : utf8ToBytes(new Buffer(val, encoding).toString())
    var len = bytes.length
    for (i = 0; i < end - start; ++i) {
      this[i + start] = bytes[i % len]
    }
  }

  return this
}

// HELPER FUNCTIONS
// ================

var INVALID_BASE64_RE = /[^+\/0-9A-Za-z-_]/g

function base64clean (str) {
  // Node strips out invalid characters like \n and \t from the string, base64-js does not
  str = stringtrim(str).replace(INVALID_BASE64_RE, '')
  // Node converts strings with length < 2 to ''
  if (str.length < 2) return ''
  // Node allows for non-padded base64 strings (missing trailing ===), base64-js does not
  while (str.length % 4 !== 0) {
    str = str + '='
  }
  return str
}

function stringtrim (str) {
  if (str.trim) return str.trim()
  return str.replace(/^\s+|\s+$/g, '')
}

function toHex (n) {
  if (n < 16) return '0' + n.toString(16)
  return n.toString(16)
}

function utf8ToBytes (string, units) {
  units = units || Infinity
  var codePoint
  var length = string.length
  var leadSurrogate = null
  var bytes = []

  for (var i = 0; i < length; ++i) {
    codePoint = string.charCodeAt(i)

    // is surrogate component
    if (codePoint > 0xD7FF && codePoint < 0xE000) {
      // last char was a lead
      if (!leadSurrogate) {
        // no lead yet
        if (codePoint > 0xDBFF) {
          // unexpected trail
          if ((units -= 3) > -1) bytes.push(0xEF, 0xBF, 0xBD)
          continue
        } else if (i + 1 === length) {
          // unpaired lead
          if ((units -= 3) > -1) bytes.push(0xEF, 0xBF, 0xBD)
          continue
        }

        // valid lead
        leadSurrogate = codePoint

        continue
      }

      // 2 leads in a row
      if (codePoint < 0xDC00) {
        if ((units -= 3) > -1) bytes.push(0xEF, 0xBF, 0xBD)
        leadSurrogate = codePoint
        continue
      }

      // valid surrogate pair
      codePoint = (leadSurrogate - 0xD800 << 10 | codePoint - 0xDC00) + 0x10000
    } else if (leadSurrogate) {
      // valid bmp char, but last char was a lead
      if ((units -= 3) > -1) bytes.push(0xEF, 0xBF, 0xBD)
    }

    leadSurrogate = null

    // encode utf8
    if (codePoint < 0x80) {
      if ((units -= 1) < 0) break
      bytes.push(codePoint)
    } else if (codePoint < 0x800) {
      if ((units -= 2) < 0) break
      bytes.push(
        codePoint >> 0x6 | 0xC0,
        codePoint & 0x3F | 0x80
      )
    } else if (codePoint < 0x10000) {
      if ((units -= 3) < 0) break
      bytes.push(
        codePoint >> 0xC | 0xE0,
        codePoint >> 0x6 & 0x3F | 0x80,
        codePoint & 0x3F | 0x80
      )
    } else if (codePoint < 0x110000) {
      if ((units -= 4) < 0) break
      bytes.push(
        codePoint >> 0x12 | 0xF0,
        codePoint >> 0xC & 0x3F | 0x80,
        codePoint >> 0x6 & 0x3F | 0x80,
        codePoint & 0x3F | 0x80
      )
    } else {
      throw new Error('Invalid code point')
    }
  }

  return bytes
}

function asciiToBytes (str) {
  var byteArray = []
  for (var i = 0; i < str.length; ++i) {
    // Node's code seems to be doing this and not & 0x7F..
    byteArray.push(str.charCodeAt(i) & 0xFF)
  }
  return byteArray
}

function utf16leToBytes (str, units) {
  var c, hi, lo
  var byteArray = []
  for (var i = 0; i < str.length; ++i) {
    if ((units -= 2) < 0) break

    c = str.charCodeAt(i)
    hi = c >> 8
    lo = c % 256
    byteArray.push(lo)
    byteArray.push(hi)
  }

  return byteArray
}

function base64ToBytes (str) {
  return base64.toByteArray(base64clean(str))
}

function blitBuffer (src, dst, offset, length) {
  for (var i = 0; i < length; ++i) {
    if ((i + offset >= dst.length) || (i >= src.length)) break
    dst[i + offset] = src[i]
  }
  return i
}

function isnan (val) {
  return val !== val // eslint-disable-line no-self-compare
}

}).call(this,typeof global !== "undefined" ? global : typeof self !== "undefined" ? self : typeof window !== "undefined" ? window : {})

},{"base64-js":1,"ieee754":6,"isarray":8}],6:[function(require,module,exports){
exports.read = function (buffer, offset, isLE, mLen, nBytes) {
  var e, m
  var eLen = nBytes * 8 - mLen - 1
  var eMax = (1 << eLen) - 1
  var eBias = eMax >> 1
  var nBits = -7
  var i = isLE ? (nBytes - 1) : 0
  var d = isLE ? -1 : 1
  var s = buffer[offset + i]

  i += d

  e = s & ((1 << (-nBits)) - 1)
  s >>= (-nBits)
  nBits += eLen
  for (; nBits > 0; e = e * 256 + buffer[offset + i], i += d, nBits -= 8) {}

  m = e & ((1 << (-nBits)) - 1)
  e >>= (-nBits)
  nBits += mLen
  for (; nBits > 0; m = m * 256 + buffer[offset + i], i += d, nBits -= 8) {}

  if (e === 0) {
    e = 1 - eBias
  } else if (e === eMax) {
    return m ? NaN : ((s ? -1 : 1) * Infinity)
  } else {
    m = m + Math.pow(2, mLen)
    e = e - eBias
  }
  return (s ? -1 : 1) * m * Math.pow(2, e - mLen)
}

exports.write = function (buffer, value, offset, isLE, mLen, nBytes) {
  var e, m, c
  var eLen = nBytes * 8 - mLen - 1
  var eMax = (1 << eLen) - 1
  var eBias = eMax >> 1
  var rt = (mLen === 23 ? Math.pow(2, -24) - Math.pow(2, -77) : 0)
  var i = isLE ? 0 : (nBytes - 1)
  var d = isLE ? 1 : -1
  var s = value < 0 || (value === 0 && 1 / value < 0) ? 1 : 0

  value = Math.abs(value)

  if (isNaN(value) || value === Infinity) {
    m = isNaN(value) ? 1 : 0
    e = eMax
  } else {
    e = Math.floor(Math.log(value) / Math.LN2)
    if (value * (c = Math.pow(2, -e)) < 1) {
      e--
      c *= 2
    }
    if (e + eBias >= 1) {
      value += rt / c
    } else {
      value += rt * Math.pow(2, 1 - eBias)
    }
    if (value * c >= 2) {
      e++
      c /= 2
    }

    if (e + eBias >= eMax) {
      m = 0
      e = eMax
    } else if (e + eBias >= 1) {
      m = (value * c - 1) * Math.pow(2, mLen)
      e = e + eBias
    } else {
      m = value * Math.pow(2, eBias - 1) * Math.pow(2, mLen)
      e = 0
    }
  }

  for (; mLen >= 8; buffer[offset + i] = m & 0xff, i += d, m /= 256, mLen -= 8) {}

  e = (e << mLen) | m
  eLen += mLen
  for (; eLen > 0; buffer[offset + i] = e & 0xff, i += d, e /= 256, eLen -= 8) {}

  buffer[offset + i - d] |= s * 128
}

},{}],7:[function(require,module,exports){
if (typeof Object.create === 'function') {
  // implementation from standard node.js 'util' module
  module.exports = function inherits(ctor, superCtor) {
    ctor.super_ = superCtor
    ctor.prototype = Object.create(superCtor.prototype, {
      constructor: {
        value: ctor,
        enumerable: false,
        writable: true,
        configurable: true
      }
    });
  };
} else {
  // old school shim for old browsers
  module.exports = function inherits(ctor, superCtor) {
    ctor.super_ = superCtor
    var TempCtor = function () {}
    TempCtor.prototype = superCtor.prototype
    ctor.prototype = new TempCtor()
    ctor.prototype.constructor = ctor
  }
}

},{}],8:[function(require,module,exports){
var toString = {}.toString;

module.exports = Array.isArray || function (arr) {
  return toString.call(arr) == '[object Array]';
};

},{}],9:[function(require,module,exports){
/*
object-assign
(c) Sindre Sorhus
@license MIT
*/

'use strict';
/* eslint-disable no-unused-vars */
var getOwnPropertySymbols = Object.getOwnPropertySymbols;
var hasOwnProperty = Object.prototype.hasOwnProperty;
var propIsEnumerable = Object.prototype.propertyIsEnumerable;

function toObject(val) {
	if (val === null || val === undefined) {
		throw new TypeError('Object.assign cannot be called with null or undefined');
	}

	return Object(val);
}

function shouldUseNative() {
	try {
		if (!Object.assign) {
			return false;
		}

		// Detect buggy property enumeration order in older V8 versions.

		// https://bugs.chromium.org/p/v8/issues/detail?id=4118
		var test1 = new String('abc');  // eslint-disable-line no-new-wrappers
		test1[5] = 'de';
		if (Object.getOwnPropertyNames(test1)[0] === '5') {
			return false;
		}

		// https://bugs.chromium.org/p/v8/issues/detail?id=3056
		var test2 = {};
		for (var i = 0; i < 10; i++) {
			test2['_' + String.fromCharCode(i)] = i;
		}
		var order2 = Object.getOwnPropertyNames(test2).map(function (n) {
			return test2[n];
		});
		if (order2.join('') !== '0123456789') {
			return false;
		}

		// https://bugs.chromium.org/p/v8/issues/detail?id=3056
		var test3 = {};
		'abcdefghijklmnopqrst'.split('').forEach(function (letter) {
			test3[letter] = letter;
		});
		if (Object.keys(Object.assign({}, test3)).join('') !==
				'abcdefghijklmnopqrst') {
			return false;
		}

		return true;
	} catch (err) {
		// We don't expect any of the above to throw, but better to be safe.
		return false;
	}
}

module.exports = shouldUseNative() ? Object.assign : function (target, source) {
	var from;
	var to = toObject(target);
	var symbols;

	for (var s = 1; s < arguments.length; s++) {
		from = Object(arguments[s]);

		for (var key in from) {
			if (hasOwnProperty.call(from, key)) {
				to[key] = from[key];
			}
		}

		if (getOwnPropertySymbols) {
			symbols = getOwnPropertySymbols(from);
			for (var i = 0; i < symbols.length; i++) {
				if (propIsEnumerable.call(from, symbols[i])) {
					to[symbols[i]] = from[symbols[i]];
				}
			}
		}
	}

	return to;
};

},{}],10:[function(require,module,exports){
(function (Buffer){
// prototype class for hash functions
function Hash (blockSize, finalSize) {
  this._block = new Buffer(blockSize)
  this._finalSize = finalSize
  this._blockSize = blockSize
  this._len = 0
  this._s = 0
}

Hash.prototype.update = function (data, enc) {
  if (typeof data === 'string') {
    enc = enc || 'utf8'
    data = new Buffer(data, enc)
  }

  var l = this._len += data.length
  var s = this._s || 0
  var f = 0
  var buffer = this._block

  while (s < l) {
    var t = Math.min(data.length, f + this._blockSize - (s % this._blockSize))
    var ch = (t - f)

    for (var i = 0; i < ch; i++) {
      buffer[(s % this._blockSize) + i] = data[i + f]
    }

    s += ch
    f += ch

    if ((s % this._blockSize) === 0) {
      this._update(buffer)
    }
  }
  this._s = s

  return this
}

Hash.prototype.digest = function (enc) {
  // Suppose the length of the message M, in bits, is l
  var l = this._len * 8

  // Append the bit 1 to the end of the message
  this._block[this._len % this._blockSize] = 0x80

  // and then k zero bits, where k is the smallest non-negative solution to the equation (l + 1 + k) === finalSize mod blockSize
  this._block.fill(0, this._len % this._blockSize + 1)

  if (l % (this._blockSize * 8) >= this._finalSize * 8) {
    this._update(this._block)
    this._block.fill(0)
  }

  // to this append the block which is equal to the number l written in binary
  // TODO: handle case where l is > Math.pow(2, 29)
  this._block.writeInt32BE(l, this._blockSize - 4)

  var hash = this._update(this._block) || this._hash()

  return enc ? hash.toString(enc) : hash
}

Hash.prototype._update = function () {
  throw new Error('_update must be implemented by subclass')
}

module.exports = Hash

}).call(this,require("buffer").Buffer)

},{"buffer":5}],11:[function(require,module,exports){
var exports = module.exports = function SHA (algorithm) {
  algorithm = algorithm.toLowerCase()

  var Algorithm = exports[algorithm]
  if (!Algorithm) throw new Error(algorithm + ' is not supported (we accept pull requests)')

  return new Algorithm()
}

exports.sha = require('./sha')
exports.sha1 = require('./sha1')
exports.sha224 = require('./sha224')
exports.sha256 = require('./sha256')
exports.sha384 = require('./sha384')
exports.sha512 = require('./sha512')

},{"./sha":12,"./sha1":13,"./sha224":14,"./sha256":15,"./sha384":16,"./sha512":17}],12:[function(require,module,exports){
(function (Buffer){
/*
 * A JavaScript implementation of the Secure Hash Algorithm, SHA-0, as defined
 * in FIPS PUB 180-1
 * This source code is derived from sha1.js of the same repository.
 * The difference between SHA-0 and SHA-1 is just a bitwise rotate left
 * operation was added.
 */

var inherits = require('inherits')
var Hash = require('./hash')

var K = [
  0x5a827999, 0x6ed9eba1, 0x8f1bbcdc | 0, 0xca62c1d6 | 0
]

var W = new Array(80)

function Sha () {
  this.init()
  this._w = W

  Hash.call(this, 64, 56)
}

inherits(Sha, Hash)

Sha.prototype.init = function () {
  this._a = 0x67452301
  this._b = 0xefcdab89
  this._c = 0x98badcfe
  this._d = 0x10325476
  this._e = 0xc3d2e1f0

  return this
}

function rotl5 (num) {
  return (num << 5) | (num >>> 27)
}

function rotl30 (num) {
  return (num << 30) | (num >>> 2)
}

function ft (s, b, c, d) {
  if (s === 0) return (b & c) | ((~b) & d)
  if (s === 2) return (b & c) | (b & d) | (c & d)
  return b ^ c ^ d
}

Sha.prototype._update = function (M) {
  var W = this._w

  var a = this._a | 0
  var b = this._b | 0
  var c = this._c | 0
  var d = this._d | 0
  var e = this._e | 0

  for (var i = 0; i < 16; ++i) W[i] = M.readInt32BE(i * 4)
  for (; i < 80; ++i) W[i] = W[i - 3] ^ W[i - 8] ^ W[i - 14] ^ W[i - 16]

  for (var j = 0; j < 80; ++j) {
    var s = ~~(j / 20)
    var t = (rotl5(a) + ft(s, b, c, d) + e + W[j] + K[s]) | 0

    e = d
    d = c
    c = rotl30(b)
    b = a
    a = t
  }

  this._a = (a + this._a) | 0
  this._b = (b + this._b) | 0
  this._c = (c + this._c) | 0
  this._d = (d + this._d) | 0
  this._e = (e + this._e) | 0
}

Sha.prototype._hash = function () {
  var H = new Buffer(20)

  H.writeInt32BE(this._a | 0, 0)
  H.writeInt32BE(this._b | 0, 4)
  H.writeInt32BE(this._c | 0, 8)
  H.writeInt32BE(this._d | 0, 12)
  H.writeInt32BE(this._e | 0, 16)

  return H
}

module.exports = Sha

}).call(this,require("buffer").Buffer)

},{"./hash":10,"buffer":5,"inherits":7}],13:[function(require,module,exports){
(function (Buffer){
/*
 * A JavaScript implementation of the Secure Hash Algorithm, SHA-1, as defined
 * in FIPS PUB 180-1
 * Version 2.1a Copyright Paul Johnston 2000 - 2002.
 * Other contributors: Greg Holt, Andrew Kepert, Ydnar, Lostinet
 * Distributed under the BSD License
 * See http://pajhome.org.uk/crypt/md5 for details.
 */

var inherits = require('inherits')
var Hash = require('./hash')

var K = [
  0x5a827999, 0x6ed9eba1, 0x8f1bbcdc | 0, 0xca62c1d6 | 0
]

var W = new Array(80)

function Sha1 () {
  this.init()
  this._w = W

  Hash.call(this, 64, 56)
}

inherits(Sha1, Hash)

Sha1.prototype.init = function () {
  this._a = 0x67452301
  this._b = 0xefcdab89
  this._c = 0x98badcfe
  this._d = 0x10325476
  this._e = 0xc3d2e1f0

  return this
}

function rotl1 (num) {
  return (num << 1) | (num >>> 31)
}

function rotl5 (num) {
  return (num << 5) | (num >>> 27)
}

function rotl30 (num) {
  return (num << 30) | (num >>> 2)
}

function ft (s, b, c, d) {
  if (s === 0) return (b & c) | ((~b) & d)
  if (s === 2) return (b & c) | (b & d) | (c & d)
  return b ^ c ^ d
}

Sha1.prototype._update = function (M) {
  var W = this._w

  var a = this._a | 0
  var b = this._b | 0
  var c = this._c | 0
  var d = this._d | 0
  var e = this._e | 0

  for (var i = 0; i < 16; ++i) W[i] = M.readInt32BE(i * 4)
  for (; i < 80; ++i) W[i] = rotl1(W[i - 3] ^ W[i - 8] ^ W[i - 14] ^ W[i - 16])

  for (var j = 0; j < 80; ++j) {
    var s = ~~(j / 20)
    var t = (rotl5(a) + ft(s, b, c, d) + e + W[j] + K[s]) | 0

    e = d
    d = c
    c = rotl30(b)
    b = a
    a = t
  }

  this._a = (a + this._a) | 0
  this._b = (b + this._b) | 0
  this._c = (c + this._c) | 0
  this._d = (d + this._d) | 0
  this._e = (e + this._e) | 0
}

Sha1.prototype._hash = function () {
  var H = new Buffer(20)

  H.writeInt32BE(this._a | 0, 0)
  H.writeInt32BE(this._b | 0, 4)
  H.writeInt32BE(this._c | 0, 8)
  H.writeInt32BE(this._d | 0, 12)
  H.writeInt32BE(this._e | 0, 16)

  return H
}

module.exports = Sha1

}).call(this,require("buffer").Buffer)

},{"./hash":10,"buffer":5,"inherits":7}],14:[function(require,module,exports){
(function (Buffer){
/**
 * A JavaScript implementation of the Secure Hash Algorithm, SHA-256, as defined
 * in FIPS 180-2
 * Version 2.2-beta Copyright Angel Marin, Paul Johnston 2000 - 2009.
 * Other contributors: Greg Holt, Andrew Kepert, Ydnar, Lostinet
 *
 */

var inherits = require('inherits')
var Sha256 = require('./sha256')
var Hash = require('./hash')

var W = new Array(64)

function Sha224 () {
  this.init()

  this._w = W // new Array(64)

  Hash.call(this, 64, 56)
}

inherits(Sha224, Sha256)

Sha224.prototype.init = function () {
  this._a = 0xc1059ed8
  this._b = 0x367cd507
  this._c = 0x3070dd17
  this._d = 0xf70e5939
  this._e = 0xffc00b31
  this._f = 0x68581511
  this._g = 0x64f98fa7
  this._h = 0xbefa4fa4

  return this
}

Sha224.prototype._hash = function () {
  var H = new Buffer(28)

  H.writeInt32BE(this._a, 0)
  H.writeInt32BE(this._b, 4)
  H.writeInt32BE(this._c, 8)
  H.writeInt32BE(this._d, 12)
  H.writeInt32BE(this._e, 16)
  H.writeInt32BE(this._f, 20)
  H.writeInt32BE(this._g, 24)

  return H
}

module.exports = Sha224

}).call(this,require("buffer").Buffer)

},{"./hash":10,"./sha256":15,"buffer":5,"inherits":7}],15:[function(require,module,exports){
(function (Buffer){
/**
 * A JavaScript implementation of the Secure Hash Algorithm, SHA-256, as defined
 * in FIPS 180-2
 * Version 2.2-beta Copyright Angel Marin, Paul Johnston 2000 - 2009.
 * Other contributors: Greg Holt, Andrew Kepert, Ydnar, Lostinet
 *
 */

var inherits = require('inherits')
var Hash = require('./hash')

var K = [
  0x428A2F98, 0x71374491, 0xB5C0FBCF, 0xE9B5DBA5,
  0x3956C25B, 0x59F111F1, 0x923F82A4, 0xAB1C5ED5,
  0xD807AA98, 0x12835B01, 0x243185BE, 0x550C7DC3,
  0x72BE5D74, 0x80DEB1FE, 0x9BDC06A7, 0xC19BF174,
  0xE49B69C1, 0xEFBE4786, 0x0FC19DC6, 0x240CA1CC,
  0x2DE92C6F, 0x4A7484AA, 0x5CB0A9DC, 0x76F988DA,
  0x983E5152, 0xA831C66D, 0xB00327C8, 0xBF597FC7,
  0xC6E00BF3, 0xD5A79147, 0x06CA6351, 0x14292967,
  0x27B70A85, 0x2E1B2138, 0x4D2C6DFC, 0x53380D13,
  0x650A7354, 0x766A0ABB, 0x81C2C92E, 0x92722C85,
  0xA2BFE8A1, 0xA81A664B, 0xC24B8B70, 0xC76C51A3,
  0xD192E819, 0xD6990624, 0xF40E3585, 0x106AA070,
  0x19A4C116, 0x1E376C08, 0x2748774C, 0x34B0BCB5,
  0x391C0CB3, 0x4ED8AA4A, 0x5B9CCA4F, 0x682E6FF3,
  0x748F82EE, 0x78A5636F, 0x84C87814, 0x8CC70208,
  0x90BEFFFA, 0xA4506CEB, 0xBEF9A3F7, 0xC67178F2
]

var W = new Array(64)

function Sha256 () {
  this.init()

  this._w = W // new Array(64)

  Hash.call(this, 64, 56)
}

inherits(Sha256, Hash)

Sha256.prototype.init = function () {
  this._a = 0x6a09e667
  this._b = 0xbb67ae85
  this._c = 0x3c6ef372
  this._d = 0xa54ff53a
  this._e = 0x510e527f
  this._f = 0x9b05688c
  this._g = 0x1f83d9ab
  this._h = 0x5be0cd19

  return this
}

function ch (x, y, z) {
  return z ^ (x & (y ^ z))
}

function maj (x, y, z) {
  return (x & y) | (z & (x | y))
}

function sigma0 (x) {
  return (x >>> 2 | x << 30) ^ (x >>> 13 | x << 19) ^ (x >>> 22 | x << 10)
}

function sigma1 (x) {
  return (x >>> 6 | x << 26) ^ (x >>> 11 | x << 21) ^ (x >>> 25 | x << 7)
}

function gamma0 (x) {
  return (x >>> 7 | x << 25) ^ (x >>> 18 | x << 14) ^ (x >>> 3)
}

function gamma1 (x) {
  return (x >>> 17 | x << 15) ^ (x >>> 19 | x << 13) ^ (x >>> 10)
}

Sha256.prototype._update = function (M) {
  var W = this._w

  var a = this._a | 0
  var b = this._b | 0
  var c = this._c | 0
  var d = this._d | 0
  var e = this._e | 0
  var f = this._f | 0
  var g = this._g | 0
  var h = this._h | 0

  for (var i = 0; i < 16; ++i) W[i] = M.readInt32BE(i * 4)
  for (; i < 64; ++i) W[i] = (gamma1(W[i - 2]) + W[i - 7] + gamma0(W[i - 15]) + W[i - 16]) | 0

  for (var j = 0; j < 64; ++j) {
    var T1 = (h + sigma1(e) + ch(e, f, g) + K[j] + W[j]) | 0
    var T2 = (sigma0(a) + maj(a, b, c)) | 0

    h = g
    g = f
    f = e
    e = (d + T1) | 0
    d = c
    c = b
    b = a
    a = (T1 + T2) | 0
  }

  this._a = (a + this._a) | 0
  this._b = (b + this._b) | 0
  this._c = (c + this._c) | 0
  this._d = (d + this._d) | 0
  this._e = (e + this._e) | 0
  this._f = (f + this._f) | 0
  this._g = (g + this._g) | 0
  this._h = (h + this._h) | 0
}

Sha256.prototype._hash = function () {
  var H = new Buffer(32)

  H.writeInt32BE(this._a, 0)
  H.writeInt32BE(this._b, 4)
  H.writeInt32BE(this._c, 8)
  H.writeInt32BE(this._d, 12)
  H.writeInt32BE(this._e, 16)
  H.writeInt32BE(this._f, 20)
  H.writeInt32BE(this._g, 24)
  H.writeInt32BE(this._h, 28)

  return H
}

module.exports = Sha256

}).call(this,require("buffer").Buffer)

},{"./hash":10,"buffer":5,"inherits":7}],16:[function(require,module,exports){
(function (Buffer){
var inherits = require('inherits')
var SHA512 = require('./sha512')
var Hash = require('./hash')

var W = new Array(160)

function Sha384 () {
  this.init()
  this._w = W

  Hash.call(this, 128, 112)
}

inherits(Sha384, SHA512)

Sha384.prototype.init = function () {
  this._ah = 0xcbbb9d5d
  this._bh = 0x629a292a
  this._ch = 0x9159015a
  this._dh = 0x152fecd8
  this._eh = 0x67332667
  this._fh = 0x8eb44a87
  this._gh = 0xdb0c2e0d
  this._hh = 0x47b5481d

  this._al = 0xc1059ed8
  this._bl = 0x367cd507
  this._cl = 0x3070dd17
  this._dl = 0xf70e5939
  this._el = 0xffc00b31
  this._fl = 0x68581511
  this._gl = 0x64f98fa7
  this._hl = 0xbefa4fa4

  return this
}

Sha384.prototype._hash = function () {
  var H = new Buffer(48)

  function writeInt64BE (h, l, offset) {
    H.writeInt32BE(h, offset)
    H.writeInt32BE(l, offset + 4)
  }

  writeInt64BE(this._ah, this._al, 0)
  writeInt64BE(this._bh, this._bl, 8)
  writeInt64BE(this._ch, this._cl, 16)
  writeInt64BE(this._dh, this._dl, 24)
  writeInt64BE(this._eh, this._el, 32)
  writeInt64BE(this._fh, this._fl, 40)

  return H
}

module.exports = Sha384

}).call(this,require("buffer").Buffer)

},{"./hash":10,"./sha512":17,"buffer":5,"inherits":7}],17:[function(require,module,exports){
(function (Buffer){
var inherits = require('inherits')
var Hash = require('./hash')

var K = [
  0x428a2f98, 0xd728ae22, 0x71374491, 0x23ef65cd,
  0xb5c0fbcf, 0xec4d3b2f, 0xe9b5dba5, 0x8189dbbc,
  0x3956c25b, 0xf348b538, 0x59f111f1, 0xb605d019,
  0x923f82a4, 0xaf194f9b, 0xab1c5ed5, 0xda6d8118,
  0xd807aa98, 0xa3030242, 0x12835b01, 0x45706fbe,
  0x243185be, 0x4ee4b28c, 0x550c7dc3, 0xd5ffb4e2,
  0x72be5d74, 0xf27b896f, 0x80deb1fe, 0x3b1696b1,
  0x9bdc06a7, 0x25c71235, 0xc19bf174, 0xcf692694,
  0xe49b69c1, 0x9ef14ad2, 0xefbe4786, 0x384f25e3,
  0x0fc19dc6, 0x8b8cd5b5, 0x240ca1cc, 0x77ac9c65,
  0x2de92c6f, 0x592b0275, 0x4a7484aa, 0x6ea6e483,
  0x5cb0a9dc, 0xbd41fbd4, 0x76f988da, 0x831153b5,
  0x983e5152, 0xee66dfab, 0xa831c66d, 0x2db43210,
  0xb00327c8, 0x98fb213f, 0xbf597fc7, 0xbeef0ee4,
  0xc6e00bf3, 0x3da88fc2, 0xd5a79147, 0x930aa725,
  0x06ca6351, 0xe003826f, 0x14292967, 0x0a0e6e70,
  0x27b70a85, 0x46d22ffc, 0x2e1b2138, 0x5c26c926,
  0x4d2c6dfc, 0x5ac42aed, 0x53380d13, 0x9d95b3df,
  0x650a7354, 0x8baf63de, 0x766a0abb, 0x3c77b2a8,
  0x81c2c92e, 0x47edaee6, 0x92722c85, 0x1482353b,
  0xa2bfe8a1, 0x4cf10364, 0xa81a664b, 0xbc423001,
  0xc24b8b70, 0xd0f89791, 0xc76c51a3, 0x0654be30,
  0xd192e819, 0xd6ef5218, 0xd6990624, 0x5565a910,
  0xf40e3585, 0x5771202a, 0x106aa070, 0x32bbd1b8,
  0x19a4c116, 0xb8d2d0c8, 0x1e376c08, 0x5141ab53,
  0x2748774c, 0xdf8eeb99, 0x34b0bcb5, 0xe19b48a8,
  0x391c0cb3, 0xc5c95a63, 0x4ed8aa4a, 0xe3418acb,
  0x5b9cca4f, 0x7763e373, 0x682e6ff3, 0xd6b2b8a3,
  0x748f82ee, 0x5defb2fc, 0x78a5636f, 0x43172f60,
  0x84c87814, 0xa1f0ab72, 0x8cc70208, 0x1a6439ec,
  0x90befffa, 0x23631e28, 0xa4506ceb, 0xde82bde9,
  0xbef9a3f7, 0xb2c67915, 0xc67178f2, 0xe372532b,
  0xca273ece, 0xea26619c, 0xd186b8c7, 0x21c0c207,
  0xeada7dd6, 0xcde0eb1e, 0xf57d4f7f, 0xee6ed178,
  0x06f067aa, 0x72176fba, 0x0a637dc5, 0xa2c898a6,
  0x113f9804, 0xbef90dae, 0x1b710b35, 0x131c471b,
  0x28db77f5, 0x23047d84, 0x32caab7b, 0x40c72493,
  0x3c9ebe0a, 0x15c9bebc, 0x431d67c4, 0x9c100d4c,
  0x4cc5d4be, 0xcb3e42b6, 0x597f299c, 0xfc657e2a,
  0x5fcb6fab, 0x3ad6faec, 0x6c44198c, 0x4a475817
]

var W = new Array(160)

function Sha512 () {
  this.init()
  this._w = W

  Hash.call(this, 128, 112)
}

inherits(Sha512, Hash)

Sha512.prototype.init = function () {
  this._ah = 0x6a09e667
  this._bh = 0xbb67ae85
  this._ch = 0x3c6ef372
  this._dh = 0xa54ff53a
  this._eh = 0x510e527f
  this._fh = 0x9b05688c
  this._gh = 0x1f83d9ab
  this._hh = 0x5be0cd19

  this._al = 0xf3bcc908
  this._bl = 0x84caa73b
  this._cl = 0xfe94f82b
  this._dl = 0x5f1d36f1
  this._el = 0xade682d1
  this._fl = 0x2b3e6c1f
  this._gl = 0xfb41bd6b
  this._hl = 0x137e2179

  return this
}

function Ch (x, y, z) {
  return z ^ (x & (y ^ z))
}

function maj (x, y, z) {
  return (x & y) | (z & (x | y))
}

function sigma0 (x, xl) {
  return (x >>> 28 | xl << 4) ^ (xl >>> 2 | x << 30) ^ (xl >>> 7 | x << 25)
}

function sigma1 (x, xl) {
  return (x >>> 14 | xl << 18) ^ (x >>> 18 | xl << 14) ^ (xl >>> 9 | x << 23)
}

function Gamma0 (x, xl) {
  return (x >>> 1 | xl << 31) ^ (x >>> 8 | xl << 24) ^ (x >>> 7)
}

function Gamma0l (x, xl) {
  return (x >>> 1 | xl << 31) ^ (x >>> 8 | xl << 24) ^ (x >>> 7 | xl << 25)
}

function Gamma1 (x, xl) {
  return (x >>> 19 | xl << 13) ^ (xl >>> 29 | x << 3) ^ (x >>> 6)
}

function Gamma1l (x, xl) {
  return (x >>> 19 | xl << 13) ^ (xl >>> 29 | x << 3) ^ (x >>> 6 | xl << 26)
}

function getCarry (a, b) {
  return (a >>> 0) < (b >>> 0) ? 1 : 0
}

Sha512.prototype._update = function (M) {
  var W = this._w

  var ah = this._ah | 0
  var bh = this._bh | 0
  var ch = this._ch | 0
  var dh = this._dh | 0
  var eh = this._eh | 0
  var fh = this._fh | 0
  var gh = this._gh | 0
  var hh = this._hh | 0

  var al = this._al | 0
  var bl = this._bl | 0
  var cl = this._cl | 0
  var dl = this._dl | 0
  var el = this._el | 0
  var fl = this._fl | 0
  var gl = this._gl | 0
  var hl = this._hl | 0

  for (var i = 0; i < 32; i += 2) {
    W[i] = M.readInt32BE(i * 4)
    W[i + 1] = M.readInt32BE(i * 4 + 4)
  }
  for (; i < 160; i += 2) {
    var xh = W[i - 15 * 2]
    var xl = W[i - 15 * 2 + 1]
    var gamma0 = Gamma0(xh, xl)
    var gamma0l = Gamma0l(xl, xh)

    xh = W[i - 2 * 2]
    xl = W[i - 2 * 2 + 1]
    var gamma1 = Gamma1(xh, xl)
    var gamma1l = Gamma1l(xl, xh)

    // W[i] = gamma0 + W[i - 7] + gamma1 + W[i - 16]
    var Wi7h = W[i - 7 * 2]
    var Wi7l = W[i - 7 * 2 + 1]

    var Wi16h = W[i - 16 * 2]
    var Wi16l = W[i - 16 * 2 + 1]

    var Wil = (gamma0l + Wi7l) | 0
    var Wih = (gamma0 + Wi7h + getCarry(Wil, gamma0l)) | 0
    Wil = (Wil + gamma1l) | 0
    Wih = (Wih + gamma1 + getCarry(Wil, gamma1l)) | 0
    Wil = (Wil + Wi16l) | 0
    Wih = (Wih + Wi16h + getCarry(Wil, Wi16l)) | 0

    W[i] = Wih
    W[i + 1] = Wil
  }

  for (var j = 0; j < 160; j += 2) {
    Wih = W[j]
    Wil = W[j + 1]

    var majh = maj(ah, bh, ch)
    var majl = maj(al, bl, cl)

    var sigma0h = sigma0(ah, al)
    var sigma0l = sigma0(al, ah)
    var sigma1h = sigma1(eh, el)
    var sigma1l = sigma1(el, eh)

    // t1 = h + sigma1 + ch + K[j] + W[j]
    var Kih = K[j]
    var Kil = K[j + 1]

    var chh = Ch(eh, fh, gh)
    var chl = Ch(el, fl, gl)

    var t1l = (hl + sigma1l) | 0
    var t1h = (hh + sigma1h + getCarry(t1l, hl)) | 0
    t1l = (t1l + chl) | 0
    t1h = (t1h + chh + getCarry(t1l, chl)) | 0
    t1l = (t1l + Kil) | 0
    t1h = (t1h + Kih + getCarry(t1l, Kil)) | 0
    t1l = (t1l + Wil) | 0
    t1h = (t1h + Wih + getCarry(t1l, Wil)) | 0

    // t2 = sigma0 + maj
    var t2l = (sigma0l + majl) | 0
    var t2h = (sigma0h + majh + getCarry(t2l, sigma0l)) | 0

    hh = gh
    hl = gl
    gh = fh
    gl = fl
    fh = eh
    fl = el
    el = (dl + t1l) | 0
    eh = (dh + t1h + getCarry(el, dl)) | 0
    dh = ch
    dl = cl
    ch = bh
    cl = bl
    bh = ah
    bl = al
    al = (t1l + t2l) | 0
    ah = (t1h + t2h + getCarry(al, t1l)) | 0
  }

  this._al = (this._al + al) | 0
  this._bl = (this._bl + bl) | 0
  this._cl = (this._cl + cl) | 0
  this._dl = (this._dl + dl) | 0
  this._el = (this._el + el) | 0
  this._fl = (this._fl + fl) | 0
  this._gl = (this._gl + gl) | 0
  this._hl = (this._hl + hl) | 0

  this._ah = (this._ah + ah + getCarry(this._al, al)) | 0
  this._bh = (this._bh + bh + getCarry(this._bl, bl)) | 0
  this._ch = (this._ch + ch + getCarry(this._cl, cl)) | 0
  this._dh = (this._dh + dh + getCarry(this._dl, dl)) | 0
  this._eh = (this._eh + eh + getCarry(this._el, el)) | 0
  this._fh = (this._fh + fh + getCarry(this._fl, fl)) | 0
  this._gh = (this._gh + gh + getCarry(this._gl, gl)) | 0
  this._hh = (this._hh + hh + getCarry(this._hl, hl)) | 0
}

Sha512.prototype._hash = function () {
  var H = new Buffer(64)

  function writeInt64BE (h, l, offset) {
    H.writeInt32BE(h, offset)
    H.writeInt32BE(l, offset + 4)
  }

  writeInt64BE(this._ah, this._al, 0)
  writeInt64BE(this._bh, this._bl, 8)
  writeInt64BE(this._ch, this._cl, 16)
  writeInt64BE(this._dh, this._dl, 24)
  writeInt64BE(this._eh, this._el, 32)
  writeInt64BE(this._fh, this._fl, 40)
  writeInt64BE(this._gh, this._gl, 48)
  writeInt64BE(this._hh, this._hl, 56)

  return H
}

module.exports = Sha512

}).call(this,require("buffer").Buffer)

},{"./hash":10,"buffer":5,"inherits":7}],18:[function(require,module,exports){
(function(nacl) {
'use strict';

// Ported in 2014 by Dmitry Chestnykh and Devi Mandiri.
// Public domain.
//
// Implementation derived from TweetNaCl version 20140427.
// See for details: http://tweetnacl.cr.yp.to/

var gf = function(init) {
  var i, r = new Float64Array(16);
  if (init) for (i = 0; i < init.length; i++) r[i] = init[i];
  return r;
};

//  Pluggable, initialized in high-level API below.
var randombytes = function(/* x, n */) { throw new Error('no PRNG'); };

var _0 = new Uint8Array(16);
var _9 = new Uint8Array(32); _9[0] = 9;

var gf0 = gf(),
    gf1 = gf([1]),
    _121665 = gf([0xdb41, 1]),
    D = gf([0x78a3, 0x1359, 0x4dca, 0x75eb, 0xd8ab, 0x4141, 0x0a4d, 0x0070, 0xe898, 0x7779, 0x4079, 0x8cc7, 0xfe73, 0x2b6f, 0x6cee, 0x5203]),
    D2 = gf([0xf159, 0x26b2, 0x9b94, 0xebd6, 0xb156, 0x8283, 0x149a, 0x00e0, 0xd130, 0xeef3, 0x80f2, 0x198e, 0xfce7, 0x56df, 0xd9dc, 0x2406]),
    X = gf([0xd51a, 0x8f25, 0x2d60, 0xc956, 0xa7b2, 0x9525, 0xc760, 0x692c, 0xdc5c, 0xfdd6, 0xe231, 0xc0a4, 0x53fe, 0xcd6e, 0x36d3, 0x2169]),
    Y = gf([0x6658, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666, 0x6666]),
    I = gf([0xa0b0, 0x4a0e, 0x1b27, 0xc4ee, 0xe478, 0xad2f, 0x1806, 0x2f43, 0xd7a7, 0x3dfb, 0x0099, 0x2b4d, 0xdf0b, 0x4fc1, 0x2480, 0x2b83]);

function ts64(x, i, h, l) {
  x[i]   = (h >> 24) & 0xff;
  x[i+1] = (h >> 16) & 0xff;
  x[i+2] = (h >>  8) & 0xff;
  x[i+3] = h & 0xff;
  x[i+4] = (l >> 24)  & 0xff;
  x[i+5] = (l >> 16)  & 0xff;
  x[i+6] = (l >>  8)  & 0xff;
  x[i+7] = l & 0xff;
}

function vn(x, xi, y, yi, n) {
  var i,d = 0;
  for (i = 0; i < n; i++) d |= x[xi+i]^y[yi+i];
  return (1 & ((d - 1) >>> 8)) - 1;
}

function crypto_verify_16(x, xi, y, yi) {
  return vn(x,xi,y,yi,16);
}

function crypto_verify_32(x, xi, y, yi) {
  return vn(x,xi,y,yi,32);
}

function core_salsa20(o, p, k, c) {
  var j0  = c[ 0] & 0xff | (c[ 1] & 0xff)<<8 | (c[ 2] & 0xff)<<16 | (c[ 3] & 0xff)<<24,
      j1  = k[ 0] & 0xff | (k[ 1] & 0xff)<<8 | (k[ 2] & 0xff)<<16 | (k[ 3] & 0xff)<<24,
      j2  = k[ 4] & 0xff | (k[ 5] & 0xff)<<8 | (k[ 6] & 0xff)<<16 | (k[ 7] & 0xff)<<24,
      j3  = k[ 8] & 0xff | (k[ 9] & 0xff)<<8 | (k[10] & 0xff)<<16 | (k[11] & 0xff)<<24,
      j4  = k[12] & 0xff | (k[13] & 0xff)<<8 | (k[14] & 0xff)<<16 | (k[15] & 0xff)<<24,
      j5  = c[ 4] & 0xff | (c[ 5] & 0xff)<<8 | (c[ 6] & 0xff)<<16 | (c[ 7] & 0xff)<<24,
      j6  = p[ 0] & 0xff | (p[ 1] & 0xff)<<8 | (p[ 2] & 0xff)<<16 | (p[ 3] & 0xff)<<24,
      j7  = p[ 4] & 0xff | (p[ 5] & 0xff)<<8 | (p[ 6] & 0xff)<<16 | (p[ 7] & 0xff)<<24,
      j8  = p[ 8] & 0xff | (p[ 9] & 0xff)<<8 | (p[10] & 0xff)<<16 | (p[11] & 0xff)<<24,
      j9  = p[12] & 0xff | (p[13] & 0xff)<<8 | (p[14] & 0xff)<<16 | (p[15] & 0xff)<<24,
      j10 = c[ 8] & 0xff | (c[ 9] & 0xff)<<8 | (c[10] & 0xff)<<16 | (c[11] & 0xff)<<24,
      j11 = k[16] & 0xff | (k[17] & 0xff)<<8 | (k[18] & 0xff)<<16 | (k[19] & 0xff)<<24,
      j12 = k[20] & 0xff | (k[21] & 0xff)<<8 | (k[22] & 0xff)<<16 | (k[23] & 0xff)<<24,
      j13 = k[24] & 0xff | (k[25] & 0xff)<<8 | (k[26] & 0xff)<<16 | (k[27] & 0xff)<<24,
      j14 = k[28] & 0xff | (k[29] & 0xff)<<8 | (k[30] & 0xff)<<16 | (k[31] & 0xff)<<24,
      j15 = c[12] & 0xff | (c[13] & 0xff)<<8 | (c[14] & 0xff)<<16 | (c[15] & 0xff)<<24;

  var x0 = j0, x1 = j1, x2 = j2, x3 = j3, x4 = j4, x5 = j5, x6 = j6, x7 = j7,
      x8 = j8, x9 = j9, x10 = j10, x11 = j11, x12 = j12, x13 = j13, x14 = j14,
      x15 = j15, u;

  for (var i = 0; i < 20; i += 2) {
    u = x0 + x12 | 0;
    x4 ^= u<<7 | u>>>(32-7);
    u = x4 + x0 | 0;
    x8 ^= u<<9 | u>>>(32-9);
    u = x8 + x4 | 0;
    x12 ^= u<<13 | u>>>(32-13);
    u = x12 + x8 | 0;
    x0 ^= u<<18 | u>>>(32-18);

    u = x5 + x1 | 0;
    x9 ^= u<<7 | u>>>(32-7);
    u = x9 + x5 | 0;
    x13 ^= u<<9 | u>>>(32-9);
    u = x13 + x9 | 0;
    x1 ^= u<<13 | u>>>(32-13);
    u = x1 + x13 | 0;
    x5 ^= u<<18 | u>>>(32-18);

    u = x10 + x6 | 0;
    x14 ^= u<<7 | u>>>(32-7);
    u = x14 + x10 | 0;
    x2 ^= u<<9 | u>>>(32-9);
    u = x2 + x14 | 0;
    x6 ^= u<<13 | u>>>(32-13);
    u = x6 + x2 | 0;
    x10 ^= u<<18 | u>>>(32-18);

    u = x15 + x11 | 0;
    x3 ^= u<<7 | u>>>(32-7);
    u = x3 + x15 | 0;
    x7 ^= u<<9 | u>>>(32-9);
    u = x7 + x3 | 0;
    x11 ^= u<<13 | u>>>(32-13);
    u = x11 + x7 | 0;
    x15 ^= u<<18 | u>>>(32-18);

    u = x0 + x3 | 0;
    x1 ^= u<<7 | u>>>(32-7);
    u = x1 + x0 | 0;
    x2 ^= u<<9 | u>>>(32-9);
    u = x2 + x1 | 0;
    x3 ^= u<<13 | u>>>(32-13);
    u = x3 + x2 | 0;
    x0 ^= u<<18 | u>>>(32-18);

    u = x5 + x4 | 0;
    x6 ^= u<<7 | u>>>(32-7);
    u = x6 + x5 | 0;
    x7 ^= u<<9 | u>>>(32-9);
    u = x7 + x6 | 0;
    x4 ^= u<<13 | u>>>(32-13);
    u = x4 + x7 | 0;
    x5 ^= u<<18 | u>>>(32-18);

    u = x10 + x9 | 0;
    x11 ^= u<<7 | u>>>(32-7);
    u = x11 + x10 | 0;
    x8 ^= u<<9 | u>>>(32-9);
    u = x8 + x11 | 0;
    x9 ^= u<<13 | u>>>(32-13);
    u = x9 + x8 | 0;
    x10 ^= u<<18 | u>>>(32-18);

    u = x15 + x14 | 0;
    x12 ^= u<<7 | u>>>(32-7);
    u = x12 + x15 | 0;
    x13 ^= u<<9 | u>>>(32-9);
    u = x13 + x12 | 0;
    x14 ^= u<<13 | u>>>(32-13);
    u = x14 + x13 | 0;
    x15 ^= u<<18 | u>>>(32-18);
  }
   x0 =  x0 +  j0 | 0;
   x1 =  x1 +  j1 | 0;
   x2 =  x2 +  j2 | 0;
   x3 =  x3 +  j3 | 0;
   x4 =  x4 +  j4 | 0;
   x5 =  x5 +  j5 | 0;
   x6 =  x6 +  j6 | 0;
   x7 =  x7 +  j7 | 0;
   x8 =  x8 +  j8 | 0;
   x9 =  x9 +  j9 | 0;
  x10 = x10 + j10 | 0;
  x11 = x11 + j11 | 0;
  x12 = x12 + j12 | 0;
  x13 = x13 + j13 | 0;
  x14 = x14 + j14 | 0;
  x15 = x15 + j15 | 0;

  o[ 0] = x0 >>>  0 & 0xff;
  o[ 1] = x0 >>>  8 & 0xff;
  o[ 2] = x0 >>> 16 & 0xff;
  o[ 3] = x0 >>> 24 & 0xff;

  o[ 4] = x1 >>>  0 & 0xff;
  o[ 5] = x1 >>>  8 & 0xff;
  o[ 6] = x1 >>> 16 & 0xff;
  o[ 7] = x1 >>> 24 & 0xff;

  o[ 8] = x2 >>>  0 & 0xff;
  o[ 9] = x2 >>>  8 & 0xff;
  o[10] = x2 >>> 16 & 0xff;
  o[11] = x2 >>> 24 & 0xff;

  o[12] = x3 >>>  0 & 0xff;
  o[13] = x3 >>>  8 & 0xff;
  o[14] = x3 >>> 16 & 0xff;
  o[15] = x3 >>> 24 & 0xff;

  o[16] = x4 >>>  0 & 0xff;
  o[17] = x4 >>>  8 & 0xff;
  o[18] = x4 >>> 16 & 0xff;
  o[19] = x4 >>> 24 & 0xff;

  o[20] = x5 >>>  0 & 0xff;
  o[21] = x5 >>>  8 & 0xff;
  o[22] = x5 >>> 16 & 0xff;
  o[23] = x5 >>> 24 & 0xff;

  o[24] = x6 >>>  0 & 0xff;
  o[25] = x6 >>>  8 & 0xff;
  o[26] = x6 >>> 16 & 0xff;
  o[27] = x6 >>> 24 & 0xff;

  o[28] = x7 >>>  0 & 0xff;
  o[29] = x7 >>>  8 & 0xff;
  o[30] = x7 >>> 16 & 0xff;
  o[31] = x7 >>> 24 & 0xff;

  o[32] = x8 >>>  0 & 0xff;
  o[33] = x8 >>>  8 & 0xff;
  o[34] = x8 >>> 16 & 0xff;
  o[35] = x8 >>> 24 & 0xff;

  o[36] = x9 >>>  0 & 0xff;
  o[37] = x9 >>>  8 & 0xff;
  o[38] = x9 >>> 16 & 0xff;
  o[39] = x9 >>> 24 & 0xff;

  o[40] = x10 >>>  0 & 0xff;
  o[41] = x10 >>>  8 & 0xff;
  o[42] = x10 >>> 16 & 0xff;
  o[43] = x10 >>> 24 & 0xff;

  o[44] = x11 >>>  0 & 0xff;
  o[45] = x11 >>>  8 & 0xff;
  o[46] = x11 >>> 16 & 0xff;
  o[47] = x11 >>> 24 & 0xff;

  o[48] = x12 >>>  0 & 0xff;
  o[49] = x12 >>>  8 & 0xff;
  o[50] = x12 >>> 16 & 0xff;
  o[51] = x12 >>> 24 & 0xff;

  o[52] = x13 >>>  0 & 0xff;
  o[53] = x13 >>>  8 & 0xff;
  o[54] = x13 >>> 16 & 0xff;
  o[55] = x13 >>> 24 & 0xff;

  o[56] = x14 >>>  0 & 0xff;
  o[57] = x14 >>>  8 & 0xff;
  o[58] = x14 >>> 16 & 0xff;
  o[59] = x14 >>> 24 & 0xff;

  o[60] = x15 >>>  0 & 0xff;
  o[61] = x15 >>>  8 & 0xff;
  o[62] = x15 >>> 16 & 0xff;
  o[63] = x15 >>> 24 & 0xff;
}

function core_hsalsa20(o,p,k,c) {
  var j0  = c[ 0] & 0xff | (c[ 1] & 0xff)<<8 | (c[ 2] & 0xff)<<16 | (c[ 3] & 0xff)<<24,
      j1  = k[ 0] & 0xff | (k[ 1] & 0xff)<<8 | (k[ 2] & 0xff)<<16 | (k[ 3] & 0xff)<<24,
      j2  = k[ 4] & 0xff | (k[ 5] & 0xff)<<8 | (k[ 6] & 0xff)<<16 | (k[ 7] & 0xff)<<24,
      j3  = k[ 8] & 0xff | (k[ 9] & 0xff)<<8 | (k[10] & 0xff)<<16 | (k[11] & 0xff)<<24,
      j4  = k[12] & 0xff | (k[13] & 0xff)<<8 | (k[14] & 0xff)<<16 | (k[15] & 0xff)<<24,
      j5  = c[ 4] & 0xff | (c[ 5] & 0xff)<<8 | (c[ 6] & 0xff)<<16 | (c[ 7] & 0xff)<<24,
      j6  = p[ 0] & 0xff | (p[ 1] & 0xff)<<8 | (p[ 2] & 0xff)<<16 | (p[ 3] & 0xff)<<24,
      j7  = p[ 4] & 0xff | (p[ 5] & 0xff)<<8 | (p[ 6] & 0xff)<<16 | (p[ 7] & 0xff)<<24,
      j8  = p[ 8] & 0xff | (p[ 9] & 0xff)<<8 | (p[10] & 0xff)<<16 | (p[11] & 0xff)<<24,
      j9  = p[12] & 0xff | (p[13] & 0xff)<<8 | (p[14] & 0xff)<<16 | (p[15] & 0xff)<<24,
      j10 = c[ 8] & 0xff | (c[ 9] & 0xff)<<8 | (c[10] & 0xff)<<16 | (c[11] & 0xff)<<24,
      j11 = k[16] & 0xff | (k[17] & 0xff)<<8 | (k[18] & 0xff)<<16 | (k[19] & 0xff)<<24,
      j12 = k[20] & 0xff | (k[21] & 0xff)<<8 | (k[22] & 0xff)<<16 | (k[23] & 0xff)<<24,
      j13 = k[24] & 0xff | (k[25] & 0xff)<<8 | (k[26] & 0xff)<<16 | (k[27] & 0xff)<<24,
      j14 = k[28] & 0xff | (k[29] & 0xff)<<8 | (k[30] & 0xff)<<16 | (k[31] & 0xff)<<24,
      j15 = c[12] & 0xff | (c[13] & 0xff)<<8 | (c[14] & 0xff)<<16 | (c[15] & 0xff)<<24;

  var x0 = j0, x1 = j1, x2 = j2, x3 = j3, x4 = j4, x5 = j5, x6 = j6, x7 = j7,
      x8 = j8, x9 = j9, x10 = j10, x11 = j11, x12 = j12, x13 = j13, x14 = j14,
      x15 = j15, u;

  for (var i = 0; i < 20; i += 2) {
    u = x0 + x12 | 0;
    x4 ^= u<<7 | u>>>(32-7);
    u = x4 + x0 | 0;
    x8 ^= u<<9 | u>>>(32-9);
    u = x8 + x4 | 0;
    x12 ^= u<<13 | u>>>(32-13);
    u = x12 + x8 | 0;
    x0 ^= u<<18 | u>>>(32-18);

    u = x5 + x1 | 0;
    x9 ^= u<<7 | u>>>(32-7);
    u = x9 + x5 | 0;
    x13 ^= u<<9 | u>>>(32-9);
    u = x13 + x9 | 0;
    x1 ^= u<<13 | u>>>(32-13);
    u = x1 + x13 | 0;
    x5 ^= u<<18 | u>>>(32-18);

    u = x10 + x6 | 0;
    x14 ^= u<<7 | u>>>(32-7);
    u = x14 + x10 | 0;
    x2 ^= u<<9 | u>>>(32-9);
    u = x2 + x14 | 0;
    x6 ^= u<<13 | u>>>(32-13);
    u = x6 + x2 | 0;
    x10 ^= u<<18 | u>>>(32-18);

    u = x15 + x11 | 0;
    x3 ^= u<<7 | u>>>(32-7);
    u = x3 + x15 | 0;
    x7 ^= u<<9 | u>>>(32-9);
    u = x7 + x3 | 0;
    x11 ^= u<<13 | u>>>(32-13);
    u = x11 + x7 | 0;
    x15 ^= u<<18 | u>>>(32-18);

    u = x0 + x3 | 0;
    x1 ^= u<<7 | u>>>(32-7);
    u = x1 + x0 | 0;
    x2 ^= u<<9 | u>>>(32-9);
    u = x2 + x1 | 0;
    x3 ^= u<<13 | u>>>(32-13);
    u = x3 + x2 | 0;
    x0 ^= u<<18 | u>>>(32-18);

    u = x5 + x4 | 0;
    x6 ^= u<<7 | u>>>(32-7);
    u = x6 + x5 | 0;
    x7 ^= u<<9 | u>>>(32-9);
    u = x7 + x6 | 0;
    x4 ^= u<<13 | u>>>(32-13);
    u = x4 + x7 | 0;
    x5 ^= u<<18 | u>>>(32-18);

    u = x10 + x9 | 0;
    x11 ^= u<<7 | u>>>(32-7);
    u = x11 + x10 | 0;
    x8 ^= u<<9 | u>>>(32-9);
    u = x8 + x11 | 0;
    x9 ^= u<<13 | u>>>(32-13);
    u = x9 + x8 | 0;
    x10 ^= u<<18 | u>>>(32-18);

    u = x15 + x14 | 0;
    x12 ^= u<<7 | u>>>(32-7);
    u = x12 + x15 | 0;
    x13 ^= u<<9 | u>>>(32-9);
    u = x13 + x12 | 0;
    x14 ^= u<<13 | u>>>(32-13);
    u = x14 + x13 | 0;
    x15 ^= u<<18 | u>>>(32-18);
  }

  o[ 0] = x0 >>>  0 & 0xff;
  o[ 1] = x0 >>>  8 & 0xff;
  o[ 2] = x0 >>> 16 & 0xff;
  o[ 3] = x0 >>> 24 & 0xff;

  o[ 4] = x5 >>>  0 & 0xff;
  o[ 5] = x5 >>>  8 & 0xff;
  o[ 6] = x5 >>> 16 & 0xff;
  o[ 7] = x5 >>> 24 & 0xff;

  o[ 8] = x10 >>>  0 & 0xff;
  o[ 9] = x10 >>>  8 & 0xff;
  o[10] = x10 >>> 16 & 0xff;
  o[11] = x10 >>> 24 & 0xff;

  o[12] = x15 >>>  0 & 0xff;
  o[13] = x15 >>>  8 & 0xff;
  o[14] = x15 >>> 16 & 0xff;
  o[15] = x15 >>> 24 & 0xff;

  o[16] = x6 >>>  0 & 0xff;
  o[17] = x6 >>>  8 & 0xff;
  o[18] = x6 >>> 16 & 0xff;
  o[19] = x6 >>> 24 & 0xff;

  o[20] = x7 >>>  0 & 0xff;
  o[21] = x7 >>>  8 & 0xff;
  o[22] = x7 >>> 16 & 0xff;
  o[23] = x7 >>> 24 & 0xff;

  o[24] = x8 >>>  0 & 0xff;
  o[25] = x8 >>>  8 & 0xff;
  o[26] = x8 >>> 16 & 0xff;
  o[27] = x8 >>> 24 & 0xff;

  o[28] = x9 >>>  0 & 0xff;
  o[29] = x9 >>>  8 & 0xff;
  o[30] = x9 >>> 16 & 0xff;
  o[31] = x9 >>> 24 & 0xff;
}

function crypto_core_salsa20(out,inp,k,c) {
  core_salsa20(out,inp,k,c);
}

function crypto_core_hsalsa20(out,inp,k,c) {
  core_hsalsa20(out,inp,k,c);
}

var sigma = new Uint8Array([101, 120, 112, 97, 110, 100, 32, 51, 50, 45, 98, 121, 116, 101, 32, 107]);
            // "expand 32-byte k"

function crypto_stream_salsa20_xor(c,cpos,m,mpos,b,n,k) {
  var z = new Uint8Array(16), x = new Uint8Array(64);
  var u, i;
  for (i = 0; i < 16; i++) z[i] = 0;
  for (i = 0; i < 8; i++) z[i] = n[i];
  while (b >= 64) {
    crypto_core_salsa20(x,z,k,sigma);
    for (i = 0; i < 64; i++) c[cpos+i] = m[mpos+i] ^ x[i];
    u = 1;
    for (i = 8; i < 16; i++) {
      u = u + (z[i] & 0xff) | 0;
      z[i] = u & 0xff;
      u >>>= 8;
    }
    b -= 64;
    cpos += 64;
    mpos += 64;
  }
  if (b > 0) {
    crypto_core_salsa20(x,z,k,sigma);
    for (i = 0; i < b; i++) c[cpos+i] = m[mpos+i] ^ x[i];
  }
  return 0;
}

function crypto_stream_salsa20(c,cpos,b,n,k) {
  var z = new Uint8Array(16), x = new Uint8Array(64);
  var u, i;
  for (i = 0; i < 16; i++) z[i] = 0;
  for (i = 0; i < 8; i++) z[i] = n[i];
  while (b >= 64) {
    crypto_core_salsa20(x,z,k,sigma);
    for (i = 0; i < 64; i++) c[cpos+i] = x[i];
    u = 1;
    for (i = 8; i < 16; i++) {
      u = u + (z[i] & 0xff) | 0;
      z[i] = u & 0xff;
      u >>>= 8;
    }
    b -= 64;
    cpos += 64;
  }
  if (b > 0) {
    crypto_core_salsa20(x,z,k,sigma);
    for (i = 0; i < b; i++) c[cpos+i] = x[i];
  }
  return 0;
}

function crypto_stream(c,cpos,d,n,k) {
  var s = new Uint8Array(32);
  crypto_core_hsalsa20(s,n,k,sigma);
  var sn = new Uint8Array(8);
  for (var i = 0; i < 8; i++) sn[i] = n[i+16];
  return crypto_stream_salsa20(c,cpos,d,sn,s);
}

function crypto_stream_xor(c,cpos,m,mpos,d,n,k) {
  var s = new Uint8Array(32);
  crypto_core_hsalsa20(s,n,k,sigma);
  var sn = new Uint8Array(8);
  for (var i = 0; i < 8; i++) sn[i] = n[i+16];
  return crypto_stream_salsa20_xor(c,cpos,m,mpos,d,sn,s);
}

/*
* Port of Andrew Moon's Poly1305-donna-16. Public domain.
* https://github.com/floodyberry/poly1305-donna
*/

var poly1305 = function(key) {
  this.buffer = new Uint8Array(16);
  this.r = new Uint16Array(10);
  this.h = new Uint16Array(10);
  this.pad = new Uint16Array(8);
  this.leftover = 0;
  this.fin = 0;

  var t0, t1, t2, t3, t4, t5, t6, t7;

  t0 = key[ 0] & 0xff | (key[ 1] & 0xff) << 8; this.r[0] = ( t0                     ) & 0x1fff;
  t1 = key[ 2] & 0xff | (key[ 3] & 0xff) << 8; this.r[1] = ((t0 >>> 13) | (t1 <<  3)) & 0x1fff;
  t2 = key[ 4] & 0xff | (key[ 5] & 0xff) << 8; this.r[2] = ((t1 >>> 10) | (t2 <<  6)) & 0x1f03;
  t3 = key[ 6] & 0xff | (key[ 7] & 0xff) << 8; this.r[3] = ((t2 >>>  7) | (t3 <<  9)) & 0x1fff;
  t4 = key[ 8] & 0xff | (key[ 9] & 0xff) << 8; this.r[4] = ((t3 >>>  4) | (t4 << 12)) & 0x00ff;
  this.r[5] = ((t4 >>>  1)) & 0x1ffe;
  t5 = key[10] & 0xff | (key[11] & 0xff) << 8; this.r[6] = ((t4 >>> 14) | (t5 <<  2)) & 0x1fff;
  t6 = key[12] & 0xff | (key[13] & 0xff) << 8; this.r[7] = ((t5 >>> 11) | (t6 <<  5)) & 0x1f81;
  t7 = key[14] & 0xff | (key[15] & 0xff) << 8; this.r[8] = ((t6 >>>  8) | (t7 <<  8)) & 0x1fff;
  this.r[9] = ((t7 >>>  5)) & 0x007f;

  this.pad[0] = key[16] & 0xff | (key[17] & 0xff) << 8;
  this.pad[1] = key[18] & 0xff | (key[19] & 0xff) << 8;
  this.pad[2] = key[20] & 0xff | (key[21] & 0xff) << 8;
  this.pad[3] = key[22] & 0xff | (key[23] & 0xff) << 8;
  this.pad[4] = key[24] & 0xff | (key[25] & 0xff) << 8;
  this.pad[5] = key[26] & 0xff | (key[27] & 0xff) << 8;
  this.pad[6] = key[28] & 0xff | (key[29] & 0xff) << 8;
  this.pad[7] = key[30] & 0xff | (key[31] & 0xff) << 8;
};

poly1305.prototype.blocks = function(m, mpos, bytes) {
  var hibit = this.fin ? 0 : (1 << 11);
  var t0, t1, t2, t3, t4, t5, t6, t7, c;
  var d0, d1, d2, d3, d4, d5, d6, d7, d8, d9;

  var h0 = this.h[0],
      h1 = this.h[1],
      h2 = this.h[2],
      h3 = this.h[3],
      h4 = this.h[4],
      h5 = this.h[5],
      h6 = this.h[6],
      h7 = this.h[7],
      h8 = this.h[8],
      h9 = this.h[9];

  var r0 = this.r[0],
      r1 = this.r[1],
      r2 = this.r[2],
      r3 = this.r[3],
      r4 = this.r[4],
      r5 = this.r[5],
      r6 = this.r[6],
      r7 = this.r[7],
      r8 = this.r[8],
      r9 = this.r[9];

  while (bytes >= 16) {
    t0 = m[mpos+ 0] & 0xff | (m[mpos+ 1] & 0xff) << 8; h0 += ( t0                     ) & 0x1fff;
    t1 = m[mpos+ 2] & 0xff | (m[mpos+ 3] & 0xff) << 8; h1 += ((t0 >>> 13) | (t1 <<  3)) & 0x1fff;
    t2 = m[mpos+ 4] & 0xff | (m[mpos+ 5] & 0xff) << 8; h2 += ((t1 >>> 10) | (t2 <<  6)) & 0x1fff;
    t3 = m[mpos+ 6] & 0xff | (m[mpos+ 7] & 0xff) << 8; h3 += ((t2 >>>  7) | (t3 <<  9)) & 0x1fff;
    t4 = m[mpos+ 8] & 0xff | (m[mpos+ 9] & 0xff) << 8; h4 += ((t3 >>>  4) | (t4 << 12)) & 0x1fff;
    h5 += ((t4 >>>  1)) & 0x1fff;
    t5 = m[mpos+10] & 0xff | (m[mpos+11] & 0xff) << 8; h6 += ((t4 >>> 14) | (t5 <<  2)) & 0x1fff;
    t6 = m[mpos+12] & 0xff | (m[mpos+13] & 0xff) << 8; h7 += ((t5 >>> 11) | (t6 <<  5)) & 0x1fff;
    t7 = m[mpos+14] & 0xff | (m[mpos+15] & 0xff) << 8; h8 += ((t6 >>>  8) | (t7 <<  8)) & 0x1fff;
    h9 += ((t7 >>> 5)) | hibit;

    c = 0;

    d0 = c;
    d0 += h0 * r0;
    d0 += h1 * (5 * r9);
    d0 += h2 * (5 * r8);
    d0 += h3 * (5 * r7);
    d0 += h4 * (5 * r6);
    c = (d0 >>> 13); d0 &= 0x1fff;
    d0 += h5 * (5 * r5);
    d0 += h6 * (5 * r4);
    d0 += h7 * (5 * r3);
    d0 += h8 * (5 * r2);
    d0 += h9 * (5 * r1);
    c += (d0 >>> 13); d0 &= 0x1fff;

    d1 = c;
    d1 += h0 * r1;
    d1 += h1 * r0;
    d1 += h2 * (5 * r9);
    d1 += h3 * (5 * r8);
    d1 += h4 * (5 * r7);
    c = (d1 >>> 13); d1 &= 0x1fff;
    d1 += h5 * (5 * r6);
    d1 += h6 * (5 * r5);
    d1 += h7 * (5 * r4);
    d1 += h8 * (5 * r3);
    d1 += h9 * (5 * r2);
    c += (d1 >>> 13); d1 &= 0x1fff;

    d2 = c;
    d2 += h0 * r2;
    d2 += h1 * r1;
    d2 += h2 * r0;
    d2 += h3 * (5 * r9);
    d2 += h4 * (5 * r8);
    c = (d2 >>> 13); d2 &= 0x1fff;
    d2 += h5 * (5 * r7);
    d2 += h6 * (5 * r6);
    d2 += h7 * (5 * r5);
    d2 += h8 * (5 * r4);
    d2 += h9 * (5 * r3);
    c += (d2 >>> 13); d2 &= 0x1fff;

    d3 = c;
    d3 += h0 * r3;
    d3 += h1 * r2;
    d3 += h2 * r1;
    d3 += h3 * r0;
    d3 += h4 * (5 * r9);
    c = (d3 >>> 13); d3 &= 0x1fff;
    d3 += h5 * (5 * r8);
    d3 += h6 * (5 * r7);
    d3 += h7 * (5 * r6);
    d3 += h8 * (5 * r5);
    d3 += h9 * (5 * r4);
    c += (d3 >>> 13); d3 &= 0x1fff;

    d4 = c;
    d4 += h0 * r4;
    d4 += h1 * r3;
    d4 += h2 * r2;
    d4 += h3 * r1;
    d4 += h4 * r0;
    c = (d4 >>> 13); d4 &= 0x1fff;
    d4 += h5 * (5 * r9);
    d4 += h6 * (5 * r8);
    d4 += h7 * (5 * r7);
    d4 += h8 * (5 * r6);
    d4 += h9 * (5 * r5);
    c += (d4 >>> 13); d4 &= 0x1fff;

    d5 = c;
    d5 += h0 * r5;
    d5 += h1 * r4;
    d5 += h2 * r3;
    d5 += h3 * r2;
    d5 += h4 * r1;
    c = (d5 >>> 13); d5 &= 0x1fff;
    d5 += h5 * r0;
    d5 += h6 * (5 * r9);
    d5 += h7 * (5 * r8);
    d5 += h8 * (5 * r7);
    d5 += h9 * (5 * r6);
    c += (d5 >>> 13); d5 &= 0x1fff;

    d6 = c;
    d6 += h0 * r6;
    d6 += h1 * r5;
    d6 += h2 * r4;
    d6 += h3 * r3;
    d6 += h4 * r2;
    c = (d6 >>> 13); d6 &= 0x1fff;
    d6 += h5 * r1;
    d6 += h6 * r0;
    d6 += h7 * (5 * r9);
    d6 += h8 * (5 * r8);
    d6 += h9 * (5 * r7);
    c += (d6 >>> 13); d6 &= 0x1fff;

    d7 = c;
    d7 += h0 * r7;
    d7 += h1 * r6;
    d7 += h2 * r5;
    d7 += h3 * r4;
    d7 += h4 * r3;
    c = (d7 >>> 13); d7 &= 0x1fff;
    d7 += h5 * r2;
    d7 += h6 * r1;
    d7 += h7 * r0;
    d7 += h8 * (5 * r9);
    d7 += h9 * (5 * r8);
    c += (d7 >>> 13); d7 &= 0x1fff;

    d8 = c;
    d8 += h0 * r8;
    d8 += h1 * r7;
    d8 += h2 * r6;
    d8 += h3 * r5;
    d8 += h4 * r4;
    c = (d8 >>> 13); d8 &= 0x1fff;
    d8 += h5 * r3;
    d8 += h6 * r2;
    d8 += h7 * r1;
    d8 += h8 * r0;
    d8 += h9 * (5 * r9);
    c += (d8 >>> 13); d8 &= 0x1fff;

    d9 = c;
    d9 += h0 * r9;
    d9 += h1 * r8;
    d9 += h2 * r7;
    d9 += h3 * r6;
    d9 += h4 * r5;
    c = (d9 >>> 13); d9 &= 0x1fff;
    d9 += h5 * r4;
    d9 += h6 * r3;
    d9 += h7 * r2;
    d9 += h8 * r1;
    d9 += h9 * r0;
    c += (d9 >>> 13); d9 &= 0x1fff;

    c = (((c << 2) + c)) | 0;
    c = (c + d0) | 0;
    d0 = c & 0x1fff;
    c = (c >>> 13);
    d1 += c;

    h0 = d0;
    h1 = d1;
    h2 = d2;
    h3 = d3;
    h4 = d4;
    h5 = d5;
    h6 = d6;
    h7 = d7;
    h8 = d8;
    h9 = d9;

    mpos += 16;
    bytes -= 16;
  }
  this.h[0] = h0;
  this.h[1] = h1;
  this.h[2] = h2;
  this.h[3] = h3;
  this.h[4] = h4;
  this.h[5] = h5;
  this.h[6] = h6;
  this.h[7] = h7;
  this.h[8] = h8;
  this.h[9] = h9;
};

poly1305.prototype.finish = function(mac, macpos) {
  var g = new Uint16Array(10);
  var c, mask, f, i;

  if (this.leftover) {
    i = this.leftover;
    this.buffer[i++] = 1;
    for (; i < 16; i++) this.buffer[i] = 0;
    this.fin = 1;
    this.blocks(this.buffer, 0, 16);
  }

  c = this.h[1] >>> 13;
  this.h[1] &= 0x1fff;
  for (i = 2; i < 10; i++) {
    this.h[i] += c;
    c = this.h[i] >>> 13;
    this.h[i] &= 0x1fff;
  }
  this.h[0] += (c * 5);
  c = this.h[0] >>> 13;
  this.h[0] &= 0x1fff;
  this.h[1] += c;
  c = this.h[1] >>> 13;
  this.h[1] &= 0x1fff;
  this.h[2] += c;

  g[0] = this.h[0] + 5;
  c = g[0] >>> 13;
  g[0] &= 0x1fff;
  for (i = 1; i < 10; i++) {
    g[i] = this.h[i] + c;
    c = g[i] >>> 13;
    g[i] &= 0x1fff;
  }
  g[9] -= (1 << 13);

  mask = (c ^ 1) - 1;
  for (i = 0; i < 10; i++) g[i] &= mask;
  mask = ~mask;
  for (i = 0; i < 10; i++) this.h[i] = (this.h[i] & mask) | g[i];

  this.h[0] = ((this.h[0]       ) | (this.h[1] << 13)                    ) & 0xffff;
  this.h[1] = ((this.h[1] >>>  3) | (this.h[2] << 10)                    ) & 0xffff;
  this.h[2] = ((this.h[2] >>>  6) | (this.h[3] <<  7)                    ) & 0xffff;
  this.h[3] = ((this.h[3] >>>  9) | (this.h[4] <<  4)                    ) & 0xffff;
  this.h[4] = ((this.h[4] >>> 12) | (this.h[5] <<  1) | (this.h[6] << 14)) & 0xffff;
  this.h[5] = ((this.h[6] >>>  2) | (this.h[7] << 11)                    ) & 0xffff;
  this.h[6] = ((this.h[7] >>>  5) | (this.h[8] <<  8)                    ) & 0xffff;
  this.h[7] = ((this.h[8] >>>  8) | (this.h[9] <<  5)                    ) & 0xffff;

  f = this.h[0] + this.pad[0];
  this.h[0] = f & 0xffff;
  for (i = 1; i < 8; i++) {
    f = (((this.h[i] + this.pad[i]) | 0) + (f >>> 16)) | 0;
    this.h[i] = f & 0xffff;
  }

  mac[macpos+ 0] = (this.h[0] >>> 0) & 0xff;
  mac[macpos+ 1] = (this.h[0] >>> 8) & 0xff;
  mac[macpos+ 2] = (this.h[1] >>> 0) & 0xff;
  mac[macpos+ 3] = (this.h[1] >>> 8) & 0xff;
  mac[macpos+ 4] = (this.h[2] >>> 0) & 0xff;
  mac[macpos+ 5] = (this.h[2] >>> 8) & 0xff;
  mac[macpos+ 6] = (this.h[3] >>> 0) & 0xff;
  mac[macpos+ 7] = (this.h[3] >>> 8) & 0xff;
  mac[macpos+ 8] = (this.h[4] >>> 0) & 0xff;
  mac[macpos+ 9] = (this.h[4] >>> 8) & 0xff;
  mac[macpos+10] = (this.h[5] >>> 0) & 0xff;
  mac[macpos+11] = (this.h[5] >>> 8) & 0xff;
  mac[macpos+12] = (this.h[6] >>> 0) & 0xff;
  mac[macpos+13] = (this.h[6] >>> 8) & 0xff;
  mac[macpos+14] = (this.h[7] >>> 0) & 0xff;
  mac[macpos+15] = (this.h[7] >>> 8) & 0xff;
};

poly1305.prototype.update = function(m, mpos, bytes) {
  var i, want;

  if (this.leftover) {
    want = (16 - this.leftover);
    if (want > bytes)
      want = bytes;
    for (i = 0; i < want; i++)
      this.buffer[this.leftover + i] = m[mpos+i];
    bytes -= want;
    mpos += want;
    this.leftover += want;
    if (this.leftover < 16)
      return;
    this.blocks(this.buffer, 0, 16);
    this.leftover = 0;
  }

  if (bytes >= 16) {
    want = bytes - (bytes % 16);
    this.blocks(m, mpos, want);
    mpos += want;
    bytes -= want;
  }

  if (bytes) {
    for (i = 0; i < bytes; i++)
      this.buffer[this.leftover + i] = m[mpos+i];
    this.leftover += bytes;
  }
};

function crypto_onetimeauth(out, outpos, m, mpos, n, k) {
  var s = new poly1305(k);
  s.update(m, mpos, n);
  s.finish(out, outpos);
  return 0;
}

function crypto_onetimeauth_verify(h, hpos, m, mpos, n, k) {
  var x = new Uint8Array(16);
  crypto_onetimeauth(x,0,m,mpos,n,k);
  return crypto_verify_16(h,hpos,x,0);
}

function crypto_secretbox(c,m,d,n,k) {
  var i;
  if (d < 32) return -1;
  crypto_stream_xor(c,0,m,0,d,n,k);
  crypto_onetimeauth(c, 16, c, 32, d - 32, c);
  for (i = 0; i < 16; i++) c[i] = 0;
  return 0;
}

function crypto_secretbox_open(m,c,d,n,k) {
  var i;
  var x = new Uint8Array(32);
  if (d < 32) return -1;
  crypto_stream(x,0,32,n,k);
  if (crypto_onetimeauth_verify(c, 16,c, 32,d - 32,x) !== 0) return -1;
  crypto_stream_xor(m,0,c,0,d,n,k);
  for (i = 0; i < 32; i++) m[i] = 0;
  return 0;
}

function set25519(r, a) {
  var i;
  for (i = 0; i < 16; i++) r[i] = a[i]|0;
}

function car25519(o) {
  var i, v, c = 1;
  for (i = 0; i < 16; i++) {
    v = o[i] + c + 65535;
    c = Math.floor(v / 65536);
    o[i] = v - c * 65536;
  }
  o[0] += c-1 + 37 * (c-1);
}

function sel25519(p, q, b) {
  var t, c = ~(b-1);
  for (var i = 0; i < 16; i++) {
    t = c & (p[i] ^ q[i]);
    p[i] ^= t;
    q[i] ^= t;
  }
}

function pack25519(o, n) {
  var i, j, b;
  var m = gf(), t = gf();
  for (i = 0; i < 16; i++) t[i] = n[i];
  car25519(t);
  car25519(t);
  car25519(t);
  for (j = 0; j < 2; j++) {
    m[0] = t[0] - 0xffed;
    for (i = 1; i < 15; i++) {
      m[i] = t[i] - 0xffff - ((m[i-1]>>16) & 1);
      m[i-1] &= 0xffff;
    }
    m[15] = t[15] - 0x7fff - ((m[14]>>16) & 1);
    b = (m[15]>>16) & 1;
    m[14] &= 0xffff;
    sel25519(t, m, 1-b);
  }
  for (i = 0; i < 16; i++) {
    o[2*i] = t[i] & 0xff;
    o[2*i+1] = t[i]>>8;
  }
}

function neq25519(a, b) {
  var c = new Uint8Array(32), d = new Uint8Array(32);
  pack25519(c, a);
  pack25519(d, b);
  return crypto_verify_32(c, 0, d, 0);
}

function par25519(a) {
  var d = new Uint8Array(32);
  pack25519(d, a);
  return d[0] & 1;
}

function unpack25519(o, n) {
  var i;
  for (i = 0; i < 16; i++) o[i] = n[2*i] + (n[2*i+1] << 8);
  o[15] &= 0x7fff;
}

function A(o, a, b) {
  for (var i = 0; i < 16; i++) o[i] = a[i] + b[i];
}

function Z(o, a, b) {
  for (var i = 0; i < 16; i++) o[i] = a[i] - b[i];
}

function M(o, a, b) {
  var v, c,
     t0 = 0,  t1 = 0,  t2 = 0,  t3 = 0,  t4 = 0,  t5 = 0,  t6 = 0,  t7 = 0,
     t8 = 0,  t9 = 0, t10 = 0, t11 = 0, t12 = 0, t13 = 0, t14 = 0, t15 = 0,
    t16 = 0, t17 = 0, t18 = 0, t19 = 0, t20 = 0, t21 = 0, t22 = 0, t23 = 0,
    t24 = 0, t25 = 0, t26 = 0, t27 = 0, t28 = 0, t29 = 0, t30 = 0,
    b0 = b[0],
    b1 = b[1],
    b2 = b[2],
    b3 = b[3],
    b4 = b[4],
    b5 = b[5],
    b6 = b[6],
    b7 = b[7],
    b8 = b[8],
    b9 = b[9],
    b10 = b[10],
    b11 = b[11],
    b12 = b[12],
    b13 = b[13],
    b14 = b[14],
    b15 = b[15];

  v = a[0];
  t0 += v * b0;
  t1 += v * b1;
  t2 += v * b2;
  t3 += v * b3;
  t4 += v * b4;
  t5 += v * b5;
  t6 += v * b6;
  t7 += v * b7;
  t8 += v * b8;
  t9 += v * b9;
  t10 += v * b10;
  t11 += v * b11;
  t12 += v * b12;
  t13 += v * b13;
  t14 += v * b14;
  t15 += v * b15;
  v = a[1];
  t1 += v * b0;
  t2 += v * b1;
  t3 += v * b2;
  t4 += v * b3;
  t5 += v * b4;
  t6 += v * b5;
  t7 += v * b6;
  t8 += v * b7;
  t9 += v * b8;
  t10 += v * b9;
  t11 += v * b10;
  t12 += v * b11;
  t13 += v * b12;
  t14 += v * b13;
  t15 += v * b14;
  t16 += v * b15;
  v = a[2];
  t2 += v * b0;
  t3 += v * b1;
  t4 += v * b2;
  t5 += v * b3;
  t6 += v * b4;
  t7 += v * b5;
  t8 += v * b6;
  t9 += v * b7;
  t10 += v * b8;
  t11 += v * b9;
  t12 += v * b10;
  t13 += v * b11;
  t14 += v * b12;
  t15 += v * b13;
  t16 += v * b14;
  t17 += v * b15;
  v = a[3];
  t3 += v * b0;
  t4 += v * b1;
  t5 += v * b2;
  t6 += v * b3;
  t7 += v * b4;
  t8 += v * b5;
  t9 += v * b6;
  t10 += v * b7;
  t11 += v * b8;
  t12 += v * b9;
  t13 += v * b10;
  t14 += v * b11;
  t15 += v * b12;
  t16 += v * b13;
  t17 += v * b14;
  t18 += v * b15;
  v = a[4];
  t4 += v * b0;
  t5 += v * b1;
  t6 += v * b2;
  t7 += v * b3;
  t8 += v * b4;
  t9 += v * b5;
  t10 += v * b6;
  t11 += v * b7;
  t12 += v * b8;
  t13 += v * b9;
  t14 += v * b10;
  t15 += v * b11;
  t16 += v * b12;
  t17 += v * b13;
  t18 += v * b14;
  t19 += v * b15;
  v = a[5];
  t5 += v * b0;
  t6 += v * b1;
  t7 += v * b2;
  t8 += v * b3;
  t9 += v * b4;
  t10 += v * b5;
  t11 += v * b6;
  t12 += v * b7;
  t13 += v * b8;
  t14 += v * b9;
  t15 += v * b10;
  t16 += v * b11;
  t17 += v * b12;
  t18 += v * b13;
  t19 += v * b14;
  t20 += v * b15;
  v = a[6];
  t6 += v * b0;
  t7 += v * b1;
  t8 += v * b2;
  t9 += v * b3;
  t10 += v * b4;
  t11 += v * b5;
  t12 += v * b6;
  t13 += v * b7;
  t14 += v * b8;
  t15 += v * b9;
  t16 += v * b10;
  t17 += v * b11;
  t18 += v * b12;
  t19 += v * b13;
  t20 += v * b14;
  t21 += v * b15;
  v = a[7];
  t7 += v * b0;
  t8 += v * b1;
  t9 += v * b2;
  t10 += v * b3;
  t11 += v * b4;
  t12 += v * b5;
  t13 += v * b6;
  t14 += v * b7;
  t15 += v * b8;
  t16 += v * b9;
  t17 += v * b10;
  t18 += v * b11;
  t19 += v * b12;
  t20 += v * b13;
  t21 += v * b14;
  t22 += v * b15;
  v = a[8];
  t8 += v * b0;
  t9 += v * b1;
  t10 += v * b2;
  t11 += v * b3;
  t12 += v * b4;
  t13 += v * b5;
  t14 += v * b6;
  t15 += v * b7;
  t16 += v * b8;
  t17 += v * b9;
  t18 += v * b10;
  t19 += v * b11;
  t20 += v * b12;
  t21 += v * b13;
  t22 += v * b14;
  t23 += v * b15;
  v = a[9];
  t9 += v * b0;
  t10 += v * b1;
  t11 += v * b2;
  t12 += v * b3;
  t13 += v * b4;
  t14 += v * b5;
  t15 += v * b6;
  t16 += v * b7;
  t17 += v * b8;
  t18 += v * b9;
  t19 += v * b10;
  t20 += v * b11;
  t21 += v * b12;
  t22 += v * b13;
  t23 += v * b14;
  t24 += v * b15;
  v = a[10];
  t10 += v * b0;
  t11 += v * b1;
  t12 += v * b2;
  t13 += v * b3;
  t14 += v * b4;
  t15 += v * b5;
  t16 += v * b6;
  t17 += v * b7;
  t18 += v * b8;
  t19 += v * b9;
  t20 += v * b10;
  t21 += v * b11;
  t22 += v * b12;
  t23 += v * b13;
  t24 += v * b14;
  t25 += v * b15;
  v = a[11];
  t11 += v * b0;
  t12 += v * b1;
  t13 += v * b2;
  t14 += v * b3;
  t15 += v * b4;
  t16 += v * b5;
  t17 += v * b6;
  t18 += v * b7;
  t19 += v * b8;
  t20 += v * b9;
  t21 += v * b10;
  t22 += v * b11;
  t23 += v * b12;
  t24 += v * b13;
  t25 += v * b14;
  t26 += v * b15;
  v = a[12];
  t12 += v * b0;
  t13 += v * b1;
  t14 += v * b2;
  t15 += v * b3;
  t16 += v * b4;
  t17 += v * b5;
  t18 += v * b6;
  t19 += v * b7;
  t20 += v * b8;
  t21 += v * b9;
  t22 += v * b10;
  t23 += v * b11;
  t24 += v * b12;
  t25 += v * b13;
  t26 += v * b14;
  t27 += v * b15;
  v = a[13];
  t13 += v * b0;
  t14 += v * b1;
  t15 += v * b2;
  t16 += v * b3;
  t17 += v * b4;
  t18 += v * b5;
  t19 += v * b6;
  t20 += v * b7;
  t21 += v * b8;
  t22 += v * b9;
  t23 += v * b10;
  t24 += v * b11;
  t25 += v * b12;
  t26 += v * b13;
  t27 += v * b14;
  t28 += v * b15;
  v = a[14];
  t14 += v * b0;
  t15 += v * b1;
  t16 += v * b2;
  t17 += v * b3;
  t18 += v * b4;
  t19 += v * b5;
  t20 += v * b6;
  t21 += v * b7;
  t22 += v * b8;
  t23 += v * b9;
  t24 += v * b10;
  t25 += v * b11;
  t26 += v * b12;
  t27 += v * b13;
  t28 += v * b14;
  t29 += v * b15;
  v = a[15];
  t15 += v * b0;
  t16 += v * b1;
  t17 += v * b2;
  t18 += v * b3;
  t19 += v * b4;
  t20 += v * b5;
  t21 += v * b6;
  t22 += v * b7;
  t23 += v * b8;
  t24 += v * b9;
  t25 += v * b10;
  t26 += v * b11;
  t27 += v * b12;
  t28 += v * b13;
  t29 += v * b14;
  t30 += v * b15;

  t0  += 38 * t16;
  t1  += 38 * t17;
  t2  += 38 * t18;
  t3  += 38 * t19;
  t4  += 38 * t20;
  t5  += 38 * t21;
  t6  += 38 * t22;
  t7  += 38 * t23;
  t8  += 38 * t24;
  t9  += 38 * t25;
  t10 += 38 * t26;
  t11 += 38 * t27;
  t12 += 38 * t28;
  t13 += 38 * t29;
  t14 += 38 * t30;
  // t15 left as is

  // first car
  c = 1;
  v =  t0 + c + 65535; c = Math.floor(v / 65536);  t0 = v - c * 65536;
  v =  t1 + c + 65535; c = Math.floor(v / 65536);  t1 = v - c * 65536;
  v =  t2 + c + 65535; c = Math.floor(v / 65536);  t2 = v - c * 65536;
  v =  t3 + c + 65535; c = Math.floor(v / 65536);  t3 = v - c * 65536;
  v =  t4 + c + 65535; c = Math.floor(v / 65536);  t4 = v - c * 65536;
  v =  t5 + c + 65535; c = Math.floor(v / 65536);  t5 = v - c * 65536;
  v =  t6 + c + 65535; c = Math.floor(v / 65536);  t6 = v - c * 65536;
  v =  t7 + c + 65535; c = Math.floor(v / 65536);  t7 = v - c * 65536;
  v =  t8 + c + 65535; c = Math.floor(v / 65536);  t8 = v - c * 65536;
  v =  t9 + c + 65535; c = Math.floor(v / 65536);  t9 = v - c * 65536;
  v = t10 + c + 65535; c = Math.floor(v / 65536); t10 = v - c * 65536;
  v = t11 + c + 65535; c = Math.floor(v / 65536); t11 = v - c * 65536;
  v = t12 + c + 65535; c = Math.floor(v / 65536); t12 = v - c * 65536;
  v = t13 + c + 65535; c = Math.floor(v / 65536); t13 = v - c * 65536;
  v = t14 + c + 65535; c = Math.floor(v / 65536); t14 = v - c * 65536;
  v = t15 + c + 65535; c = Math.floor(v / 65536); t15 = v - c * 65536;
  t0 += c-1 + 37 * (c-1);

  // second car
  c = 1;
  v =  t0 + c + 65535; c = Math.floor(v / 65536);  t0 = v - c * 65536;
  v =  t1 + c + 65535; c = Math.floor(v / 65536);  t1 = v - c * 65536;
  v =  t2 + c + 65535; c = Math.floor(v / 65536);  t2 = v - c * 65536;
  v =  t3 + c + 65535; c = Math.floor(v / 65536);  t3 = v - c * 65536;
  v =  t4 + c + 65535; c = Math.floor(v / 65536);  t4 = v - c * 65536;
  v =  t5 + c + 65535; c = Math.floor(v / 65536);  t5 = v - c * 65536;
  v =  t6 + c + 65535; c = Math.floor(v / 65536);  t6 = v - c * 65536;
  v =  t7 + c + 65535; c = Math.floor(v / 65536);  t7 = v - c * 65536;
  v =  t8 + c + 65535; c = Math.floor(v / 65536);  t8 = v - c * 65536;
  v =  t9 + c + 65535; c = Math.floor(v / 65536);  t9 = v - c * 65536;
  v = t10 + c + 65535; c = Math.floor(v / 65536); t10 = v - c * 65536;
  v = t11 + c + 65535; c = Math.floor(v / 65536); t11 = v - c * 65536;
  v = t12 + c + 65535; c = Math.floor(v / 65536); t12 = v - c * 65536;
  v = t13 + c + 65535; c = Math.floor(v / 65536); t13 = v - c * 65536;
  v = t14 + c + 65535; c = Math.floor(v / 65536); t14 = v - c * 65536;
  v = t15 + c + 65535; c = Math.floor(v / 65536); t15 = v - c * 65536;
  t0 += c-1 + 37 * (c-1);

  o[ 0] = t0;
  o[ 1] = t1;
  o[ 2] = t2;
  o[ 3] = t3;
  o[ 4] = t4;
  o[ 5] = t5;
  o[ 6] = t6;
  o[ 7] = t7;
  o[ 8] = t8;
  o[ 9] = t9;
  o[10] = t10;
  o[11] = t11;
  o[12] = t12;
  o[13] = t13;
  o[14] = t14;
  o[15] = t15;
}

function S(o, a) {
  M(o, a, a);
}

function inv25519(o, i) {
  var c = gf();
  var a;
  for (a = 0; a < 16; a++) c[a] = i[a];
  for (a = 253; a >= 0; a--) {
    S(c, c);
    if(a !== 2 && a !== 4) M(c, c, i);
  }
  for (a = 0; a < 16; a++) o[a] = c[a];
}

function pow2523(o, i) {
  var c = gf();
  var a;
  for (a = 0; a < 16; a++) c[a] = i[a];
  for (a = 250; a >= 0; a--) {
      S(c, c);
      if(a !== 1) M(c, c, i);
  }
  for (a = 0; a < 16; a++) o[a] = c[a];
}

function crypto_scalarmult(q, n, p) {
  var z = new Uint8Array(32);
  var x = new Float64Array(80), r, i;
  var a = gf(), b = gf(), c = gf(),
      d = gf(), e = gf(), f = gf();
  for (i = 0; i < 31; i++) z[i] = n[i];
  z[31]=(n[31]&127)|64;
  z[0]&=248;
  unpack25519(x,p);
  for (i = 0; i < 16; i++) {
    b[i]=x[i];
    d[i]=a[i]=c[i]=0;
  }
  a[0]=d[0]=1;
  for (i=254; i>=0; --i) {
    r=(z[i>>>3]>>>(i&7))&1;
    sel25519(a,b,r);
    sel25519(c,d,r);
    A(e,a,c);
    Z(a,a,c);
    A(c,b,d);
    Z(b,b,d);
    S(d,e);
    S(f,a);
    M(a,c,a);
    M(c,b,e);
    A(e,a,c);
    Z(a,a,c);
    S(b,a);
    Z(c,d,f);
    M(a,c,_121665);
    A(a,a,d);
    M(c,c,a);
    M(a,d,f);
    M(d,b,x);
    S(b,e);
    sel25519(a,b,r);
    sel25519(c,d,r);
  }
  for (i = 0; i < 16; i++) {
    x[i+16]=a[i];
    x[i+32]=c[i];
    x[i+48]=b[i];
    x[i+64]=d[i];
  }
  var x32 = x.subarray(32);
  var x16 = x.subarray(16);
  inv25519(x32,x32);
  M(x16,x16,x32);
  pack25519(q,x16);
  return 0;
}

function crypto_scalarmult_base(q, n) {
  return crypto_scalarmult(q, n, _9);
}

function crypto_box_keypair(y, x) {
  randombytes(x, 32);
  return crypto_scalarmult_base(y, x);
}

function crypto_box_beforenm(k, y, x) {
  var s = new Uint8Array(32);
  crypto_scalarmult(s, x, y);
  return crypto_core_hsalsa20(k, _0, s, sigma);
}

var crypto_box_afternm = crypto_secretbox;
var crypto_box_open_afternm = crypto_secretbox_open;

function crypto_box(c, m, d, n, y, x) {
  var k = new Uint8Array(32);
  crypto_box_beforenm(k, y, x);
  return crypto_box_afternm(c, m, d, n, k);
}

function crypto_box_open(m, c, d, n, y, x) {
  var k = new Uint8Array(32);
  crypto_box_beforenm(k, y, x);
  return crypto_box_open_afternm(m, c, d, n, k);
}

var K = [
  0x428a2f98, 0xd728ae22, 0x71374491, 0x23ef65cd,
  0xb5c0fbcf, 0xec4d3b2f, 0xe9b5dba5, 0x8189dbbc,
  0x3956c25b, 0xf348b538, 0x59f111f1, 0xb605d019,
  0x923f82a4, 0xaf194f9b, 0xab1c5ed5, 0xda6d8118,
  0xd807aa98, 0xa3030242, 0x12835b01, 0x45706fbe,
  0x243185be, 0x4ee4b28c, 0x550c7dc3, 0xd5ffb4e2,
  0x72be5d74, 0xf27b896f, 0x80deb1fe, 0x3b1696b1,
  0x9bdc06a7, 0x25c71235, 0xc19bf174, 0xcf692694,
  0xe49b69c1, 0x9ef14ad2, 0xefbe4786, 0x384f25e3,
  0x0fc19dc6, 0x8b8cd5b5, 0x240ca1cc, 0x77ac9c65,
  0x2de92c6f, 0x592b0275, 0x4a7484aa, 0x6ea6e483,
  0x5cb0a9dc, 0xbd41fbd4, 0x76f988da, 0x831153b5,
  0x983e5152, 0xee66dfab, 0xa831c66d, 0x2db43210,
  0xb00327c8, 0x98fb213f, 0xbf597fc7, 0xbeef0ee4,
  0xc6e00bf3, 0x3da88fc2, 0xd5a79147, 0x930aa725,
  0x06ca6351, 0xe003826f, 0x14292967, 0x0a0e6e70,
  0x27b70a85, 0x46d22ffc, 0x2e1b2138, 0x5c26c926,
  0x4d2c6dfc, 0x5ac42aed, 0x53380d13, 0x9d95b3df,
  0x650a7354, 0x8baf63de, 0x766a0abb, 0x3c77b2a8,
  0x81c2c92e, 0x47edaee6, 0x92722c85, 0x1482353b,
  0xa2bfe8a1, 0x4cf10364, 0xa81a664b, 0xbc423001,
  0xc24b8b70, 0xd0f89791, 0xc76c51a3, 0x0654be30,
  0xd192e819, 0xd6ef5218, 0xd6990624, 0x5565a910,
  0xf40e3585, 0x5771202a, 0x106aa070, 0x32bbd1b8,
  0x19a4c116, 0xb8d2d0c8, 0x1e376c08, 0x5141ab53,
  0x2748774c, 0xdf8eeb99, 0x34b0bcb5, 0xe19b48a8,
  0x391c0cb3, 0xc5c95a63, 0x4ed8aa4a, 0xe3418acb,
  0x5b9cca4f, 0x7763e373, 0x682e6ff3, 0xd6b2b8a3,
  0x748f82ee, 0x5defb2fc, 0x78a5636f, 0x43172f60,
  0x84c87814, 0xa1f0ab72, 0x8cc70208, 0x1a6439ec,
  0x90befffa, 0x23631e28, 0xa4506ceb, 0xde82bde9,
  0xbef9a3f7, 0xb2c67915, 0xc67178f2, 0xe372532b,
  0xca273ece, 0xea26619c, 0xd186b8c7, 0x21c0c207,
  0xeada7dd6, 0xcde0eb1e, 0xf57d4f7f, 0xee6ed178,
  0x06f067aa, 0x72176fba, 0x0a637dc5, 0xa2c898a6,
  0x113f9804, 0xbef90dae, 0x1b710b35, 0x131c471b,
  0x28db77f5, 0x23047d84, 0x32caab7b, 0x40c72493,
  0x3c9ebe0a, 0x15c9bebc, 0x431d67c4, 0x9c100d4c,
  0x4cc5d4be, 0xcb3e42b6, 0x597f299c, 0xfc657e2a,
  0x5fcb6fab, 0x3ad6faec, 0x6c44198c, 0x4a475817
];

function crypto_hashblocks_hl(hh, hl, m, n) {
  var wh = new Int32Array(16), wl = new Int32Array(16),
      bh0, bh1, bh2, bh3, bh4, bh5, bh6, bh7,
      bl0, bl1, bl2, bl3, bl4, bl5, bl6, bl7,
      th, tl, i, j, h, l, a, b, c, d;

  var ah0 = hh[0],
      ah1 = hh[1],
      ah2 = hh[2],
      ah3 = hh[3],
      ah4 = hh[4],
      ah5 = hh[5],
      ah6 = hh[6],
      ah7 = hh[7],

      al0 = hl[0],
      al1 = hl[1],
      al2 = hl[2],
      al3 = hl[3],
      al4 = hl[4],
      al5 = hl[5],
      al6 = hl[6],
      al7 = hl[7];

  var pos = 0;
  while (n >= 128) {
    for (i = 0; i < 16; i++) {
      j = 8 * i + pos;
      wh[i] = (m[j+0] << 24) | (m[j+1] << 16) | (m[j+2] << 8) | m[j+3];
      wl[i] = (m[j+4] << 24) | (m[j+5] << 16) | (m[j+6] << 8) | m[j+7];
    }
    for (i = 0; i < 80; i++) {
      bh0 = ah0;
      bh1 = ah1;
      bh2 = ah2;
      bh3 = ah3;
      bh4 = ah4;
      bh5 = ah5;
      bh6 = ah6;
      bh7 = ah7;

      bl0 = al0;
      bl1 = al1;
      bl2 = al2;
      bl3 = al3;
      bl4 = al4;
      bl5 = al5;
      bl6 = al6;
      bl7 = al7;

      // add
      h = ah7;
      l = al7;

      a = l & 0xffff; b = l >>> 16;
      c = h & 0xffff; d = h >>> 16;

      // Sigma1
      h = ((ah4 >>> 14) | (al4 << (32-14))) ^ ((ah4 >>> 18) | (al4 << (32-18))) ^ ((al4 >>> (41-32)) | (ah4 << (32-(41-32))));
      l = ((al4 >>> 14) | (ah4 << (32-14))) ^ ((al4 >>> 18) | (ah4 << (32-18))) ^ ((ah4 >>> (41-32)) | (al4 << (32-(41-32))));

      a += l & 0xffff; b += l >>> 16;
      c += h & 0xffff; d += h >>> 16;

      // Ch
      h = (ah4 & ah5) ^ (~ah4 & ah6);
      l = (al4 & al5) ^ (~al4 & al6);

      a += l & 0xffff; b += l >>> 16;
      c += h & 0xffff; d += h >>> 16;

      // K
      h = K[i*2];
      l = K[i*2+1];

      a += l & 0xffff; b += l >>> 16;
      c += h & 0xffff; d += h >>> 16;

      // w
      h = wh[i%16];
      l = wl[i%16];

      a += l & 0xffff; b += l >>> 16;
      c += h & 0xffff; d += h >>> 16;

      b += a >>> 16;
      c += b >>> 16;
      d += c >>> 16;

      th = c & 0xffff | d << 16;
      tl = a & 0xffff | b << 16;

      // add
      h = th;
      l = tl;

      a = l & 0xffff; b = l >>> 16;
      c = h & 0xffff; d = h >>> 16;

      // Sigma0
      h = ((ah0 >>> 28) | (al0 << (32-28))) ^ ((al0 >>> (34-32)) | (ah0 << (32-(34-32)))) ^ ((al0 >>> (39-32)) | (ah0 << (32-(39-32))));
      l = ((al0 >>> 28) | (ah0 << (32-28))) ^ ((ah0 >>> (34-32)) | (al0 << (32-(34-32)))) ^ ((ah0 >>> (39-32)) | (al0 << (32-(39-32))));

      a += l & 0xffff; b += l >>> 16;
      c += h & 0xffff; d += h >>> 16;

      // Maj
      h = (ah0 & ah1) ^ (ah0 & ah2) ^ (ah1 & ah2);
      l = (al0 & al1) ^ (al0 & al2) ^ (al1 & al2);

      a += l & 0xffff; b += l >>> 16;
      c += h & 0xffff; d += h >>> 16;

      b += a >>> 16;
      c += b >>> 16;
      d += c >>> 16;

      bh7 = (c & 0xffff) | (d << 16);
      bl7 = (a & 0xffff) | (b << 16);

      // add
      h = bh3;
      l = bl3;

      a = l & 0xffff; b = l >>> 16;
      c = h & 0xffff; d = h >>> 16;

      h = th;
      l = tl;

      a += l & 0xffff; b += l >>> 16;
      c += h & 0xffff; d += h >>> 16;

      b += a >>> 16;
      c += b >>> 16;
      d += c >>> 16;

      bh3 = (c & 0xffff) | (d << 16);
      bl3 = (a & 0xffff) | (b << 16);

      ah1 = bh0;
      ah2 = bh1;
      ah3 = bh2;
      ah4 = bh3;
      ah5 = bh4;
      ah6 = bh5;
      ah7 = bh6;
      ah0 = bh7;

      al1 = bl0;
      al2 = bl1;
      al3 = bl2;
      al4 = bl3;
      al5 = bl4;
      al6 = bl5;
      al7 = bl6;
      al0 = bl7;

      if (i%16 === 15) {
        for (j = 0; j < 16; j++) {
          // add
          h = wh[j];
          l = wl[j];

          a = l & 0xffff; b = l >>> 16;
          c = h & 0xffff; d = h >>> 16;

          h = wh[(j+9)%16];
          l = wl[(j+9)%16];

          a += l & 0xffff; b += l >>> 16;
          c += h & 0xffff; d += h >>> 16;

          // sigma0
          th = wh[(j+1)%16];
          tl = wl[(j+1)%16];
          h = ((th >>> 1) | (tl << (32-1))) ^ ((th >>> 8) | (tl << (32-8))) ^ (th >>> 7);
          l = ((tl >>> 1) | (th << (32-1))) ^ ((tl >>> 8) | (th << (32-8))) ^ ((tl >>> 7) | (th << (32-7)));

          a += l & 0xffff; b += l >>> 16;
          c += h & 0xffff; d += h >>> 16;

          // sigma1
          th = wh[(j+14)%16];
          tl = wl[(j+14)%16];
          h = ((th >>> 19) | (tl << (32-19))) ^ ((tl >>> (61-32)) | (th << (32-(61-32)))) ^ (th >>> 6);
          l = ((tl >>> 19) | (th << (32-19))) ^ ((th >>> (61-32)) | (tl << (32-(61-32)))) ^ ((tl >>> 6) | (th << (32-6)));

          a += l & 0xffff; b += l >>> 16;
          c += h & 0xffff; d += h >>> 16;

          b += a >>> 16;
          c += b >>> 16;
          d += c >>> 16;

          wh[j] = (c & 0xffff) | (d << 16);
          wl[j] = (a & 0xffff) | (b << 16);
        }
      }
    }

    // add
    h = ah0;
    l = al0;

    a = l & 0xffff; b = l >>> 16;
    c = h & 0xffff; d = h >>> 16;

    h = hh[0];
    l = hl[0];

    a += l & 0xffff; b += l >>> 16;
    c += h & 0xffff; d += h >>> 16;

    b += a >>> 16;
    c += b >>> 16;
    d += c >>> 16;

    hh[0] = ah0 = (c & 0xffff) | (d << 16);
    hl[0] = al0 = (a & 0xffff) | (b << 16);

    h = ah1;
    l = al1;

    a = l & 0xffff; b = l >>> 16;
    c = h & 0xffff; d = h >>> 16;

    h = hh[1];
    l = hl[1];

    a += l & 0xffff; b += l >>> 16;
    c += h & 0xffff; d += h >>> 16;

    b += a >>> 16;
    c += b >>> 16;
    d += c >>> 16;

    hh[1] = ah1 = (c & 0xffff) | (d << 16);
    hl[1] = al1 = (a & 0xffff) | (b << 16);

    h = ah2;
    l = al2;

    a = l & 0xffff; b = l >>> 16;
    c = h & 0xffff; d = h >>> 16;

    h = hh[2];
    l = hl[2];

    a += l & 0xffff; b += l >>> 16;
    c += h & 0xffff; d += h >>> 16;

    b += a >>> 16;
    c += b >>> 16;
    d += c >>> 16;

    hh[2] = ah2 = (c & 0xffff) | (d << 16);
    hl[2] = al2 = (a & 0xffff) | (b << 16);

    h = ah3;
    l = al3;

    a = l & 0xffff; b = l >>> 16;
    c = h & 0xffff; d = h >>> 16;

    h = hh[3];
    l = hl[3];

    a += l & 0xffff; b += l >>> 16;
    c += h & 0xffff; d += h >>> 16;

    b += a >>> 16;
    c += b >>> 16;
    d += c >>> 16;

    hh[3] = ah3 = (c & 0xffff) | (d << 16);
    hl[3] = al3 = (a & 0xffff) | (b << 16);

    h = ah4;
    l = al4;

    a = l & 0xffff; b = l >>> 16;
    c = h & 0xffff; d = h >>> 16;

    h = hh[4];
    l = hl[4];

    a += l & 0xffff; b += l >>> 16;
    c += h & 0xffff; d += h >>> 16;

    b += a >>> 16;
    c += b >>> 16;
    d += c >>> 16;

    hh[4] = ah4 = (c & 0xffff) | (d << 16);
    hl[4] = al4 = (a & 0xffff) | (b << 16);

    h = ah5;
    l = al5;

    a = l & 0xffff; b = l >>> 16;
    c = h & 0xffff; d = h >>> 16;

    h = hh[5];
    l = hl[5];

    a += l & 0xffff; b += l >>> 16;
    c += h & 0xffff; d += h >>> 16;

    b += a >>> 16;
    c += b >>> 16;
    d += c >>> 16;

    hh[5] = ah5 = (c & 0xffff) | (d << 16);
    hl[5] = al5 = (a & 0xffff) | (b << 16);

    h = ah6;
    l = al6;

    a = l & 0xffff; b = l >>> 16;
    c = h & 0xffff; d = h >>> 16;

    h = hh[6];
    l = hl[6];

    a += l & 0xffff; b += l >>> 16;
    c += h & 0xffff; d += h >>> 16;

    b += a >>> 16;
    c += b >>> 16;
    d += c >>> 16;

    hh[6] = ah6 = (c & 0xffff) | (d << 16);
    hl[6] = al6 = (a & 0xffff) | (b << 16);

    h = ah7;
    l = al7;

    a = l & 0xffff; b = l >>> 16;
    c = h & 0xffff; d = h >>> 16;

    h = hh[7];
    l = hl[7];

    a += l & 0xffff; b += l >>> 16;
    c += h & 0xffff; d += h >>> 16;

    b += a >>> 16;
    c += b >>> 16;
    d += c >>> 16;

    hh[7] = ah7 = (c & 0xffff) | (d << 16);
    hl[7] = al7 = (a & 0xffff) | (b << 16);

    pos += 128;
    n -= 128;
  }

  return n;
}

function crypto_hash(out, m, n) {
  var hh = new Int32Array(8),
      hl = new Int32Array(8),
      x = new Uint8Array(256),
      i, b = n;

  hh[0] = 0x6a09e667;
  hh[1] = 0xbb67ae85;
  hh[2] = 0x3c6ef372;
  hh[3] = 0xa54ff53a;
  hh[4] = 0x510e527f;
  hh[5] = 0x9b05688c;
  hh[6] = 0x1f83d9ab;
  hh[7] = 0x5be0cd19;

  hl[0] = 0xf3bcc908;
  hl[1] = 0x84caa73b;
  hl[2] = 0xfe94f82b;
  hl[3] = 0x5f1d36f1;
  hl[4] = 0xade682d1;
  hl[5] = 0x2b3e6c1f;
  hl[6] = 0xfb41bd6b;
  hl[7] = 0x137e2179;

  crypto_hashblocks_hl(hh, hl, m, n);
  n %= 128;

  for (i = 0; i < n; i++) x[i] = m[b-n+i];
  x[n] = 128;

  n = 256-128*(n<112?1:0);
  x[n-9] = 0;
  ts64(x, n-8,  (b / 0x20000000) | 0, b << 3);
  crypto_hashblocks_hl(hh, hl, x, n);

  for (i = 0; i < 8; i++) ts64(out, 8*i, hh[i], hl[i]);

  return 0;
}

function add(p, q) {
  var a = gf(), b = gf(), c = gf(),
      d = gf(), e = gf(), f = gf(),
      g = gf(), h = gf(), t = gf();

  Z(a, p[1], p[0]);
  Z(t, q[1], q[0]);
  M(a, a, t);
  A(b, p[0], p[1]);
  A(t, q[0], q[1]);
  M(b, b, t);
  M(c, p[3], q[3]);
  M(c, c, D2);
  M(d, p[2], q[2]);
  A(d, d, d);
  Z(e, b, a);
  Z(f, d, c);
  A(g, d, c);
  A(h, b, a);

  M(p[0], e, f);
  M(p[1], h, g);
  M(p[2], g, f);
  M(p[3], e, h);
}

function cswap(p, q, b) {
  var i;
  for (i = 0; i < 4; i++) {
    sel25519(p[i], q[i], b);
  }
}

function pack(r, p) {
  var tx = gf(), ty = gf(), zi = gf();
  inv25519(zi, p[2]);
  M(tx, p[0], zi);
  M(ty, p[1], zi);
  pack25519(r, ty);
  r[31] ^= par25519(tx) << 7;
}

function scalarmult(p, q, s) {
  var b, i;
  set25519(p[0], gf0);
  set25519(p[1], gf1);
  set25519(p[2], gf1);
  set25519(p[3], gf0);
  for (i = 255; i >= 0; --i) {
    b = (s[(i/8)|0] >> (i&7)) & 1;
    cswap(p, q, b);
    add(q, p);
    add(p, p);
    cswap(p, q, b);
  }
}

function scalarbase(p, s) {
  var q = [gf(), gf(), gf(), gf()];
  set25519(q[0], X);
  set25519(q[1], Y);
  set25519(q[2], gf1);
  M(q[3], X, Y);
  scalarmult(p, q, s);
}

function crypto_sign_keypair(pk, sk, seeded) {
  var d = new Uint8Array(64);
  var p = [gf(), gf(), gf(), gf()];
  var i;

  if (!seeded) randombytes(sk, 32);
  crypto_hash(d, sk, 32);
  d[0] &= 248;
  d[31] &= 127;
  d[31] |= 64;

  scalarbase(p, d);
  pack(pk, p);

  for (i = 0; i < 32; i++) sk[i+32] = pk[i];
  return 0;
}

var L = new Float64Array([0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10]);

function modL(r, x) {
  var carry, i, j, k;
  for (i = 63; i >= 32; --i) {
    carry = 0;
    for (j = i - 32, k = i - 12; j < k; ++j) {
      x[j] += carry - 16 * x[i] * L[j - (i - 32)];
      carry = (x[j] + 128) >> 8;
      x[j] -= carry * 256;
    }
    x[j] += carry;
    x[i] = 0;
  }
  carry = 0;
  for (j = 0; j < 32; j++) {
    x[j] += carry - (x[31] >> 4) * L[j];
    carry = x[j] >> 8;
    x[j] &= 255;
  }
  for (j = 0; j < 32; j++) x[j] -= carry * L[j];
  for (i = 0; i < 32; i++) {
    x[i+1] += x[i] >> 8;
    r[i] = x[i] & 255;
  }
}

function reduce(r) {
  var x = new Float64Array(64), i;
  for (i = 0; i < 64; i++) x[i] = r[i];
  for (i = 0; i < 64; i++) r[i] = 0;
  modL(r, x);
}

// Note: difference from C - smlen returned, not passed as argument.
function crypto_sign(sm, m, n, sk) {
  var d = new Uint8Array(64), h = new Uint8Array(64), r = new Uint8Array(64);
  var i, j, x = new Float64Array(64);
  var p = [gf(), gf(), gf(), gf()];

  crypto_hash(d, sk, 32);
  d[0] &= 248;
  d[31] &= 127;
  d[31] |= 64;

  var smlen = n + 64;
  for (i = 0; i < n; i++) sm[64 + i] = m[i];
  for (i = 0; i < 32; i++) sm[32 + i] = d[32 + i];

  crypto_hash(r, sm.subarray(32), n+32);
  reduce(r);
  scalarbase(p, r);
  pack(sm, p);

  for (i = 32; i < 64; i++) sm[i] = sk[i];
  crypto_hash(h, sm, n + 64);
  reduce(h);

  for (i = 0; i < 64; i++) x[i] = 0;
  for (i = 0; i < 32; i++) x[i] = r[i];
  for (i = 0; i < 32; i++) {
    for (j = 0; j < 32; j++) {
      x[i+j] += h[i] * d[j];
    }
  }

  modL(sm.subarray(32), x);
  return smlen;
}

function unpackneg(r, p) {
  var t = gf(), chk = gf(), num = gf(),
      den = gf(), den2 = gf(), den4 = gf(),
      den6 = gf();

  set25519(r[2], gf1);
  unpack25519(r[1], p);
  S(num, r[1]);
  M(den, num, D);
  Z(num, num, r[2]);
  A(den, r[2], den);

  S(den2, den);
  S(den4, den2);
  M(den6, den4, den2);
  M(t, den6, num);
  M(t, t, den);

  pow2523(t, t);
  M(t, t, num);
  M(t, t, den);
  M(t, t, den);
  M(r[0], t, den);

  S(chk, r[0]);
  M(chk, chk, den);
  if (neq25519(chk, num)) M(r[0], r[0], I);

  S(chk, r[0]);
  M(chk, chk, den);
  if (neq25519(chk, num)) return -1;

  if (par25519(r[0]) === (p[31]>>7)) Z(r[0], gf0, r[0]);

  M(r[3], r[0], r[1]);
  return 0;
}

function crypto_sign_open(m, sm, n, pk) {
  var i, mlen;
  var t = new Uint8Array(32), h = new Uint8Array(64);
  var p = [gf(), gf(), gf(), gf()],
      q = [gf(), gf(), gf(), gf()];

  mlen = -1;
  if (n < 64) return -1;

  if (unpackneg(q, pk)) return -1;

  for (i = 0; i < n; i++) m[i] = sm[i];
  for (i = 0; i < 32; i++) m[i+32] = pk[i];
  crypto_hash(h, m, n);
  reduce(h);
  scalarmult(p, q, h);

  scalarbase(q, sm.subarray(32));
  add(p, q);
  pack(t, p);

  n -= 64;
  if (crypto_verify_32(sm, 0, t, 0)) {
    for (i = 0; i < n; i++) m[i] = 0;
    return -1;
  }

  for (i = 0; i < n; i++) m[i] = sm[i + 64];
  mlen = n;
  return mlen;
}

var crypto_secretbox_KEYBYTES = 32,
    crypto_secretbox_NONCEBYTES = 24,
    crypto_secretbox_ZEROBYTES = 32,
    crypto_secretbox_BOXZEROBYTES = 16,
    crypto_scalarmult_BYTES = 32,
    crypto_scalarmult_SCALARBYTES = 32,
    crypto_box_PUBLICKEYBYTES = 32,
    crypto_box_SECRETKEYBYTES = 32,
    crypto_box_BEFORENMBYTES = 32,
    crypto_box_NONCEBYTES = crypto_secretbox_NONCEBYTES,
    crypto_box_ZEROBYTES = crypto_secretbox_ZEROBYTES,
    crypto_box_BOXZEROBYTES = crypto_secretbox_BOXZEROBYTES,
    crypto_sign_BYTES = 64,
    crypto_sign_PUBLICKEYBYTES = 32,
    crypto_sign_SECRETKEYBYTES = 64,
    crypto_sign_SEEDBYTES = 32,
    crypto_hash_BYTES = 64;

nacl.lowlevel = {
  crypto_core_hsalsa20: crypto_core_hsalsa20,
  crypto_stream_xor: crypto_stream_xor,
  crypto_stream: crypto_stream,
  crypto_stream_salsa20_xor: crypto_stream_salsa20_xor,
  crypto_stream_salsa20: crypto_stream_salsa20,
  crypto_onetimeauth: crypto_onetimeauth,
  crypto_onetimeauth_verify: crypto_onetimeauth_verify,
  crypto_verify_16: crypto_verify_16,
  crypto_verify_32: crypto_verify_32,
  crypto_secretbox: crypto_secretbox,
  crypto_secretbox_open: crypto_secretbox_open,
  crypto_scalarmult: crypto_scalarmult,
  crypto_scalarmult_base: crypto_scalarmult_base,
  crypto_box_beforenm: crypto_box_beforenm,
  crypto_box_afternm: crypto_box_afternm,
  crypto_box: crypto_box,
  crypto_box_open: crypto_box_open,
  crypto_box_keypair: crypto_box_keypair,
  crypto_hash: crypto_hash,
  crypto_sign: crypto_sign,
  crypto_sign_keypair: crypto_sign_keypair,
  crypto_sign_open: crypto_sign_open,

  crypto_secretbox_KEYBYTES: crypto_secretbox_KEYBYTES,
  crypto_secretbox_NONCEBYTES: crypto_secretbox_NONCEBYTES,
  crypto_secretbox_ZEROBYTES: crypto_secretbox_ZEROBYTES,
  crypto_secretbox_BOXZEROBYTES: crypto_secretbox_BOXZEROBYTES,
  crypto_scalarmult_BYTES: crypto_scalarmult_BYTES,
  crypto_scalarmult_SCALARBYTES: crypto_scalarmult_SCALARBYTES,
  crypto_box_PUBLICKEYBYTES: crypto_box_PUBLICKEYBYTES,
  crypto_box_SECRETKEYBYTES: crypto_box_SECRETKEYBYTES,
  crypto_box_BEFORENMBYTES: crypto_box_BEFORENMBYTES,
  crypto_box_NONCEBYTES: crypto_box_NONCEBYTES,
  crypto_box_ZEROBYTES: crypto_box_ZEROBYTES,
  crypto_box_BOXZEROBYTES: crypto_box_BOXZEROBYTES,
  crypto_sign_BYTES: crypto_sign_BYTES,
  crypto_sign_PUBLICKEYBYTES: crypto_sign_PUBLICKEYBYTES,
  crypto_sign_SECRETKEYBYTES: crypto_sign_SECRETKEYBYTES,
  crypto_sign_SEEDBYTES: crypto_sign_SEEDBYTES,
  crypto_hash_BYTES: crypto_hash_BYTES
};

/* High-level API */

function checkLengths(k, n) {
  if (k.length !== crypto_secretbox_KEYBYTES) throw new Error('bad key size');
  if (n.length !== crypto_secretbox_NONCEBYTES) throw new Error('bad nonce size');
}

function checkBoxLengths(pk, sk) {
  if (pk.length !== crypto_box_PUBLICKEYBYTES) throw new Error('bad public key size');
  if (sk.length !== crypto_box_SECRETKEYBYTES) throw new Error('bad secret key size');
}

function checkArrayTypes() {
  var t, i;
  for (i = 0; i < arguments.length; i++) {
     if ((t = Object.prototype.toString.call(arguments[i])) !== '[object Uint8Array]')
       throw new TypeError('unexpected type ' + t + ', use Uint8Array');
  }
}

function cleanup(arr) {
  for (var i = 0; i < arr.length; i++) arr[i] = 0;
}

// TODO: Completely remove this in v0.15.
if (!nacl.util) {
  nacl.util = {};
  nacl.util.decodeUTF8 = nacl.util.encodeUTF8 = nacl.util.encodeBase64 = nacl.util.decodeBase64 = function() {
    throw new Error('nacl.util moved into separate package: https://github.com/dchest/tweetnacl-util-js');
  };
}

nacl.randomBytes = function(n) {
  var b = new Uint8Array(n);
  randombytes(b, n);
  return b;
};

nacl.secretbox = function(msg, nonce, key) {
  checkArrayTypes(msg, nonce, key);
  checkLengths(key, nonce);
  var m = new Uint8Array(crypto_secretbox_ZEROBYTES + msg.length);
  var c = new Uint8Array(m.length);
  for (var i = 0; i < msg.length; i++) m[i+crypto_secretbox_ZEROBYTES] = msg[i];
  crypto_secretbox(c, m, m.length, nonce, key);
  return c.subarray(crypto_secretbox_BOXZEROBYTES);
};

nacl.secretbox.open = function(box, nonce, key) {
  checkArrayTypes(box, nonce, key);
  checkLengths(key, nonce);
  var c = new Uint8Array(crypto_secretbox_BOXZEROBYTES + box.length);
  var m = new Uint8Array(c.length);
  for (var i = 0; i < box.length; i++) c[i+crypto_secretbox_BOXZEROBYTES] = box[i];
  if (c.length < 32) return false;
  if (crypto_secretbox_open(m, c, c.length, nonce, key) !== 0) return false;
  return m.subarray(crypto_secretbox_ZEROBYTES);
};

nacl.secretbox.keyLength = crypto_secretbox_KEYBYTES;
nacl.secretbox.nonceLength = crypto_secretbox_NONCEBYTES;
nacl.secretbox.overheadLength = crypto_secretbox_BOXZEROBYTES;

nacl.scalarMult = function(n, p) {
  checkArrayTypes(n, p);
  if (n.length !== crypto_scalarmult_SCALARBYTES) throw new Error('bad n size');
  if (p.length !== crypto_scalarmult_BYTES) throw new Error('bad p size');
  var q = new Uint8Array(crypto_scalarmult_BYTES);
  crypto_scalarmult(q, n, p);
  return q;
};

nacl.scalarMult.base = function(n) {
  checkArrayTypes(n);
  if (n.length !== crypto_scalarmult_SCALARBYTES) throw new Error('bad n size');
  var q = new Uint8Array(crypto_scalarmult_BYTES);
  crypto_scalarmult_base(q, n);
  return q;
};

nacl.scalarMult.scalarLength = crypto_scalarmult_SCALARBYTES;
nacl.scalarMult.groupElementLength = crypto_scalarmult_BYTES;

nacl.box = function(msg, nonce, publicKey, secretKey) {
  var k = nacl.box.before(publicKey, secretKey);
  return nacl.secretbox(msg, nonce, k);
};

nacl.box.before = function(publicKey, secretKey) {
  checkArrayTypes(publicKey, secretKey);
  checkBoxLengths(publicKey, secretKey);
  var k = new Uint8Array(crypto_box_BEFORENMBYTES);
  crypto_box_beforenm(k, publicKey, secretKey);
  return k;
};

nacl.box.after = nacl.secretbox;

nacl.box.open = function(msg, nonce, publicKey, secretKey) {
  var k = nacl.box.before(publicKey, secretKey);
  return nacl.secretbox.open(msg, nonce, k);
};

nacl.box.open.after = nacl.secretbox.open;

nacl.box.keyPair = function() {
  var pk = new Uint8Array(crypto_box_PUBLICKEYBYTES);
  var sk = new Uint8Array(crypto_box_SECRETKEYBYTES);
  crypto_box_keypair(pk, sk);
  return {publicKey: pk, secretKey: sk};
};

nacl.box.keyPair.fromSecretKey = function(secretKey) {
  checkArrayTypes(secretKey);
  if (secretKey.length !== crypto_box_SECRETKEYBYTES)
    throw new Error('bad secret key size');
  var pk = new Uint8Array(crypto_box_PUBLICKEYBYTES);
  crypto_scalarmult_base(pk, secretKey);
  return {publicKey: pk, secretKey: new Uint8Array(secretKey)};
};

nacl.box.publicKeyLength = crypto_box_PUBLICKEYBYTES;
nacl.box.secretKeyLength = crypto_box_SECRETKEYBYTES;
nacl.box.sharedKeyLength = crypto_box_BEFORENMBYTES;
nacl.box.nonceLength = crypto_box_NONCEBYTES;
nacl.box.overheadLength = nacl.secretbox.overheadLength;

nacl.sign = function(msg, secretKey) {
  checkArrayTypes(msg, secretKey);
  if (secretKey.length !== crypto_sign_SECRETKEYBYTES)
    throw new Error('bad secret key size');
  var signedMsg = new Uint8Array(crypto_sign_BYTES+msg.length);
  crypto_sign(signedMsg, msg, msg.length, secretKey);
  return signedMsg;
};

nacl.sign.open = function(signedMsg, publicKey) {
  if (arguments.length !== 2)
    throw new Error('nacl.sign.open accepts 2 arguments; did you mean to use nacl.sign.detached.verify?');
  checkArrayTypes(signedMsg, publicKey);
  if (publicKey.length !== crypto_sign_PUBLICKEYBYTES)
    throw new Error('bad public key size');
  var tmp = new Uint8Array(signedMsg.length);
  var mlen = crypto_sign_open(tmp, signedMsg, signedMsg.length, publicKey);
  if (mlen < 0) return null;
  var m = new Uint8Array(mlen);
  for (var i = 0; i < m.length; i++) m[i] = tmp[i];
  return m;
};

nacl.sign.detached = function(msg, secretKey) {
  var signedMsg = nacl.sign(msg, secretKey);
  var sig = new Uint8Array(crypto_sign_BYTES);
  for (var i = 0; i < sig.length; i++) sig[i] = signedMsg[i];
  return sig;
};

nacl.sign.detached.verify = function(msg, sig, publicKey) {
  checkArrayTypes(msg, sig, publicKey);
  if (sig.length !== crypto_sign_BYTES)
    throw new Error('bad signature size');
  if (publicKey.length !== crypto_sign_PUBLICKEYBYTES)
    throw new Error('bad public key size');
  var sm = new Uint8Array(crypto_sign_BYTES + msg.length);
  var m = new Uint8Array(crypto_sign_BYTES + msg.length);
  var i;
  for (i = 0; i < crypto_sign_BYTES; i++) sm[i] = sig[i];
  for (i = 0; i < msg.length; i++) sm[i+crypto_sign_BYTES] = msg[i];
  return (crypto_sign_open(m, sm, sm.length, publicKey) >= 0);
};

nacl.sign.keyPair = function() {
  var pk = new Uint8Array(crypto_sign_PUBLICKEYBYTES);
  var sk = new Uint8Array(crypto_sign_SECRETKEYBYTES);
  crypto_sign_keypair(pk, sk);
  return {publicKey: pk, secretKey: sk};
};

nacl.sign.keyPair.fromSecretKey = function(secretKey) {
  checkArrayTypes(secretKey);
  if (secretKey.length !== crypto_sign_SECRETKEYBYTES)
    throw new Error('bad secret key size');
  var pk = new Uint8Array(crypto_sign_PUBLICKEYBYTES);
  for (var i = 0; i < pk.length; i++) pk[i] = secretKey[32+i];
  return {publicKey: pk, secretKey: new Uint8Array(secretKey)};
};

nacl.sign.keyPair.fromSeed = function(seed) {
  checkArrayTypes(seed);
  if (seed.length !== crypto_sign_SEEDBYTES)
    throw new Error('bad seed size');
  var pk = new Uint8Array(crypto_sign_PUBLICKEYBYTES);
  var sk = new Uint8Array(crypto_sign_SECRETKEYBYTES);
  for (var i = 0; i < 32; i++) sk[i] = seed[i];
  crypto_sign_keypair(pk, sk, true);
  return {publicKey: pk, secretKey: sk};
};

nacl.sign.publicKeyLength = crypto_sign_PUBLICKEYBYTES;
nacl.sign.secretKeyLength = crypto_sign_SECRETKEYBYTES;
nacl.sign.seedLength = crypto_sign_SEEDBYTES;
nacl.sign.signatureLength = crypto_sign_BYTES;

nacl.hash = function(msg) {
  checkArrayTypes(msg);
  var h = new Uint8Array(crypto_hash_BYTES);
  crypto_hash(h, msg, msg.length);
  return h;
};

nacl.hash.hashLength = crypto_hash_BYTES;

nacl.verify = function(x, y) {
  checkArrayTypes(x, y);
  // Zero length arguments are considered not equal.
  if (x.length === 0 || y.length === 0) return false;
  if (x.length !== y.length) return false;
  return (vn(x, 0, y, 0, x.length) === 0) ? true : false;
};

nacl.setPRNG = function(fn) {
  randombytes = fn;
};

(function() {
  // Initialize PRNG if environment provides CSPRNG.
  // If not, methods calling randombytes will throw.
  var crypto = typeof self !== 'undefined' ? (self.crypto || self.msCrypto) : null;
  if (crypto && crypto.getRandomValues) {
    // Browsers.
    var QUOTA = 65536;
    nacl.setPRNG(function(x, n) {
      var i, v = new Uint8Array(n);
      for (i = 0; i < n; i += QUOTA) {
        crypto.getRandomValues(v.subarray(i, i + Math.min(n - i, QUOTA)));
      }
      for (i = 0; i < n; i++) x[i] = v[i];
      cleanup(v);
    });
  } else if (typeof require !== 'undefined') {
    // Node.js.
    crypto = require('crypto');
    if (crypto && crypto.randomBytes) {
      nacl.setPRNG(function(x, n) {
        var i, v = crypto.randomBytes(n);
        for (i = 0; i < n; i++) x[i] = v[i];
        cleanup(v);
      });
    }
  }
})();

})(typeof module !== 'undefined' && module.exports ? module.exports : (self.nacl = self.nacl || {}));

},{"crypto":3}],19:[function(require,module,exports){
var Exonum = require('./index.js');

window.Exonum = window.Exonum || Exonum;

},{"./index.js":20}],20:[function(require,module,exports){
"use strict";

var sha = require('sha.js');
var nacl = require('tweetnacl');
var objectAssign = require('object-assign');
var fs = require('fs');
var bigInt = require("big-integer");

var ExonumClient = (function() {

    var CONST = {
        MIN_INT8: -128,
        MAX_INT8: 127,
        MIN_INT16: -32768,
        MAX_INT16: 32767,
        MIN_INT32: -2147483648,
        MAX_INT32: 2147483647,
        MIN_INT64: '-9223372036854775808',
        MAX_INT64: '9223372036854775807',
        MAX_UINT8: 255,
        MAX_UINT16: 65535,
        MAX_UINT32: 4294967295,
        MAX_UINT64: '18446744073709551615',
        MERKLE_PATRICIA_KEY_LENGTH: 32,
        SIGNATURE_LENGTH: 64,
        NETWORK_ID: 0
    };
    var DBKey = createNewType({
        size: 34,
        fields: {
            variant: {type: Uint8, size: 1, from: 0, to: 1},
            key: {type: Hash, size: 32, from: 1, to: 33},
            length: {type: Uint8, size: 1, from: 33, to: 34}
        }
    });
    var Branch = createNewType({
        size: 132,
        fields: {
            left_hash: {type: Hash, size: 32, from: 0, to: 32},
            right_hash: {type: Hash, size: 32, from: 32, to: 64},
            left_key: {type: DBKey, size: 34, from: 64, to: 98},
            right_key: {type: DBKey, size: 34, from: 98, to: 132}
        }
    });
    var RootBranch = createNewType({
        size: 66,
        fields: {
            key: {type: DBKey, size: 34, from: 0, to: 34},
            hash: {type: Hash, size: 32, from: 34, to: 66}
        }
    });
    var Block = createNewType({
        size: 116,
        fields: {
            height: {type: Uint64, size: 8, from: 0, to: 8},
            propose_round: {type: Uint32, size: 4, from: 8, to: 12},
            time: {type: Timespec, size: 8, from: 12, to: 20},
            prev_hash: {type: Hash, size: 32, from: 20, to: 52},
            tx_hash: {type: Hash, size: 32, from: 52, to: 84},
            state_hash: {type: Hash, size: 32, from: 84, to: 116}
        }
    });
    var MessageHead = createNewType({
        size: 10,
        fields: {
            network_id: {type: Uint8, size: 1, from: 0, to: 1},
            version: {type: Uint8, size: 1, from: 1, to: 2},
            message_id: {type: Uint16, size: 2, from: 2, to: 4},
            service_id: {type: Uint16, size: 2, from: 4, to: 6},
            payload: {type: Uint32, size: 4, from: 6, to: 10}
        }
    });
    var Precommit = createNewMessage({
        size: 84,
        service_id: 0,
        message_id: 4,
        fields: {
            validator: {type: Uint32, size: 4, from: 0, to: 4},
            height: {type: Uint64, size: 8, from: 8, to: 16},
            round: {type: Uint32, size: 4, from: 16, to: 20},
            propose_hash: {type: Hash, size: 32, from: 20, to: 52},
            block_hash: {type: Hash, size: 32, from: 52, to: 84}
        }
    });

    /**
     * @constructor
     * @param {Object} type
     */
    function NewType(type) {
        this.size = type.size;
        this.fields = type.fields;
    }

    /**
     * Built-in method to serialize data into array of 8-bit integers
     * @param {Object} data
     * @returns {Array}
     */
    NewType.prototype.serialize = function(data) {
        return serialize([], 0, data, this);
    };

    /**
     * Create element of NewType class
     * @param {Object} type
     * @returns {NewType}
     */
    function createNewType(type) {
        return new NewType(type);
    }

    /**
     * @constructor
     * @param {Object} type
     */
    function NewMessage(type) {
        this.size = type.size;
        this.message_id = type.message_id;
        this.service_id = type.service_id;
        this.signature = type.signature;
        this.fields = type.fields;
    }

    /**
     * Built-in method to serialize data into array of 8-bit integers
     * @param {Object} data
     * @param {Boolean} cutSignature
     * @returns {Array}
     */
    NewMessage.prototype.serialize = function(data, cutSignature) {
        var buffer = MessageHead.serialize({
            network_id: CONST.NETWORK_ID,
            version: 0,
            message_id: this.message_id,
            service_id: this.service_id
        });

        // serialize and append message body
        buffer = serialize(buffer, MessageHead.size, data, this);
        if (typeof buffer === 'undefined') {
            return;
        }

        // calculate payload and insert it into buffer
        Uint32(buffer.length + CONST.SIGNATURE_LENGTH, buffer, MessageHead.fields.payload.from, MessageHead.fields.payload.to);

        if (cutSignature !== true) {
            // append signature
            Digest(this.signature, buffer, buffer.length, buffer.length + CONST.SIGNATURE_LENGTH);
        }

        return buffer;
    };

    /**
     * Create element of NewMessage class
     * @param {Object} type
     * @returns {NewMessage}
     */
    function createNewMessage(type) {
        return new NewMessage(type);
    }

    /**
     * Built-in data type
     * @param {Number || String} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Int8(value, buffer, from, to) {
        if (!validateInteger(value, CONST.MIN_INT8, CONST.MAX_INT8, from, to, 1)) {
            return;
        }

        if (value < 0) {
            value = CONST.MAX_UINT8 + value + 1
        }

        insertIntegerToByteArray(value, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {Number || String} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Int16(value, buffer, from, to) {
        if (!validateInteger(value, CONST.MIN_INT16, CONST.MAX_INT16, from, to, 2)) {
            return;
        }

        if (value < 0) {
            value = CONST.MAX_UINT16 + value + 1;
        }

        insertIntegerToByteArray(value, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {Number || String} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Int32(value, buffer, from, to) {
        if (!validateInteger(value, CONST.MIN_INT32, CONST.MAX_INT32, from, to, 4)) {
            return;
        }

        if (value < 0) {
            value = CONST.MAX_UINT32 + value + 1;
        }

        insertIntegerToByteArray(value, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {Number || String} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Int64(value, buffer, from, to) {
        var val = validateBigInteger(value, CONST.MIN_INT64, CONST.MAX_INT64, from, to, 8);

        if (val === false) {
            return;
        } else if (!bigInt.isInstance(val)) {
            return;
        }

        if (val.isNegative()) {
            val = bigInt(CONST.MAX_UINT64).plus(1).plus(val);
        }

        insertBigIntegerToByteArray(val, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {Number || String} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Uint8(value, buffer, from, to) {
        if (!validateInteger(value, 0, CONST.MAX_UINT8, from, to, 1)) {
            return;
        }

        insertIntegerToByteArray(value, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {Number || String} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Uint16(value, buffer, from, to) {
        if (!validateInteger(value, 0, CONST.MAX_UINT16, from, to, 2)) {
            return;
        }

        insertIntegerToByteArray(value, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {Number || String} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Uint32(value, buffer, from, to) {
        if (!validateInteger(value, 0, CONST.MAX_UINT32, from, to, 4)) {
            return;
        }

        insertIntegerToByteArray(value, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {Number || String} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Uint64(value, buffer, from, to) {
        var val = validateBigInteger(value, 0, CONST.MAX_UINT64, from, to, 8);

        if (val === false) {
            return;
        } else if (!bigInt.isInstance(val)) {
            return;
        }

        insertBigIntegerToByteArray(val, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {String} string
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function String(string, buffer, from, to) {
        if (typeof string !== 'string') {
            console.error('Wrong data type is passed as String. String is required');
            return;
        } else if ((to - from) !== 8) {
            console.error('String segment is of wrong length. 8 bytes long is required to store transmitted value.');
            return;
        }

        var bufferLength = buffer.length;
        Uint32(bufferLength, buffer, from, from + 4); // index where string content starts in buffer
        insertStringToByteArray(string, buffer, bufferLength); // string content
        Uint32(buffer.length - bufferLength, buffer, from + 4, from + 8); // string length

        return buffer;
    }

    /**
     * Built-in data type
     * @param {String} hash
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Hash(hash, buffer, from, to) {
        if (!validateHexHash(hash)) {
            return;
        } else if ((to - from) !== 32) {
            console.error('Hash segment is of wrong length. 32 bytes long is required to store transmitted value.');
            return;
        }

        insertHexadecimalToArray(hash, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {String} digest
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Digest(digest, buffer, from, to) {
        if (!validateHexHash(digest, 64)) {
            return;
        } else if ((to - from) !== 64) {
            console.error('Digest segment is of wrong length. 64 bytes long is required to store transmitted value.');
            return;
        }

        insertHexadecimalToArray(digest, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {String} publicKey
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function PublicKey(publicKey, buffer, from, to) {
        if (!validateHexHash(publicKey)) {
            return;
        } else if ((to - from) !== 32) {
            console.error('PublicKey segment is of wrong length. 32 bytes long is required to store transmitted value.');
            return;
        }

        insertHexadecimalToArray(publicKey, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {Number || String} nanoseconds
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Timespec(nanoseconds, buffer, from, to) {
        var val = validateBigInteger(nanoseconds, 0, CONST.MAX_UINT64, from, to, 8);

        if (val === false) {
            return;
        } else if (!bigInt.isInstance(val)) {
            return;
        }

        insertBigIntegerToByteArray(val, buffer, from, to);

        return buffer;
    }

    /**
     * Built-in data type
     * @param {boolean} value
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function Bool(value, buffer, from, to) {
        if (typeof value !== 'boolean') {
            console.error('Wrong data type is passed as Boolean. Boolean is required');
            return;
        } else if ((to - from) !== 1) {
            console.error('Bool segment is of wrong length. 1 bytes long is required to store transmitted value.');
            return;
        }

        insertIntegerToByteArray(value ? 1 : 0, buffer, from, to);

        return buffer;
    }

    /**
     * Check integer number validity
     * @param {Number} value
     * @param {Number} min
     * @param {Number} max
     * @param {Number} from
     * @param {Number} to
     * @param {Number} length
     * @returns {Boolean}
     */
    function validateInteger(value, min, max, from, to, length) {
        if (typeof value !== 'number') {
            console.error('Wrong data type is passed as number. Should be of type Number.');
            return false;
        } else if (value < min) {
            console.error('Number should be more or equal to ' + min + '.');
            return false;
        } else if (value > max) {
            console.error('Number should be less or equal to ' + max + '.');
            return false;
        } else if ((to - from) !== length) {
            console.error('Number segment is of wrong length. ' + length + ' bytes long is required to store transmitted value.');
            return false;
        }

        return true;
    }

    /**
     * Check bit integer number validity
     * @param {Number || String} value
     * @param {Number} min
     * @param {Number} max
     * @param {Number} from
     * @param {Number} to
     * @param {Number} length
     * @returns {bigInt || Boolean}
     */
    function validateBigInteger(value, min, max, from, to, length) {
        var val;

        if (!(typeof value === 'number' || typeof value === 'string')) {
            console.error('Wrong data type is passed as number. Should be of type Number or String.');
            return false;
        } else if ((to - from) !== length) {
            console.error('Number segment is of wrong length. ' + length + ' bytes long is required to store transmitted value.');
            return false;
        }

        try {
            val = bigInt(value);

            if (val.lt(min)) {
                console.error('Number should be more or equal to ' + min + '.');
                return false;
            } else if (val.gt(max)) {
                console.error('Number should be less or equal to ' + max + '.');
                return false;
            }

            return val;
        } catch (e) {
            console.error('Wrong data type is passed as number. Should be of type Number or String.');
            return false;
        }
    }

    /**
     * Check hash validity
     * @param {String} hash
     * @param {Number} bytes
     * @returns {Boolean}
     */
    function validateHexHash(hash, bytes) {
        var bytes = bytes || 32;

        if (typeof hash !== 'string') {
            console.error('Wrong data type is passed as hexadecimal string. String is required');
            return false;
        } else if (hash.length !== bytes * 2) {
            console.error('Hexadecimal string is of wrong length. ' + bytes * 2 + ' char symbols long is required. ' + hash.length + ' is passed.');
            return false;
        }

        for (var i = 0, len = hash.length; i < len; i++) {
            if (isNaN(parseInt(hash[i], 16))) {
                console.error('Invalid symbol in hexadecimal string.');
                return false;
            }
        }

        return true;
    }

    /**
     * Check array of 8-bit integers validity
     * @param {Array} arr
     * @param {Number} bytes
     * @returns {Boolean}
     */
    function validateBytesArray(arr, bytes) {
        if (bytes && arr.length !== bytes) {
            console.error('Array of 8-bit integers validity is of wrong length. ' + bytes * 2 + ' char symbols long is required. ' + hash.length + ' is passed.');
            return false;
        }

        for (var i = 0, len = arr.length; i < len; i++) {
            if (typeof arr[i] !== 'number') {
                console.error('Wrong data type is passed as byte. Number is required');
                return false;
            } else if (arr[i] < 0 || arr[i] > 255) {
                console.error('Byte should be in [0..255] range.');
                return false;
            }
        }

        return true;
    }

    /**
     * Check binary string validity
     * @param {String} str
     * @param {Number} bits
     * @returns {Boolean}
     */
    function validateBinaryString(str, bits) {
        if (typeof bits !== 'undefined' && str.length !== bits) {
            console.error('Binary string is of wrong length.');
            return null;
        }

        var bit;
        for (var i = 0; i < str.length; i++) {
            bit = parseInt(str[i]);
            if (isNaN(bit)) {
                console.error('Wrong bit is passed in binary string.');
                return false;
            } else if (bit > 1) {
                console.error('Wrong bit is passed in binary string. Bit should be 0 or 1.');
                return false;
            }
        }

        return true;
    }

    /**
     * Serialize data into array of 8-bit integers and insert into buffer
     * @param {Array} buffer
     * @param {Number} shift - the index to start write into buffer
     * @param {Object} data
     * @param type - can be {NewType} or one of built-in types
     */
    function serialize(buffer, shift, data, type) {
        for (var i = 0, len = type.size; i < len; i++) {
            buffer[shift + i] = 0;
        }

        for (var fieldName in data) {
            if (!data.hasOwnProperty(fieldName)) {
                continue;
            }

            var fieldType = type.fields[fieldName];

            if (typeof fieldType === 'undefined') {
                console.error(fieldName + ' field was not found in configuration of type.');
                return;
            }

            var fieldData = data[fieldName];
            var from = shift + fieldType.from;

            if (fieldType.type instanceof NewType) {
                var isFixed = (function checkIfIsFixed(fields) {
                    for (var fieldName in fields) {
                        if (!fields.hasOwnProperty(fieldName)) {
                            continue;
                        }

                        if (fields[fieldName].type instanceof NewType) {
                            checkIfIsFixed(fields[fieldName].type.fields);
                        } else if (fields[fieldName].type === String) {
                            return false;
                        }
                    }
                    return true;
                })(fieldType.type.fields);

                if (isFixed === true) {
                    serialize(buffer, from, fieldData, fieldType.type);
                } else {
                    var end = buffer.length;
                    Uint32(end, buffer, from, from + 4);
                    serialize(buffer, end, fieldData, fieldType.type);
                    Uint32(buffer.length - end, buffer, from + 4, from + 8);
                }
            } else {
                buffer = fieldType.type(fieldData, buffer, from, shift + fieldType.to);
                if (typeof buffer === 'undefined') {
                    return;
                }
            }
        }

        return buffer;
    }

    /**
     * Convert hexadecimal into array of 8-bit integers and insert into buffer
     * @param {String} str
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function insertHexadecimalToArray(str, buffer, from, to) {
        for (var i = 0, len = str.length; i < len; i += 2) {
            buffer[from] = parseInt(str.substr(i, 2), 16);
            from++;

            if (from > to) {
                break;
            }
        }
    }

    /**
     * Convert integer into array of 8-bit integers and insert into buffer
     * @param {Number} number
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function insertIntegerToByteArray(number, buffer, from, to) {
        var str = number.toString(16);

        insertNumberInHexToByteArray(str, buffer, from, to);
    }

    /**
     * Convert big integer into array of 8-bit integers and insert into buffer
     * @param {bigInt} number
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function insertBigIntegerToByteArray(number, buffer, from, to) {
        var str = number.toString(16);

        insertNumberInHexToByteArray(str, buffer, from, to);
    }

    /**
     * Insert number in hex into buffer
     * @param {String} number
     * @param {Array} buffer
     * @param {Number} from
     * @param {Number} to
     */
    function insertNumberInHexToByteArray(number, buffer, from, to) {
        // store Number as little-endian
        if (number.length < 3) {
            buffer[from] = parseInt(number, 16);
            return true;
        }

        for (var i = number.length; i > 0; i -= 2) {
            if (i > 1) {
                buffer[from] = parseInt(number.substr(i - 2, 2), 16);
            } else {
                buffer[from] = parseInt(number.substr(0, 1), 16);
            }

            from++;

            if (from >= to) {
                break;
            }
        }
    }

    /**
     * Convert String into array of 8-bit integers and insert into buffer
     * @param {String} str
     * @param {Array} buffer
     * @param {Number} from
     */
    function insertStringToByteArray(str, buffer, from) {
        for (var i = 0; i < str.length; i++) {
            var c = str.charCodeAt(i);

            if (c < 128) {
                buffer[from++] = c;
            } else if (c < 2048) {
                buffer[from++] = (c >> 6) | 192;
                buffer[from++] = (c & 63) | 128;
            } else if (((c & 0xFC00) == 0xD800) && (i + 1) < str.length && ((str.charCodeAt(i + 1) & 0xFC00) == 0xDC00)) {
                // surrogate pair
                c = 0x10000 + ((c & 0x03FF) << 10) + (str.charCodeAt(++i) & 0x03FF);
                buffer[from++] = (c >> 18) | 240;
                buffer[from++] = ((c >> 12) & 63) | 128;
                buffer[from++] = ((c >> 6) & 63) | 128;
                buffer[from++] = (c & 63) | 128;
            } else {
                buffer[from++] = (c >> 12) | 224;
                buffer[from++] = ((c >> 6) & 63) | 128;
                buffer[from++] = (c & 63) | 128;
            }
        }
    }

    /**
     * Convert hexadecimal string into array of 8-bit integers
     * @param {String} str
     *  hexadecimal
     * @returns {Uint8Array}
     */
    function hexadecimalToUint8Array(str) {
        if (typeof str !== 'string') {
            console.error('Wrong data type of hexadecimal string');
            return new Uint8Array([]);
        }

        var array = [];
        for (var i = 0, len = str.length; i < len; i += 2) {
            array.push(parseInt(str.substr(i, 2), 16));
        }

        return new Uint8Array(array);
    }

    /**
     * Convert hexadecimal string into array of 8-bit integers
     * @param {String} str
     *  hexadecimal
     * @returns {String}
     */
    function hexadecimalToBinaryString(str) {
        var binaryStr = '';
        for (var i = 0, len = str.length; i < len; i += 2) {
            binaryStr += parseInt(str.substr(i, 2), 16).toString(2);
        }
        return binaryStr;
    }

    /**
     * Convert array of 8-bit integers into hexadecimal string
     * @param {Uint8Array} uint8arr
     * @returns {String}
     */
    function uint8ArrayToHexadecimal(uint8arr) {
        if (!(uint8arr instanceof Uint8Array)) {
            console.error('Wrong data type of array of 8-bit integers');
            return '';
        }

        var str = '';
        for (var i = 0, len = uint8arr.length; i < len; i++) {
            var hex = (uint8arr[i]).toString(16);
            hex = (hex.length === 1) ? '0' + hex : hex;
            str += hex;
        }

        return str.toLowerCase();
    }

    /**
     * Convert binary string into array of 8-bit integers
     * @param {String} binaryStr
     * @returns {Uint8Array}
     */
    function binaryStringToUint8Array(binaryStr) {
        var array = [];
        for (var i = 0, len = binaryStr.length; i < len; i += 8) {
            array.push(parseInt(binaryStr.substr(i, 8), 2));
        }

        return new Uint8Array(array);
    }

    /**
     * Convert binary string into hexadecimal string
     * @param {String} binaryStr
     * @returns {string}
     */
    function binaryStringToHexadecimal(binaryStr) {
        var str = '';
        for (var i = 0, len = binaryStr.length; i < len; i += 8) {
            var hex = (parseInt(binaryStr.substr(i, 8), 2)).toString(16);
            hex = (hex.length === 1) ? '0' + hex : hex;
            str += hex;
        }

        return str.toLowerCase();
    }

    /**
     * Check if element is of type {Object}
     * @param obj
     * @returns {boolean}
     */
    function isObject(obj) {
        return (typeof obj === 'object' && !Array.isArray(obj) && obj !== null && !(obj instanceof Date));
    }

    /**
     * Get length of object
     * @param {Object} obj
     * @returns {Number}
     */
    function getObjectLength(obj) {
        var l = 0;
        for (var prop in obj) {
            if (obj.hasOwnProperty(prop)) {
                l++;
            }
        }
        return l;
    }

    /**
     * Get SHA256 hash
     * Method with overloading. Accept two combinations of arguments:
     * 1) {Object} data, {NewType} type
     * 2) {Array} buffer
     * @return {String}
     */
    function hash(data, type) {
        var buffer;
        if (type instanceof NewType) {
            if (isObject(data)) {
                buffer = type.serialize(data);
                if (typeof buffer === 'undefined') {
                    console.error('Invalid data parameter. Instance of NewType is expected.');
                    return;
                }
            } else {
                console.error('Wrong type of data. Should be object.');
                return;
            }
        } else if (type instanceof NewMessage) {
            if (isObject(data)) {
                buffer = type.serialize(data);
                if (typeof buffer === 'undefined') {
                    console.error('Invalid data parameter. Instance of NewMessage is expected.');
                    return;
                }
            } else {
                console.error('Wrong type of data. Should be object.');
                return;
            }
        } else if (data instanceof Uint8Array) {
            buffer = data;
        } else if (Array.isArray(data)) {
            buffer = new Uint8Array(data);
        } else {
            console.error('Invalid type parameter.');
            return;
        }

        return sha('sha256').update(buffer, 'utf8').digest('hex');
    }

    /**
     * Get ED25519 signature
     * Method with overloading. Accept two combinations of arguments:
     * 1) {Object} data, {NewType || NewMessage} type, {String} secretKey
     * 2) {Array} buffer, {String} secretKey
     * @return {String}
     */
    function sign(data, type, secretKey) {
        var buffer;
        var secretKey = secretKey;
        var signature;

        if (typeof secretKey !== 'undefined') {
            if (type instanceof NewType) {
                buffer = type.serialize(data);
                if (typeof buffer === 'undefined') {
                    console.error('Invalid data parameter. Instance of NewType is expected.');
                    return;
                }
            } else if (type instanceof NewMessage) {
                buffer = type.serialize(data, true);
                if (typeof buffer === 'undefined') {
                    console.error('Invalid data parameter. Instance of NewMessage is expected.');
                    return;
                }
            } else {
                console.error('Invalid type parameter.');
                return;
            }
        } else {
            if (!validateBytesArray(data)) {
                console.error('Invalid data parameter.');
                return;
            }

            buffer = data;
            secretKey = type;
        }

        if (!validateHexHash(secretKey, 64)) {
            console.error('Invalid secretKey parameter.');
            return;
        }

        buffer = new Uint8Array(buffer);
        secretKey = hexadecimalToUint8Array(secretKey);
        signature = nacl.sign.detached(buffer, secretKey);

        return uint8ArrayToHexadecimal(signature);
    }

    /**
     * Verifies ED25519 signature
     * Method with overloading. Accept two combinations of arguments:
     * 1) {Object} data, {NewType || NewMessage} type, {String} signature, {String} publicKey
     * 2) {Array} buffer, {String} signature, {String} publicKey
     * @return {Boolean}
     */
    function verifySignature(data, type, signature, publicKey) {
        var buffer;
        var signature = signature;
        var publicKey = publicKey;

        if (typeof publicKey !== 'undefined') {
            if (type instanceof NewType) {
                buffer = type.serialize(data);
                if (typeof buffer === 'undefined') {
                    console.error('Invalid data parameter. Instance of NewType is expected.');
                    return;
                }
            } else if (type instanceof NewMessage) {
                buffer = type.serialize(data, true);
                if (typeof buffer === 'undefined') {
                    console.error('Invalid data parameter. Instance of NewMessage is expected.');
                    return;
                }
            } else {
                console.error('Invalid type parameter.');
                return;
            }
        } else {
            if (!validateBytesArray(data)) {
                console.error('Invalid data parameter.');
                return;
            }

            buffer = data;
            publicKey = signature;
            signature = type;
        }

        if (!validateHexHash(publicKey)) {
            console.error('Invalid publicKey parameter.');
            return;
        } else if (!validateHexHash(signature, 64)) {
            console.error('Invalid signature parameter.');
            return;
        }

        buffer = new Uint8Array(buffer);
        signature = hexadecimalToUint8Array(signature);
        publicKey = hexadecimalToUint8Array(publicKey);

        return nacl.sign.detached.verify(buffer, signature, publicKey);
    }

    /**
     * Check Merkle tree proof and return array of elements
     * @param {String} rootHash
     * @param {Number} count
     * @param {Object} proofNode
     * @param {Array} range
     * @param {NewType} type (optional)
     * @return {Array}
     */
    function merkleProof(rootHash, count, proofNode, range, type) {
        /**
         * Calculate height of merkle tree
         * @param {bigInt} count
         * @return {Number}
         */
        function calcHeight(count) {
            var i = 0;
            while (bigInt(2).pow(i).lt(count)) {
                i++;
            }
            return i;
        }

        /**
         * Get value from node, insert into elements array and return its hash
         * @param data
         * @param {Number} depth
         * @param {Number} index
         * @returns {String}
         */
        function getHash(data, depth, index) {
            var element;
            var elementsHash;

            if (depth !== 0 && (depth + 1) !== height) {
                console.error('Value node is on wrong height in tree.');
                return;
            } else if (start.gt(index)  || end.lt(index)) {
                console.error('Wrong index of value node.');
                return;
            } else if (start.plus(elements.length).neq(index)) {
                console.error('Value node is on wrong position in tree.');
                return;
            }

            if (typeof data === 'string') {
                if (validateHexHash(data)) {
                    element = data;
                    elementsHash = hash(hexadecimalToUint8Array(element));
                } else {
                    console.error('Invalid hexadecimal string is passed as value in tree.');
                    return;
                }
            } else if (Array.isArray(data)) {
                if (validateBytesArray(data)) {
                    element = data.slice(0); // clone array of 8-bit integers
                    elementsHash = hash(element);
                } else {
                    console.error('Invalid array of 8-bit integers in tree.');
                    return;
                }
            } else if (isObject(data)) {
                if (NewType.prototype.isPrototypeOf(type)) {
                    element = objectAssign(data); // deep copy
                    elementsHash = hash(element, type);
                } else {
                    console.error('Invalid type of type parameter.');
                    return;
                }
            } else {
                console.error('Invalid value of data parameter.');
                return;
            }

            if (typeof elementsHash === 'undefined') {
                return;
            }

            elements.push(element);
            return elementsHash;
        }

        /**
         * Recursive tree traversal function
         * @param {Object} node
         * @param {Number} depth
         * @param {Number} index
         * @returns {String}
         */
        function recursive(node, depth, index) {
            var hashLeft;
            var hashRight;
            var summingBuffer;

            // case with single node in tree
            if (depth === 0 && typeof node.val !== 'undefined') {
                return getHash(node.val, depth, index * 2);
            }

            if (typeof node.left !== 'undefined') {
                if (typeof node.left === 'string') {
                    if (!validateHexHash(node.left)) {
                        return null;
                    }
                    hashLeft = node.left;
                } else if (isObject(node.left)) {
                    if (typeof node.left.val !== 'undefined') {
                        hashLeft = getHash(node.left.val, depth, index * 2);
                    } else {
                        hashLeft = recursive(node.left, depth + 1, index * 2);
                    }
                    if (typeof hashLeft === 'undefined') {
                        return null;
                    }
                } else {
                    console.error('Invalid type of left node.');
                    return null;
                }
            } else {
                console.error('Left node is missed.');
                return null;
            }

            if (depth === 0) {
                rootBranch = 'right';
            }

            if (typeof node.right !== 'undefined') {
                if (typeof node.right === 'string') {
                    if (!validateHexHash(node.right)) {
                        return null;
                    }
                    hashRight = node.right;
                } else if (isObject(node.right)) {
                    if (typeof node.right.val !== 'undefined') {
                        hashRight = getHash(node.right.val, depth, index * 2 + 1);
                    } else {
                        hashRight = recursive(node.right, depth + 1, index * 2 + 1);
                    }
                    if (typeof hashRight === 'undefined') {
                        return null;
                    }
                } else {
                    console.error('Invalid type of right node.');
                    return null;
                }

                summingBuffer = new Uint8Array(64);
                summingBuffer.set(hexadecimalToUint8Array(hashLeft));
                summingBuffer.set(hexadecimalToUint8Array(hashRight), 32);
            } else if (depth === 0 || rootBranch === 'left') {
                console.error('Right leaf is missed in left branch of tree.');
                return null;
            } else {
                summingBuffer = hexadecimalToUint8Array(hashLeft);
            }

            return hash(summingBuffer);
        }

        var elements = [];
        var rootBranch = 'left';

        // validate rootHash
        if (!validateHexHash(rootHash)) {
            return undefined;
        }

        // validate count
        if (!(typeof count === 'number' || typeof count === 'string')) {
            console.error('Invalid value is passed as count parameter. Number or string is expected.');
            return undefined;
        }
        try {
            count = bigInt(count);
        } catch (e) {
            console.error('Invalid value is passed as count parameter.');
            return undefined;
        }
        if (count.lt(0)) {
            console.error('Invalid count parameter. Count can\'t be below zero.');
            return undefined;
        }

        // validate range
        if (!Array.isArray(range) || range.length !== 2) {
            console.error('Invalid type of range parameter. Array of two elements expected.');
            return undefined;
        } else if (!(typeof range[0] === 'number' || typeof range[0] === 'string')) {
            console.error('Invalid value is passed as start of range parameter.');
            return undefined;
        } else if (!(typeof range[1] === 'number' || typeof range[1] === 'string')) {
            console.error('Invalid value is passed as end of range parameter.');
            return undefined;
        }
        try {
            var rangeStart = bigInt(range[0]);
        } catch (e) {
            console.error('Invalid value is passed as start of range parameter. Number or string is expected.');
            return undefined;
        }
        try {
            var rangeEnd = bigInt(range[1]);
        } catch (e) {
            console.error('Invalid value is passed as end of range parameter. Number or string is expected.');
            return undefined;
        }
        if (rangeStart.gt(rangeEnd)) {
            console.error('Invalid range parameter. Start index can\'t be out of range.');
            return undefined;
        } else if (rangeStart.lt(0)) {
            console.error('Invalid range parameter. Start index can\'t be below zero.');
            return undefined;
        } else if (rangeEnd.lt(0)) {
            console.error('Invalid range parameter. End index can\'t be below zero.');
            return undefined;
        } else if (rangeStart.gt(count.minus(1))) {
            return [];
        }

        // validate proofNode
        if (!isObject(proofNode)) {
            console.error('Invalid type of proofNode parameter. Object expected.');
            return undefined;
        }

        var height = calcHeight(count);
        var start = rangeStart;
        var end = rangeEnd.lt(count) ? rangeEnd : count.minus(1);
        var actualHash = recursive(proofNode, 0, 0);

        if (typeof actualHash === 'undefined') { // tree is invalid
            return undefined;
        } else if (rootHash.toLowerCase() !== actualHash) {
            console.error('rootHash parameter is not equal to actual hash.');
            return undefined;
        } else if (bigInt(elements.length).neq(end.eq(start) ? 1 : end.minus(start).plus(1))) {
            console.error('Actual elements in tree amount is not equal to requested.');
            return undefined;
        }

        return elements;
    }

    /**
     * Check Merkle Patricia tree proof and return element
     * @param {String} rootHash
     * @param {Object} proofNode
     * @param key
     * @param {NewType} type (optional)
     * @return {Object}
     */
    function merklePatriciaProof(rootHash, proofNode, key, type) {
        /**
         * Get value from node
         * @param data
         * @returns {String || Array || Object}
         */
        function getHash(data) {
            var elementsHash;

            if (typeof data === 'string') {
                if (validateHexHash(data)) {
                    element = data;
                    elementsHash = hash(hexadecimalToUint8Array(element));
                } else {
                    console.error('Invalid hexadecimal string is passed as value in tree.');
                    return;
                }
            } else if (Array.isArray(data)) {
                if (validateBytesArray(data)) {
                    element = data.slice(0); // clone array of 8-bit integers
                    elementsHash = hash(element);
                } else {
                    return;
                }
            } else if (isObject(data)) {
                element = objectAssign(data); // deep copy
                elementsHash = hash(element, type);
            } else {
                console.error('Invalid value node in tree. Object expected.');
                return;
            }

            if (typeof elementsHash === 'undefined') {
                return;
            }

            return elementsHash;
        }

        /**
         * Check either suffix is a part of search key
         * @param {String} prefix
         * @param {String} suffix
         * @returns {Boolean}
         */
        function isPartOfSearchKey(prefix, suffix) {
            var diff = keyBinary.substr(prefix.length);
            return diff[0] === suffix[0];
        }

        /**
         * Recursive tree traversal function
         * @param {Object} node
         * @param {String} keyPrefix
         * @returns {String}
         */
        function recursive(node, keyPrefix) {
            if (getObjectLength(node) !== 2) {
                console.error('Invalid number of children in the tree node.');
                return null;
            }

            var levelData = {};

            for (var keySuffix in node) {
                if (!node.hasOwnProperty(keySuffix)) {
                    continue;
                }

                // validate key
                if (!validateBinaryString(keySuffix)) {
                    return null;
                }

                var branchValueHash;
                var fullKey = keyPrefix + keySuffix;
                var nodeValue = node[keySuffix];
                var branchType;
                var branchKey;
                var branchKeyHash;

                if (fullKey.length === CONST.MERKLE_PATRICIA_KEY_LENGTH * 8) {
                    if (typeof nodeValue === 'string') {
                        if (!validateHexHash(nodeValue)) {
                            return null;
                        }
                        branchValueHash = nodeValue;
                        branchType = 'hash';
                    } else if (isObject(nodeValue)) {
                        if (typeof nodeValue.val === 'undefined') {
                            console.error('Leaf tree contains invalid data.');
                            return null;
                        } else if (typeof element !== 'undefined') {
                            console.error('Tree can not contains more than one node with value.');
                            return null;
                        }

                        branchValueHash = getHash(nodeValue.val);
                        if (typeof branchValueHash === 'undefined') {
                            return null;
                        }
                        branchType = 'value';
                    }  else {
                        console.error('Invalid type of node in tree leaf.');
                        return null;
                    }

                    branchKeyHash = binaryStringToHexadecimal(fullKey);
                    branchKey = {
                        variant: 1,
                        key: branchKeyHash,
                        length: 0
                    };
                } else if (fullKey.length < CONST.MERKLE_PATRICIA_KEY_LENGTH * 8) { // node is branch
                    if (typeof nodeValue === 'string') {
                        if (!validateHexHash(nodeValue)) {
                            return null;
                        }
                        branchValueHash = nodeValue;
                        branchType = 'hash';
                    } else if (isObject(nodeValue)) {
                        if (typeof nodeValue.val !== 'undefined') {
                            console.error('Node with value is at non-leaf position in tree.');
                            return null;
                        }

                        branchValueHash = recursive(nodeValue, fullKey);
                        if (branchValueHash === null) {
                            return null;
                        }
                        branchType = 'branch';
                    }  else {
                        console.error('Invalid type of node in tree.');
                        return null;
                    }

                    var binaryKeyLength = fullKey.length;
                    var binaryKey = fullKey;

                    for (var j = 0; j < (CONST.MERKLE_PATRICIA_KEY_LENGTH * 8 - fullKey.length); j++) {
                        binaryKey += '0';
                    }

                    branchKeyHash = binaryStringToHexadecimal(binaryKey);
                    branchKey = {
                        variant: 0,
                        key: branchKeyHash,
                        length: binaryKeyLength
                    };
                } else {
                    console.error('Invalid length of key in tree.');
                    return null;
                }

                if (keySuffix[0] === '0') { // '0' at the beginning means left branch/leaf
                    if (typeof levelData.left === 'undefined') {
                        levelData.left = {
                            hash: branchValueHash,
                            key: branchKey,
                            type: branchType,
                            suffix: keySuffix,
                            size: fullKey.length
                        };
                    } else {
                        console.error('Left node is duplicated in tree.');
                        return null;
                    }
                } else { // '1' at the beginning means right branch/leaf
                    if (typeof levelData.right === 'undefined') {
                        levelData.right = {
                            hash: branchValueHash,
                            key: branchKey,
                            type: branchType,
                            suffix: keySuffix,
                            size: fullKey.length
                        };
                    } else {
                        console.error('Right node is duplicated in tree.');
                        return null;
                    }
                }
            }

            if ((levelData.left.type === 'hash') && (levelData.right.type === 'hash') && (fullKey.length < CONST.MERKLE_PATRICIA_KEY_LENGTH * 8)) {
                if (isPartOfSearchKey(keyPrefix, levelData.left.suffix)) {
                    console.error('Tree is invalid. Left key is a part of search key but its branch is not expanded.');
                    return null;
                } else if (isPartOfSearchKey(keyPrefix, levelData.right.suffix)) {
                    console.error('Tree is invalid. Right key is a part of search key but its branch is not expanded.');
                    return null;
                }
            }

            return hash({
                left_hash: levelData.left.hash,
                right_hash: levelData.right.hash,
                left_key: levelData.left.key,
                right_key: levelData.right.key
            }, Branch);
        }

        var element;

        // validate rootHash parameter
        if (!validateHexHash(rootHash)) {
            return undefined;
        }
        var rootHash = rootHash.toLowerCase();

        // validate proofNode parameter
        if (!isObject(proofNode)) {
            console.error('Invalid type of proofNode parameter. Object expected.');
            return undefined;
        }

        // validate key parameter
        var key = key;
        if (Array.isArray(key)) {
            if (validateBytesArray(key, CONST.MERKLE_PATRICIA_KEY_LENGTH)) {
                key = uint8ArrayToHexadecimal(key);
            } else {
                return undefined;
            }
        } else if (typeof key === 'string') {
            if (!validateHexHash(key, CONST.MERKLE_PATRICIA_KEY_LENGTH)) {
                return undefined;
            }
        } else {
            console.error('Invalid type of key parameter. Aarray of 8-bit integers or hexadecimal string is expected.');
            return undefined;
        }
        var keyBinary = hexadecimalToBinaryString(key);

        var proofNodeRootNumberOfNodes = getObjectLength(proofNode);
        if (proofNodeRootNumberOfNodes === 0) {
            if (rootHash === (new Uint8Array(CONST.MERKLE_PATRICIA_KEY_LENGTH * 2)).join('')) {
                return null;
            } else {
                console.error('Invalid rootHash parameter of empty tree.');
                return undefined;
            }
        } else if (proofNodeRootNumberOfNodes === 1) {
            for (var i in proofNode) {
                if (!proofNode.hasOwnProperty(i)) {
                    continue;
                }

                if (!validateBinaryString(i, 256)) {
                    return undefined;
                }

                var data = proofNode[i];
                var nodeKeyBuffer = binaryStringToUint8Array(i);
                var nodeKey = uint8ArrayToHexadecimal(nodeKeyBuffer);
                var nodeHash;

                if (typeof data === 'string') {
                    if (!validateHexHash(data)) {
                        return undefined;
                    }

                    nodeHash = hash({
                        key: {
                            variant: 1,
                            key: nodeKey,
                            length: 0
                        },
                        hash: data
                    }, RootBranch);

                    if (rootHash === nodeHash) {
                        if (key !== nodeKey) {
                            return null; // no element with data in tree
                        } else {
                            console.error('Invalid key with hash is in the root of proofNode parameter.');
                            return undefined;
                        }
                    } else {
                        console.error('rootHash parameter is not equal to actual hash.');
                        return undefined;
                    }
                } else if (isObject(data)) {
                    var elementsHash = getHash(data.val);
                    if (typeof elementsHash === 'undefined') {
                        return undefined;
                    }

                    nodeHash = hash({
                        key: {
                            variant: 1,
                            key: nodeKey,
                            length: 0
                        },
                        hash: elementsHash
                    }, RootBranch);

                    if (rootHash === nodeHash) {
                        if (key === nodeKey) {
                            return element;
                        } else {
                            console.error('Invalid key with value is in the root of proofNode parameter.');
                            return undefined
                        }
                    } else {
                        console.error('rootHash parameter is not equal to actual hash.');
                        return undefined;
                    }
                } else {
                    console.error('Invalid type of value in the root of proofNode parameter.');
                    return undefined;
                }

            }
        } else {
            var actualHash = recursive(proofNode, '');

            if (actualHash === null) { // tree is invalid
                return undefined;
            } else if (rootHash !== actualHash) {
                console.error('rootHash parameter is not equal to actual hash.');
                return undefined;
            } else if (typeof element === 'undefined') {
                return null; // no element with data in tree
            }

            return element;
        }
    }

    /**
     * Verifies block
     * @param {Object} data
     * @param {Array} validators
     * @return {Boolean}
     */
    function verifyBlock(data, validators) {
        if (!isObject(data)) {
            console.error('Wrong type of data parameter. Object is expected.');
            return false;
        } else if (!isObject(data.block)) {
            console.error('Wrong type of block field in data parameter. Object is expected.');
            return false;
        } else if (!Array.isArray(data.precommits)) {
            console.error('Wrong type of precommits field in data parameter. Array is expected.');
            return false;
        } else if (!Array.isArray(validators)) {
            console.error('Wrong type of validatorsparameter. Array is expected.');
            return false;
        }

        for (var i in validators) {
            if (!validators.hasOwnProperty(i)) {
                continue;
            }

            if (!validateHexHash(validators[i])) {
                return false;
            }
        }

        var validatorsTotalNumber = validators.length;
        var uniqueValidators = [];

        var round;
        var blockHash = hash(data.block, Block);

        for (var i in data.precommits) {
            if (!data.precommits.hasOwnProperty(i)) {
                continue;
            }

            var precommit = data.precommits[i];

            if (!isObject(precommit.body)) {
                console.error('Wrong type of precommits body. Object is expected.');
                return false;
            } else if (!validateHexHash(precommit.signature, 64)) {
                console.error('Wrong type of precommits signature. Hexadecimal of 64 length is expected.');
                return false;
            }

            if (precommit.body.validator >= validatorsTotalNumber) {
                console.error('Wrong index passed. Validator does not exist.');
                return false;
            }

            if (uniqueValidators.indexOf(precommit.body.validator) === -1) {
                uniqueValidators.push(precommit.body.validator);
            }

            if (precommit.body.height !== data.block.height) {
                console.error('Wrong height of block in precommit.');
                return false;
            } else if (precommit.body.block_hash !== blockHash) {
                console.error('Wrong hash of block in precommit.');
                return false;
            }

            if (typeof round === 'undefined') {
                round = precommit.body.round;
            } else if (precommit.body.round !== round) {
                console.error('Wrong round in precommit.');
                return false;
            }

            var publicKey = validators[precommit.body.validator];

            if (!verifySignature(precommit.body, Precommit, precommit.signature, publicKey)) {
                console.error('Wrong signature of precommit.');
                return false;
            }
        }

        if (uniqueValidators.length <= validatorsTotalNumber * 2 / 3) {
            console.error('Not enough precommits from unique validators.');
            return false;
        }

        return true;
    }

    /**
     * Generate random pair of publicKey and secretKey
     * @return {Object}
     *  publicKey {String}
     *  secretKey {String}
     */
    function keyPair() {
        var pair = nacl.sign.keyPair();
        var publicKey = uint8ArrayToHexadecimal(pair.publicKey);
        var secretKey = uint8ArrayToHexadecimal(pair.secretKey);

        return {
            publicKey: publicKey,
            secretKey: secretKey
        }
    }

    /** Generate random number of type of Uint64
     * @return {String}
     */
    function randomUint64() {
        return bigInt.randBetween(0, CONST.MAX_UINT64);
    }

    return {

        // API methods
        newType: createNewType,
        newMessage: createNewMessage,
        hash: hash,
        sign: sign,
        verifySignature: verifySignature,
        merkleProof: merkleProof,
        merklePatriciaProof: merklePatriciaProof,
        verifyBlock: verifyBlock,
        keyPair: keyPair,
        randomUint64: randomUint64,

        // built-in types
        Int8: Int8,
        Int16: Int16,
        Int32: Int32,
        Int64: Int64,
        Uint8: Uint8,
        Uint16: Uint16,
        Uint32: Uint32,
        Uint64: Uint64,
        String: String,
        Hash: Hash,
        Digest: Digest,
        PublicKey: PublicKey,
        Timespec: Timespec,
        Bool: Bool

    };

})();

module.exports = ExonumClient;

},{"big-integer":2,"fs":4,"object-assign":9,"sha.js":11,"tweetnacl":18}]},{},[19])
//# sourceMappingURL=data:application/json;charset=utf-8;base64,eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbIm5vZGVfbW9kdWxlcy9icm93c2VyLXBhY2svX3ByZWx1ZGUuanMiLCJub2RlX21vZHVsZXMvYmFzZTY0LWpzL2luZGV4LmpzIiwibm9kZV9tb2R1bGVzL2JpZy1pbnRlZ2VyL0JpZ0ludGVnZXIuanMiLCJub2RlX21vZHVsZXMvYnJvd3Nlci1yZXNvbHZlL2VtcHR5LmpzIiwibm9kZV9tb2R1bGVzL2J1ZmZlci9pbmRleC5qcyIsIm5vZGVfbW9kdWxlcy9pZWVlNzU0L2luZGV4LmpzIiwibm9kZV9tb2R1bGVzL2luaGVyaXRzL2luaGVyaXRzX2Jyb3dzZXIuanMiLCJub2RlX21vZHVsZXMvaXNhcnJheS9pbmRleC5qcyIsIm5vZGVfbW9kdWxlcy9vYmplY3QtYXNzaWduL2luZGV4LmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9oYXNoLmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9pbmRleC5qcyIsIm5vZGVfbW9kdWxlcy9zaGEuanMvc2hhLmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9zaGExLmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9zaGEyMjQuanMiLCJub2RlX21vZHVsZXMvc2hhLmpzL3NoYTI1Ni5qcyIsIm5vZGVfbW9kdWxlcy9zaGEuanMvc2hhMzg0LmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9zaGE1MTIuanMiLCJub2RlX21vZHVsZXMvdHdlZXRuYWNsL25hY2wtZmFzdC5qcyIsInNyYy9icm93c2VyLmpzIiwic3JjL2luZGV4LmpzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBO0FDQUE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbEhBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUM5ckNBOzs7OztBQ0FBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7Ozs7QUM3dkRBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3BGQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDdkJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNMQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7O0FDMUZBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7O0FDckVBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7QUNmQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7Ozs7QUM3RkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7OztBQ2xHQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7OztBQ3BEQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7Ozs7O0FDdElBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7Ozs7QUN4REE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7OztBQ25RQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNwMUVBO0FBQ0E7QUFDQTtBQUNBOztBQ0hBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQSIsImZpbGUiOiJnZW5lcmF0ZWQuanMiLCJzb3VyY2VSb290IjoiIiwic291cmNlc0NvbnRlbnQiOlsiKGZ1bmN0aW9uIGUodCxuLHIpe2Z1bmN0aW9uIHMobyx1KXtpZighbltvXSl7aWYoIXRbb10pe3ZhciBhPXR5cGVvZiByZXF1aXJlPT1cImZ1bmN0aW9uXCImJnJlcXVpcmU7aWYoIXUmJmEpcmV0dXJuIGEobywhMCk7aWYoaSlyZXR1cm4gaShvLCEwKTt2YXIgZj1uZXcgRXJyb3IoXCJDYW5ub3QgZmluZCBtb2R1bGUgJ1wiK28rXCInXCIpO3Rocm93IGYuY29kZT1cIk1PRFVMRV9OT1RfRk9VTkRcIixmfXZhciBsPW5bb109e2V4cG9ydHM6e319O3Rbb11bMF0uY2FsbChsLmV4cG9ydHMsZnVuY3Rpb24oZSl7dmFyIG49dFtvXVsxXVtlXTtyZXR1cm4gcyhuP246ZSl9LGwsbC5leHBvcnRzLGUsdCxuLHIpfXJldHVybiBuW29dLmV4cG9ydHN9dmFyIGk9dHlwZW9mIHJlcXVpcmU9PVwiZnVuY3Rpb25cIiYmcmVxdWlyZTtmb3IodmFyIG89MDtvPHIubGVuZ3RoO28rKylzKHJbb10pO3JldHVybiBzfSkiLCIndXNlIHN0cmljdCdcblxuZXhwb3J0cy5ieXRlTGVuZ3RoID0gYnl0ZUxlbmd0aFxuZXhwb3J0cy50b0J5dGVBcnJheSA9IHRvQnl0ZUFycmF5XG5leHBvcnRzLmZyb21CeXRlQXJyYXkgPSBmcm9tQnl0ZUFycmF5XG5cbnZhciBsb29rdXAgPSBbXVxudmFyIHJldkxvb2t1cCA9IFtdXG52YXIgQXJyID0gdHlwZW9mIFVpbnQ4QXJyYXkgIT09ICd1bmRlZmluZWQnID8gVWludDhBcnJheSA6IEFycmF5XG5cbnZhciBjb2RlID0gJ0FCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaYWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXowMTIzNDU2Nzg5Ky8nXG5mb3IgKHZhciBpID0gMCwgbGVuID0gY29kZS5sZW5ndGg7IGkgPCBsZW47ICsraSkge1xuICBsb29rdXBbaV0gPSBjb2RlW2ldXG4gIHJldkxvb2t1cFtjb2RlLmNoYXJDb2RlQXQoaSldID0gaVxufVxuXG5yZXZMb29rdXBbJy0nLmNoYXJDb2RlQXQoMCldID0gNjJcbnJldkxvb2t1cFsnXycuY2hhckNvZGVBdCgwKV0gPSA2M1xuXG5mdW5jdGlvbiBwbGFjZUhvbGRlcnNDb3VudCAoYjY0KSB7XG4gIHZhciBsZW4gPSBiNjQubGVuZ3RoXG4gIGlmIChsZW4gJSA0ID4gMCkge1xuICAgIHRocm93IG5ldyBFcnJvcignSW52YWxpZCBzdHJpbmcuIExlbmd0aCBtdXN0IGJlIGEgbXVsdGlwbGUgb2YgNCcpXG4gIH1cblxuICAvLyB0aGUgbnVtYmVyIG9mIGVxdWFsIHNpZ25zIChwbGFjZSBob2xkZXJzKVxuICAvLyBpZiB0aGVyZSBhcmUgdHdvIHBsYWNlaG9sZGVycywgdGhhbiB0aGUgdHdvIGNoYXJhY3RlcnMgYmVmb3JlIGl0XG4gIC8vIHJlcHJlc2VudCBvbmUgYnl0ZVxuICAvLyBpZiB0aGVyZSBpcyBvbmx5IG9uZSwgdGhlbiB0aGUgdGhyZWUgY2hhcmFjdGVycyBiZWZvcmUgaXQgcmVwcmVzZW50IDIgYnl0ZXNcbiAgLy8gdGhpcyBpcyBqdXN0IGEgY2hlYXAgaGFjayB0byBub3QgZG8gaW5kZXhPZiB0d2ljZVxuICByZXR1cm4gYjY0W2xlbiAtIDJdID09PSAnPScgPyAyIDogYjY0W2xlbiAtIDFdID09PSAnPScgPyAxIDogMFxufVxuXG5mdW5jdGlvbiBieXRlTGVuZ3RoIChiNjQpIHtcbiAgLy8gYmFzZTY0IGlzIDQvMyArIHVwIHRvIHR3byBjaGFyYWN0ZXJzIG9mIHRoZSBvcmlnaW5hbCBkYXRhXG4gIHJldHVybiBiNjQubGVuZ3RoICogMyAvIDQgLSBwbGFjZUhvbGRlcnNDb3VudChiNjQpXG59XG5cbmZ1bmN0aW9uIHRvQnl0ZUFycmF5IChiNjQpIHtcbiAgdmFyIGksIGosIGwsIHRtcCwgcGxhY2VIb2xkZXJzLCBhcnJcbiAgdmFyIGxlbiA9IGI2NC5sZW5ndGhcbiAgcGxhY2VIb2xkZXJzID0gcGxhY2VIb2xkZXJzQ291bnQoYjY0KVxuXG4gIGFyciA9IG5ldyBBcnIobGVuICogMyAvIDQgLSBwbGFjZUhvbGRlcnMpXG5cbiAgLy8gaWYgdGhlcmUgYXJlIHBsYWNlaG9sZGVycywgb25seSBnZXQgdXAgdG8gdGhlIGxhc3QgY29tcGxldGUgNCBjaGFyc1xuICBsID0gcGxhY2VIb2xkZXJzID4gMCA/IGxlbiAtIDQgOiBsZW5cblxuICB2YXIgTCA9IDBcblxuICBmb3IgKGkgPSAwLCBqID0gMDsgaSA8IGw7IGkgKz0gNCwgaiArPSAzKSB7XG4gICAgdG1wID0gKHJldkxvb2t1cFtiNjQuY2hhckNvZGVBdChpKV0gPDwgMTgpIHwgKHJldkxvb2t1cFtiNjQuY2hhckNvZGVBdChpICsgMSldIDw8IDEyKSB8IChyZXZMb29rdXBbYjY0LmNoYXJDb2RlQXQoaSArIDIpXSA8PCA2KSB8IHJldkxvb2t1cFtiNjQuY2hhckNvZGVBdChpICsgMyldXG4gICAgYXJyW0wrK10gPSAodG1wID4+IDE2KSAmIDB4RkZcbiAgICBhcnJbTCsrXSA9ICh0bXAgPj4gOCkgJiAweEZGXG4gICAgYXJyW0wrK10gPSB0bXAgJiAweEZGXG4gIH1cblxuICBpZiAocGxhY2VIb2xkZXJzID09PSAyKSB7XG4gICAgdG1wID0gKHJldkxvb2t1cFtiNjQuY2hhckNvZGVBdChpKV0gPDwgMikgfCAocmV2TG9va3VwW2I2NC5jaGFyQ29kZUF0KGkgKyAxKV0gPj4gNClcbiAgICBhcnJbTCsrXSA9IHRtcCAmIDB4RkZcbiAgfSBlbHNlIGlmIChwbGFjZUhvbGRlcnMgPT09IDEpIHtcbiAgICB0bXAgPSAocmV2TG9va3VwW2I2NC5jaGFyQ29kZUF0KGkpXSA8PCAxMCkgfCAocmV2TG9va3VwW2I2NC5jaGFyQ29kZUF0KGkgKyAxKV0gPDwgNCkgfCAocmV2TG9va3VwW2I2NC5jaGFyQ29kZUF0KGkgKyAyKV0gPj4gMilcbiAgICBhcnJbTCsrXSA9ICh0bXAgPj4gOCkgJiAweEZGXG4gICAgYXJyW0wrK10gPSB0bXAgJiAweEZGXG4gIH1cblxuICByZXR1cm4gYXJyXG59XG5cbmZ1bmN0aW9uIHRyaXBsZXRUb0Jhc2U2NCAobnVtKSB7XG4gIHJldHVybiBsb29rdXBbbnVtID4+IDE4ICYgMHgzRl0gKyBsb29rdXBbbnVtID4+IDEyICYgMHgzRl0gKyBsb29rdXBbbnVtID4+IDYgJiAweDNGXSArIGxvb2t1cFtudW0gJiAweDNGXVxufVxuXG5mdW5jdGlvbiBlbmNvZGVDaHVuayAodWludDgsIHN0YXJ0LCBlbmQpIHtcbiAgdmFyIHRtcFxuICB2YXIgb3V0cHV0ID0gW11cbiAgZm9yICh2YXIgaSA9IHN0YXJ0OyBpIDwgZW5kOyBpICs9IDMpIHtcbiAgICB0bXAgPSAodWludDhbaV0gPDwgMTYpICsgKHVpbnQ4W2kgKyAxXSA8PCA4KSArICh1aW50OFtpICsgMl0pXG4gICAgb3V0cHV0LnB1c2godHJpcGxldFRvQmFzZTY0KHRtcCkpXG4gIH1cbiAgcmV0dXJuIG91dHB1dC5qb2luKCcnKVxufVxuXG5mdW5jdGlvbiBmcm9tQnl0ZUFycmF5ICh1aW50OCkge1xuICB2YXIgdG1wXG4gIHZhciBsZW4gPSB1aW50OC5sZW5ndGhcbiAgdmFyIGV4dHJhQnl0ZXMgPSBsZW4gJSAzIC8vIGlmIHdlIGhhdmUgMSBieXRlIGxlZnQsIHBhZCAyIGJ5dGVzXG4gIHZhciBvdXRwdXQgPSAnJ1xuICB2YXIgcGFydHMgPSBbXVxuICB2YXIgbWF4Q2h1bmtMZW5ndGggPSAxNjM4MyAvLyBtdXN0IGJlIG11bHRpcGxlIG9mIDNcblxuICAvLyBnbyB0aHJvdWdoIHRoZSBhcnJheSBldmVyeSB0aHJlZSBieXRlcywgd2UnbGwgZGVhbCB3aXRoIHRyYWlsaW5nIHN0dWZmIGxhdGVyXG4gIGZvciAodmFyIGkgPSAwLCBsZW4yID0gbGVuIC0gZXh0cmFCeXRlczsgaSA8IGxlbjI7IGkgKz0gbWF4Q2h1bmtMZW5ndGgpIHtcbiAgICBwYXJ0cy5wdXNoKGVuY29kZUNodW5rKHVpbnQ4LCBpLCAoaSArIG1heENodW5rTGVuZ3RoKSA+IGxlbjIgPyBsZW4yIDogKGkgKyBtYXhDaHVua0xlbmd0aCkpKVxuICB9XG5cbiAgLy8gcGFkIHRoZSBlbmQgd2l0aCB6ZXJvcywgYnV0IG1ha2Ugc3VyZSB0byBub3QgZm9yZ2V0IHRoZSBleHRyYSBieXRlc1xuICBpZiAoZXh0cmFCeXRlcyA9PT0gMSkge1xuICAgIHRtcCA9IHVpbnQ4W2xlbiAtIDFdXG4gICAgb3V0cHV0ICs9IGxvb2t1cFt0bXAgPj4gMl1cbiAgICBvdXRwdXQgKz0gbG9va3VwWyh0bXAgPDwgNCkgJiAweDNGXVxuICAgIG91dHB1dCArPSAnPT0nXG4gIH0gZWxzZSBpZiAoZXh0cmFCeXRlcyA9PT0gMikge1xuICAgIHRtcCA9ICh1aW50OFtsZW4gLSAyXSA8PCA4KSArICh1aW50OFtsZW4gLSAxXSlcbiAgICBvdXRwdXQgKz0gbG9va3VwW3RtcCA+PiAxMF1cbiAgICBvdXRwdXQgKz0gbG9va3VwWyh0bXAgPj4gNCkgJiAweDNGXVxuICAgIG91dHB1dCArPSBsb29rdXBbKHRtcCA8PCAyKSAmIDB4M0ZdXG4gICAgb3V0cHV0ICs9ICc9J1xuICB9XG5cbiAgcGFydHMucHVzaChvdXRwdXQpXG5cbiAgcmV0dXJuIHBhcnRzLmpvaW4oJycpXG59XG4iLCJ2YXIgYmlnSW50ID0gKGZ1bmN0aW9uICh1bmRlZmluZWQpIHtcclxuICAgIFwidXNlIHN0cmljdFwiO1xyXG5cclxuICAgIHZhciBCQVNFID0gMWU3LFxyXG4gICAgICAgIExPR19CQVNFID0gNyxcclxuICAgICAgICBNQVhfSU5UID0gOTAwNzE5OTI1NDc0MDk5MixcclxuICAgICAgICBNQVhfSU5UX0FSUiA9IHNtYWxsVG9BcnJheShNQVhfSU5UKSxcclxuICAgICAgICBMT0dfTUFYX0lOVCA9IE1hdGgubG9nKE1BWF9JTlQpO1xyXG5cclxuICAgIGZ1bmN0aW9uIEludGVnZXIodiwgcmFkaXgpIHtcclxuICAgICAgICBpZiAodHlwZW9mIHYgPT09IFwidW5kZWZpbmVkXCIpIHJldHVybiBJbnRlZ2VyWzBdO1xyXG4gICAgICAgIGlmICh0eXBlb2YgcmFkaXggIT09IFwidW5kZWZpbmVkXCIpIHJldHVybiArcmFkaXggPT09IDEwID8gcGFyc2VWYWx1ZSh2KSA6IHBhcnNlQmFzZSh2LCByYWRpeCk7XHJcbiAgICAgICAgcmV0dXJuIHBhcnNlVmFsdWUodik7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gQmlnSW50ZWdlcih2YWx1ZSwgc2lnbikge1xyXG4gICAgICAgIHRoaXMudmFsdWUgPSB2YWx1ZTtcclxuICAgICAgICB0aGlzLnNpZ24gPSBzaWduO1xyXG4gICAgICAgIHRoaXMuaXNTbWFsbCA9IGZhbHNlO1xyXG4gICAgfVxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUgPSBPYmplY3QuY3JlYXRlKEludGVnZXIucHJvdG90eXBlKTtcclxuXHJcbiAgICBmdW5jdGlvbiBTbWFsbEludGVnZXIodmFsdWUpIHtcclxuICAgICAgICB0aGlzLnZhbHVlID0gdmFsdWU7XHJcbiAgICAgICAgdGhpcy5zaWduID0gdmFsdWUgPCAwO1xyXG4gICAgICAgIHRoaXMuaXNTbWFsbCA9IHRydWU7XHJcbiAgICB9XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlID0gT2JqZWN0LmNyZWF0ZShJbnRlZ2VyLnByb3RvdHlwZSk7XHJcblxyXG4gICAgZnVuY3Rpb24gaXNQcmVjaXNlKG4pIHtcclxuICAgICAgICByZXR1cm4gLU1BWF9JTlQgPCBuICYmIG4gPCBNQVhfSU5UO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIHNtYWxsVG9BcnJheShuKSB7IC8vIEZvciBwZXJmb3JtYW5jZSByZWFzb25zIGRvZXNuJ3QgcmVmZXJlbmNlIEJBU0UsIG5lZWQgdG8gY2hhbmdlIHRoaXMgZnVuY3Rpb24gaWYgQkFTRSBjaGFuZ2VzXHJcbiAgICAgICAgaWYgKG4gPCAxZTcpXHJcbiAgICAgICAgICAgIHJldHVybiBbbl07XHJcbiAgICAgICAgaWYgKG4gPCAxZTE0KVxyXG4gICAgICAgICAgICByZXR1cm4gW24gJSAxZTcsIE1hdGguZmxvb3IobiAvIDFlNyldO1xyXG4gICAgICAgIHJldHVybiBbbiAlIDFlNywgTWF0aC5mbG9vcihuIC8gMWU3KSAlIDFlNywgTWF0aC5mbG9vcihuIC8gMWUxNCldO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIGFycmF5VG9TbWFsbChhcnIpIHsgLy8gSWYgQkFTRSBjaGFuZ2VzIHRoaXMgZnVuY3Rpb24gbWF5IG5lZWQgdG8gY2hhbmdlXHJcbiAgICAgICAgdHJpbShhcnIpO1xyXG4gICAgICAgIHZhciBsZW5ndGggPSBhcnIubGVuZ3RoO1xyXG4gICAgICAgIGlmIChsZW5ndGggPCA0ICYmIGNvbXBhcmVBYnMoYXJyLCBNQVhfSU5UX0FSUikgPCAwKSB7XHJcbiAgICAgICAgICAgIHN3aXRjaCAobGVuZ3RoKSB7XHJcbiAgICAgICAgICAgICAgICBjYXNlIDA6IHJldHVybiAwO1xyXG4gICAgICAgICAgICAgICAgY2FzZSAxOiByZXR1cm4gYXJyWzBdO1xyXG4gICAgICAgICAgICAgICAgY2FzZSAyOiByZXR1cm4gYXJyWzBdICsgYXJyWzFdICogQkFTRTtcclxuICAgICAgICAgICAgICAgIGRlZmF1bHQ6IHJldHVybiBhcnJbMF0gKyAoYXJyWzFdICsgYXJyWzJdICogQkFTRSkgKiBCQVNFO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBhcnI7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gdHJpbSh2KSB7XHJcbiAgICAgICAgdmFyIGkgPSB2Lmxlbmd0aDtcclxuICAgICAgICB3aGlsZSAodlstLWldID09PSAwKTtcclxuICAgICAgICB2Lmxlbmd0aCA9IGkgKyAxO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIGNyZWF0ZUFycmF5KGxlbmd0aCkgeyAvLyBmdW5jdGlvbiBzaGFtZWxlc3NseSBzdG9sZW4gZnJvbSBZYWZmbGUncyBsaWJyYXJ5IGh0dHBzOi8vZ2l0aHViLmNvbS9ZYWZmbGUvQmlnSW50ZWdlclxyXG4gICAgICAgIHZhciB4ID0gbmV3IEFycmF5KGxlbmd0aCk7XHJcbiAgICAgICAgdmFyIGkgPSAtMTtcclxuICAgICAgICB3aGlsZSAoKytpIDwgbGVuZ3RoKSB7XHJcbiAgICAgICAgICAgIHhbaV0gPSAwO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4geDtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiB0cnVuY2F0ZShuKSB7XHJcbiAgICAgICAgaWYgKG4gPiAwKSByZXR1cm4gTWF0aC5mbG9vcihuKTtcclxuICAgICAgICByZXR1cm4gTWF0aC5jZWlsKG4pO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIGFkZChhLCBiKSB7IC8vIGFzc3VtZXMgYSBhbmQgYiBhcmUgYXJyYXlzIHdpdGggYS5sZW5ndGggPj0gYi5sZW5ndGhcclxuICAgICAgICB2YXIgbF9hID0gYS5sZW5ndGgsXHJcbiAgICAgICAgICAgIGxfYiA9IGIubGVuZ3RoLFxyXG4gICAgICAgICAgICByID0gbmV3IEFycmF5KGxfYSksXHJcbiAgICAgICAgICAgIGNhcnJ5ID0gMCxcclxuICAgICAgICAgICAgYmFzZSA9IEJBU0UsXHJcbiAgICAgICAgICAgIHN1bSwgaTtcclxuICAgICAgICBmb3IgKGkgPSAwOyBpIDwgbF9iOyBpKyspIHtcclxuICAgICAgICAgICAgc3VtID0gYVtpXSArIGJbaV0gKyBjYXJyeTtcclxuICAgICAgICAgICAgY2FycnkgPSBzdW0gPj0gYmFzZSA/IDEgOiAwO1xyXG4gICAgICAgICAgICByW2ldID0gc3VtIC0gY2FycnkgKiBiYXNlO1xyXG4gICAgICAgIH1cclxuICAgICAgICB3aGlsZSAoaSA8IGxfYSkge1xyXG4gICAgICAgICAgICBzdW0gPSBhW2ldICsgY2Fycnk7XHJcbiAgICAgICAgICAgIGNhcnJ5ID0gc3VtID09PSBiYXNlID8gMSA6IDA7XHJcbiAgICAgICAgICAgIHJbaSsrXSA9IHN1bSAtIGNhcnJ5ICogYmFzZTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKGNhcnJ5ID4gMCkgci5wdXNoKGNhcnJ5KTtcclxuICAgICAgICByZXR1cm4gcjtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBhZGRBbnkoYSwgYikge1xyXG4gICAgICAgIGlmIChhLmxlbmd0aCA+PSBiLmxlbmd0aCkgcmV0dXJuIGFkZChhLCBiKTtcclxuICAgICAgICByZXR1cm4gYWRkKGIsIGEpO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIGFkZFNtYWxsKGEsIGNhcnJ5KSB7IC8vIGFzc3VtZXMgYSBpcyBhcnJheSwgY2FycnkgaXMgbnVtYmVyIHdpdGggMCA8PSBjYXJyeSA8IE1BWF9JTlRcclxuICAgICAgICB2YXIgbCA9IGEubGVuZ3RoLFxyXG4gICAgICAgICAgICByID0gbmV3IEFycmF5KGwpLFxyXG4gICAgICAgICAgICBiYXNlID0gQkFTRSxcclxuICAgICAgICAgICAgc3VtLCBpO1xyXG4gICAgICAgIGZvciAoaSA9IDA7IGkgPCBsOyBpKyspIHtcclxuICAgICAgICAgICAgc3VtID0gYVtpXSAtIGJhc2UgKyBjYXJyeTtcclxuICAgICAgICAgICAgY2FycnkgPSBNYXRoLmZsb29yKHN1bSAvIGJhc2UpO1xyXG4gICAgICAgICAgICByW2ldID0gc3VtIC0gY2FycnkgKiBiYXNlO1xyXG4gICAgICAgICAgICBjYXJyeSArPSAxO1xyXG4gICAgICAgIH1cclxuICAgICAgICB3aGlsZSAoY2FycnkgPiAwKSB7XHJcbiAgICAgICAgICAgIHJbaSsrXSA9IGNhcnJ5ICUgYmFzZTtcclxuICAgICAgICAgICAgY2FycnkgPSBNYXRoLmZsb29yKGNhcnJ5IC8gYmFzZSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiByO1xyXG4gICAgfVxyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmFkZCA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgdmFyIHZhbHVlLCBuID0gcGFyc2VWYWx1ZSh2KTtcclxuICAgICAgICBpZiAodGhpcy5zaWduICE9PSBuLnNpZ24pIHtcclxuICAgICAgICAgICAgcmV0dXJuIHRoaXMuc3VidHJhY3Qobi5uZWdhdGUoKSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHZhciBhID0gdGhpcy52YWx1ZSwgYiA9IG4udmFsdWU7XHJcbiAgICAgICAgaWYgKG4uaXNTbWFsbCkge1xyXG4gICAgICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIoYWRkU21hbGwoYSwgTWF0aC5hYnMoYikpLCB0aGlzLnNpZ24pO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIoYWRkQW55KGEsIGIpLCB0aGlzLnNpZ24pO1xyXG4gICAgfTtcclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLnBsdXMgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5hZGQ7XHJcblxyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5hZGQgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHZhciBuID0gcGFyc2VWYWx1ZSh2KTtcclxuICAgICAgICB2YXIgYSA9IHRoaXMudmFsdWU7XHJcbiAgICAgICAgaWYgKGEgPCAwICE9PSBuLnNpZ24pIHtcclxuICAgICAgICAgICAgcmV0dXJuIHRoaXMuc3VidHJhY3Qobi5uZWdhdGUoKSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHZhciBiID0gbi52YWx1ZTtcclxuICAgICAgICBpZiAobi5pc1NtYWxsKSB7XHJcbiAgICAgICAgICAgIGlmIChpc1ByZWNpc2UoYSArIGIpKSByZXR1cm4gbmV3IFNtYWxsSW50ZWdlcihhICsgYik7XHJcbiAgICAgICAgICAgIGIgPSBzbWFsbFRvQXJyYXkoTWF0aC5hYnMoYikpO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIoYWRkU21hbGwoYiwgTWF0aC5hYnMoYSkpLCBhIDwgMCk7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5wbHVzID0gU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5hZGQ7XHJcblxyXG4gICAgZnVuY3Rpb24gc3VidHJhY3QoYSwgYikgeyAvLyBhc3N1bWVzIGEgYW5kIGIgYXJlIGFycmF5cyB3aXRoIGEgPj0gYlxyXG4gICAgICAgIHZhciBhX2wgPSBhLmxlbmd0aCxcclxuICAgICAgICAgICAgYl9sID0gYi5sZW5ndGgsXHJcbiAgICAgICAgICAgIHIgPSBuZXcgQXJyYXkoYV9sKSxcclxuICAgICAgICAgICAgYm9ycm93ID0gMCxcclxuICAgICAgICAgICAgYmFzZSA9IEJBU0UsXHJcbiAgICAgICAgICAgIGksIGRpZmZlcmVuY2U7XHJcbiAgICAgICAgZm9yIChpID0gMDsgaSA8IGJfbDsgaSsrKSB7XHJcbiAgICAgICAgICAgIGRpZmZlcmVuY2UgPSBhW2ldIC0gYm9ycm93IC0gYltpXTtcclxuICAgICAgICAgICAgaWYgKGRpZmZlcmVuY2UgPCAwKSB7XHJcbiAgICAgICAgICAgICAgICBkaWZmZXJlbmNlICs9IGJhc2U7XHJcbiAgICAgICAgICAgICAgICBib3Jyb3cgPSAxO1xyXG4gICAgICAgICAgICB9IGVsc2UgYm9ycm93ID0gMDtcclxuICAgICAgICAgICAgcltpXSA9IGRpZmZlcmVuY2U7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGZvciAoaSA9IGJfbDsgaSA8IGFfbDsgaSsrKSB7XHJcbiAgICAgICAgICAgIGRpZmZlcmVuY2UgPSBhW2ldIC0gYm9ycm93O1xyXG4gICAgICAgICAgICBpZiAoZGlmZmVyZW5jZSA8IDApIGRpZmZlcmVuY2UgKz0gYmFzZTtcclxuICAgICAgICAgICAgZWxzZSB7XHJcbiAgICAgICAgICAgICAgICByW2krK10gPSBkaWZmZXJlbmNlO1xyXG4gICAgICAgICAgICAgICAgYnJlYWs7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgcltpXSA9IGRpZmZlcmVuY2U7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGZvciAoOyBpIDwgYV9sOyBpKyspIHtcclxuICAgICAgICAgICAgcltpXSA9IGFbaV07XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHRyaW0ocik7XHJcbiAgICAgICAgcmV0dXJuIHI7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gc3VidHJhY3RBbnkoYSwgYiwgc2lnbikge1xyXG4gICAgICAgIHZhciB2YWx1ZSwgaXNTbWFsbDtcclxuICAgICAgICBpZiAoY29tcGFyZUFicyhhLCBiKSA+PSAwKSB7XHJcbiAgICAgICAgICAgIHZhbHVlID0gc3VidHJhY3QoYSxiKTtcclxuICAgICAgICB9IGVsc2Uge1xyXG4gICAgICAgICAgICB2YWx1ZSA9IHN1YnRyYWN0KGIsIGEpO1xyXG4gICAgICAgICAgICBzaWduID0gIXNpZ247XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHZhbHVlID0gYXJyYXlUb1NtYWxsKHZhbHVlKTtcclxuICAgICAgICBpZiAodHlwZW9mIHZhbHVlID09PSBcIm51bWJlclwiKSB7XHJcbiAgICAgICAgICAgIGlmIChzaWduKSB2YWx1ZSA9IC12YWx1ZTtcclxuICAgICAgICAgICAgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIodmFsdWUpO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIodmFsdWUsIHNpZ24pO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIHN1YnRyYWN0U21hbGwoYSwgYiwgc2lnbikgeyAvLyBhc3N1bWVzIGEgaXMgYXJyYXksIGIgaXMgbnVtYmVyIHdpdGggMCA8PSBiIDwgTUFYX0lOVFxyXG4gICAgICAgIHZhciBsID0gYS5sZW5ndGgsXHJcbiAgICAgICAgICAgIHIgPSBuZXcgQXJyYXkobCksXHJcbiAgICAgICAgICAgIGNhcnJ5ID0gLWIsXHJcbiAgICAgICAgICAgIGJhc2UgPSBCQVNFLFxyXG4gICAgICAgICAgICBpLCBkaWZmZXJlbmNlO1xyXG4gICAgICAgIGZvciAoaSA9IDA7IGkgPCBsOyBpKyspIHtcclxuICAgICAgICAgICAgZGlmZmVyZW5jZSA9IGFbaV0gKyBjYXJyeTtcclxuICAgICAgICAgICAgY2FycnkgPSBNYXRoLmZsb29yKGRpZmZlcmVuY2UgLyBiYXNlKTtcclxuICAgICAgICAgICAgZGlmZmVyZW5jZSAlPSBiYXNlO1xyXG4gICAgICAgICAgICByW2ldID0gZGlmZmVyZW5jZSA8IDAgPyBkaWZmZXJlbmNlICsgYmFzZSA6IGRpZmZlcmVuY2U7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHIgPSBhcnJheVRvU21hbGwocik7XHJcbiAgICAgICAgaWYgKHR5cGVvZiByID09PSBcIm51bWJlclwiKSB7XHJcbiAgICAgICAgICAgIGlmIChzaWduKSByID0gLXI7XHJcbiAgICAgICAgICAgIHJldHVybiBuZXcgU21hbGxJbnRlZ2VyKHIpO1xyXG4gICAgICAgIH0gcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKHIsIHNpZ24pO1xyXG4gICAgfVxyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLnN1YnRyYWN0ID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICB2YXIgbiA9IHBhcnNlVmFsdWUodik7XHJcbiAgICAgICAgaWYgKHRoaXMuc2lnbiAhPT0gbi5zaWduKSB7XHJcbiAgICAgICAgICAgIHJldHVybiB0aGlzLmFkZChuLm5lZ2F0ZSgpKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgdmFyIGEgPSB0aGlzLnZhbHVlLCBiID0gbi52YWx1ZTtcclxuICAgICAgICBpZiAobi5pc1NtYWxsKVxyXG4gICAgICAgICAgICByZXR1cm4gc3VidHJhY3RTbWFsbChhLCBNYXRoLmFicyhiKSwgdGhpcy5zaWduKTtcclxuICAgICAgICByZXR1cm4gc3VidHJhY3RBbnkoYSwgYiwgdGhpcy5zaWduKTtcclxuICAgIH07XHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5taW51cyA9IEJpZ0ludGVnZXIucHJvdG90eXBlLnN1YnRyYWN0O1xyXG5cclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuc3VidHJhY3QgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHZhciBuID0gcGFyc2VWYWx1ZSh2KTtcclxuICAgICAgICB2YXIgYSA9IHRoaXMudmFsdWU7XHJcbiAgICAgICAgaWYgKGEgPCAwICE9PSBuLnNpZ24pIHtcclxuICAgICAgICAgICAgcmV0dXJuIHRoaXMuYWRkKG4ubmVnYXRlKCkpO1xyXG4gICAgICAgIH1cclxuICAgICAgICB2YXIgYiA9IG4udmFsdWU7XHJcbiAgICAgICAgaWYgKG4uaXNTbWFsbCkge1xyXG4gICAgICAgICAgICByZXR1cm4gbmV3IFNtYWxsSW50ZWdlcihhIC0gYik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBzdWJ0cmFjdFNtYWxsKGIsIE1hdGguYWJzKGEpLCBhID49IDApO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUubWludXMgPSBTbWFsbEludGVnZXIucHJvdG90eXBlLnN1YnRyYWN0O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLm5lZ2F0ZSA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIodGhpcy52YWx1ZSwgIXRoaXMuc2lnbik7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5uZWdhdGUgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgdmFyIHNpZ24gPSB0aGlzLnNpZ247XHJcbiAgICAgICAgdmFyIHNtYWxsID0gbmV3IFNtYWxsSW50ZWdlcigtdGhpcy52YWx1ZSk7XHJcbiAgICAgICAgc21hbGwuc2lnbiA9ICFzaWduO1xyXG4gICAgICAgIHJldHVybiBzbWFsbDtcclxuICAgIH07XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuYWJzID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcih0aGlzLnZhbHVlLCBmYWxzZSk7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5hYnMgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIoTWF0aC5hYnModGhpcy52YWx1ZSkpO1xyXG4gICAgfTtcclxuXHJcbiAgICBmdW5jdGlvbiBtdWx0aXBseUxvbmcoYSwgYikge1xyXG4gICAgICAgIHZhciBhX2wgPSBhLmxlbmd0aCxcclxuICAgICAgICAgICAgYl9sID0gYi5sZW5ndGgsXHJcbiAgICAgICAgICAgIGwgPSBhX2wgKyBiX2wsXHJcbiAgICAgICAgICAgIHIgPSBjcmVhdGVBcnJheShsKSxcclxuICAgICAgICAgICAgYmFzZSA9IEJBU0UsXHJcbiAgICAgICAgICAgIHByb2R1Y3QsIGNhcnJ5LCBpLCBhX2ksIGJfajtcclxuICAgICAgICBmb3IgKGkgPSAwOyBpIDwgYV9sOyArK2kpIHtcclxuICAgICAgICAgICAgYV9pID0gYVtpXTtcclxuICAgICAgICAgICAgZm9yICh2YXIgaiA9IDA7IGogPCBiX2w7ICsraikge1xyXG4gICAgICAgICAgICAgICAgYl9qID0gYltqXTtcclxuICAgICAgICAgICAgICAgIHByb2R1Y3QgPSBhX2kgKiBiX2ogKyByW2kgKyBqXTtcclxuICAgICAgICAgICAgICAgIGNhcnJ5ID0gTWF0aC5mbG9vcihwcm9kdWN0IC8gYmFzZSk7XHJcbiAgICAgICAgICAgICAgICByW2kgKyBqXSA9IHByb2R1Y3QgLSBjYXJyeSAqIGJhc2U7XHJcbiAgICAgICAgICAgICAgICByW2kgKyBqICsgMV0gKz0gY2Fycnk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICB9XHJcbiAgICAgICAgdHJpbShyKTtcclxuICAgICAgICByZXR1cm4gcjtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBtdWx0aXBseVNtYWxsKGEsIGIpIHsgLy8gYXNzdW1lcyBhIGlzIGFycmF5LCBiIGlzIG51bWJlciB3aXRoIHxifCA8IEJBU0VcclxuICAgICAgICB2YXIgbCA9IGEubGVuZ3RoLFxyXG4gICAgICAgICAgICByID0gbmV3IEFycmF5KGwpLFxyXG4gICAgICAgICAgICBiYXNlID0gQkFTRSxcclxuICAgICAgICAgICAgY2FycnkgPSAwLFxyXG4gICAgICAgICAgICBwcm9kdWN0LCBpO1xyXG4gICAgICAgIGZvciAoaSA9IDA7IGkgPCBsOyBpKyspIHtcclxuICAgICAgICAgICAgcHJvZHVjdCA9IGFbaV0gKiBiICsgY2Fycnk7XHJcbiAgICAgICAgICAgIGNhcnJ5ID0gTWF0aC5mbG9vcihwcm9kdWN0IC8gYmFzZSk7XHJcbiAgICAgICAgICAgIHJbaV0gPSBwcm9kdWN0IC0gY2FycnkgKiBiYXNlO1xyXG4gICAgICAgIH1cclxuICAgICAgICB3aGlsZSAoY2FycnkgPiAwKSB7XHJcbiAgICAgICAgICAgIHJbaSsrXSA9IGNhcnJ5ICUgYmFzZTtcclxuICAgICAgICAgICAgY2FycnkgPSBNYXRoLmZsb29yKGNhcnJ5IC8gYmFzZSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiByO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIHNoaWZ0TGVmdCh4LCBuKSB7XHJcbiAgICAgICAgdmFyIHIgPSBbXTtcclxuICAgICAgICB3aGlsZSAobi0tID4gMCkgci5wdXNoKDApO1xyXG4gICAgICAgIHJldHVybiByLmNvbmNhdCh4KTtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBtdWx0aXBseUthcmF0c3ViYSh4LCB5KSB7XHJcbiAgICAgICAgdmFyIG4gPSBNYXRoLm1heCh4Lmxlbmd0aCwgeS5sZW5ndGgpO1xyXG5cclxuICAgICAgICBpZiAobiA8PSAzMCkgcmV0dXJuIG11bHRpcGx5TG9uZyh4LCB5KTtcclxuICAgICAgICBuID0gTWF0aC5jZWlsKG4gLyAyKTtcclxuXHJcbiAgICAgICAgdmFyIGIgPSB4LnNsaWNlKG4pLFxyXG4gICAgICAgICAgICBhID0geC5zbGljZSgwLCBuKSxcclxuICAgICAgICAgICAgZCA9IHkuc2xpY2UobiksXHJcbiAgICAgICAgICAgIGMgPSB5LnNsaWNlKDAsIG4pO1xyXG5cclxuICAgICAgICB2YXIgYWMgPSBtdWx0aXBseUthcmF0c3ViYShhLCBjKSxcclxuICAgICAgICAgICAgYmQgPSBtdWx0aXBseUthcmF0c3ViYShiLCBkKSxcclxuICAgICAgICAgICAgYWJjZCA9IG11bHRpcGx5S2FyYXRzdWJhKGFkZEFueShhLCBiKSwgYWRkQW55KGMsIGQpKTtcclxuXHJcbiAgICAgICAgdmFyIHByb2R1Y3QgPSBhZGRBbnkoYWRkQW55KGFjLCBzaGlmdExlZnQoc3VidHJhY3Qoc3VidHJhY3QoYWJjZCwgYWMpLCBiZCksIG4pKSwgc2hpZnRMZWZ0KGJkLCAyICogbikpO1xyXG4gICAgICAgIHRyaW0ocHJvZHVjdCk7XHJcbiAgICAgICAgcmV0dXJuIHByb2R1Y3Q7XHJcbiAgICB9XHJcblxyXG4gICAgLy8gVGhlIGZvbGxvd2luZyBmdW5jdGlvbiBpcyBkZXJpdmVkIGZyb20gYSBzdXJmYWNlIGZpdCBvZiBhIGdyYXBoIHBsb3R0aW5nIHRoZSBwZXJmb3JtYW5jZSBkaWZmZXJlbmNlXHJcbiAgICAvLyBiZXR3ZWVuIGxvbmcgbXVsdGlwbGljYXRpb24gYW5kIGthcmF0c3ViYSBtdWx0aXBsaWNhdGlvbiB2ZXJzdXMgdGhlIGxlbmd0aHMgb2YgdGhlIHR3byBhcnJheXMuXHJcbiAgICBmdW5jdGlvbiB1c2VLYXJhdHN1YmEobDEsIGwyKSB7XHJcbiAgICAgICAgcmV0dXJuIC0wLjAxMiAqIGwxIC0gMC4wMTIgKiBsMiArIDAuMDAwMDE1ICogbDEgKiBsMiA+IDA7XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUubXVsdGlwbHkgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHZhciB2YWx1ZSwgbiA9IHBhcnNlVmFsdWUodiksXHJcbiAgICAgICAgICAgIGEgPSB0aGlzLnZhbHVlLCBiID0gbi52YWx1ZSxcclxuICAgICAgICAgICAgc2lnbiA9IHRoaXMuc2lnbiAhPT0gbi5zaWduLFxyXG4gICAgICAgICAgICBhYnM7XHJcbiAgICAgICAgaWYgKG4uaXNTbWFsbCkge1xyXG4gICAgICAgICAgICBpZiAoYiA9PT0gMCkgcmV0dXJuIEludGVnZXJbMF07XHJcbiAgICAgICAgICAgIGlmIChiID09PSAxKSByZXR1cm4gdGhpcztcclxuICAgICAgICAgICAgaWYgKGIgPT09IC0xKSByZXR1cm4gdGhpcy5uZWdhdGUoKTtcclxuICAgICAgICAgICAgYWJzID0gTWF0aC5hYnMoYik7XHJcbiAgICAgICAgICAgIGlmIChhYnMgPCBCQVNFKSB7XHJcbiAgICAgICAgICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIobXVsdGlwbHlTbWFsbChhLCBhYnMpLCBzaWduKTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICBiID0gc21hbGxUb0FycmF5KGFicyk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGlmICh1c2VLYXJhdHN1YmEoYS5sZW5ndGgsIGIubGVuZ3RoKSkgLy8gS2FyYXRzdWJhIGlzIG9ubHkgZmFzdGVyIGZvciBjZXJ0YWluIGFycmF5IHNpemVzXHJcbiAgICAgICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihtdWx0aXBseUthcmF0c3ViYShhLCBiKSwgc2lnbik7XHJcbiAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKG11bHRpcGx5TG9uZyhhLCBiKSwgc2lnbik7XHJcbiAgICB9O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLnRpbWVzID0gQmlnSW50ZWdlci5wcm90b3R5cGUubXVsdGlwbHk7XHJcblxyXG4gICAgZnVuY3Rpb24gbXVsdGlwbHlTbWFsbEFuZEFycmF5KGEsIGIsIHNpZ24pIHsgLy8gYSA+PSAwXHJcbiAgICAgICAgaWYgKGEgPCBCQVNFKSB7XHJcbiAgICAgICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihtdWx0aXBseVNtYWxsKGIsIGEpLCBzaWduKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKG11bHRpcGx5TG9uZyhiLCBzbWFsbFRvQXJyYXkoYSkpLCBzaWduKTtcclxuICAgIH1cclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuX211bHRpcGx5QnlTbWFsbCA9IGZ1bmN0aW9uIChhKSB7XHJcbiAgICAgICAgICAgIGlmIChpc1ByZWNpc2UoYS52YWx1ZSAqIHRoaXMudmFsdWUpKSB7XHJcbiAgICAgICAgICAgICAgICByZXR1cm4gbmV3IFNtYWxsSW50ZWdlcihhLnZhbHVlICogdGhpcy52YWx1ZSk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgcmV0dXJuIG11bHRpcGx5U21hbGxBbmRBcnJheShNYXRoLmFicyhhLnZhbHVlKSwgc21hbGxUb0FycmF5KE1hdGguYWJzKHRoaXMudmFsdWUpKSwgdGhpcy5zaWduICE9PSBhLnNpZ24pO1xyXG4gICAgfTtcclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLl9tdWx0aXBseUJ5U21hbGwgPSBmdW5jdGlvbiAoYSkge1xyXG4gICAgICAgICAgICBpZiAoYS52YWx1ZSA9PT0gMCkgcmV0dXJuIEludGVnZXJbMF07XHJcbiAgICAgICAgICAgIGlmIChhLnZhbHVlID09PSAxKSByZXR1cm4gdGhpcztcclxuICAgICAgICAgICAgaWYgKGEudmFsdWUgPT09IC0xKSByZXR1cm4gdGhpcy5uZWdhdGUoKTtcclxuICAgICAgICAgICAgcmV0dXJuIG11bHRpcGx5U21hbGxBbmRBcnJheShNYXRoLmFicyhhLnZhbHVlKSwgdGhpcy52YWx1ZSwgdGhpcy5zaWduICE9PSBhLnNpZ24pO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUubXVsdGlwbHkgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHJldHVybiBwYXJzZVZhbHVlKHYpLl9tdWx0aXBseUJ5U21hbGwodGhpcyk7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS50aW1lcyA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUubXVsdGlwbHk7XHJcblxyXG4gICAgZnVuY3Rpb24gc3F1YXJlKGEpIHtcclxuICAgICAgICB2YXIgbCA9IGEubGVuZ3RoLFxyXG4gICAgICAgICAgICByID0gY3JlYXRlQXJyYXkobCArIGwpLFxyXG4gICAgICAgICAgICBiYXNlID0gQkFTRSxcclxuICAgICAgICAgICAgcHJvZHVjdCwgY2FycnksIGksIGFfaSwgYV9qO1xyXG4gICAgICAgIGZvciAoaSA9IDA7IGkgPCBsOyBpKyspIHtcclxuICAgICAgICAgICAgYV9pID0gYVtpXTtcclxuICAgICAgICAgICAgZm9yICh2YXIgaiA9IDA7IGogPCBsOyBqKyspIHtcclxuICAgICAgICAgICAgICAgIGFfaiA9IGFbal07XHJcbiAgICAgICAgICAgICAgICBwcm9kdWN0ID0gYV9pICogYV9qICsgcltpICsgal07XHJcbiAgICAgICAgICAgICAgICBjYXJyeSA9IE1hdGguZmxvb3IocHJvZHVjdCAvIGJhc2UpO1xyXG4gICAgICAgICAgICAgICAgcltpICsgal0gPSBwcm9kdWN0IC0gY2FycnkgKiBiYXNlO1xyXG4gICAgICAgICAgICAgICAgcltpICsgaiArIDFdICs9IGNhcnJ5O1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHRyaW0ocik7XHJcbiAgICAgICAgcmV0dXJuIHI7XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuc3F1YXJlID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihzcXVhcmUodGhpcy52YWx1ZSksIGZhbHNlKTtcclxuICAgIH07XHJcblxyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5zcXVhcmUgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgdmFyIHZhbHVlID0gdGhpcy52YWx1ZSAqIHRoaXMudmFsdWU7XHJcbiAgICAgICAgaWYgKGlzUHJlY2lzZSh2YWx1ZSkpIHJldHVybiBuZXcgU21hbGxJbnRlZ2VyKHZhbHVlKTtcclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIoc3F1YXJlKHNtYWxsVG9BcnJheShNYXRoLmFicyh0aGlzLnZhbHVlKSkpLCBmYWxzZSk7XHJcbiAgICB9O1xyXG5cclxuICAgIGZ1bmN0aW9uIGRpdk1vZDEoYSwgYikgeyAvLyBMZWZ0IG92ZXIgZnJvbSBwcmV2aW91cyB2ZXJzaW9uLiBQZXJmb3JtcyBmYXN0ZXIgdGhhbiBkaXZNb2QyIG9uIHNtYWxsZXIgaW5wdXQgc2l6ZXMuXHJcbiAgICAgICAgdmFyIGFfbCA9IGEubGVuZ3RoLFxyXG4gICAgICAgICAgICBiX2wgPSBiLmxlbmd0aCxcclxuICAgICAgICAgICAgYmFzZSA9IEJBU0UsXHJcbiAgICAgICAgICAgIHJlc3VsdCA9IGNyZWF0ZUFycmF5KGIubGVuZ3RoKSxcclxuICAgICAgICAgICAgZGl2aXNvck1vc3RTaWduaWZpY2FudERpZ2l0ID0gYltiX2wgLSAxXSxcclxuICAgICAgICAgICAgLy8gbm9ybWFsaXphdGlvblxyXG4gICAgICAgICAgICBsYW1iZGEgPSBNYXRoLmNlaWwoYmFzZSAvICgyICogZGl2aXNvck1vc3RTaWduaWZpY2FudERpZ2l0KSksXHJcbiAgICAgICAgICAgIHJlbWFpbmRlciA9IG11bHRpcGx5U21hbGwoYSwgbGFtYmRhKSxcclxuICAgICAgICAgICAgZGl2aXNvciA9IG11bHRpcGx5U21hbGwoYiwgbGFtYmRhKSxcclxuICAgICAgICAgICAgcXVvdGllbnREaWdpdCwgc2hpZnQsIGNhcnJ5LCBib3Jyb3csIGksIGwsIHE7XHJcbiAgICAgICAgaWYgKHJlbWFpbmRlci5sZW5ndGggPD0gYV9sKSByZW1haW5kZXIucHVzaCgwKTtcclxuICAgICAgICBkaXZpc29yLnB1c2goMCk7XHJcbiAgICAgICAgZGl2aXNvck1vc3RTaWduaWZpY2FudERpZ2l0ID0gZGl2aXNvcltiX2wgLSAxXTtcclxuICAgICAgICBmb3IgKHNoaWZ0ID0gYV9sIC0gYl9sOyBzaGlmdCA+PSAwOyBzaGlmdC0tKSB7XHJcbiAgICAgICAgICAgIHF1b3RpZW50RGlnaXQgPSBiYXNlIC0gMTtcclxuICAgICAgICAgICAgaWYgKHJlbWFpbmRlcltzaGlmdCArIGJfbF0gIT09IGRpdmlzb3JNb3N0U2lnbmlmaWNhbnREaWdpdCkge1xyXG4gICAgICAgICAgICAgIHF1b3RpZW50RGlnaXQgPSBNYXRoLmZsb29yKChyZW1haW5kZXJbc2hpZnQgKyBiX2xdICogYmFzZSArIHJlbWFpbmRlcltzaGlmdCArIGJfbCAtIDFdKSAvIGRpdmlzb3JNb3N0U2lnbmlmaWNhbnREaWdpdCk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgLy8gcXVvdGllbnREaWdpdCA8PSBiYXNlIC0gMVxyXG4gICAgICAgICAgICBjYXJyeSA9IDA7XHJcbiAgICAgICAgICAgIGJvcnJvdyA9IDA7XHJcbiAgICAgICAgICAgIGwgPSBkaXZpc29yLmxlbmd0aDtcclxuICAgICAgICAgICAgZm9yIChpID0gMDsgaSA8IGw7IGkrKykge1xyXG4gICAgICAgICAgICAgICAgY2FycnkgKz0gcXVvdGllbnREaWdpdCAqIGRpdmlzb3JbaV07XHJcbiAgICAgICAgICAgICAgICBxID0gTWF0aC5mbG9vcihjYXJyeSAvIGJhc2UpO1xyXG4gICAgICAgICAgICAgICAgYm9ycm93ICs9IHJlbWFpbmRlcltzaGlmdCArIGldIC0gKGNhcnJ5IC0gcSAqIGJhc2UpO1xyXG4gICAgICAgICAgICAgICAgY2FycnkgPSBxO1xyXG4gICAgICAgICAgICAgICAgaWYgKGJvcnJvdyA8IDApIHtcclxuICAgICAgICAgICAgICAgICAgICByZW1haW5kZXJbc2hpZnQgKyBpXSA9IGJvcnJvdyArIGJhc2U7XHJcbiAgICAgICAgICAgICAgICAgICAgYm9ycm93ID0gLTE7XHJcbiAgICAgICAgICAgICAgICB9IGVsc2Uge1xyXG4gICAgICAgICAgICAgICAgICAgIHJlbWFpbmRlcltzaGlmdCArIGldID0gYm9ycm93O1xyXG4gICAgICAgICAgICAgICAgICAgIGJvcnJvdyA9IDA7XHJcbiAgICAgICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgd2hpbGUgKGJvcnJvdyAhPT0gMCkge1xyXG4gICAgICAgICAgICAgICAgcXVvdGllbnREaWdpdCAtPSAxO1xyXG4gICAgICAgICAgICAgICAgY2FycnkgPSAwO1xyXG4gICAgICAgICAgICAgICAgZm9yIChpID0gMDsgaSA8IGw7IGkrKykge1xyXG4gICAgICAgICAgICAgICAgICAgIGNhcnJ5ICs9IHJlbWFpbmRlcltzaGlmdCArIGldIC0gYmFzZSArIGRpdmlzb3JbaV07XHJcbiAgICAgICAgICAgICAgICAgICAgaWYgKGNhcnJ5IDwgMCkge1xyXG4gICAgICAgICAgICAgICAgICAgICAgICByZW1haW5kZXJbc2hpZnQgKyBpXSA9IGNhcnJ5ICsgYmFzZTtcclxuICAgICAgICAgICAgICAgICAgICAgICAgY2FycnkgPSAwO1xyXG4gICAgICAgICAgICAgICAgICAgIH0gZWxzZSB7XHJcbiAgICAgICAgICAgICAgICAgICAgICAgIHJlbWFpbmRlcltzaGlmdCArIGldID0gY2Fycnk7XHJcbiAgICAgICAgICAgICAgICAgICAgICAgIGNhcnJ5ID0gMTtcclxuICAgICAgICAgICAgICAgICAgICB9XHJcbiAgICAgICAgICAgICAgICB9XHJcbiAgICAgICAgICAgICAgICBib3Jyb3cgKz0gY2Fycnk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgcmVzdWx0W3NoaWZ0XSA9IHF1b3RpZW50RGlnaXQ7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIC8vIGRlbm9ybWFsaXphdGlvblxyXG4gICAgICAgIHJlbWFpbmRlciA9IGRpdk1vZFNtYWxsKHJlbWFpbmRlciwgbGFtYmRhKVswXTtcclxuICAgICAgICByZXR1cm4gW2FycmF5VG9TbWFsbChyZXN1bHQpLCBhcnJheVRvU21hbGwocmVtYWluZGVyKV07XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gZGl2TW9kMihhLCBiKSB7IC8vIEltcGxlbWVudGF0aW9uIGlkZWEgc2hhbWVsZXNzbHkgc3RvbGVuIGZyb20gU2lsZW50IE1hdHQncyBsaWJyYXJ5IGh0dHA6Ly9zaWxlbnRtYXR0LmNvbS9iaWdpbnRlZ2VyL1xyXG4gICAgICAgIC8vIFBlcmZvcm1zIGZhc3RlciB0aGFuIGRpdk1vZDEgb24gbGFyZ2VyIGlucHV0IHNpemVzLlxyXG4gICAgICAgIHZhciBhX2wgPSBhLmxlbmd0aCxcclxuICAgICAgICAgICAgYl9sID0gYi5sZW5ndGgsXHJcbiAgICAgICAgICAgIHJlc3VsdCA9IFtdLFxyXG4gICAgICAgICAgICBwYXJ0ID0gW10sXHJcbiAgICAgICAgICAgIGJhc2UgPSBCQVNFLFxyXG4gICAgICAgICAgICBndWVzcywgeGxlbiwgaGlnaHgsIGhpZ2h5LCBjaGVjaztcclxuICAgICAgICB3aGlsZSAoYV9sKSB7XHJcbiAgICAgICAgICAgIHBhcnQudW5zaGlmdChhWy0tYV9sXSk7XHJcbiAgICAgICAgICAgIGlmIChjb21wYXJlQWJzKHBhcnQsIGIpIDwgMCkge1xyXG4gICAgICAgICAgICAgICAgcmVzdWx0LnB1c2goMCk7XHJcbiAgICAgICAgICAgICAgICBjb250aW51ZTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICB4bGVuID0gcGFydC5sZW5ndGg7XHJcbiAgICAgICAgICAgIGhpZ2h4ID0gcGFydFt4bGVuIC0gMV0gKiBiYXNlICsgcGFydFt4bGVuIC0gMl07XHJcbiAgICAgICAgICAgIGhpZ2h5ID0gYltiX2wgLSAxXSAqIGJhc2UgKyBiW2JfbCAtIDJdO1xyXG4gICAgICAgICAgICBpZiAoeGxlbiA+IGJfbCkge1xyXG4gICAgICAgICAgICAgICAgaGlnaHggPSAoaGlnaHggKyAxKSAqIGJhc2U7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgZ3Vlc3MgPSBNYXRoLmNlaWwoaGlnaHggLyBoaWdoeSk7XHJcbiAgICAgICAgICAgIGRvIHtcclxuICAgICAgICAgICAgICAgIGNoZWNrID0gbXVsdGlwbHlTbWFsbChiLCBndWVzcyk7XHJcbiAgICAgICAgICAgICAgICBpZiAoY29tcGFyZUFicyhjaGVjaywgcGFydCkgPD0gMCkgYnJlYWs7XHJcbiAgICAgICAgICAgICAgICBndWVzcy0tO1xyXG4gICAgICAgICAgICB9IHdoaWxlIChndWVzcyk7XHJcbiAgICAgICAgICAgIHJlc3VsdC5wdXNoKGd1ZXNzKTtcclxuICAgICAgICAgICAgcGFydCA9IHN1YnRyYWN0KHBhcnQsIGNoZWNrKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmVzdWx0LnJldmVyc2UoKTtcclxuICAgICAgICByZXR1cm4gW2FycmF5VG9TbWFsbChyZXN1bHQpLCBhcnJheVRvU21hbGwocGFydCldO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIGRpdk1vZFNtYWxsKHZhbHVlLCBsYW1iZGEpIHtcclxuICAgICAgICB2YXIgbGVuZ3RoID0gdmFsdWUubGVuZ3RoLFxyXG4gICAgICAgICAgICBxdW90aWVudCA9IGNyZWF0ZUFycmF5KGxlbmd0aCksXHJcbiAgICAgICAgICAgIGJhc2UgPSBCQVNFLFxyXG4gICAgICAgICAgICBpLCBxLCByZW1haW5kZXIsIGRpdmlzb3I7XHJcbiAgICAgICAgcmVtYWluZGVyID0gMDtcclxuICAgICAgICBmb3IgKGkgPSBsZW5ndGggLSAxOyBpID49IDA7IC0taSkge1xyXG4gICAgICAgICAgICBkaXZpc29yID0gcmVtYWluZGVyICogYmFzZSArIHZhbHVlW2ldO1xyXG4gICAgICAgICAgICBxID0gdHJ1bmNhdGUoZGl2aXNvciAvIGxhbWJkYSk7XHJcbiAgICAgICAgICAgIHJlbWFpbmRlciA9IGRpdmlzb3IgLSBxICogbGFtYmRhO1xyXG4gICAgICAgICAgICBxdW90aWVudFtpXSA9IHEgfCAwO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gW3F1b3RpZW50LCByZW1haW5kZXIgfCAwXTtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBkaXZNb2RBbnkoc2VsZiwgdikge1xyXG4gICAgICAgIHZhciB2YWx1ZSwgbiA9IHBhcnNlVmFsdWUodik7XHJcbiAgICAgICAgdmFyIGEgPSBzZWxmLnZhbHVlLCBiID0gbi52YWx1ZTtcclxuICAgICAgICB2YXIgcXVvdGllbnQ7XHJcbiAgICAgICAgaWYgKGIgPT09IDApIHRocm93IG5ldyBFcnJvcihcIkNhbm5vdCBkaXZpZGUgYnkgemVyb1wiKTtcclxuICAgICAgICBpZiAoc2VsZi5pc1NtYWxsKSB7XHJcbiAgICAgICAgICAgIGlmIChuLmlzU21hbGwpIHtcclxuICAgICAgICAgICAgICAgIHJldHVybiBbbmV3IFNtYWxsSW50ZWdlcih0cnVuY2F0ZShhIC8gYikpLCBuZXcgU21hbGxJbnRlZ2VyKGEgJSBiKV07XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgcmV0dXJuIFtJbnRlZ2VyWzBdLCBzZWxmXTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKG4uaXNTbWFsbCkge1xyXG4gICAgICAgICAgICBpZiAoYiA9PT0gMSkgcmV0dXJuIFtzZWxmLCBJbnRlZ2VyWzBdXTtcclxuICAgICAgICAgICAgaWYgKGIgPT0gLTEpIHJldHVybiBbc2VsZi5uZWdhdGUoKSwgSW50ZWdlclswXV07XHJcbiAgICAgICAgICAgIHZhciBhYnMgPSBNYXRoLmFicyhiKTtcclxuICAgICAgICAgICAgaWYgKGFicyA8IEJBU0UpIHtcclxuICAgICAgICAgICAgICAgIHZhbHVlID0gZGl2TW9kU21hbGwoYSwgYWJzKTtcclxuICAgICAgICAgICAgICAgIHF1b3RpZW50ID0gYXJyYXlUb1NtYWxsKHZhbHVlWzBdKTtcclxuICAgICAgICAgICAgICAgIHZhciByZW1haW5kZXIgPSB2YWx1ZVsxXTtcclxuICAgICAgICAgICAgICAgIGlmIChzZWxmLnNpZ24pIHJlbWFpbmRlciA9IC1yZW1haW5kZXI7XHJcbiAgICAgICAgICAgICAgICBpZiAodHlwZW9mIHF1b3RpZW50ID09PSBcIm51bWJlclwiKSB7XHJcbiAgICAgICAgICAgICAgICAgICAgaWYgKHNlbGYuc2lnbiAhPT0gbi5zaWduKSBxdW90aWVudCA9IC1xdW90aWVudDtcclxuICAgICAgICAgICAgICAgICAgICByZXR1cm4gW25ldyBTbWFsbEludGVnZXIocXVvdGllbnQpLCBuZXcgU21hbGxJbnRlZ2VyKHJlbWFpbmRlcildO1xyXG4gICAgICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICAgICAgcmV0dXJuIFtuZXcgQmlnSW50ZWdlcihxdW90aWVudCwgc2VsZi5zaWduICE9PSBuLnNpZ24pLCBuZXcgU21hbGxJbnRlZ2VyKHJlbWFpbmRlcildO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIGIgPSBzbWFsbFRvQXJyYXkoYWJzKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgdmFyIGNvbXBhcmlzb24gPSBjb21wYXJlQWJzKGEsIGIpO1xyXG4gICAgICAgIGlmIChjb21wYXJpc29uID09PSAtMSkgcmV0dXJuIFtJbnRlZ2VyWzBdLCBzZWxmXTtcclxuICAgICAgICBpZiAoY29tcGFyaXNvbiA9PT0gMCkgcmV0dXJuIFtJbnRlZ2VyW3NlbGYuc2lnbiA9PT0gbi5zaWduID8gMSA6IC0xXSwgSW50ZWdlclswXV07XHJcblxyXG4gICAgICAgIC8vIGRpdk1vZDEgaXMgZmFzdGVyIG9uIHNtYWxsZXIgaW5wdXQgc2l6ZXNcclxuICAgICAgICBpZiAoYS5sZW5ndGggKyBiLmxlbmd0aCA8PSAyMDApXHJcbiAgICAgICAgICAgIHZhbHVlID0gZGl2TW9kMShhLCBiKTtcclxuICAgICAgICBlbHNlIHZhbHVlID0gZGl2TW9kMihhLCBiKTtcclxuXHJcbiAgICAgICAgcXVvdGllbnQgPSB2YWx1ZVswXTtcclxuICAgICAgICB2YXIgcVNpZ24gPSBzZWxmLnNpZ24gIT09IG4uc2lnbixcclxuICAgICAgICAgICAgbW9kID0gdmFsdWVbMV0sXHJcbiAgICAgICAgICAgIG1TaWduID0gc2VsZi5zaWduO1xyXG4gICAgICAgIGlmICh0eXBlb2YgcXVvdGllbnQgPT09IFwibnVtYmVyXCIpIHtcclxuICAgICAgICAgICAgaWYgKHFTaWduKSBxdW90aWVudCA9IC1xdW90aWVudDtcclxuICAgICAgICAgICAgcXVvdGllbnQgPSBuZXcgU21hbGxJbnRlZ2VyKHF1b3RpZW50KTtcclxuICAgICAgICB9IGVsc2UgcXVvdGllbnQgPSBuZXcgQmlnSW50ZWdlcihxdW90aWVudCwgcVNpZ24pO1xyXG4gICAgICAgIGlmICh0eXBlb2YgbW9kID09PSBcIm51bWJlclwiKSB7XHJcbiAgICAgICAgICAgIGlmIChtU2lnbikgbW9kID0gLW1vZDtcclxuICAgICAgICAgICAgbW9kID0gbmV3IFNtYWxsSW50ZWdlcihtb2QpO1xyXG4gICAgICAgIH0gZWxzZSBtb2QgPSBuZXcgQmlnSW50ZWdlcihtb2QsIG1TaWduKTtcclxuICAgICAgICByZXR1cm4gW3F1b3RpZW50LCBtb2RdO1xyXG4gICAgfVxyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmRpdm1vZCA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgdmFyIHJlc3VsdCA9IGRpdk1vZEFueSh0aGlzLCB2KTtcclxuICAgICAgICByZXR1cm4ge1xyXG4gICAgICAgICAgICBxdW90aWVudDogcmVzdWx0WzBdLFxyXG4gICAgICAgICAgICByZW1haW5kZXI6IHJlc3VsdFsxXVxyXG4gICAgICAgIH07XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5kaXZtb2QgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5kaXZtb2Q7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuZGl2aWRlID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICByZXR1cm4gZGl2TW9kQW55KHRoaXMsIHYpWzBdO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUub3ZlciA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUuZGl2aWRlID0gQmlnSW50ZWdlci5wcm90b3R5cGUub3ZlciA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmRpdmlkZTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5tb2QgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHJldHVybiBkaXZNb2RBbnkodGhpcywgdilbMV07XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5yZW1haW5kZXIgPSBTbWFsbEludGVnZXIucHJvdG90eXBlLm1vZCA9IEJpZ0ludGVnZXIucHJvdG90eXBlLnJlbWFpbmRlciA9IEJpZ0ludGVnZXIucHJvdG90eXBlLm1vZDtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5wb3cgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHZhciBuID0gcGFyc2VWYWx1ZSh2KSxcclxuICAgICAgICAgICAgYSA9IHRoaXMudmFsdWUsXHJcbiAgICAgICAgICAgIGIgPSBuLnZhbHVlLFxyXG4gICAgICAgICAgICB2YWx1ZSwgeCwgeTtcclxuICAgICAgICBpZiAoYiA9PT0gMCkgcmV0dXJuIEludGVnZXJbMV07XHJcbiAgICAgICAgaWYgKGEgPT09IDApIHJldHVybiBJbnRlZ2VyWzBdO1xyXG4gICAgICAgIGlmIChhID09PSAxKSByZXR1cm4gSW50ZWdlclsxXTtcclxuICAgICAgICBpZiAoYSA9PT0gLTEpIHJldHVybiBuLmlzRXZlbigpID8gSW50ZWdlclsxXSA6IEludGVnZXJbLTFdO1xyXG4gICAgICAgIGlmIChuLnNpZ24pIHtcclxuICAgICAgICAgICAgcmV0dXJuIEludGVnZXJbMF07XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGlmICghbi5pc1NtYWxsKSB0aHJvdyBuZXcgRXJyb3IoXCJUaGUgZXhwb25lbnQgXCIgKyBuLnRvU3RyaW5nKCkgKyBcIiBpcyB0b28gbGFyZ2UuXCIpO1xyXG4gICAgICAgIGlmICh0aGlzLmlzU21hbGwpIHtcclxuICAgICAgICAgICAgaWYgKGlzUHJlY2lzZSh2YWx1ZSA9IE1hdGgucG93KGEsIGIpKSlcclxuICAgICAgICAgICAgICAgIHJldHVybiBuZXcgU21hbGxJbnRlZ2VyKHRydW5jYXRlKHZhbHVlKSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHggPSB0aGlzO1xyXG4gICAgICAgIHkgPSBJbnRlZ2VyWzFdO1xyXG4gICAgICAgIHdoaWxlICh0cnVlKSB7XHJcbiAgICAgICAgICAgIGlmIChiICYgMSA9PT0gMSkge1xyXG4gICAgICAgICAgICAgICAgeSA9IHkudGltZXMoeCk7XHJcbiAgICAgICAgICAgICAgICAtLWI7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgaWYgKGIgPT09IDApIGJyZWFrO1xyXG4gICAgICAgICAgICBiIC89IDI7XHJcbiAgICAgICAgICAgIHggPSB4LnNxdWFyZSgpO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4geTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnBvdyA9IEJpZ0ludGVnZXIucHJvdG90eXBlLnBvdztcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5tb2RQb3cgPSBmdW5jdGlvbiAoZXhwLCBtb2QpIHtcclxuICAgICAgICBleHAgPSBwYXJzZVZhbHVlKGV4cCk7XHJcbiAgICAgICAgbW9kID0gcGFyc2VWYWx1ZShtb2QpO1xyXG4gICAgICAgIGlmIChtb2QuaXNaZXJvKCkpIHRocm93IG5ldyBFcnJvcihcIkNhbm5vdCB0YWtlIG1vZFBvdyB3aXRoIG1vZHVsdXMgMFwiKTtcclxuICAgICAgICB2YXIgciA9IEludGVnZXJbMV0sXHJcbiAgICAgICAgICAgIGJhc2UgPSB0aGlzLm1vZChtb2QpO1xyXG4gICAgICAgIHdoaWxlIChleHAuaXNQb3NpdGl2ZSgpKSB7XHJcbiAgICAgICAgICAgIGlmIChiYXNlLmlzWmVybygpKSByZXR1cm4gSW50ZWdlclswXTtcclxuICAgICAgICAgICAgaWYgKGV4cC5pc09kZCgpKSByID0gci5tdWx0aXBseShiYXNlKS5tb2QobW9kKTtcclxuICAgICAgICAgICAgZXhwID0gZXhwLmRpdmlkZSgyKTtcclxuICAgICAgICAgICAgYmFzZSA9IGJhc2Uuc3F1YXJlKCkubW9kKG1vZCk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiByO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUubW9kUG93ID0gQmlnSW50ZWdlci5wcm90b3R5cGUubW9kUG93O1xyXG5cclxuICAgIGZ1bmN0aW9uIGNvbXBhcmVBYnMoYSwgYikge1xyXG4gICAgICAgIGlmIChhLmxlbmd0aCAhPT0gYi5sZW5ndGgpIHtcclxuICAgICAgICAgICAgcmV0dXJuIGEubGVuZ3RoID4gYi5sZW5ndGggPyAxIDogLTE7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGZvciAodmFyIGkgPSBhLmxlbmd0aCAtIDE7IGkgPj0gMDsgaS0tKSB7XHJcbiAgICAgICAgICAgIGlmIChhW2ldICE9PSBiW2ldKSByZXR1cm4gYVtpXSA+IGJbaV0gPyAxIDogLTE7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiAwO1xyXG4gICAgfVxyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmNvbXBhcmVBYnMgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHZhciBuID0gcGFyc2VWYWx1ZSh2KSxcclxuICAgICAgICAgICAgYSA9IHRoaXMudmFsdWUsXHJcbiAgICAgICAgICAgIGIgPSBuLnZhbHVlO1xyXG4gICAgICAgIGlmIChuLmlzU21hbGwpIHJldHVybiAxO1xyXG4gICAgICAgIHJldHVybiBjb21wYXJlQWJzKGEsIGIpO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuY29tcGFyZUFicyA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgdmFyIG4gPSBwYXJzZVZhbHVlKHYpLFxyXG4gICAgICAgICAgICBhID0gTWF0aC5hYnModGhpcy52YWx1ZSksXHJcbiAgICAgICAgICAgIGIgPSBuLnZhbHVlO1xyXG4gICAgICAgIGlmIChuLmlzU21hbGwpIHtcclxuICAgICAgICAgICAgYiA9IE1hdGguYWJzKGIpO1xyXG4gICAgICAgICAgICByZXR1cm4gYSA9PT0gYiA/IDAgOiBhID4gYiA/IDEgOiAtMTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIC0xO1xyXG4gICAgfTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5jb21wYXJlID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICAvLyBTZWUgZGlzY3Vzc2lvbiBhYm91dCBjb21wYXJpc29uIHdpdGggSW5maW5pdHk6XHJcbiAgICAgICAgLy8gaHR0cHM6Ly9naXRodWIuY29tL3BldGVyb2xzb24vQmlnSW50ZWdlci5qcy9pc3N1ZXMvNjFcclxuICAgICAgICBpZiAodiA9PT0gSW5maW5pdHkpIHtcclxuICAgICAgICAgICAgcmV0dXJuIC0xO1xyXG4gICAgICAgIH1cclxuICAgICAgICBpZiAodiA9PT0gLUluZmluaXR5KSB7XHJcbiAgICAgICAgICAgIHJldHVybiAxO1xyXG4gICAgICAgIH1cclxuXHJcbiAgICAgICAgdmFyIG4gPSBwYXJzZVZhbHVlKHYpLFxyXG4gICAgICAgICAgICBhID0gdGhpcy52YWx1ZSxcclxuICAgICAgICAgICAgYiA9IG4udmFsdWU7XHJcbiAgICAgICAgaWYgKHRoaXMuc2lnbiAhPT0gbi5zaWduKSB7XHJcbiAgICAgICAgICAgIHJldHVybiBuLnNpZ24gPyAxIDogLTE7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGlmIChuLmlzU21hbGwpIHtcclxuICAgICAgICAgICAgcmV0dXJuIHRoaXMuc2lnbiA/IC0xIDogMTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIGNvbXBhcmVBYnMoYSwgYikgKiAodGhpcy5zaWduID8gLTEgOiAxKTtcclxuICAgIH07XHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5jb21wYXJlVG8gPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5jb21wYXJlO1xyXG5cclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuY29tcGFyZSA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgaWYgKHYgPT09IEluZmluaXR5KSB7XHJcbiAgICAgICAgICAgIHJldHVybiAtMTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKHYgPT09IC1JbmZpbml0eSkge1xyXG4gICAgICAgICAgICByZXR1cm4gMTtcclxuICAgICAgICB9XHJcblxyXG4gICAgICAgIHZhciBuID0gcGFyc2VWYWx1ZSh2KSxcclxuICAgICAgICAgICAgYSA9IHRoaXMudmFsdWUsXHJcbiAgICAgICAgICAgIGIgPSBuLnZhbHVlO1xyXG4gICAgICAgIGlmIChuLmlzU21hbGwpIHtcclxuICAgICAgICAgICAgcmV0dXJuIGEgPT0gYiA/IDAgOiBhID4gYiA/IDEgOiAtMTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKGEgPCAwICE9PSBuLnNpZ24pIHtcclxuICAgICAgICAgICAgcmV0dXJuIGEgPCAwID8gLTEgOiAxO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gYSA8IDAgPyAxIDogLTE7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5jb21wYXJlVG8gPSBTbWFsbEludGVnZXIucHJvdG90eXBlLmNvbXBhcmU7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuZXF1YWxzID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy5jb21wYXJlKHYpID09PSAwO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuZXEgPSBTbWFsbEludGVnZXIucHJvdG90eXBlLmVxdWFscyA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmVxID0gQmlnSW50ZWdlci5wcm90b3R5cGUuZXF1YWxzO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLm5vdEVxdWFscyA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgcmV0dXJuIHRoaXMuY29tcGFyZSh2KSAhPT0gMDtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLm5lcSA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUubm90RXF1YWxzID0gQmlnSW50ZWdlci5wcm90b3R5cGUubmVxID0gQmlnSW50ZWdlci5wcm90b3R5cGUubm90RXF1YWxzO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmdyZWF0ZXIgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHJldHVybiB0aGlzLmNvbXBhcmUodikgPiAwO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuZ3QgPSBTbWFsbEludGVnZXIucHJvdG90eXBlLmdyZWF0ZXIgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5ndCA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmdyZWF0ZXI7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUubGVzc2VyID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy5jb21wYXJlKHYpIDwgMDtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmx0ID0gU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5sZXNzZXIgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5sdCA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmxlc3NlcjtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5ncmVhdGVyT3JFcXVhbHMgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHJldHVybiB0aGlzLmNvbXBhcmUodikgPj0gMDtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmdlcSA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUuZ3JlYXRlck9yRXF1YWxzID0gQmlnSW50ZWdlci5wcm90b3R5cGUuZ2VxID0gQmlnSW50ZWdlci5wcm90b3R5cGUuZ3JlYXRlck9yRXF1YWxzO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmxlc3Nlck9yRXF1YWxzID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy5jb21wYXJlKHYpIDw9IDA7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5sZXEgPSBTbWFsbEludGVnZXIucHJvdG90eXBlLmxlc3Nlck9yRXF1YWxzID0gQmlnSW50ZWdlci5wcm90b3R5cGUubGVxID0gQmlnSW50ZWdlci5wcm90b3R5cGUubGVzc2VyT3JFcXVhbHM7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuaXNFdmVuID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiAodGhpcy52YWx1ZVswXSAmIDEpID09PSAwO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuaXNFdmVuID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiAodGhpcy52YWx1ZSAmIDEpID09PSAwO1xyXG4gICAgfTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc09kZCA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gKHRoaXMudmFsdWVbMF0gJiAxKSA9PT0gMTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmlzT2RkID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiAodGhpcy52YWx1ZSAmIDEpID09PSAxO1xyXG4gICAgfTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc1Bvc2l0aXZlID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiAhdGhpcy5zaWduO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuaXNQb3NpdGl2ZSA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy52YWx1ZSA+IDA7XHJcbiAgICB9O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmlzTmVnYXRpdmUgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIHRoaXMuc2lnbjtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmlzTmVnYXRpdmUgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIHRoaXMudmFsdWUgPCAwO1xyXG4gICAgfTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc1VuaXQgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIGZhbHNlO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuaXNVbml0ID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiBNYXRoLmFicyh0aGlzLnZhbHVlKSA9PT0gMTtcclxuICAgIH07XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuaXNaZXJvID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiBmYWxzZTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmlzWmVybyA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy52YWx1ZSA9PT0gMDtcclxuICAgIH07XHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc0RpdmlzaWJsZUJ5ID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICB2YXIgbiA9IHBhcnNlVmFsdWUodik7XHJcbiAgICAgICAgdmFyIHZhbHVlID0gbi52YWx1ZTtcclxuICAgICAgICBpZiAodmFsdWUgPT09IDApIHJldHVybiBmYWxzZTtcclxuICAgICAgICBpZiAodmFsdWUgPT09IDEpIHJldHVybiB0cnVlO1xyXG4gICAgICAgIGlmICh2YWx1ZSA9PT0gMikgcmV0dXJuIHRoaXMuaXNFdmVuKCk7XHJcbiAgICAgICAgcmV0dXJuIHRoaXMubW9kKG4pLmVxdWFscyhJbnRlZ2VyWzBdKTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmlzRGl2aXNpYmxlQnkgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc0RpdmlzaWJsZUJ5O1xyXG5cclxuICAgIGZ1bmN0aW9uIGlzQmFzaWNQcmltZSh2KSB7XHJcbiAgICAgICAgdmFyIG4gPSB2LmFicygpO1xyXG4gICAgICAgIGlmIChuLmlzVW5pdCgpKSByZXR1cm4gZmFsc2U7XHJcbiAgICAgICAgaWYgKG4uZXF1YWxzKDIpIHx8IG4uZXF1YWxzKDMpIHx8IG4uZXF1YWxzKDUpKSByZXR1cm4gdHJ1ZTtcclxuICAgICAgICBpZiAobi5pc0V2ZW4oKSB8fCBuLmlzRGl2aXNpYmxlQnkoMykgfHwgbi5pc0RpdmlzaWJsZUJ5KDUpKSByZXR1cm4gZmFsc2U7XHJcbiAgICAgICAgaWYgKG4ubGVzc2VyKDI1KSkgcmV0dXJuIHRydWU7XHJcbiAgICAgICAgLy8gd2UgZG9uJ3Qga25vdyBpZiBpdCdzIHByaW1lOiBsZXQgdGhlIG90aGVyIGZ1bmN0aW9ucyBmaWd1cmUgaXQgb3V0XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuaXNQcmltZSA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICB2YXIgaXNQcmltZSA9IGlzQmFzaWNQcmltZSh0aGlzKTtcclxuICAgICAgICBpZiAoaXNQcmltZSAhPT0gdW5kZWZpbmVkKSByZXR1cm4gaXNQcmltZTtcclxuICAgICAgICB2YXIgbiA9IHRoaXMuYWJzKCksXHJcbiAgICAgICAgICAgIG5QcmV2ID0gbi5wcmV2KCk7XHJcbiAgICAgICAgdmFyIGEgPSBbMiwgMywgNSwgNywgMTEsIDEzLCAxNywgMTldLFxyXG4gICAgICAgICAgICBiID0gblByZXYsXHJcbiAgICAgICAgICAgIGQsIHQsIGksIHg7XHJcbiAgICAgICAgd2hpbGUgKGIuaXNFdmVuKCkpIGIgPSBiLmRpdmlkZSgyKTtcclxuICAgICAgICBmb3IgKGkgPSAwOyBpIDwgYS5sZW5ndGg7IGkrKykge1xyXG4gICAgICAgICAgICB4ID0gYmlnSW50KGFbaV0pLm1vZFBvdyhiLCBuKTtcclxuICAgICAgICAgICAgaWYgKHguZXF1YWxzKEludGVnZXJbMV0pIHx8IHguZXF1YWxzKG5QcmV2KSkgY29udGludWU7XHJcbiAgICAgICAgICAgIGZvciAodCA9IHRydWUsIGQgPSBiOyB0ICYmIGQubGVzc2VyKG5QcmV2KSA7IGQgPSBkLm11bHRpcGx5KDIpKSB7XHJcbiAgICAgICAgICAgICAgICB4ID0geC5zcXVhcmUoKS5tb2Qobik7XHJcbiAgICAgICAgICAgICAgICBpZiAoeC5lcXVhbHMoblByZXYpKSB0ID0gZmFsc2U7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgaWYgKHQpIHJldHVybiBmYWxzZTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHRydWU7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5pc1ByaW1lID0gQmlnSW50ZWdlci5wcm90b3R5cGUuaXNQcmltZTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc1Byb2JhYmxlUHJpbWUgPSBmdW5jdGlvbiAoaXRlcmF0aW9ucykge1xyXG4gICAgICAgIHZhciBpc1ByaW1lID0gaXNCYXNpY1ByaW1lKHRoaXMpO1xyXG4gICAgICAgIGlmIChpc1ByaW1lICE9PSB1bmRlZmluZWQpIHJldHVybiBpc1ByaW1lO1xyXG4gICAgICAgIHZhciBuID0gdGhpcy5hYnMoKTtcclxuICAgICAgICB2YXIgdCA9IGl0ZXJhdGlvbnMgPT09IHVuZGVmaW5lZCA/IDUgOiBpdGVyYXRpb25zO1xyXG4gICAgICAgIC8vIHVzZSB0aGUgRmVybWF0IHByaW1hbGl0eSB0ZXN0XHJcbiAgICAgICAgZm9yICh2YXIgaSA9IDA7IGkgPCB0OyBpKyspIHtcclxuICAgICAgICAgICAgdmFyIGEgPSBiaWdJbnQucmFuZEJldHdlZW4oMiwgbi5taW51cygyKSk7XHJcbiAgICAgICAgICAgIGlmICghYS5tb2RQb3cobi5wcmV2KCksIG4pLmlzVW5pdCgpKSByZXR1cm4gZmFsc2U7IC8vIGRlZmluaXRlbHkgY29tcG9zaXRlXHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiB0cnVlOyAvLyBsYXJnZSBjaGFuY2Ugb2YgYmVpbmcgcHJpbWVcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmlzUHJvYmFibGVQcmltZSA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmlzUHJvYmFibGVQcmltZTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5tb2RJbnYgPSBmdW5jdGlvbiAobikge1xyXG4gICAgICAgIHZhciB0ID0gYmlnSW50Lnplcm8sIG5ld1QgPSBiaWdJbnQub25lLCByID0gcGFyc2VWYWx1ZShuKSwgbmV3UiA9IHRoaXMuYWJzKCksIHEsIGxhc3RULCBsYXN0UjtcclxuICAgICAgICB3aGlsZSAoIW5ld1IuZXF1YWxzKGJpZ0ludC56ZXJvKSkge1xyXG4gICAgICAgIFx0cSA9IHIuZGl2aWRlKG5ld1IpO1xyXG4gICAgICAgICAgbGFzdFQgPSB0O1xyXG4gICAgICAgICAgbGFzdFIgPSByO1xyXG4gICAgICAgICAgdCA9IG5ld1Q7XHJcbiAgICAgICAgICByID0gbmV3UjtcclxuICAgICAgICAgIG5ld1QgPSBsYXN0VC5zdWJ0cmFjdChxLm11bHRpcGx5KG5ld1QpKTtcclxuICAgICAgICAgIG5ld1IgPSBsYXN0Ui5zdWJ0cmFjdChxLm11bHRpcGx5KG5ld1IpKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKCFyLmVxdWFscygxKSkgdGhyb3cgbmV3IEVycm9yKHRoaXMudG9TdHJpbmcoKSArIFwiIGFuZCBcIiArIG4udG9TdHJpbmcoKSArIFwiIGFyZSBub3QgY28tcHJpbWVcIik7XHJcbiAgICAgICAgaWYgKHQuY29tcGFyZSgwKSA9PT0gLTEpIHtcclxuICAgICAgICBcdHQgPSB0LmFkZChuKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHQ7XHJcbiAgICB9XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLm1vZEludiA9IEJpZ0ludGVnZXIucHJvdG90eXBlLm1vZEludjtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5uZXh0ID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHZhciB2YWx1ZSA9IHRoaXMudmFsdWU7XHJcbiAgICAgICAgaWYgKHRoaXMuc2lnbikge1xyXG4gICAgICAgICAgICByZXR1cm4gc3VidHJhY3RTbWFsbCh2YWx1ZSwgMSwgdGhpcy5zaWduKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKGFkZFNtYWxsKHZhbHVlLCAxKSwgdGhpcy5zaWduKTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLm5leHQgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgdmFyIHZhbHVlID0gdGhpcy52YWx1ZTtcclxuICAgICAgICBpZiAodmFsdWUgKyAxIDwgTUFYX0lOVCkgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIodmFsdWUgKyAxKTtcclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIoTUFYX0lOVF9BUlIsIGZhbHNlKTtcclxuICAgIH07XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUucHJldiA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICB2YXIgdmFsdWUgPSB0aGlzLnZhbHVlO1xyXG4gICAgICAgIGlmICh0aGlzLnNpZ24pIHtcclxuICAgICAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKGFkZFNtYWxsKHZhbHVlLCAxKSwgdHJ1ZSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBzdWJ0cmFjdFNtYWxsKHZhbHVlLCAxLCB0aGlzLnNpZ24pO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUucHJldiA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICB2YXIgdmFsdWUgPSB0aGlzLnZhbHVlO1xyXG4gICAgICAgIGlmICh2YWx1ZSAtIDEgPiAtTUFYX0lOVCkgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIodmFsdWUgLSAxKTtcclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIoTUFYX0lOVF9BUlIsIHRydWUpO1xyXG4gICAgfTtcclxuXHJcbiAgICB2YXIgcG93ZXJzT2ZUd28gPSBbMV07XHJcbiAgICB3aGlsZSAocG93ZXJzT2ZUd29bcG93ZXJzT2ZUd28ubGVuZ3RoIC0gMV0gPD0gQkFTRSkgcG93ZXJzT2ZUd28ucHVzaCgyICogcG93ZXJzT2ZUd29bcG93ZXJzT2ZUd28ubGVuZ3RoIC0gMV0pO1xyXG4gICAgdmFyIHBvd2VyczJMZW5ndGggPSBwb3dlcnNPZlR3by5sZW5ndGgsIGhpZ2hlc3RQb3dlcjIgPSBwb3dlcnNPZlR3b1twb3dlcnMyTGVuZ3RoIC0gMV07XHJcblxyXG4gICAgZnVuY3Rpb24gc2hpZnRfaXNTbWFsbChuKSB7XHJcbiAgICAgICAgcmV0dXJuICgodHlwZW9mIG4gPT09IFwibnVtYmVyXCIgfHwgdHlwZW9mIG4gPT09IFwic3RyaW5nXCIpICYmICtNYXRoLmFicyhuKSA8PSBCQVNFKSB8fFxyXG4gICAgICAgICAgICAobiBpbnN0YW5jZW9mIEJpZ0ludGVnZXIgJiYgbi52YWx1ZS5sZW5ndGggPD0gMSk7XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuc2hpZnRMZWZ0ID0gZnVuY3Rpb24gKG4pIHtcclxuICAgICAgICBpZiAoIXNoaWZ0X2lzU21hbGwobikpIHtcclxuICAgICAgICAgICAgdGhyb3cgbmV3IEVycm9yKFN0cmluZyhuKSArIFwiIGlzIHRvbyBsYXJnZSBmb3Igc2hpZnRpbmcuXCIpO1xyXG4gICAgICAgIH1cclxuICAgICAgICBuID0gK247XHJcbiAgICAgICAgaWYgKG4gPCAwKSByZXR1cm4gdGhpcy5zaGlmdFJpZ2h0KC1uKTtcclxuICAgICAgICB2YXIgcmVzdWx0ID0gdGhpcztcclxuICAgICAgICB3aGlsZSAobiA+PSBwb3dlcnMyTGVuZ3RoKSB7XHJcbiAgICAgICAgICAgIHJlc3VsdCA9IHJlc3VsdC5tdWx0aXBseShoaWdoZXN0UG93ZXIyKTtcclxuICAgICAgICAgICAgbiAtPSBwb3dlcnMyTGVuZ3RoIC0gMTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHJlc3VsdC5tdWx0aXBseShwb3dlcnNPZlR3b1tuXSk7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5zaGlmdExlZnQgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5zaGlmdExlZnQ7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuc2hpZnRSaWdodCA9IGZ1bmN0aW9uIChuKSB7XHJcbiAgICAgICAgdmFyIHJlbVF1bztcclxuICAgICAgICBpZiAoIXNoaWZ0X2lzU21hbGwobikpIHtcclxuICAgICAgICAgICAgdGhyb3cgbmV3IEVycm9yKFN0cmluZyhuKSArIFwiIGlzIHRvbyBsYXJnZSBmb3Igc2hpZnRpbmcuXCIpO1xyXG4gICAgICAgIH1cclxuICAgICAgICBuID0gK247XHJcbiAgICAgICAgaWYgKG4gPCAwKSByZXR1cm4gdGhpcy5zaGlmdExlZnQoLW4pO1xyXG4gICAgICAgIHZhciByZXN1bHQgPSB0aGlzO1xyXG4gICAgICAgIHdoaWxlIChuID49IHBvd2VyczJMZW5ndGgpIHtcclxuICAgICAgICAgICAgaWYgKHJlc3VsdC5pc1plcm8oKSkgcmV0dXJuIHJlc3VsdDtcclxuICAgICAgICAgICAgcmVtUXVvID0gZGl2TW9kQW55KHJlc3VsdCwgaGlnaGVzdFBvd2VyMik7XHJcbiAgICAgICAgICAgIHJlc3VsdCA9IHJlbVF1b1sxXS5pc05lZ2F0aXZlKCkgPyByZW1RdW9bMF0ucHJldigpIDogcmVtUXVvWzBdO1xyXG4gICAgICAgICAgICBuIC09IHBvd2VyczJMZW5ndGggLSAxO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZW1RdW8gPSBkaXZNb2RBbnkocmVzdWx0LCBwb3dlcnNPZlR3b1tuXSk7XHJcbiAgICAgICAgcmV0dXJuIHJlbVF1b1sxXS5pc05lZ2F0aXZlKCkgPyByZW1RdW9bMF0ucHJldigpIDogcmVtUXVvWzBdO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuc2hpZnRSaWdodCA9IEJpZ0ludGVnZXIucHJvdG90eXBlLnNoaWZ0UmlnaHQ7XHJcblxyXG4gICAgZnVuY3Rpb24gYml0d2lzZSh4LCB5LCBmbikge1xyXG4gICAgICAgIHkgPSBwYXJzZVZhbHVlKHkpO1xyXG4gICAgICAgIHZhciB4U2lnbiA9IHguaXNOZWdhdGl2ZSgpLCB5U2lnbiA9IHkuaXNOZWdhdGl2ZSgpO1xyXG4gICAgICAgIHZhciB4UmVtID0geFNpZ24gPyB4Lm5vdCgpIDogeCxcclxuICAgICAgICAgICAgeVJlbSA9IHlTaWduID8geS5ub3QoKSA6IHk7XHJcbiAgICAgICAgdmFyIHhCaXRzID0gW10sIHlCaXRzID0gW107XHJcbiAgICAgICAgdmFyIHhTdG9wID0gZmFsc2UsIHlTdG9wID0gZmFsc2U7XHJcbiAgICAgICAgd2hpbGUgKCF4U3RvcCB8fCAheVN0b3ApIHtcclxuICAgICAgICAgICAgaWYgKHhSZW0uaXNaZXJvKCkpIHsgLy8gdmlydHVhbCBzaWduIGV4dGVuc2lvbiBmb3Igc2ltdWxhdGluZyB0d28ncyBjb21wbGVtZW50XHJcbiAgICAgICAgICAgICAgICB4U3RvcCA9IHRydWU7XHJcbiAgICAgICAgICAgICAgICB4Qml0cy5wdXNoKHhTaWduID8gMSA6IDApO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIGVsc2UgaWYgKHhTaWduKSB4Qml0cy5wdXNoKHhSZW0uaXNFdmVuKCkgPyAxIDogMCk7IC8vIHR3bydzIGNvbXBsZW1lbnQgZm9yIG5lZ2F0aXZlIG51bWJlcnNcclxuICAgICAgICAgICAgZWxzZSB4Qml0cy5wdXNoKHhSZW0uaXNFdmVuKCkgPyAwIDogMSk7XHJcblxyXG4gICAgICAgICAgICBpZiAoeVJlbS5pc1plcm8oKSkge1xyXG4gICAgICAgICAgICAgICAgeVN0b3AgPSB0cnVlO1xyXG4gICAgICAgICAgICAgICAgeUJpdHMucHVzaCh5U2lnbiA/IDEgOiAwKTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICBlbHNlIGlmICh5U2lnbikgeUJpdHMucHVzaCh5UmVtLmlzRXZlbigpID8gMSA6IDApO1xyXG4gICAgICAgICAgICBlbHNlIHlCaXRzLnB1c2goeVJlbS5pc0V2ZW4oKSA/IDAgOiAxKTtcclxuXHJcbiAgICAgICAgICAgIHhSZW0gPSB4UmVtLm92ZXIoMik7XHJcbiAgICAgICAgICAgIHlSZW0gPSB5UmVtLm92ZXIoMik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHZhciByZXN1bHQgPSBbXTtcclxuICAgICAgICBmb3IgKHZhciBpID0gMDsgaSA8IHhCaXRzLmxlbmd0aDsgaSsrKSByZXN1bHQucHVzaChmbih4Qml0c1tpXSwgeUJpdHNbaV0pKTtcclxuICAgICAgICB2YXIgc3VtID0gYmlnSW50KHJlc3VsdC5wb3AoKSkubmVnYXRlKCkudGltZXMoYmlnSW50KDIpLnBvdyhyZXN1bHQubGVuZ3RoKSk7XHJcbiAgICAgICAgd2hpbGUgKHJlc3VsdC5sZW5ndGgpIHtcclxuICAgICAgICAgICAgc3VtID0gc3VtLmFkZChiaWdJbnQocmVzdWx0LnBvcCgpKS50aW1lcyhiaWdJbnQoMikucG93KHJlc3VsdC5sZW5ndGgpKSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBzdW07XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUubm90ID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiB0aGlzLm5lZ2F0ZSgpLnByZXYoKTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLm5vdCA9IEJpZ0ludGVnZXIucHJvdG90eXBlLm5vdDtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5hbmQgPSBmdW5jdGlvbiAobikge1xyXG4gICAgICAgIHJldHVybiBiaXR3aXNlKHRoaXMsIG4sIGZ1bmN0aW9uIChhLCBiKSB7IHJldHVybiBhICYgYjsgfSk7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5hbmQgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5hbmQ7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUub3IgPSBmdW5jdGlvbiAobikge1xyXG4gICAgICAgIHJldHVybiBiaXR3aXNlKHRoaXMsIG4sIGZ1bmN0aW9uIChhLCBiKSB7IHJldHVybiBhIHwgYjsgfSk7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5vciA9IEJpZ0ludGVnZXIucHJvdG90eXBlLm9yO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLnhvciA9IGZ1bmN0aW9uIChuKSB7XHJcbiAgICAgICAgcmV0dXJuIGJpdHdpc2UodGhpcywgbiwgZnVuY3Rpb24gKGEsIGIpIHsgcmV0dXJuIGEgXiBiOyB9KTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnhvciA9IEJpZ0ludGVnZXIucHJvdG90eXBlLnhvcjtcclxuXHJcbiAgICB2YXIgTE9CTUFTS19JID0gMSA8PCAzMCwgTE9CTUFTS19CSSA9IChCQVNFICYgLUJBU0UpICogKEJBU0UgJiAtQkFTRSkgfCBMT0JNQVNLX0k7XHJcbiAgICBmdW5jdGlvbiByb3VnaExPQihuKSB7IC8vIGdldCBsb3dlc3RPbmVCaXQgKHJvdWdoKVxyXG4gICAgICAgIC8vIFNtYWxsSW50ZWdlcjogcmV0dXJuIE1pbihsb3dlc3RPbmVCaXQobiksIDEgPDwgMzApXHJcbiAgICAgICAgLy8gQmlnSW50ZWdlcjogcmV0dXJuIE1pbihsb3dlc3RPbmVCaXQobiksIDEgPDwgMTQpIFtCQVNFPTFlN11cclxuICAgICAgICB2YXIgdiA9IG4udmFsdWUsIHggPSB0eXBlb2YgdiA9PT0gXCJudW1iZXJcIiA/IHYgfCBMT0JNQVNLX0kgOiB2WzBdICsgdlsxXSAqIEJBU0UgfCBMT0JNQVNLX0JJO1xyXG4gICAgICAgIHJldHVybiB4ICYgLXg7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gbWF4KGEsIGIpIHtcclxuICAgICAgICBhID0gcGFyc2VWYWx1ZShhKTtcclxuICAgICAgICBiID0gcGFyc2VWYWx1ZShiKTtcclxuICAgICAgICByZXR1cm4gYS5ncmVhdGVyKGIpID8gYSA6IGI7XHJcbiAgICB9XHJcbiAgICBmdW5jdGlvbiBtaW4oYSxiKSB7XHJcbiAgICAgICAgYSA9IHBhcnNlVmFsdWUoYSk7XHJcbiAgICAgICAgYiA9IHBhcnNlVmFsdWUoYik7XHJcbiAgICAgICAgcmV0dXJuIGEubGVzc2VyKGIpID8gYSA6IGI7XHJcbiAgICB9XHJcbiAgICBmdW5jdGlvbiBnY2QoYSwgYikge1xyXG4gICAgICAgIGEgPSBwYXJzZVZhbHVlKGEpLmFicygpO1xyXG4gICAgICAgIGIgPSBwYXJzZVZhbHVlKGIpLmFicygpO1xyXG4gICAgICAgIGlmIChhLmVxdWFscyhiKSkgcmV0dXJuIGE7XHJcbiAgICAgICAgaWYgKGEuaXNaZXJvKCkpIHJldHVybiBiO1xyXG4gICAgICAgIGlmIChiLmlzWmVybygpKSByZXR1cm4gYTtcclxuICAgICAgICB2YXIgYyA9IEludGVnZXJbMV0sIGQsIHQ7XHJcbiAgICAgICAgd2hpbGUgKGEuaXNFdmVuKCkgJiYgYi5pc0V2ZW4oKSkge1xyXG4gICAgICAgICAgICBkID0gTWF0aC5taW4ocm91Z2hMT0IoYSksIHJvdWdoTE9CKGIpKTtcclxuICAgICAgICAgICAgYSA9IGEuZGl2aWRlKGQpO1xyXG4gICAgICAgICAgICBiID0gYi5kaXZpZGUoZCk7XHJcbiAgICAgICAgICAgIGMgPSBjLm11bHRpcGx5KGQpO1xyXG4gICAgICAgIH1cclxuICAgICAgICB3aGlsZSAoYS5pc0V2ZW4oKSkge1xyXG4gICAgICAgICAgICBhID0gYS5kaXZpZGUocm91Z2hMT0IoYSkpO1xyXG4gICAgICAgIH1cclxuICAgICAgICBkbyB7XHJcbiAgICAgICAgICAgIHdoaWxlIChiLmlzRXZlbigpKSB7XHJcbiAgICAgICAgICAgICAgICBiID0gYi5kaXZpZGUocm91Z2hMT0IoYikpO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIGlmIChhLmdyZWF0ZXIoYikpIHtcclxuICAgICAgICAgICAgICAgIHQgPSBiOyBiID0gYTsgYSA9IHQ7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgYiA9IGIuc3VidHJhY3QoYSk7XHJcbiAgICAgICAgfSB3aGlsZSAoIWIuaXNaZXJvKCkpO1xyXG4gICAgICAgIHJldHVybiBjLmlzVW5pdCgpID8gYSA6IGEubXVsdGlwbHkoYyk7XHJcbiAgICB9XHJcbiAgICBmdW5jdGlvbiBsY20oYSwgYikge1xyXG4gICAgICAgIGEgPSBwYXJzZVZhbHVlKGEpLmFicygpO1xyXG4gICAgICAgIGIgPSBwYXJzZVZhbHVlKGIpLmFicygpO1xyXG4gICAgICAgIHJldHVybiBhLmRpdmlkZShnY2QoYSwgYikpLm11bHRpcGx5KGIpO1xyXG4gICAgfVxyXG4gICAgZnVuY3Rpb24gcmFuZEJldHdlZW4oYSwgYikge1xyXG4gICAgICAgIGEgPSBwYXJzZVZhbHVlKGEpO1xyXG4gICAgICAgIGIgPSBwYXJzZVZhbHVlKGIpO1xyXG4gICAgICAgIHZhciBsb3cgPSBtaW4oYSwgYiksIGhpZ2ggPSBtYXgoYSwgYik7XHJcbiAgICAgICAgdmFyIHJhbmdlID0gaGlnaC5zdWJ0cmFjdChsb3cpO1xyXG4gICAgICAgIGlmIChyYW5nZS5pc1NtYWxsKSByZXR1cm4gbG93LmFkZChNYXRoLnJvdW5kKE1hdGgucmFuZG9tKCkgKiByYW5nZSkpO1xyXG4gICAgICAgIHZhciBsZW5ndGggPSByYW5nZS52YWx1ZS5sZW5ndGggLSAxO1xyXG4gICAgICAgIHZhciByZXN1bHQgPSBbXSwgcmVzdHJpY3RlZCA9IHRydWU7XHJcbiAgICAgICAgZm9yICh2YXIgaSA9IGxlbmd0aDsgaSA+PSAwOyBpLS0pIHtcclxuICAgICAgICAgICAgdmFyIHRvcCA9IHJlc3RyaWN0ZWQgPyByYW5nZS52YWx1ZVtpXSA6IEJBU0U7XHJcbiAgICAgICAgICAgIHZhciBkaWdpdCA9IHRydW5jYXRlKE1hdGgucmFuZG9tKCkgKiB0b3ApO1xyXG4gICAgICAgICAgICByZXN1bHQudW5zaGlmdChkaWdpdCk7XHJcbiAgICAgICAgICAgIGlmIChkaWdpdCA8IHRvcCkgcmVzdHJpY3RlZCA9IGZhbHNlO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXN1bHQgPSBhcnJheVRvU21hbGwocmVzdWx0KTtcclxuICAgICAgICByZXR1cm4gbG93LmFkZCh0eXBlb2YgcmVzdWx0ID09PSBcIm51bWJlclwiID8gbmV3IFNtYWxsSW50ZWdlcihyZXN1bHQpIDogbmV3IEJpZ0ludGVnZXIocmVzdWx0LCBmYWxzZSkpO1xyXG4gICAgfVxyXG4gICAgdmFyIHBhcnNlQmFzZSA9IGZ1bmN0aW9uICh0ZXh0LCBiYXNlKSB7XHJcbiAgICAgICAgdmFyIHZhbCA9IEludGVnZXJbMF0sIHBvdyA9IEludGVnZXJbMV0sXHJcbiAgICAgICAgICAgIGxlbmd0aCA9IHRleHQubGVuZ3RoO1xyXG4gICAgICAgIGlmICgyIDw9IGJhc2UgJiYgYmFzZSA8PSAzNikge1xyXG4gICAgICAgICAgICBpZiAobGVuZ3RoIDw9IExPR19NQVhfSU5UIC8gTWF0aC5sb2coYmFzZSkpIHtcclxuICAgICAgICAgICAgICAgIHJldHVybiBuZXcgU21hbGxJbnRlZ2VyKHBhcnNlSW50KHRleHQsIGJhc2UpKTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgIH1cclxuICAgICAgICBiYXNlID0gcGFyc2VWYWx1ZShiYXNlKTtcclxuICAgICAgICB2YXIgZGlnaXRzID0gW107XHJcbiAgICAgICAgdmFyIGk7XHJcbiAgICAgICAgdmFyIGlzTmVnYXRpdmUgPSB0ZXh0WzBdID09PSBcIi1cIjtcclxuICAgICAgICBmb3IgKGkgPSBpc05lZ2F0aXZlID8gMSA6IDA7IGkgPCB0ZXh0Lmxlbmd0aDsgaSsrKSB7XHJcbiAgICAgICAgICAgIHZhciBjID0gdGV4dFtpXS50b0xvd2VyQ2FzZSgpLFxyXG4gICAgICAgICAgICAgICAgY2hhckNvZGUgPSBjLmNoYXJDb2RlQXQoMCk7XHJcbiAgICAgICAgICAgIGlmICg0OCA8PSBjaGFyQ29kZSAmJiBjaGFyQ29kZSA8PSA1NykgZGlnaXRzLnB1c2gocGFyc2VWYWx1ZShjKSk7XHJcbiAgICAgICAgICAgIGVsc2UgaWYgKDk3IDw9IGNoYXJDb2RlICYmIGNoYXJDb2RlIDw9IDEyMikgZGlnaXRzLnB1c2gocGFyc2VWYWx1ZShjLmNoYXJDb2RlQXQoMCkgLSA4NykpO1xyXG4gICAgICAgICAgICBlbHNlIGlmIChjID09PSBcIjxcIikge1xyXG4gICAgICAgICAgICAgICAgdmFyIHN0YXJ0ID0gaTtcclxuICAgICAgICAgICAgICAgIGRvIHsgaSsrOyB9IHdoaWxlICh0ZXh0W2ldICE9PSBcIj5cIik7XHJcbiAgICAgICAgICAgICAgICBkaWdpdHMucHVzaChwYXJzZVZhbHVlKHRleHQuc2xpY2Uoc3RhcnQgKyAxLCBpKSkpO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIGVsc2UgdGhyb3cgbmV3IEVycm9yKGMgKyBcIiBpcyBub3QgYSB2YWxpZCBjaGFyYWN0ZXJcIik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGRpZ2l0cy5yZXZlcnNlKCk7XHJcbiAgICAgICAgZm9yIChpID0gMDsgaSA8IGRpZ2l0cy5sZW5ndGg7IGkrKykge1xyXG4gICAgICAgICAgICB2YWwgPSB2YWwuYWRkKGRpZ2l0c1tpXS50aW1lcyhwb3cpKTtcclxuICAgICAgICAgICAgcG93ID0gcG93LnRpbWVzKGJhc2UpO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gaXNOZWdhdGl2ZSA/IHZhbC5uZWdhdGUoKSA6IHZhbDtcclxuICAgIH07XHJcblxyXG4gICAgZnVuY3Rpb24gc3RyaW5naWZ5KGRpZ2l0KSB7XHJcbiAgICAgICAgdmFyIHYgPSBkaWdpdC52YWx1ZTtcclxuICAgICAgICBpZiAodHlwZW9mIHYgPT09IFwibnVtYmVyXCIpIHYgPSBbdl07XHJcbiAgICAgICAgaWYgKHYubGVuZ3RoID09PSAxICYmIHZbMF0gPD0gMzUpIHtcclxuICAgICAgICAgICAgcmV0dXJuIFwiMDEyMzQ1Njc4OWFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6XCIuY2hhckF0KHZbMF0pO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gXCI8XCIgKyB2ICsgXCI+XCI7XHJcbiAgICB9XHJcbiAgICBmdW5jdGlvbiB0b0Jhc2UobiwgYmFzZSkge1xyXG4gICAgICAgIGJhc2UgPSBiaWdJbnQoYmFzZSk7XHJcbiAgICAgICAgaWYgKGJhc2UuaXNaZXJvKCkpIHtcclxuICAgICAgICAgICAgaWYgKG4uaXNaZXJvKCkpIHJldHVybiBcIjBcIjtcclxuICAgICAgICAgICAgdGhyb3cgbmV3IEVycm9yKFwiQ2Fubm90IGNvbnZlcnQgbm9uemVybyBudW1iZXJzIHRvIGJhc2UgMC5cIik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGlmIChiYXNlLmVxdWFscygtMSkpIHtcclxuICAgICAgICAgICAgaWYgKG4uaXNaZXJvKCkpIHJldHVybiBcIjBcIjtcclxuICAgICAgICAgICAgaWYgKG4uaXNOZWdhdGl2ZSgpKSByZXR1cm4gbmV3IEFycmF5KDEgLSBuKS5qb2luKFwiMTBcIik7XHJcbiAgICAgICAgICAgIHJldHVybiBcIjFcIiArIG5ldyBBcnJheSgrbikuam9pbihcIjAxXCIpO1xyXG4gICAgICAgIH1cclxuICAgICAgICB2YXIgbWludXNTaWduID0gXCJcIjtcclxuICAgICAgICBpZiAobi5pc05lZ2F0aXZlKCkgJiYgYmFzZS5pc1Bvc2l0aXZlKCkpIHtcclxuICAgICAgICAgICAgbWludXNTaWduID0gXCItXCI7XHJcbiAgICAgICAgICAgIG4gPSBuLmFicygpO1xyXG4gICAgICAgIH1cclxuICAgICAgICBpZiAoYmFzZS5lcXVhbHMoMSkpIHtcclxuICAgICAgICAgICAgaWYgKG4uaXNaZXJvKCkpIHJldHVybiBcIjBcIjtcclxuICAgICAgICAgICAgcmV0dXJuIG1pbnVzU2lnbiArIG5ldyBBcnJheSgrbiArIDEpLmpvaW4oMSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHZhciBvdXQgPSBbXTtcclxuICAgICAgICB2YXIgbGVmdCA9IG4sIGRpdm1vZDtcclxuICAgICAgICB3aGlsZSAobGVmdC5pc05lZ2F0aXZlKCkgfHwgbGVmdC5jb21wYXJlQWJzKGJhc2UpID49IDApIHtcclxuICAgICAgICAgICAgZGl2bW9kID0gbGVmdC5kaXZtb2QoYmFzZSk7XHJcbiAgICAgICAgICAgIGxlZnQgPSBkaXZtb2QucXVvdGllbnQ7XHJcbiAgICAgICAgICAgIHZhciBkaWdpdCA9IGRpdm1vZC5yZW1haW5kZXI7XHJcbiAgICAgICAgICAgIGlmIChkaWdpdC5pc05lZ2F0aXZlKCkpIHtcclxuICAgICAgICAgICAgICAgIGRpZ2l0ID0gYmFzZS5taW51cyhkaWdpdCkuYWJzKCk7XHJcbiAgICAgICAgICAgICAgICBsZWZ0ID0gbGVmdC5uZXh0KCk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgb3V0LnB1c2goc3RyaW5naWZ5KGRpZ2l0KSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIG91dC5wdXNoKHN0cmluZ2lmeShsZWZ0KSk7XHJcbiAgICAgICAgcmV0dXJuIG1pbnVzU2lnbiArIG91dC5yZXZlcnNlKCkuam9pbihcIlwiKTtcclxuICAgIH1cclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS50b1N0cmluZyA9IGZ1bmN0aW9uIChyYWRpeCkge1xyXG4gICAgICAgIGlmIChyYWRpeCA9PT0gdW5kZWZpbmVkKSByYWRpeCA9IDEwO1xyXG4gICAgICAgIGlmIChyYWRpeCAhPT0gMTApIHJldHVybiB0b0Jhc2UodGhpcywgcmFkaXgpO1xyXG4gICAgICAgIHZhciB2ID0gdGhpcy52YWx1ZSwgbCA9IHYubGVuZ3RoLCBzdHIgPSBTdHJpbmcodlstLWxdKSwgemVyb3MgPSBcIjAwMDAwMDBcIiwgZGlnaXQ7XHJcbiAgICAgICAgd2hpbGUgKC0tbCA+PSAwKSB7XHJcbiAgICAgICAgICAgIGRpZ2l0ID0gU3RyaW5nKHZbbF0pO1xyXG4gICAgICAgICAgICBzdHIgKz0gemVyb3Muc2xpY2UoZGlnaXQubGVuZ3RoKSArIGRpZ2l0O1xyXG4gICAgICAgIH1cclxuICAgICAgICB2YXIgc2lnbiA9IHRoaXMuc2lnbiA/IFwiLVwiIDogXCJcIjtcclxuICAgICAgICByZXR1cm4gc2lnbiArIHN0cjtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnRvU3RyaW5nID0gZnVuY3Rpb24gKHJhZGl4KSB7XHJcbiAgICAgICAgaWYgKHJhZGl4ID09PSB1bmRlZmluZWQpIHJhZGl4ID0gMTA7XHJcbiAgICAgICAgaWYgKHJhZGl4ICE9IDEwKSByZXR1cm4gdG9CYXNlKHRoaXMsIHJhZGl4KTtcclxuICAgICAgICByZXR1cm4gU3RyaW5nKHRoaXMudmFsdWUpO1xyXG4gICAgfTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS52YWx1ZU9mID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiArdGhpcy50b1N0cmluZygpO1xyXG4gICAgfTtcclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLnRvSlNOdW1iZXIgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS52YWx1ZU9mO1xyXG5cclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUudmFsdWVPZiA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy52YWx1ZTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnRvSlNOdW1iZXIgPSBTbWFsbEludGVnZXIucHJvdG90eXBlLnZhbHVlT2Y7XHJcblxyXG4gICAgZnVuY3Rpb24gcGFyc2VTdHJpbmdWYWx1ZSh2KSB7XHJcbiAgICAgICAgICAgIGlmIChpc1ByZWNpc2UoK3YpKSB7XHJcbiAgICAgICAgICAgICAgICB2YXIgeCA9ICt2O1xyXG4gICAgICAgICAgICAgICAgaWYgKHggPT09IHRydW5jYXRlKHgpKVxyXG4gICAgICAgICAgICAgICAgICAgIHJldHVybiBuZXcgU21hbGxJbnRlZ2VyKHgpO1xyXG4gICAgICAgICAgICAgICAgdGhyb3cgXCJJbnZhbGlkIGludGVnZXI6IFwiICsgdjtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICB2YXIgc2lnbiA9IHZbMF0gPT09IFwiLVwiO1xyXG4gICAgICAgICAgICBpZiAoc2lnbikgdiA9IHYuc2xpY2UoMSk7XHJcbiAgICAgICAgICAgIHZhciBzcGxpdCA9IHYuc3BsaXQoL2UvaSk7XHJcbiAgICAgICAgICAgIGlmIChzcGxpdC5sZW5ndGggPiAyKSB0aHJvdyBuZXcgRXJyb3IoXCJJbnZhbGlkIGludGVnZXI6IFwiICsgc3BsaXQuam9pbihcImVcIikpO1xyXG4gICAgICAgICAgICBpZiAoc3BsaXQubGVuZ3RoID09PSAyKSB7XHJcbiAgICAgICAgICAgICAgICB2YXIgZXhwID0gc3BsaXRbMV07XHJcbiAgICAgICAgICAgICAgICBpZiAoZXhwWzBdID09PSBcIitcIikgZXhwID0gZXhwLnNsaWNlKDEpO1xyXG4gICAgICAgICAgICAgICAgZXhwID0gK2V4cDtcclxuICAgICAgICAgICAgICAgIGlmIChleHAgIT09IHRydW5jYXRlKGV4cCkgfHwgIWlzUHJlY2lzZShleHApKSB0aHJvdyBuZXcgRXJyb3IoXCJJbnZhbGlkIGludGVnZXI6IFwiICsgZXhwICsgXCIgaXMgbm90IGEgdmFsaWQgZXhwb25lbnQuXCIpO1xyXG4gICAgICAgICAgICAgICAgdmFyIHRleHQgPSBzcGxpdFswXTtcclxuICAgICAgICAgICAgICAgIHZhciBkZWNpbWFsUGxhY2UgPSB0ZXh0LmluZGV4T2YoXCIuXCIpO1xyXG4gICAgICAgICAgICAgICAgaWYgKGRlY2ltYWxQbGFjZSA+PSAwKSB7XHJcbiAgICAgICAgICAgICAgICAgICAgZXhwIC09IHRleHQubGVuZ3RoIC0gZGVjaW1hbFBsYWNlIC0gMTtcclxuICAgICAgICAgICAgICAgICAgICB0ZXh0ID0gdGV4dC5zbGljZSgwLCBkZWNpbWFsUGxhY2UpICsgdGV4dC5zbGljZShkZWNpbWFsUGxhY2UgKyAxKTtcclxuICAgICAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgICAgIGlmIChleHAgPCAwKSB0aHJvdyBuZXcgRXJyb3IoXCJDYW5ub3QgaW5jbHVkZSBuZWdhdGl2ZSBleHBvbmVudCBwYXJ0IGZvciBpbnRlZ2Vyc1wiKTtcclxuICAgICAgICAgICAgICAgIHRleHQgKz0gKG5ldyBBcnJheShleHAgKyAxKSkuam9pbihcIjBcIik7XHJcbiAgICAgICAgICAgICAgICB2ID0gdGV4dDtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICB2YXIgaXNWYWxpZCA9IC9eKFswLTldWzAtOV0qKSQvLnRlc3Qodik7XHJcbiAgICAgICAgICAgIGlmICghaXNWYWxpZCkgdGhyb3cgbmV3IEVycm9yKFwiSW52YWxpZCBpbnRlZ2VyOiBcIiArIHYpO1xyXG4gICAgICAgICAgICB2YXIgciA9IFtdLCBtYXggPSB2Lmxlbmd0aCwgbCA9IExPR19CQVNFLCBtaW4gPSBtYXggLSBsO1xyXG4gICAgICAgICAgICB3aGlsZSAobWF4ID4gMCkge1xyXG4gICAgICAgICAgICAgICAgci5wdXNoKCt2LnNsaWNlKG1pbiwgbWF4KSk7XHJcbiAgICAgICAgICAgICAgICBtaW4gLT0gbDtcclxuICAgICAgICAgICAgICAgIGlmIChtaW4gPCAwKSBtaW4gPSAwO1xyXG4gICAgICAgICAgICAgICAgbWF4IC09IGw7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgdHJpbShyKTtcclxuICAgICAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKHIsIHNpZ24pO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIHBhcnNlTnVtYmVyVmFsdWUodikge1xyXG4gICAgICAgIGlmIChpc1ByZWNpc2UodikpIHtcclxuICAgICAgICAgICAgaWYgKHYgIT09IHRydW5jYXRlKHYpKSB0aHJvdyBuZXcgRXJyb3IodiArIFwiIGlzIG5vdCBhbiBpbnRlZ2VyLlwiKTtcclxuICAgICAgICAgICAgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIodik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBwYXJzZVN0cmluZ1ZhbHVlKHYudG9TdHJpbmcoKSk7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gcGFyc2VWYWx1ZSh2KSB7XHJcbiAgICAgICAgaWYgKHR5cGVvZiB2ID09PSBcIm51bWJlclwiKSB7XHJcbiAgICAgICAgICAgIHJldHVybiBwYXJzZU51bWJlclZhbHVlKHYpO1xyXG4gICAgICAgIH1cclxuICAgICAgICBpZiAodHlwZW9mIHYgPT09IFwic3RyaW5nXCIpIHtcclxuICAgICAgICAgICAgcmV0dXJuIHBhcnNlU3RyaW5nVmFsdWUodik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiB2O1xyXG4gICAgfVxyXG4gICAgLy8gUHJlLWRlZmluZSBudW1iZXJzIGluIHJhbmdlIFstOTk5LDk5OV1cclxuICAgIGZvciAodmFyIGkgPSAwOyBpIDwgMTAwMDsgaSsrKSB7XHJcbiAgICAgICAgSW50ZWdlcltpXSA9IG5ldyBTbWFsbEludGVnZXIoaSk7XHJcbiAgICAgICAgaWYgKGkgPiAwKSBJbnRlZ2VyWy1pXSA9IG5ldyBTbWFsbEludGVnZXIoLWkpO1xyXG4gICAgfVxyXG4gICAgLy8gQmFja3dhcmRzIGNvbXBhdGliaWxpdHlcclxuICAgIEludGVnZXIub25lID0gSW50ZWdlclsxXTtcclxuICAgIEludGVnZXIuemVybyA9IEludGVnZXJbMF07XHJcbiAgICBJbnRlZ2VyLm1pbnVzT25lID0gSW50ZWdlclstMV07XHJcbiAgICBJbnRlZ2VyLm1heCA9IG1heDtcclxuICAgIEludGVnZXIubWluID0gbWluO1xyXG4gICAgSW50ZWdlci5nY2QgPSBnY2Q7XHJcbiAgICBJbnRlZ2VyLmxjbSA9IGxjbTtcclxuICAgIEludGVnZXIuaXNJbnN0YW5jZSA9IGZ1bmN0aW9uICh4KSB7IHJldHVybiB4IGluc3RhbmNlb2YgQmlnSW50ZWdlciB8fCB4IGluc3RhbmNlb2YgU21hbGxJbnRlZ2VyOyB9O1xyXG4gICAgSW50ZWdlci5yYW5kQmV0d2VlbiA9IHJhbmRCZXR3ZWVuO1xyXG4gICAgcmV0dXJuIEludGVnZXI7XHJcbn0pKCk7XHJcblxyXG4vLyBOb2RlLmpzIGNoZWNrXHJcbmlmICh0eXBlb2YgbW9kdWxlICE9PSBcInVuZGVmaW5lZFwiICYmIG1vZHVsZS5oYXNPd25Qcm9wZXJ0eShcImV4cG9ydHNcIikpIHtcclxuICAgIG1vZHVsZS5leHBvcnRzID0gYmlnSW50O1xyXG59XHJcbiIsIiIsIi8qIVxuICogVGhlIGJ1ZmZlciBtb2R1bGUgZnJvbSBub2RlLmpzLCBmb3IgdGhlIGJyb3dzZXIuXG4gKlxuICogQGF1dGhvciAgIEZlcm9zcyBBYm91a2hhZGlqZWggPGZlcm9zc0BmZXJvc3Mub3JnPiA8aHR0cDovL2Zlcm9zcy5vcmc+XG4gKiBAbGljZW5zZSAgTUlUXG4gKi9cbi8qIGVzbGludC1kaXNhYmxlIG5vLXByb3RvICovXG5cbid1c2Ugc3RyaWN0J1xuXG52YXIgYmFzZTY0ID0gcmVxdWlyZSgnYmFzZTY0LWpzJylcbnZhciBpZWVlNzU0ID0gcmVxdWlyZSgnaWVlZTc1NCcpXG52YXIgaXNBcnJheSA9IHJlcXVpcmUoJ2lzYXJyYXknKVxuXG5leHBvcnRzLkJ1ZmZlciA9IEJ1ZmZlclxuZXhwb3J0cy5TbG93QnVmZmVyID0gU2xvd0J1ZmZlclxuZXhwb3J0cy5JTlNQRUNUX01BWF9CWVRFUyA9IDUwXG5cbi8qKlxuICogSWYgYEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUYDpcbiAqICAgPT09IHRydWUgICAgVXNlIFVpbnQ4QXJyYXkgaW1wbGVtZW50YXRpb24gKGZhc3Rlc3QpXG4gKiAgID09PSBmYWxzZSAgIFVzZSBPYmplY3QgaW1wbGVtZW50YXRpb24gKG1vc3QgY29tcGF0aWJsZSwgZXZlbiBJRTYpXG4gKlxuICogQnJvd3NlcnMgdGhhdCBzdXBwb3J0IHR5cGVkIGFycmF5cyBhcmUgSUUgMTArLCBGaXJlZm94IDQrLCBDaHJvbWUgNyssIFNhZmFyaSA1LjErLFxuICogT3BlcmEgMTEuNissIGlPUyA0LjIrLlxuICpcbiAqIER1ZSB0byB2YXJpb3VzIGJyb3dzZXIgYnVncywgc29tZXRpbWVzIHRoZSBPYmplY3QgaW1wbGVtZW50YXRpb24gd2lsbCBiZSB1c2VkIGV2ZW5cbiAqIHdoZW4gdGhlIGJyb3dzZXIgc3VwcG9ydHMgdHlwZWQgYXJyYXlzLlxuICpcbiAqIE5vdGU6XG4gKlxuICogICAtIEZpcmVmb3ggNC0yOSBsYWNrcyBzdXBwb3J0IGZvciBhZGRpbmcgbmV3IHByb3BlcnRpZXMgdG8gYFVpbnQ4QXJyYXlgIGluc3RhbmNlcyxcbiAqICAgICBTZWU6IGh0dHBzOi8vYnVnemlsbGEubW96aWxsYS5vcmcvc2hvd19idWcuY2dpP2lkPTY5NTQzOC5cbiAqXG4gKiAgIC0gQ2hyb21lIDktMTAgaXMgbWlzc2luZyB0aGUgYFR5cGVkQXJyYXkucHJvdG90eXBlLnN1YmFycmF5YCBmdW5jdGlvbi5cbiAqXG4gKiAgIC0gSUUxMCBoYXMgYSBicm9rZW4gYFR5cGVkQXJyYXkucHJvdG90eXBlLnN1YmFycmF5YCBmdW5jdGlvbiB3aGljaCByZXR1cm5zIGFycmF5cyBvZlxuICogICAgIGluY29ycmVjdCBsZW5ndGggaW4gc29tZSBzaXR1YXRpb25zLlxuXG4gKiBXZSBkZXRlY3QgdGhlc2UgYnVnZ3kgYnJvd3NlcnMgYW5kIHNldCBgQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlRgIHRvIGBmYWxzZWAgc28gdGhleVxuICogZ2V0IHRoZSBPYmplY3QgaW1wbGVtZW50YXRpb24sIHdoaWNoIGlzIHNsb3dlciBidXQgYmVoYXZlcyBjb3JyZWN0bHkuXG4gKi9cbkJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUID0gZ2xvYmFsLlRZUEVEX0FSUkFZX1NVUFBPUlQgIT09IHVuZGVmaW5lZFxuICA/IGdsb2JhbC5UWVBFRF9BUlJBWV9TVVBQT1JUXG4gIDogdHlwZWRBcnJheVN1cHBvcnQoKVxuXG4vKlxuICogRXhwb3J0IGtNYXhMZW5ndGggYWZ0ZXIgdHlwZWQgYXJyYXkgc3VwcG9ydCBpcyBkZXRlcm1pbmVkLlxuICovXG5leHBvcnRzLmtNYXhMZW5ndGggPSBrTWF4TGVuZ3RoKClcblxuZnVuY3Rpb24gdHlwZWRBcnJheVN1cHBvcnQgKCkge1xuICB0cnkge1xuICAgIHZhciBhcnIgPSBuZXcgVWludDhBcnJheSgxKVxuICAgIGFyci5fX3Byb3RvX18gPSB7X19wcm90b19fOiBVaW50OEFycmF5LnByb3RvdHlwZSwgZm9vOiBmdW5jdGlvbiAoKSB7IHJldHVybiA0MiB9fVxuICAgIHJldHVybiBhcnIuZm9vKCkgPT09IDQyICYmIC8vIHR5cGVkIGFycmF5IGluc3RhbmNlcyBjYW4gYmUgYXVnbWVudGVkXG4gICAgICAgIHR5cGVvZiBhcnIuc3ViYXJyYXkgPT09ICdmdW5jdGlvbicgJiYgLy8gY2hyb21lIDktMTAgbGFjayBgc3ViYXJyYXlgXG4gICAgICAgIGFyci5zdWJhcnJheSgxLCAxKS5ieXRlTGVuZ3RoID09PSAwIC8vIGllMTAgaGFzIGJyb2tlbiBgc3ViYXJyYXlgXG4gIH0gY2F0Y2ggKGUpIHtcbiAgICByZXR1cm4gZmFsc2VcbiAgfVxufVxuXG5mdW5jdGlvbiBrTWF4TGVuZ3RoICgpIHtcbiAgcmV0dXJuIEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUXG4gICAgPyAweDdmZmZmZmZmXG4gICAgOiAweDNmZmZmZmZmXG59XG5cbmZ1bmN0aW9uIGNyZWF0ZUJ1ZmZlciAodGhhdCwgbGVuZ3RoKSB7XG4gIGlmIChrTWF4TGVuZ3RoKCkgPCBsZW5ndGgpIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignSW52YWxpZCB0eXBlZCBhcnJheSBsZW5ndGgnKVxuICB9XG4gIGlmIChCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkge1xuICAgIC8vIFJldHVybiBhbiBhdWdtZW50ZWQgYFVpbnQ4QXJyYXlgIGluc3RhbmNlLCBmb3IgYmVzdCBwZXJmb3JtYW5jZVxuICAgIHRoYXQgPSBuZXcgVWludDhBcnJheShsZW5ndGgpXG4gICAgdGhhdC5fX3Byb3RvX18gPSBCdWZmZXIucHJvdG90eXBlXG4gIH0gZWxzZSB7XG4gICAgLy8gRmFsbGJhY2s6IFJldHVybiBhbiBvYmplY3QgaW5zdGFuY2Ugb2YgdGhlIEJ1ZmZlciBjbGFzc1xuICAgIGlmICh0aGF0ID09PSBudWxsKSB7XG4gICAgICB0aGF0ID0gbmV3IEJ1ZmZlcihsZW5ndGgpXG4gICAgfVxuICAgIHRoYXQubGVuZ3RoID0gbGVuZ3RoXG4gIH1cblxuICByZXR1cm4gdGhhdFxufVxuXG4vKipcbiAqIFRoZSBCdWZmZXIgY29uc3RydWN0b3IgcmV0dXJucyBpbnN0YW5jZXMgb2YgYFVpbnQ4QXJyYXlgIHRoYXQgaGF2ZSB0aGVpclxuICogcHJvdG90eXBlIGNoYW5nZWQgdG8gYEJ1ZmZlci5wcm90b3R5cGVgLiBGdXJ0aGVybW9yZSwgYEJ1ZmZlcmAgaXMgYSBzdWJjbGFzcyBvZlxuICogYFVpbnQ4QXJyYXlgLCBzbyB0aGUgcmV0dXJuZWQgaW5zdGFuY2VzIHdpbGwgaGF2ZSBhbGwgdGhlIG5vZGUgYEJ1ZmZlcmAgbWV0aG9kc1xuICogYW5kIHRoZSBgVWludDhBcnJheWAgbWV0aG9kcy4gU3F1YXJlIGJyYWNrZXQgbm90YXRpb24gd29ya3MgYXMgZXhwZWN0ZWQgLS0gaXRcbiAqIHJldHVybnMgYSBzaW5nbGUgb2N0ZXQuXG4gKlxuICogVGhlIGBVaW50OEFycmF5YCBwcm90b3R5cGUgcmVtYWlucyB1bm1vZGlmaWVkLlxuICovXG5cbmZ1bmN0aW9uIEJ1ZmZlciAoYXJnLCBlbmNvZGluZ09yT2Zmc2V0LCBsZW5ndGgpIHtcbiAgaWYgKCFCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCAmJiAhKHRoaXMgaW5zdGFuY2VvZiBCdWZmZXIpKSB7XG4gICAgcmV0dXJuIG5ldyBCdWZmZXIoYXJnLCBlbmNvZGluZ09yT2Zmc2V0LCBsZW5ndGgpXG4gIH1cblxuICAvLyBDb21tb24gY2FzZS5cbiAgaWYgKHR5cGVvZiBhcmcgPT09ICdudW1iZXInKSB7XG4gICAgaWYgKHR5cGVvZiBlbmNvZGluZ09yT2Zmc2V0ID09PSAnc3RyaW5nJykge1xuICAgICAgdGhyb3cgbmV3IEVycm9yKFxuICAgICAgICAnSWYgZW5jb2RpbmcgaXMgc3BlY2lmaWVkIHRoZW4gdGhlIGZpcnN0IGFyZ3VtZW50IG11c3QgYmUgYSBzdHJpbmcnXG4gICAgICApXG4gICAgfVxuICAgIHJldHVybiBhbGxvY1Vuc2FmZSh0aGlzLCBhcmcpXG4gIH1cbiAgcmV0dXJuIGZyb20odGhpcywgYXJnLCBlbmNvZGluZ09yT2Zmc2V0LCBsZW5ndGgpXG59XG5cbkJ1ZmZlci5wb29sU2l6ZSA9IDgxOTIgLy8gbm90IHVzZWQgYnkgdGhpcyBpbXBsZW1lbnRhdGlvblxuXG4vLyBUT0RPOiBMZWdhY3ksIG5vdCBuZWVkZWQgYW55bW9yZS4gUmVtb3ZlIGluIG5leHQgbWFqb3IgdmVyc2lvbi5cbkJ1ZmZlci5fYXVnbWVudCA9IGZ1bmN0aW9uIChhcnIpIHtcbiAgYXJyLl9fcHJvdG9fXyA9IEJ1ZmZlci5wcm90b3R5cGVcbiAgcmV0dXJuIGFyclxufVxuXG5mdW5jdGlvbiBmcm9tICh0aGF0LCB2YWx1ZSwgZW5jb2RpbmdPck9mZnNldCwgbGVuZ3RoKSB7XG4gIGlmICh0eXBlb2YgdmFsdWUgPT09ICdudW1iZXInKSB7XG4gICAgdGhyb3cgbmV3IFR5cGVFcnJvcignXCJ2YWx1ZVwiIGFyZ3VtZW50IG11c3Qgbm90IGJlIGEgbnVtYmVyJylcbiAgfVxuXG4gIGlmICh0eXBlb2YgQXJyYXlCdWZmZXIgIT09ICd1bmRlZmluZWQnICYmIHZhbHVlIGluc3RhbmNlb2YgQXJyYXlCdWZmZXIpIHtcbiAgICByZXR1cm4gZnJvbUFycmF5QnVmZmVyKHRoYXQsIHZhbHVlLCBlbmNvZGluZ09yT2Zmc2V0LCBsZW5ndGgpXG4gIH1cblxuICBpZiAodHlwZW9mIHZhbHVlID09PSAnc3RyaW5nJykge1xuICAgIHJldHVybiBmcm9tU3RyaW5nKHRoYXQsIHZhbHVlLCBlbmNvZGluZ09yT2Zmc2V0KVxuICB9XG5cbiAgcmV0dXJuIGZyb21PYmplY3QodGhhdCwgdmFsdWUpXG59XG5cbi8qKlxuICogRnVuY3Rpb25hbGx5IGVxdWl2YWxlbnQgdG8gQnVmZmVyKGFyZywgZW5jb2RpbmcpIGJ1dCB0aHJvd3MgYSBUeXBlRXJyb3JcbiAqIGlmIHZhbHVlIGlzIGEgbnVtYmVyLlxuICogQnVmZmVyLmZyb20oc3RyWywgZW5jb2RpbmddKVxuICogQnVmZmVyLmZyb20oYXJyYXkpXG4gKiBCdWZmZXIuZnJvbShidWZmZXIpXG4gKiBCdWZmZXIuZnJvbShhcnJheUJ1ZmZlclssIGJ5dGVPZmZzZXRbLCBsZW5ndGhdXSlcbiAqKi9cbkJ1ZmZlci5mcm9tID0gZnVuY3Rpb24gKHZhbHVlLCBlbmNvZGluZ09yT2Zmc2V0LCBsZW5ndGgpIHtcbiAgcmV0dXJuIGZyb20obnVsbCwgdmFsdWUsIGVuY29kaW5nT3JPZmZzZXQsIGxlbmd0aClcbn1cblxuaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gIEJ1ZmZlci5wcm90b3R5cGUuX19wcm90b19fID0gVWludDhBcnJheS5wcm90b3R5cGVcbiAgQnVmZmVyLl9fcHJvdG9fXyA9IFVpbnQ4QXJyYXlcbiAgaWYgKHR5cGVvZiBTeW1ib2wgIT09ICd1bmRlZmluZWQnICYmIFN5bWJvbC5zcGVjaWVzICYmXG4gICAgICBCdWZmZXJbU3ltYm9sLnNwZWNpZXNdID09PSBCdWZmZXIpIHtcbiAgICAvLyBGaXggc3ViYXJyYXkoKSBpbiBFUzIwMTYuIFNlZTogaHR0cHM6Ly9naXRodWIuY29tL2Zlcm9zcy9idWZmZXIvcHVsbC85N1xuICAgIE9iamVjdC5kZWZpbmVQcm9wZXJ0eShCdWZmZXIsIFN5bWJvbC5zcGVjaWVzLCB7XG4gICAgICB2YWx1ZTogbnVsbCxcbiAgICAgIGNvbmZpZ3VyYWJsZTogdHJ1ZVxuICAgIH0pXG4gIH1cbn1cblxuZnVuY3Rpb24gYXNzZXJ0U2l6ZSAoc2l6ZSkge1xuICBpZiAodHlwZW9mIHNpemUgIT09ICdudW1iZXInKSB7XG4gICAgdGhyb3cgbmV3IFR5cGVFcnJvcignXCJzaXplXCIgYXJndW1lbnQgbXVzdCBiZSBhIG51bWJlcicpXG4gIH0gZWxzZSBpZiAoc2l6ZSA8IDApIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignXCJzaXplXCIgYXJndW1lbnQgbXVzdCBub3QgYmUgbmVnYXRpdmUnKVxuICB9XG59XG5cbmZ1bmN0aW9uIGFsbG9jICh0aGF0LCBzaXplLCBmaWxsLCBlbmNvZGluZykge1xuICBhc3NlcnRTaXplKHNpemUpXG4gIGlmIChzaXplIDw9IDApIHtcbiAgICByZXR1cm4gY3JlYXRlQnVmZmVyKHRoYXQsIHNpemUpXG4gIH1cbiAgaWYgKGZpbGwgIT09IHVuZGVmaW5lZCkge1xuICAgIC8vIE9ubHkgcGF5IGF0dGVudGlvbiB0byBlbmNvZGluZyBpZiBpdCdzIGEgc3RyaW5nLiBUaGlzXG4gICAgLy8gcHJldmVudHMgYWNjaWRlbnRhbGx5IHNlbmRpbmcgaW4gYSBudW1iZXIgdGhhdCB3b3VsZFxuICAgIC8vIGJlIGludGVycHJldHRlZCBhcyBhIHN0YXJ0IG9mZnNldC5cbiAgICByZXR1cm4gdHlwZW9mIGVuY29kaW5nID09PSAnc3RyaW5nJ1xuICAgICAgPyBjcmVhdGVCdWZmZXIodGhhdCwgc2l6ZSkuZmlsbChmaWxsLCBlbmNvZGluZylcbiAgICAgIDogY3JlYXRlQnVmZmVyKHRoYXQsIHNpemUpLmZpbGwoZmlsbClcbiAgfVxuICByZXR1cm4gY3JlYXRlQnVmZmVyKHRoYXQsIHNpemUpXG59XG5cbi8qKlxuICogQ3JlYXRlcyBhIG5ldyBmaWxsZWQgQnVmZmVyIGluc3RhbmNlLlxuICogYWxsb2Moc2l6ZVssIGZpbGxbLCBlbmNvZGluZ11dKVxuICoqL1xuQnVmZmVyLmFsbG9jID0gZnVuY3Rpb24gKHNpemUsIGZpbGwsIGVuY29kaW5nKSB7XG4gIHJldHVybiBhbGxvYyhudWxsLCBzaXplLCBmaWxsLCBlbmNvZGluZylcbn1cblxuZnVuY3Rpb24gYWxsb2NVbnNhZmUgKHRoYXQsIHNpemUpIHtcbiAgYXNzZXJ0U2l6ZShzaXplKVxuICB0aGF0ID0gY3JlYXRlQnVmZmVyKHRoYXQsIHNpemUgPCAwID8gMCA6IGNoZWNrZWQoc2l6ZSkgfCAwKVxuICBpZiAoIUJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgZm9yICh2YXIgaSA9IDA7IGkgPCBzaXplOyArK2kpIHtcbiAgICAgIHRoYXRbaV0gPSAwXG4gICAgfVxuICB9XG4gIHJldHVybiB0aGF0XG59XG5cbi8qKlxuICogRXF1aXZhbGVudCB0byBCdWZmZXIobnVtKSwgYnkgZGVmYXVsdCBjcmVhdGVzIGEgbm9uLXplcm8tZmlsbGVkIEJ1ZmZlciBpbnN0YW5jZS5cbiAqICovXG5CdWZmZXIuYWxsb2NVbnNhZmUgPSBmdW5jdGlvbiAoc2l6ZSkge1xuICByZXR1cm4gYWxsb2NVbnNhZmUobnVsbCwgc2l6ZSlcbn1cbi8qKlxuICogRXF1aXZhbGVudCB0byBTbG93QnVmZmVyKG51bSksIGJ5IGRlZmF1bHQgY3JlYXRlcyBhIG5vbi16ZXJvLWZpbGxlZCBCdWZmZXIgaW5zdGFuY2UuXG4gKi9cbkJ1ZmZlci5hbGxvY1Vuc2FmZVNsb3cgPSBmdW5jdGlvbiAoc2l6ZSkge1xuICByZXR1cm4gYWxsb2NVbnNhZmUobnVsbCwgc2l6ZSlcbn1cblxuZnVuY3Rpb24gZnJvbVN0cmluZyAodGhhdCwgc3RyaW5nLCBlbmNvZGluZykge1xuICBpZiAodHlwZW9mIGVuY29kaW5nICE9PSAnc3RyaW5nJyB8fCBlbmNvZGluZyA9PT0gJycpIHtcbiAgICBlbmNvZGluZyA9ICd1dGY4J1xuICB9XG5cbiAgaWYgKCFCdWZmZXIuaXNFbmNvZGluZyhlbmNvZGluZykpIHtcbiAgICB0aHJvdyBuZXcgVHlwZUVycm9yKCdcImVuY29kaW5nXCIgbXVzdCBiZSBhIHZhbGlkIHN0cmluZyBlbmNvZGluZycpXG4gIH1cblxuICB2YXIgbGVuZ3RoID0gYnl0ZUxlbmd0aChzdHJpbmcsIGVuY29kaW5nKSB8IDBcbiAgdGhhdCA9IGNyZWF0ZUJ1ZmZlcih0aGF0LCBsZW5ndGgpXG5cbiAgdmFyIGFjdHVhbCA9IHRoYXQud3JpdGUoc3RyaW5nLCBlbmNvZGluZylcblxuICBpZiAoYWN0dWFsICE9PSBsZW5ndGgpIHtcbiAgICAvLyBXcml0aW5nIGEgaGV4IHN0cmluZywgZm9yIGV4YW1wbGUsIHRoYXQgY29udGFpbnMgaW52YWxpZCBjaGFyYWN0ZXJzIHdpbGxcbiAgICAvLyBjYXVzZSBldmVyeXRoaW5nIGFmdGVyIHRoZSBmaXJzdCBpbnZhbGlkIGNoYXJhY3RlciB0byBiZSBpZ25vcmVkLiAoZS5nLlxuICAgIC8vICdhYnh4Y2QnIHdpbGwgYmUgdHJlYXRlZCBhcyAnYWInKVxuICAgIHRoYXQgPSB0aGF0LnNsaWNlKDAsIGFjdHVhbClcbiAgfVxuXG4gIHJldHVybiB0aGF0XG59XG5cbmZ1bmN0aW9uIGZyb21BcnJheUxpa2UgKHRoYXQsIGFycmF5KSB7XG4gIHZhciBsZW5ndGggPSBhcnJheS5sZW5ndGggPCAwID8gMCA6IGNoZWNrZWQoYXJyYXkubGVuZ3RoKSB8IDBcbiAgdGhhdCA9IGNyZWF0ZUJ1ZmZlcih0aGF0LCBsZW5ndGgpXG4gIGZvciAodmFyIGkgPSAwOyBpIDwgbGVuZ3RoOyBpICs9IDEpIHtcbiAgICB0aGF0W2ldID0gYXJyYXlbaV0gJiAyNTVcbiAgfVxuICByZXR1cm4gdGhhdFxufVxuXG5mdW5jdGlvbiBmcm9tQXJyYXlCdWZmZXIgKHRoYXQsIGFycmF5LCBieXRlT2Zmc2V0LCBsZW5ndGgpIHtcbiAgYXJyYXkuYnl0ZUxlbmd0aCAvLyB0aGlzIHRocm93cyBpZiBgYXJyYXlgIGlzIG5vdCBhIHZhbGlkIEFycmF5QnVmZmVyXG5cbiAgaWYgKGJ5dGVPZmZzZXQgPCAwIHx8IGFycmF5LmJ5dGVMZW5ndGggPCBieXRlT2Zmc2V0KSB7XG4gICAgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ1xcJ29mZnNldFxcJyBpcyBvdXQgb2YgYm91bmRzJylcbiAgfVxuXG4gIGlmIChhcnJheS5ieXRlTGVuZ3RoIDwgYnl0ZU9mZnNldCArIChsZW5ndGggfHwgMCkpIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignXFwnbGVuZ3RoXFwnIGlzIG91dCBvZiBib3VuZHMnKVxuICB9XG5cbiAgaWYgKGJ5dGVPZmZzZXQgPT09IHVuZGVmaW5lZCAmJiBsZW5ndGggPT09IHVuZGVmaW5lZCkge1xuICAgIGFycmF5ID0gbmV3IFVpbnQ4QXJyYXkoYXJyYXkpXG4gIH0gZWxzZSBpZiAobGVuZ3RoID09PSB1bmRlZmluZWQpIHtcbiAgICBhcnJheSA9IG5ldyBVaW50OEFycmF5KGFycmF5LCBieXRlT2Zmc2V0KVxuICB9IGVsc2Uge1xuICAgIGFycmF5ID0gbmV3IFVpbnQ4QXJyYXkoYXJyYXksIGJ5dGVPZmZzZXQsIGxlbmd0aClcbiAgfVxuXG4gIGlmIChCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkge1xuICAgIC8vIFJldHVybiBhbiBhdWdtZW50ZWQgYFVpbnQ4QXJyYXlgIGluc3RhbmNlLCBmb3IgYmVzdCBwZXJmb3JtYW5jZVxuICAgIHRoYXQgPSBhcnJheVxuICAgIHRoYXQuX19wcm90b19fID0gQnVmZmVyLnByb3RvdHlwZVxuICB9IGVsc2Uge1xuICAgIC8vIEZhbGxiYWNrOiBSZXR1cm4gYW4gb2JqZWN0IGluc3RhbmNlIG9mIHRoZSBCdWZmZXIgY2xhc3NcbiAgICB0aGF0ID0gZnJvbUFycmF5TGlrZSh0aGF0LCBhcnJheSlcbiAgfVxuICByZXR1cm4gdGhhdFxufVxuXG5mdW5jdGlvbiBmcm9tT2JqZWN0ICh0aGF0LCBvYmopIHtcbiAgaWYgKEJ1ZmZlci5pc0J1ZmZlcihvYmopKSB7XG4gICAgdmFyIGxlbiA9IGNoZWNrZWQob2JqLmxlbmd0aCkgfCAwXG4gICAgdGhhdCA9IGNyZWF0ZUJ1ZmZlcih0aGF0LCBsZW4pXG5cbiAgICBpZiAodGhhdC5sZW5ndGggPT09IDApIHtcbiAgICAgIHJldHVybiB0aGF0XG4gICAgfVxuXG4gICAgb2JqLmNvcHkodGhhdCwgMCwgMCwgbGVuKVxuICAgIHJldHVybiB0aGF0XG4gIH1cblxuICBpZiAob2JqKSB7XG4gICAgaWYgKCh0eXBlb2YgQXJyYXlCdWZmZXIgIT09ICd1bmRlZmluZWQnICYmXG4gICAgICAgIG9iai5idWZmZXIgaW5zdGFuY2VvZiBBcnJheUJ1ZmZlcikgfHwgJ2xlbmd0aCcgaW4gb2JqKSB7XG4gICAgICBpZiAodHlwZW9mIG9iai5sZW5ndGggIT09ICdudW1iZXInIHx8IGlzbmFuKG9iai5sZW5ndGgpKSB7XG4gICAgICAgIHJldHVybiBjcmVhdGVCdWZmZXIodGhhdCwgMClcbiAgICAgIH1cbiAgICAgIHJldHVybiBmcm9tQXJyYXlMaWtlKHRoYXQsIG9iailcbiAgICB9XG5cbiAgICBpZiAob2JqLnR5cGUgPT09ICdCdWZmZXInICYmIGlzQXJyYXkob2JqLmRhdGEpKSB7XG4gICAgICByZXR1cm4gZnJvbUFycmF5TGlrZSh0aGF0LCBvYmouZGF0YSlcbiAgICB9XG4gIH1cblxuICB0aHJvdyBuZXcgVHlwZUVycm9yKCdGaXJzdCBhcmd1bWVudCBtdXN0IGJlIGEgc3RyaW5nLCBCdWZmZXIsIEFycmF5QnVmZmVyLCBBcnJheSwgb3IgYXJyYXktbGlrZSBvYmplY3QuJylcbn1cblxuZnVuY3Rpb24gY2hlY2tlZCAobGVuZ3RoKSB7XG4gIC8vIE5vdGU6IGNhbm5vdCB1c2UgYGxlbmd0aCA8IGtNYXhMZW5ndGgoKWAgaGVyZSBiZWNhdXNlIHRoYXQgZmFpbHMgd2hlblxuICAvLyBsZW5ndGggaXMgTmFOICh3aGljaCBpcyBvdGhlcndpc2UgY29lcmNlZCB0byB6ZXJvLilcbiAgaWYgKGxlbmd0aCA+PSBrTWF4TGVuZ3RoKCkpIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignQXR0ZW1wdCB0byBhbGxvY2F0ZSBCdWZmZXIgbGFyZ2VyIHRoYW4gbWF4aW11bSAnICtcbiAgICAgICAgICAgICAgICAgICAgICAgICAnc2l6ZTogMHgnICsga01heExlbmd0aCgpLnRvU3RyaW5nKDE2KSArICcgYnl0ZXMnKVxuICB9XG4gIHJldHVybiBsZW5ndGggfCAwXG59XG5cbmZ1bmN0aW9uIFNsb3dCdWZmZXIgKGxlbmd0aCkge1xuICBpZiAoK2xlbmd0aCAhPSBsZW5ndGgpIHsgLy8gZXNsaW50LWRpc2FibGUtbGluZSBlcWVxZXFcbiAgICBsZW5ndGggPSAwXG4gIH1cbiAgcmV0dXJuIEJ1ZmZlci5hbGxvYygrbGVuZ3RoKVxufVxuXG5CdWZmZXIuaXNCdWZmZXIgPSBmdW5jdGlvbiBpc0J1ZmZlciAoYikge1xuICByZXR1cm4gISEoYiAhPSBudWxsICYmIGIuX2lzQnVmZmVyKVxufVxuXG5CdWZmZXIuY29tcGFyZSA9IGZ1bmN0aW9uIGNvbXBhcmUgKGEsIGIpIHtcbiAgaWYgKCFCdWZmZXIuaXNCdWZmZXIoYSkgfHwgIUJ1ZmZlci5pc0J1ZmZlcihiKSkge1xuICAgIHRocm93IG5ldyBUeXBlRXJyb3IoJ0FyZ3VtZW50cyBtdXN0IGJlIEJ1ZmZlcnMnKVxuICB9XG5cbiAgaWYgKGEgPT09IGIpIHJldHVybiAwXG5cbiAgdmFyIHggPSBhLmxlbmd0aFxuICB2YXIgeSA9IGIubGVuZ3RoXG5cbiAgZm9yICh2YXIgaSA9IDAsIGxlbiA9IE1hdGgubWluKHgsIHkpOyBpIDwgbGVuOyArK2kpIHtcbiAgICBpZiAoYVtpXSAhPT0gYltpXSkge1xuICAgICAgeCA9IGFbaV1cbiAgICAgIHkgPSBiW2ldXG4gICAgICBicmVha1xuICAgIH1cbiAgfVxuXG4gIGlmICh4IDwgeSkgcmV0dXJuIC0xXG4gIGlmICh5IDwgeCkgcmV0dXJuIDFcbiAgcmV0dXJuIDBcbn1cblxuQnVmZmVyLmlzRW5jb2RpbmcgPSBmdW5jdGlvbiBpc0VuY29kaW5nIChlbmNvZGluZykge1xuICBzd2l0Y2ggKFN0cmluZyhlbmNvZGluZykudG9Mb3dlckNhc2UoKSkge1xuICAgIGNhc2UgJ2hleCc6XG4gICAgY2FzZSAndXRmOCc6XG4gICAgY2FzZSAndXRmLTgnOlxuICAgIGNhc2UgJ2FzY2lpJzpcbiAgICBjYXNlICdsYXRpbjEnOlxuICAgIGNhc2UgJ2JpbmFyeSc6XG4gICAgY2FzZSAnYmFzZTY0JzpcbiAgICBjYXNlICd1Y3MyJzpcbiAgICBjYXNlICd1Y3MtMic6XG4gICAgY2FzZSAndXRmMTZsZSc6XG4gICAgY2FzZSAndXRmLTE2bGUnOlxuICAgICAgcmV0dXJuIHRydWVcbiAgICBkZWZhdWx0OlxuICAgICAgcmV0dXJuIGZhbHNlXG4gIH1cbn1cblxuQnVmZmVyLmNvbmNhdCA9IGZ1bmN0aW9uIGNvbmNhdCAobGlzdCwgbGVuZ3RoKSB7XG4gIGlmICghaXNBcnJheShsaXN0KSkge1xuICAgIHRocm93IG5ldyBUeXBlRXJyb3IoJ1wibGlzdFwiIGFyZ3VtZW50IG11c3QgYmUgYW4gQXJyYXkgb2YgQnVmZmVycycpXG4gIH1cblxuICBpZiAobGlzdC5sZW5ndGggPT09IDApIHtcbiAgICByZXR1cm4gQnVmZmVyLmFsbG9jKDApXG4gIH1cblxuICB2YXIgaVxuICBpZiAobGVuZ3RoID09PSB1bmRlZmluZWQpIHtcbiAgICBsZW5ndGggPSAwXG4gICAgZm9yIChpID0gMDsgaSA8IGxpc3QubGVuZ3RoOyArK2kpIHtcbiAgICAgIGxlbmd0aCArPSBsaXN0W2ldLmxlbmd0aFxuICAgIH1cbiAgfVxuXG4gIHZhciBidWZmZXIgPSBCdWZmZXIuYWxsb2NVbnNhZmUobGVuZ3RoKVxuICB2YXIgcG9zID0gMFxuICBmb3IgKGkgPSAwOyBpIDwgbGlzdC5sZW5ndGg7ICsraSkge1xuICAgIHZhciBidWYgPSBsaXN0W2ldXG4gICAgaWYgKCFCdWZmZXIuaXNCdWZmZXIoYnVmKSkge1xuICAgICAgdGhyb3cgbmV3IFR5cGVFcnJvcignXCJsaXN0XCIgYXJndW1lbnQgbXVzdCBiZSBhbiBBcnJheSBvZiBCdWZmZXJzJylcbiAgICB9XG4gICAgYnVmLmNvcHkoYnVmZmVyLCBwb3MpXG4gICAgcG9zICs9IGJ1Zi5sZW5ndGhcbiAgfVxuICByZXR1cm4gYnVmZmVyXG59XG5cbmZ1bmN0aW9uIGJ5dGVMZW5ndGggKHN0cmluZywgZW5jb2RpbmcpIHtcbiAgaWYgKEJ1ZmZlci5pc0J1ZmZlcihzdHJpbmcpKSB7XG4gICAgcmV0dXJuIHN0cmluZy5sZW5ndGhcbiAgfVxuICBpZiAodHlwZW9mIEFycmF5QnVmZmVyICE9PSAndW5kZWZpbmVkJyAmJiB0eXBlb2YgQXJyYXlCdWZmZXIuaXNWaWV3ID09PSAnZnVuY3Rpb24nICYmXG4gICAgICAoQXJyYXlCdWZmZXIuaXNWaWV3KHN0cmluZykgfHwgc3RyaW5nIGluc3RhbmNlb2YgQXJyYXlCdWZmZXIpKSB7XG4gICAgcmV0dXJuIHN0cmluZy5ieXRlTGVuZ3RoXG4gIH1cbiAgaWYgKHR5cGVvZiBzdHJpbmcgIT09ICdzdHJpbmcnKSB7XG4gICAgc3RyaW5nID0gJycgKyBzdHJpbmdcbiAgfVxuXG4gIHZhciBsZW4gPSBzdHJpbmcubGVuZ3RoXG4gIGlmIChsZW4gPT09IDApIHJldHVybiAwXG5cbiAgLy8gVXNlIGEgZm9yIGxvb3AgdG8gYXZvaWQgcmVjdXJzaW9uXG4gIHZhciBsb3dlcmVkQ2FzZSA9IGZhbHNlXG4gIGZvciAoOzspIHtcbiAgICBzd2l0Y2ggKGVuY29kaW5nKSB7XG4gICAgICBjYXNlICdhc2NpaSc6XG4gICAgICBjYXNlICdsYXRpbjEnOlxuICAgICAgY2FzZSAnYmluYXJ5JzpcbiAgICAgICAgcmV0dXJuIGxlblxuICAgICAgY2FzZSAndXRmOCc6XG4gICAgICBjYXNlICd1dGYtOCc6XG4gICAgICBjYXNlIHVuZGVmaW5lZDpcbiAgICAgICAgcmV0dXJuIHV0ZjhUb0J5dGVzKHN0cmluZykubGVuZ3RoXG4gICAgICBjYXNlICd1Y3MyJzpcbiAgICAgIGNhc2UgJ3Vjcy0yJzpcbiAgICAgIGNhc2UgJ3V0ZjE2bGUnOlxuICAgICAgY2FzZSAndXRmLTE2bGUnOlxuICAgICAgICByZXR1cm4gbGVuICogMlxuICAgICAgY2FzZSAnaGV4JzpcbiAgICAgICAgcmV0dXJuIGxlbiA+Pj4gMVxuICAgICAgY2FzZSAnYmFzZTY0JzpcbiAgICAgICAgcmV0dXJuIGJhc2U2NFRvQnl0ZXMoc3RyaW5nKS5sZW5ndGhcbiAgICAgIGRlZmF1bHQ6XG4gICAgICAgIGlmIChsb3dlcmVkQ2FzZSkgcmV0dXJuIHV0ZjhUb0J5dGVzKHN0cmluZykubGVuZ3RoIC8vIGFzc3VtZSB1dGY4XG4gICAgICAgIGVuY29kaW5nID0gKCcnICsgZW5jb2RpbmcpLnRvTG93ZXJDYXNlKClcbiAgICAgICAgbG93ZXJlZENhc2UgPSB0cnVlXG4gICAgfVxuICB9XG59XG5CdWZmZXIuYnl0ZUxlbmd0aCA9IGJ5dGVMZW5ndGhcblxuZnVuY3Rpb24gc2xvd1RvU3RyaW5nIChlbmNvZGluZywgc3RhcnQsIGVuZCkge1xuICB2YXIgbG93ZXJlZENhc2UgPSBmYWxzZVxuXG4gIC8vIE5vIG5lZWQgdG8gdmVyaWZ5IHRoYXQgXCJ0aGlzLmxlbmd0aCA8PSBNQVhfVUlOVDMyXCIgc2luY2UgaXQncyBhIHJlYWQtb25seVxuICAvLyBwcm9wZXJ0eSBvZiBhIHR5cGVkIGFycmF5LlxuXG4gIC8vIFRoaXMgYmVoYXZlcyBuZWl0aGVyIGxpa2UgU3RyaW5nIG5vciBVaW50OEFycmF5IGluIHRoYXQgd2Ugc2V0IHN0YXJ0L2VuZFxuICAvLyB0byB0aGVpciB1cHBlci9sb3dlciBib3VuZHMgaWYgdGhlIHZhbHVlIHBhc3NlZCBpcyBvdXQgb2YgcmFuZ2UuXG4gIC8vIHVuZGVmaW5lZCBpcyBoYW5kbGVkIHNwZWNpYWxseSBhcyBwZXIgRUNNQS0yNjIgNnRoIEVkaXRpb24sXG4gIC8vIFNlY3Rpb24gMTMuMy4zLjcgUnVudGltZSBTZW1hbnRpY3M6IEtleWVkQmluZGluZ0luaXRpYWxpemF0aW9uLlxuICBpZiAoc3RhcnQgPT09IHVuZGVmaW5lZCB8fCBzdGFydCA8IDApIHtcbiAgICBzdGFydCA9IDBcbiAgfVxuICAvLyBSZXR1cm4gZWFybHkgaWYgc3RhcnQgPiB0aGlzLmxlbmd0aC4gRG9uZSBoZXJlIHRvIHByZXZlbnQgcG90ZW50aWFsIHVpbnQzMlxuICAvLyBjb2VyY2lvbiBmYWlsIGJlbG93LlxuICBpZiAoc3RhcnQgPiB0aGlzLmxlbmd0aCkge1xuICAgIHJldHVybiAnJ1xuICB9XG5cbiAgaWYgKGVuZCA9PT0gdW5kZWZpbmVkIHx8IGVuZCA+IHRoaXMubGVuZ3RoKSB7XG4gICAgZW5kID0gdGhpcy5sZW5ndGhcbiAgfVxuXG4gIGlmIChlbmQgPD0gMCkge1xuICAgIHJldHVybiAnJ1xuICB9XG5cbiAgLy8gRm9yY2UgY29lcnNpb24gdG8gdWludDMyLiBUaGlzIHdpbGwgYWxzbyBjb2VyY2UgZmFsc2V5L05hTiB2YWx1ZXMgdG8gMC5cbiAgZW5kID4+Pj0gMFxuICBzdGFydCA+Pj49IDBcblxuICBpZiAoZW5kIDw9IHN0YXJ0KSB7XG4gICAgcmV0dXJuICcnXG4gIH1cblxuICBpZiAoIWVuY29kaW5nKSBlbmNvZGluZyA9ICd1dGY4J1xuXG4gIHdoaWxlICh0cnVlKSB7XG4gICAgc3dpdGNoIChlbmNvZGluZykge1xuICAgICAgY2FzZSAnaGV4JzpcbiAgICAgICAgcmV0dXJuIGhleFNsaWNlKHRoaXMsIHN0YXJ0LCBlbmQpXG5cbiAgICAgIGNhc2UgJ3V0ZjgnOlxuICAgICAgY2FzZSAndXRmLTgnOlxuICAgICAgICByZXR1cm4gdXRmOFNsaWNlKHRoaXMsIHN0YXJ0LCBlbmQpXG5cbiAgICAgIGNhc2UgJ2FzY2lpJzpcbiAgICAgICAgcmV0dXJuIGFzY2lpU2xpY2UodGhpcywgc3RhcnQsIGVuZClcblxuICAgICAgY2FzZSAnbGF0aW4xJzpcbiAgICAgIGNhc2UgJ2JpbmFyeSc6XG4gICAgICAgIHJldHVybiBsYXRpbjFTbGljZSh0aGlzLCBzdGFydCwgZW5kKVxuXG4gICAgICBjYXNlICdiYXNlNjQnOlxuICAgICAgICByZXR1cm4gYmFzZTY0U2xpY2UodGhpcywgc3RhcnQsIGVuZClcblxuICAgICAgY2FzZSAndWNzMic6XG4gICAgICBjYXNlICd1Y3MtMic6XG4gICAgICBjYXNlICd1dGYxNmxlJzpcbiAgICAgIGNhc2UgJ3V0Zi0xNmxlJzpcbiAgICAgICAgcmV0dXJuIHV0ZjE2bGVTbGljZSh0aGlzLCBzdGFydCwgZW5kKVxuXG4gICAgICBkZWZhdWx0OlxuICAgICAgICBpZiAobG93ZXJlZENhc2UpIHRocm93IG5ldyBUeXBlRXJyb3IoJ1Vua25vd24gZW5jb2Rpbmc6ICcgKyBlbmNvZGluZylcbiAgICAgICAgZW5jb2RpbmcgPSAoZW5jb2RpbmcgKyAnJykudG9Mb3dlckNhc2UoKVxuICAgICAgICBsb3dlcmVkQ2FzZSA9IHRydWVcbiAgICB9XG4gIH1cbn1cblxuLy8gVGhlIHByb3BlcnR5IGlzIHVzZWQgYnkgYEJ1ZmZlci5pc0J1ZmZlcmAgYW5kIGBpcy1idWZmZXJgIChpbiBTYWZhcmkgNS03KSB0byBkZXRlY3Rcbi8vIEJ1ZmZlciBpbnN0YW5jZXMuXG5CdWZmZXIucHJvdG90eXBlLl9pc0J1ZmZlciA9IHRydWVcblxuZnVuY3Rpb24gc3dhcCAoYiwgbiwgbSkge1xuICB2YXIgaSA9IGJbbl1cbiAgYltuXSA9IGJbbV1cbiAgYlttXSA9IGlcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5zd2FwMTYgPSBmdW5jdGlvbiBzd2FwMTYgKCkge1xuICB2YXIgbGVuID0gdGhpcy5sZW5ndGhcbiAgaWYgKGxlbiAlIDIgIT09IDApIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignQnVmZmVyIHNpemUgbXVzdCBiZSBhIG11bHRpcGxlIG9mIDE2LWJpdHMnKVxuICB9XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgbGVuOyBpICs9IDIpIHtcbiAgICBzd2FwKHRoaXMsIGksIGkgKyAxKVxuICB9XG4gIHJldHVybiB0aGlzXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUuc3dhcDMyID0gZnVuY3Rpb24gc3dhcDMyICgpIHtcbiAgdmFyIGxlbiA9IHRoaXMubGVuZ3RoXG4gIGlmIChsZW4gJSA0ICE9PSAwKSB7XG4gICAgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ0J1ZmZlciBzaXplIG11c3QgYmUgYSBtdWx0aXBsZSBvZiAzMi1iaXRzJylcbiAgfVxuICBmb3IgKHZhciBpID0gMDsgaSA8IGxlbjsgaSArPSA0KSB7XG4gICAgc3dhcCh0aGlzLCBpLCBpICsgMylcbiAgICBzd2FwKHRoaXMsIGkgKyAxLCBpICsgMilcbiAgfVxuICByZXR1cm4gdGhpc1xufVxuXG5CdWZmZXIucHJvdG90eXBlLnN3YXA2NCA9IGZ1bmN0aW9uIHN3YXA2NCAoKSB7XG4gIHZhciBsZW4gPSB0aGlzLmxlbmd0aFxuICBpZiAobGVuICUgOCAhPT0gMCkge1xuICAgIHRocm93IG5ldyBSYW5nZUVycm9yKCdCdWZmZXIgc2l6ZSBtdXN0IGJlIGEgbXVsdGlwbGUgb2YgNjQtYml0cycpXG4gIH1cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBsZW47IGkgKz0gOCkge1xuICAgIHN3YXAodGhpcywgaSwgaSArIDcpXG4gICAgc3dhcCh0aGlzLCBpICsgMSwgaSArIDYpXG4gICAgc3dhcCh0aGlzLCBpICsgMiwgaSArIDUpXG4gICAgc3dhcCh0aGlzLCBpICsgMywgaSArIDQpXG4gIH1cbiAgcmV0dXJuIHRoaXNcbn1cblxuQnVmZmVyLnByb3RvdHlwZS50b1N0cmluZyA9IGZ1bmN0aW9uIHRvU3RyaW5nICgpIHtcbiAgdmFyIGxlbmd0aCA9IHRoaXMubGVuZ3RoIHwgMFxuICBpZiAobGVuZ3RoID09PSAwKSByZXR1cm4gJydcbiAgaWYgKGFyZ3VtZW50cy5sZW5ndGggPT09IDApIHJldHVybiB1dGY4U2xpY2UodGhpcywgMCwgbGVuZ3RoKVxuICByZXR1cm4gc2xvd1RvU3RyaW5nLmFwcGx5KHRoaXMsIGFyZ3VtZW50cylcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5lcXVhbHMgPSBmdW5jdGlvbiBlcXVhbHMgKGIpIHtcbiAgaWYgKCFCdWZmZXIuaXNCdWZmZXIoYikpIHRocm93IG5ldyBUeXBlRXJyb3IoJ0FyZ3VtZW50IG11c3QgYmUgYSBCdWZmZXInKVxuICBpZiAodGhpcyA9PT0gYikgcmV0dXJuIHRydWVcbiAgcmV0dXJuIEJ1ZmZlci5jb21wYXJlKHRoaXMsIGIpID09PSAwXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUuaW5zcGVjdCA9IGZ1bmN0aW9uIGluc3BlY3QgKCkge1xuICB2YXIgc3RyID0gJydcbiAgdmFyIG1heCA9IGV4cG9ydHMuSU5TUEVDVF9NQVhfQllURVNcbiAgaWYgKHRoaXMubGVuZ3RoID4gMCkge1xuICAgIHN0ciA9IHRoaXMudG9TdHJpbmcoJ2hleCcsIDAsIG1heCkubWF0Y2goLy57Mn0vZykuam9pbignICcpXG4gICAgaWYgKHRoaXMubGVuZ3RoID4gbWF4KSBzdHIgKz0gJyAuLi4gJ1xuICB9XG4gIHJldHVybiAnPEJ1ZmZlciAnICsgc3RyICsgJz4nXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUuY29tcGFyZSA9IGZ1bmN0aW9uIGNvbXBhcmUgKHRhcmdldCwgc3RhcnQsIGVuZCwgdGhpc1N0YXJ0LCB0aGlzRW5kKSB7XG4gIGlmICghQnVmZmVyLmlzQnVmZmVyKHRhcmdldCkpIHtcbiAgICB0aHJvdyBuZXcgVHlwZUVycm9yKCdBcmd1bWVudCBtdXN0IGJlIGEgQnVmZmVyJylcbiAgfVxuXG4gIGlmIChzdGFydCA9PT0gdW5kZWZpbmVkKSB7XG4gICAgc3RhcnQgPSAwXG4gIH1cbiAgaWYgKGVuZCA9PT0gdW5kZWZpbmVkKSB7XG4gICAgZW5kID0gdGFyZ2V0ID8gdGFyZ2V0Lmxlbmd0aCA6IDBcbiAgfVxuICBpZiAodGhpc1N0YXJ0ID09PSB1bmRlZmluZWQpIHtcbiAgICB0aGlzU3RhcnQgPSAwXG4gIH1cbiAgaWYgKHRoaXNFbmQgPT09IHVuZGVmaW5lZCkge1xuICAgIHRoaXNFbmQgPSB0aGlzLmxlbmd0aFxuICB9XG5cbiAgaWYgKHN0YXJ0IDwgMCB8fCBlbmQgPiB0YXJnZXQubGVuZ3RoIHx8IHRoaXNTdGFydCA8IDAgfHwgdGhpc0VuZCA+IHRoaXMubGVuZ3RoKSB7XG4gICAgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ291dCBvZiByYW5nZSBpbmRleCcpXG4gIH1cblxuICBpZiAodGhpc1N0YXJ0ID49IHRoaXNFbmQgJiYgc3RhcnQgPj0gZW5kKSB7XG4gICAgcmV0dXJuIDBcbiAgfVxuICBpZiAodGhpc1N0YXJ0ID49IHRoaXNFbmQpIHtcbiAgICByZXR1cm4gLTFcbiAgfVxuICBpZiAoc3RhcnQgPj0gZW5kKSB7XG4gICAgcmV0dXJuIDFcbiAgfVxuXG4gIHN0YXJ0ID4+Pj0gMFxuICBlbmQgPj4+PSAwXG4gIHRoaXNTdGFydCA+Pj49IDBcbiAgdGhpc0VuZCA+Pj49IDBcblxuICBpZiAodGhpcyA9PT0gdGFyZ2V0KSByZXR1cm4gMFxuXG4gIHZhciB4ID0gdGhpc0VuZCAtIHRoaXNTdGFydFxuICB2YXIgeSA9IGVuZCAtIHN0YXJ0XG4gIHZhciBsZW4gPSBNYXRoLm1pbih4LCB5KVxuXG4gIHZhciB0aGlzQ29weSA9IHRoaXMuc2xpY2UodGhpc1N0YXJ0LCB0aGlzRW5kKVxuICB2YXIgdGFyZ2V0Q29weSA9IHRhcmdldC5zbGljZShzdGFydCwgZW5kKVxuXG4gIGZvciAodmFyIGkgPSAwOyBpIDwgbGVuOyArK2kpIHtcbiAgICBpZiAodGhpc0NvcHlbaV0gIT09IHRhcmdldENvcHlbaV0pIHtcbiAgICAgIHggPSB0aGlzQ29weVtpXVxuICAgICAgeSA9IHRhcmdldENvcHlbaV1cbiAgICAgIGJyZWFrXG4gICAgfVxuICB9XG5cbiAgaWYgKHggPCB5KSByZXR1cm4gLTFcbiAgaWYgKHkgPCB4KSByZXR1cm4gMVxuICByZXR1cm4gMFxufVxuXG4vLyBGaW5kcyBlaXRoZXIgdGhlIGZpcnN0IGluZGV4IG9mIGB2YWxgIGluIGBidWZmZXJgIGF0IG9mZnNldCA+PSBgYnl0ZU9mZnNldGAsXG4vLyBPUiB0aGUgbGFzdCBpbmRleCBvZiBgdmFsYCBpbiBgYnVmZmVyYCBhdCBvZmZzZXQgPD0gYGJ5dGVPZmZzZXRgLlxuLy9cbi8vIEFyZ3VtZW50czpcbi8vIC0gYnVmZmVyIC0gYSBCdWZmZXIgdG8gc2VhcmNoXG4vLyAtIHZhbCAtIGEgc3RyaW5nLCBCdWZmZXIsIG9yIG51bWJlclxuLy8gLSBieXRlT2Zmc2V0IC0gYW4gaW5kZXggaW50byBgYnVmZmVyYDsgd2lsbCBiZSBjbGFtcGVkIHRvIGFuIGludDMyXG4vLyAtIGVuY29kaW5nIC0gYW4gb3B0aW9uYWwgZW5jb2RpbmcsIHJlbGV2YW50IGlzIHZhbCBpcyBhIHN0cmluZ1xuLy8gLSBkaXIgLSB0cnVlIGZvciBpbmRleE9mLCBmYWxzZSBmb3IgbGFzdEluZGV4T2ZcbmZ1bmN0aW9uIGJpZGlyZWN0aW9uYWxJbmRleE9mIChidWZmZXIsIHZhbCwgYnl0ZU9mZnNldCwgZW5jb2RpbmcsIGRpcikge1xuICAvLyBFbXB0eSBidWZmZXIgbWVhbnMgbm8gbWF0Y2hcbiAgaWYgKGJ1ZmZlci5sZW5ndGggPT09IDApIHJldHVybiAtMVxuXG4gIC8vIE5vcm1hbGl6ZSBieXRlT2Zmc2V0XG4gIGlmICh0eXBlb2YgYnl0ZU9mZnNldCA9PT0gJ3N0cmluZycpIHtcbiAgICBlbmNvZGluZyA9IGJ5dGVPZmZzZXRcbiAgICBieXRlT2Zmc2V0ID0gMFxuICB9IGVsc2UgaWYgKGJ5dGVPZmZzZXQgPiAweDdmZmZmZmZmKSB7XG4gICAgYnl0ZU9mZnNldCA9IDB4N2ZmZmZmZmZcbiAgfSBlbHNlIGlmIChieXRlT2Zmc2V0IDwgLTB4ODAwMDAwMDApIHtcbiAgICBieXRlT2Zmc2V0ID0gLTB4ODAwMDAwMDBcbiAgfVxuICBieXRlT2Zmc2V0ID0gK2J5dGVPZmZzZXQgIC8vIENvZXJjZSB0byBOdW1iZXIuXG4gIGlmIChpc05hTihieXRlT2Zmc2V0KSkge1xuICAgIC8vIGJ5dGVPZmZzZXQ6IGl0IGl0J3MgdW5kZWZpbmVkLCBudWxsLCBOYU4sIFwiZm9vXCIsIGV0Yywgc2VhcmNoIHdob2xlIGJ1ZmZlclxuICAgIGJ5dGVPZmZzZXQgPSBkaXIgPyAwIDogKGJ1ZmZlci5sZW5ndGggLSAxKVxuICB9XG5cbiAgLy8gTm9ybWFsaXplIGJ5dGVPZmZzZXQ6IG5lZ2F0aXZlIG9mZnNldHMgc3RhcnQgZnJvbSB0aGUgZW5kIG9mIHRoZSBidWZmZXJcbiAgaWYgKGJ5dGVPZmZzZXQgPCAwKSBieXRlT2Zmc2V0ID0gYnVmZmVyLmxlbmd0aCArIGJ5dGVPZmZzZXRcbiAgaWYgKGJ5dGVPZmZzZXQgPj0gYnVmZmVyLmxlbmd0aCkge1xuICAgIGlmIChkaXIpIHJldHVybiAtMVxuICAgIGVsc2UgYnl0ZU9mZnNldCA9IGJ1ZmZlci5sZW5ndGggLSAxXG4gIH0gZWxzZSBpZiAoYnl0ZU9mZnNldCA8IDApIHtcbiAgICBpZiAoZGlyKSBieXRlT2Zmc2V0ID0gMFxuICAgIGVsc2UgcmV0dXJuIC0xXG4gIH1cblxuICAvLyBOb3JtYWxpemUgdmFsXG4gIGlmICh0eXBlb2YgdmFsID09PSAnc3RyaW5nJykge1xuICAgIHZhbCA9IEJ1ZmZlci5mcm9tKHZhbCwgZW5jb2RpbmcpXG4gIH1cblxuICAvLyBGaW5hbGx5LCBzZWFyY2ggZWl0aGVyIGluZGV4T2YgKGlmIGRpciBpcyB0cnVlKSBvciBsYXN0SW5kZXhPZlxuICBpZiAoQnVmZmVyLmlzQnVmZmVyKHZhbCkpIHtcbiAgICAvLyBTcGVjaWFsIGNhc2U6IGxvb2tpbmcgZm9yIGVtcHR5IHN0cmluZy9idWZmZXIgYWx3YXlzIGZhaWxzXG4gICAgaWYgKHZhbC5sZW5ndGggPT09IDApIHtcbiAgICAgIHJldHVybiAtMVxuICAgIH1cbiAgICByZXR1cm4gYXJyYXlJbmRleE9mKGJ1ZmZlciwgdmFsLCBieXRlT2Zmc2V0LCBlbmNvZGluZywgZGlyKVxuICB9IGVsc2UgaWYgKHR5cGVvZiB2YWwgPT09ICdudW1iZXInKSB7XG4gICAgdmFsID0gdmFsICYgMHhGRiAvLyBTZWFyY2ggZm9yIGEgYnl0ZSB2YWx1ZSBbMC0yNTVdXG4gICAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUICYmXG4gICAgICAgIHR5cGVvZiBVaW50OEFycmF5LnByb3RvdHlwZS5pbmRleE9mID09PSAnZnVuY3Rpb24nKSB7XG4gICAgICBpZiAoZGlyKSB7XG4gICAgICAgIHJldHVybiBVaW50OEFycmF5LnByb3RvdHlwZS5pbmRleE9mLmNhbGwoYnVmZmVyLCB2YWwsIGJ5dGVPZmZzZXQpXG4gICAgICB9IGVsc2Uge1xuICAgICAgICByZXR1cm4gVWludDhBcnJheS5wcm90b3R5cGUubGFzdEluZGV4T2YuY2FsbChidWZmZXIsIHZhbCwgYnl0ZU9mZnNldClcbiAgICAgIH1cbiAgICB9XG4gICAgcmV0dXJuIGFycmF5SW5kZXhPZihidWZmZXIsIFsgdmFsIF0sIGJ5dGVPZmZzZXQsIGVuY29kaW5nLCBkaXIpXG4gIH1cblxuICB0aHJvdyBuZXcgVHlwZUVycm9yKCd2YWwgbXVzdCBiZSBzdHJpbmcsIG51bWJlciBvciBCdWZmZXInKVxufVxuXG5mdW5jdGlvbiBhcnJheUluZGV4T2YgKGFyciwgdmFsLCBieXRlT2Zmc2V0LCBlbmNvZGluZywgZGlyKSB7XG4gIHZhciBpbmRleFNpemUgPSAxXG4gIHZhciBhcnJMZW5ndGggPSBhcnIubGVuZ3RoXG4gIHZhciB2YWxMZW5ndGggPSB2YWwubGVuZ3RoXG5cbiAgaWYgKGVuY29kaW5nICE9PSB1bmRlZmluZWQpIHtcbiAgICBlbmNvZGluZyA9IFN0cmluZyhlbmNvZGluZykudG9Mb3dlckNhc2UoKVxuICAgIGlmIChlbmNvZGluZyA9PT0gJ3VjczInIHx8IGVuY29kaW5nID09PSAndWNzLTInIHx8XG4gICAgICAgIGVuY29kaW5nID09PSAndXRmMTZsZScgfHwgZW5jb2RpbmcgPT09ICd1dGYtMTZsZScpIHtcbiAgICAgIGlmIChhcnIubGVuZ3RoIDwgMiB8fCB2YWwubGVuZ3RoIDwgMikge1xuICAgICAgICByZXR1cm4gLTFcbiAgICAgIH1cbiAgICAgIGluZGV4U2l6ZSA9IDJcbiAgICAgIGFyckxlbmd0aCAvPSAyXG4gICAgICB2YWxMZW5ndGggLz0gMlxuICAgICAgYnl0ZU9mZnNldCAvPSAyXG4gICAgfVxuICB9XG5cbiAgZnVuY3Rpb24gcmVhZCAoYnVmLCBpKSB7XG4gICAgaWYgKGluZGV4U2l6ZSA9PT0gMSkge1xuICAgICAgcmV0dXJuIGJ1ZltpXVxuICAgIH0gZWxzZSB7XG4gICAgICByZXR1cm4gYnVmLnJlYWRVSW50MTZCRShpICogaW5kZXhTaXplKVxuICAgIH1cbiAgfVxuXG4gIHZhciBpXG4gIGlmIChkaXIpIHtcbiAgICB2YXIgZm91bmRJbmRleCA9IC0xXG4gICAgZm9yIChpID0gYnl0ZU9mZnNldDsgaSA8IGFyckxlbmd0aDsgaSsrKSB7XG4gICAgICBpZiAocmVhZChhcnIsIGkpID09PSByZWFkKHZhbCwgZm91bmRJbmRleCA9PT0gLTEgPyAwIDogaSAtIGZvdW5kSW5kZXgpKSB7XG4gICAgICAgIGlmIChmb3VuZEluZGV4ID09PSAtMSkgZm91bmRJbmRleCA9IGlcbiAgICAgICAgaWYgKGkgLSBmb3VuZEluZGV4ICsgMSA9PT0gdmFsTGVuZ3RoKSByZXR1cm4gZm91bmRJbmRleCAqIGluZGV4U2l6ZVxuICAgICAgfSBlbHNlIHtcbiAgICAgICAgaWYgKGZvdW5kSW5kZXggIT09IC0xKSBpIC09IGkgLSBmb3VuZEluZGV4XG4gICAgICAgIGZvdW5kSW5kZXggPSAtMVxuICAgICAgfVxuICAgIH1cbiAgfSBlbHNlIHtcbiAgICBpZiAoYnl0ZU9mZnNldCArIHZhbExlbmd0aCA+IGFyckxlbmd0aCkgYnl0ZU9mZnNldCA9IGFyckxlbmd0aCAtIHZhbExlbmd0aFxuICAgIGZvciAoaSA9IGJ5dGVPZmZzZXQ7IGkgPj0gMDsgaS0tKSB7XG4gICAgICB2YXIgZm91bmQgPSB0cnVlXG4gICAgICBmb3IgKHZhciBqID0gMDsgaiA8IHZhbExlbmd0aDsgaisrKSB7XG4gICAgICAgIGlmIChyZWFkKGFyciwgaSArIGopICE9PSByZWFkKHZhbCwgaikpIHtcbiAgICAgICAgICBmb3VuZCA9IGZhbHNlXG4gICAgICAgICAgYnJlYWtcbiAgICAgICAgfVxuICAgICAgfVxuICAgICAgaWYgKGZvdW5kKSByZXR1cm4gaVxuICAgIH1cbiAgfVxuXG4gIHJldHVybiAtMVxufVxuXG5CdWZmZXIucHJvdG90eXBlLmluY2x1ZGVzID0gZnVuY3Rpb24gaW5jbHVkZXMgKHZhbCwgYnl0ZU9mZnNldCwgZW5jb2RpbmcpIHtcbiAgcmV0dXJuIHRoaXMuaW5kZXhPZih2YWwsIGJ5dGVPZmZzZXQsIGVuY29kaW5nKSAhPT0gLTFcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5pbmRleE9mID0gZnVuY3Rpb24gaW5kZXhPZiAodmFsLCBieXRlT2Zmc2V0LCBlbmNvZGluZykge1xuICByZXR1cm4gYmlkaXJlY3Rpb25hbEluZGV4T2YodGhpcywgdmFsLCBieXRlT2Zmc2V0LCBlbmNvZGluZywgdHJ1ZSlcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5sYXN0SW5kZXhPZiA9IGZ1bmN0aW9uIGxhc3RJbmRleE9mICh2YWwsIGJ5dGVPZmZzZXQsIGVuY29kaW5nKSB7XG4gIHJldHVybiBiaWRpcmVjdGlvbmFsSW5kZXhPZih0aGlzLCB2YWwsIGJ5dGVPZmZzZXQsIGVuY29kaW5nLCBmYWxzZSlcbn1cblxuZnVuY3Rpb24gaGV4V3JpdGUgKGJ1Ziwgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aCkge1xuICBvZmZzZXQgPSBOdW1iZXIob2Zmc2V0KSB8fCAwXG4gIHZhciByZW1haW5pbmcgPSBidWYubGVuZ3RoIC0gb2Zmc2V0XG4gIGlmICghbGVuZ3RoKSB7XG4gICAgbGVuZ3RoID0gcmVtYWluaW5nXG4gIH0gZWxzZSB7XG4gICAgbGVuZ3RoID0gTnVtYmVyKGxlbmd0aClcbiAgICBpZiAobGVuZ3RoID4gcmVtYWluaW5nKSB7XG4gICAgICBsZW5ndGggPSByZW1haW5pbmdcbiAgICB9XG4gIH1cblxuICAvLyBtdXN0IGJlIGFuIGV2ZW4gbnVtYmVyIG9mIGRpZ2l0c1xuICB2YXIgc3RyTGVuID0gc3RyaW5nLmxlbmd0aFxuICBpZiAoc3RyTGVuICUgMiAhPT0gMCkgdGhyb3cgbmV3IFR5cGVFcnJvcignSW52YWxpZCBoZXggc3RyaW5nJylcblxuICBpZiAobGVuZ3RoID4gc3RyTGVuIC8gMikge1xuICAgIGxlbmd0aCA9IHN0ckxlbiAvIDJcbiAgfVxuICBmb3IgKHZhciBpID0gMDsgaSA8IGxlbmd0aDsgKytpKSB7XG4gICAgdmFyIHBhcnNlZCA9IHBhcnNlSW50KHN0cmluZy5zdWJzdHIoaSAqIDIsIDIpLCAxNilcbiAgICBpZiAoaXNOYU4ocGFyc2VkKSkgcmV0dXJuIGlcbiAgICBidWZbb2Zmc2V0ICsgaV0gPSBwYXJzZWRcbiAgfVxuICByZXR1cm4gaVxufVxuXG5mdW5jdGlvbiB1dGY4V3JpdGUgKGJ1Ziwgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aCkge1xuICByZXR1cm4gYmxpdEJ1ZmZlcih1dGY4VG9CeXRlcyhzdHJpbmcsIGJ1Zi5sZW5ndGggLSBvZmZzZXQpLCBidWYsIG9mZnNldCwgbGVuZ3RoKVxufVxuXG5mdW5jdGlvbiBhc2NpaVdyaXRlIChidWYsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpIHtcbiAgcmV0dXJuIGJsaXRCdWZmZXIoYXNjaWlUb0J5dGVzKHN0cmluZyksIGJ1Ziwgb2Zmc2V0LCBsZW5ndGgpXG59XG5cbmZ1bmN0aW9uIGxhdGluMVdyaXRlIChidWYsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpIHtcbiAgcmV0dXJuIGFzY2lpV3JpdGUoYnVmLCBzdHJpbmcsIG9mZnNldCwgbGVuZ3RoKVxufVxuXG5mdW5jdGlvbiBiYXNlNjRXcml0ZSAoYnVmLCBzdHJpbmcsIG9mZnNldCwgbGVuZ3RoKSB7XG4gIHJldHVybiBibGl0QnVmZmVyKGJhc2U2NFRvQnl0ZXMoc3RyaW5nKSwgYnVmLCBvZmZzZXQsIGxlbmd0aClcbn1cblxuZnVuY3Rpb24gdWNzMldyaXRlIChidWYsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpIHtcbiAgcmV0dXJuIGJsaXRCdWZmZXIodXRmMTZsZVRvQnl0ZXMoc3RyaW5nLCBidWYubGVuZ3RoIC0gb2Zmc2V0KSwgYnVmLCBvZmZzZXQsIGxlbmd0aClcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZSA9IGZ1bmN0aW9uIHdyaXRlIChzdHJpbmcsIG9mZnNldCwgbGVuZ3RoLCBlbmNvZGluZykge1xuICAvLyBCdWZmZXIjd3JpdGUoc3RyaW5nKVxuICBpZiAob2Zmc2V0ID09PSB1bmRlZmluZWQpIHtcbiAgICBlbmNvZGluZyA9ICd1dGY4J1xuICAgIGxlbmd0aCA9IHRoaXMubGVuZ3RoXG4gICAgb2Zmc2V0ID0gMFxuICAvLyBCdWZmZXIjd3JpdGUoc3RyaW5nLCBlbmNvZGluZylcbiAgfSBlbHNlIGlmIChsZW5ndGggPT09IHVuZGVmaW5lZCAmJiB0eXBlb2Ygb2Zmc2V0ID09PSAnc3RyaW5nJykge1xuICAgIGVuY29kaW5nID0gb2Zmc2V0XG4gICAgbGVuZ3RoID0gdGhpcy5sZW5ndGhcbiAgICBvZmZzZXQgPSAwXG4gIC8vIEJ1ZmZlciN3cml0ZShzdHJpbmcsIG9mZnNldFssIGxlbmd0aF1bLCBlbmNvZGluZ10pXG4gIH0gZWxzZSBpZiAoaXNGaW5pdGUob2Zmc2V0KSkge1xuICAgIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgICBpZiAoaXNGaW5pdGUobGVuZ3RoKSkge1xuICAgICAgbGVuZ3RoID0gbGVuZ3RoIHwgMFxuICAgICAgaWYgKGVuY29kaW5nID09PSB1bmRlZmluZWQpIGVuY29kaW5nID0gJ3V0ZjgnXG4gICAgfSBlbHNlIHtcbiAgICAgIGVuY29kaW5nID0gbGVuZ3RoXG4gICAgICBsZW5ndGggPSB1bmRlZmluZWRcbiAgICB9XG4gIC8vIGxlZ2FjeSB3cml0ZShzdHJpbmcsIGVuY29kaW5nLCBvZmZzZXQsIGxlbmd0aCkgLSByZW1vdmUgaW4gdjAuMTNcbiAgfSBlbHNlIHtcbiAgICB0aHJvdyBuZXcgRXJyb3IoXG4gICAgICAnQnVmZmVyLndyaXRlKHN0cmluZywgZW5jb2RpbmcsIG9mZnNldFssIGxlbmd0aF0pIGlzIG5vIGxvbmdlciBzdXBwb3J0ZWQnXG4gICAgKVxuICB9XG5cbiAgdmFyIHJlbWFpbmluZyA9IHRoaXMubGVuZ3RoIC0gb2Zmc2V0XG4gIGlmIChsZW5ndGggPT09IHVuZGVmaW5lZCB8fCBsZW5ndGggPiByZW1haW5pbmcpIGxlbmd0aCA9IHJlbWFpbmluZ1xuXG4gIGlmICgoc3RyaW5nLmxlbmd0aCA+IDAgJiYgKGxlbmd0aCA8IDAgfHwgb2Zmc2V0IDwgMCkpIHx8IG9mZnNldCA+IHRoaXMubGVuZ3RoKSB7XG4gICAgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ0F0dGVtcHQgdG8gd3JpdGUgb3V0c2lkZSBidWZmZXIgYm91bmRzJylcbiAgfVxuXG4gIGlmICghZW5jb2RpbmcpIGVuY29kaW5nID0gJ3V0ZjgnXG5cbiAgdmFyIGxvd2VyZWRDYXNlID0gZmFsc2VcbiAgZm9yICg7Oykge1xuICAgIHN3aXRjaCAoZW5jb2RpbmcpIHtcbiAgICAgIGNhc2UgJ2hleCc6XG4gICAgICAgIHJldHVybiBoZXhXcml0ZSh0aGlzLCBzdHJpbmcsIG9mZnNldCwgbGVuZ3RoKVxuXG4gICAgICBjYXNlICd1dGY4JzpcbiAgICAgIGNhc2UgJ3V0Zi04JzpcbiAgICAgICAgcmV0dXJuIHV0ZjhXcml0ZSh0aGlzLCBzdHJpbmcsIG9mZnNldCwgbGVuZ3RoKVxuXG4gICAgICBjYXNlICdhc2NpaSc6XG4gICAgICAgIHJldHVybiBhc2NpaVdyaXRlKHRoaXMsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpXG5cbiAgICAgIGNhc2UgJ2xhdGluMSc6XG4gICAgICBjYXNlICdiaW5hcnknOlxuICAgICAgICByZXR1cm4gbGF0aW4xV3JpdGUodGhpcywgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aClcblxuICAgICAgY2FzZSAnYmFzZTY0JzpcbiAgICAgICAgLy8gV2FybmluZzogbWF4TGVuZ3RoIG5vdCB0YWtlbiBpbnRvIGFjY291bnQgaW4gYmFzZTY0V3JpdGVcbiAgICAgICAgcmV0dXJuIGJhc2U2NFdyaXRlKHRoaXMsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpXG5cbiAgICAgIGNhc2UgJ3VjczInOlxuICAgICAgY2FzZSAndWNzLTInOlxuICAgICAgY2FzZSAndXRmMTZsZSc6XG4gICAgICBjYXNlICd1dGYtMTZsZSc6XG4gICAgICAgIHJldHVybiB1Y3MyV3JpdGUodGhpcywgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aClcblxuICAgICAgZGVmYXVsdDpcbiAgICAgICAgaWYgKGxvd2VyZWRDYXNlKSB0aHJvdyBuZXcgVHlwZUVycm9yKCdVbmtub3duIGVuY29kaW5nOiAnICsgZW5jb2RpbmcpXG4gICAgICAgIGVuY29kaW5nID0gKCcnICsgZW5jb2RpbmcpLnRvTG93ZXJDYXNlKClcbiAgICAgICAgbG93ZXJlZENhc2UgPSB0cnVlXG4gICAgfVxuICB9XG59XG5cbkJ1ZmZlci5wcm90b3R5cGUudG9KU09OID0gZnVuY3Rpb24gdG9KU09OICgpIHtcbiAgcmV0dXJuIHtcbiAgICB0eXBlOiAnQnVmZmVyJyxcbiAgICBkYXRhOiBBcnJheS5wcm90b3R5cGUuc2xpY2UuY2FsbCh0aGlzLl9hcnIgfHwgdGhpcywgMClcbiAgfVxufVxuXG5mdW5jdGlvbiBiYXNlNjRTbGljZSAoYnVmLCBzdGFydCwgZW5kKSB7XG4gIGlmIChzdGFydCA9PT0gMCAmJiBlbmQgPT09IGJ1Zi5sZW5ndGgpIHtcbiAgICByZXR1cm4gYmFzZTY0LmZyb21CeXRlQXJyYXkoYnVmKVxuICB9IGVsc2Uge1xuICAgIHJldHVybiBiYXNlNjQuZnJvbUJ5dGVBcnJheShidWYuc2xpY2Uoc3RhcnQsIGVuZCkpXG4gIH1cbn1cblxuZnVuY3Rpb24gdXRmOFNsaWNlIChidWYsIHN0YXJ0LCBlbmQpIHtcbiAgZW5kID0gTWF0aC5taW4oYnVmLmxlbmd0aCwgZW5kKVxuICB2YXIgcmVzID0gW11cblxuICB2YXIgaSA9IHN0YXJ0XG4gIHdoaWxlIChpIDwgZW5kKSB7XG4gICAgdmFyIGZpcnN0Qnl0ZSA9IGJ1ZltpXVxuICAgIHZhciBjb2RlUG9pbnQgPSBudWxsXG4gICAgdmFyIGJ5dGVzUGVyU2VxdWVuY2UgPSAoZmlyc3RCeXRlID4gMHhFRikgPyA0XG4gICAgICA6IChmaXJzdEJ5dGUgPiAweERGKSA/IDNcbiAgICAgIDogKGZpcnN0Qnl0ZSA+IDB4QkYpID8gMlxuICAgICAgOiAxXG5cbiAgICBpZiAoaSArIGJ5dGVzUGVyU2VxdWVuY2UgPD0gZW5kKSB7XG4gICAgICB2YXIgc2Vjb25kQnl0ZSwgdGhpcmRCeXRlLCBmb3VydGhCeXRlLCB0ZW1wQ29kZVBvaW50XG5cbiAgICAgIHN3aXRjaCAoYnl0ZXNQZXJTZXF1ZW5jZSkge1xuICAgICAgICBjYXNlIDE6XG4gICAgICAgICAgaWYgKGZpcnN0Qnl0ZSA8IDB4ODApIHtcbiAgICAgICAgICAgIGNvZGVQb2ludCA9IGZpcnN0Qnl0ZVxuICAgICAgICAgIH1cbiAgICAgICAgICBicmVha1xuICAgICAgICBjYXNlIDI6XG4gICAgICAgICAgc2Vjb25kQnl0ZSA9IGJ1ZltpICsgMV1cbiAgICAgICAgICBpZiAoKHNlY29uZEJ5dGUgJiAweEMwKSA9PT0gMHg4MCkge1xuICAgICAgICAgICAgdGVtcENvZGVQb2ludCA9IChmaXJzdEJ5dGUgJiAweDFGKSA8PCAweDYgfCAoc2Vjb25kQnl0ZSAmIDB4M0YpXG4gICAgICAgICAgICBpZiAodGVtcENvZGVQb2ludCA+IDB4N0YpIHtcbiAgICAgICAgICAgICAgY29kZVBvaW50ID0gdGVtcENvZGVQb2ludFxuICAgICAgICAgICAgfVxuICAgICAgICAgIH1cbiAgICAgICAgICBicmVha1xuICAgICAgICBjYXNlIDM6XG4gICAgICAgICAgc2Vjb25kQnl0ZSA9IGJ1ZltpICsgMV1cbiAgICAgICAgICB0aGlyZEJ5dGUgPSBidWZbaSArIDJdXG4gICAgICAgICAgaWYgKChzZWNvbmRCeXRlICYgMHhDMCkgPT09IDB4ODAgJiYgKHRoaXJkQnl0ZSAmIDB4QzApID09PSAweDgwKSB7XG4gICAgICAgICAgICB0ZW1wQ29kZVBvaW50ID0gKGZpcnN0Qnl0ZSAmIDB4RikgPDwgMHhDIHwgKHNlY29uZEJ5dGUgJiAweDNGKSA8PCAweDYgfCAodGhpcmRCeXRlICYgMHgzRilcbiAgICAgICAgICAgIGlmICh0ZW1wQ29kZVBvaW50ID4gMHg3RkYgJiYgKHRlbXBDb2RlUG9pbnQgPCAweEQ4MDAgfHwgdGVtcENvZGVQb2ludCA+IDB4REZGRikpIHtcbiAgICAgICAgICAgICAgY29kZVBvaW50ID0gdGVtcENvZGVQb2ludFxuICAgICAgICAgICAgfVxuICAgICAgICAgIH1cbiAgICAgICAgICBicmVha1xuICAgICAgICBjYXNlIDQ6XG4gICAgICAgICAgc2Vjb25kQnl0ZSA9IGJ1ZltpICsgMV1cbiAgICAgICAgICB0aGlyZEJ5dGUgPSBidWZbaSArIDJdXG4gICAgICAgICAgZm91cnRoQnl0ZSA9IGJ1ZltpICsgM11cbiAgICAgICAgICBpZiAoKHNlY29uZEJ5dGUgJiAweEMwKSA9PT0gMHg4MCAmJiAodGhpcmRCeXRlICYgMHhDMCkgPT09IDB4ODAgJiYgKGZvdXJ0aEJ5dGUgJiAweEMwKSA9PT0gMHg4MCkge1xuICAgICAgICAgICAgdGVtcENvZGVQb2ludCA9IChmaXJzdEJ5dGUgJiAweEYpIDw8IDB4MTIgfCAoc2Vjb25kQnl0ZSAmIDB4M0YpIDw8IDB4QyB8ICh0aGlyZEJ5dGUgJiAweDNGKSA8PCAweDYgfCAoZm91cnRoQnl0ZSAmIDB4M0YpXG4gICAgICAgICAgICBpZiAodGVtcENvZGVQb2ludCA+IDB4RkZGRiAmJiB0ZW1wQ29kZVBvaW50IDwgMHgxMTAwMDApIHtcbiAgICAgICAgICAgICAgY29kZVBvaW50ID0gdGVtcENvZGVQb2ludFxuICAgICAgICAgICAgfVxuICAgICAgICAgIH1cbiAgICAgIH1cbiAgICB9XG5cbiAgICBpZiAoY29kZVBvaW50ID09PSBudWxsKSB7XG4gICAgICAvLyB3ZSBkaWQgbm90IGdlbmVyYXRlIGEgdmFsaWQgY29kZVBvaW50IHNvIGluc2VydCBhXG4gICAgICAvLyByZXBsYWNlbWVudCBjaGFyIChVK0ZGRkQpIGFuZCBhZHZhbmNlIG9ubHkgMSBieXRlXG4gICAgICBjb2RlUG9pbnQgPSAweEZGRkRcbiAgICAgIGJ5dGVzUGVyU2VxdWVuY2UgPSAxXG4gICAgfSBlbHNlIGlmIChjb2RlUG9pbnQgPiAweEZGRkYpIHtcbiAgICAgIC8vIGVuY29kZSB0byB1dGYxNiAoc3Vycm9nYXRlIHBhaXIgZGFuY2UpXG4gICAgICBjb2RlUG9pbnQgLT0gMHgxMDAwMFxuICAgICAgcmVzLnB1c2goY29kZVBvaW50ID4+PiAxMCAmIDB4M0ZGIHwgMHhEODAwKVxuICAgICAgY29kZVBvaW50ID0gMHhEQzAwIHwgY29kZVBvaW50ICYgMHgzRkZcbiAgICB9XG5cbiAgICByZXMucHVzaChjb2RlUG9pbnQpXG4gICAgaSArPSBieXRlc1BlclNlcXVlbmNlXG4gIH1cblxuICByZXR1cm4gZGVjb2RlQ29kZVBvaW50c0FycmF5KHJlcylcbn1cblxuLy8gQmFzZWQgb24gaHR0cDovL3N0YWNrb3ZlcmZsb3cuY29tL2EvMjI3NDcyNzIvNjgwNzQyLCB0aGUgYnJvd3NlciB3aXRoXG4vLyB0aGUgbG93ZXN0IGxpbWl0IGlzIENocm9tZSwgd2l0aCAweDEwMDAwIGFyZ3MuXG4vLyBXZSBnbyAxIG1hZ25pdHVkZSBsZXNzLCBmb3Igc2FmZXR5XG52YXIgTUFYX0FSR1VNRU5UU19MRU5HVEggPSAweDEwMDBcblxuZnVuY3Rpb24gZGVjb2RlQ29kZVBvaW50c0FycmF5IChjb2RlUG9pbnRzKSB7XG4gIHZhciBsZW4gPSBjb2RlUG9pbnRzLmxlbmd0aFxuICBpZiAobGVuIDw9IE1BWF9BUkdVTUVOVFNfTEVOR1RIKSB7XG4gICAgcmV0dXJuIFN0cmluZy5mcm9tQ2hhckNvZGUuYXBwbHkoU3RyaW5nLCBjb2RlUG9pbnRzKSAvLyBhdm9pZCBleHRyYSBzbGljZSgpXG4gIH1cblxuICAvLyBEZWNvZGUgaW4gY2h1bmtzIHRvIGF2b2lkIFwiY2FsbCBzdGFjayBzaXplIGV4Y2VlZGVkXCIuXG4gIHZhciByZXMgPSAnJ1xuICB2YXIgaSA9IDBcbiAgd2hpbGUgKGkgPCBsZW4pIHtcbiAgICByZXMgKz0gU3RyaW5nLmZyb21DaGFyQ29kZS5hcHBseShcbiAgICAgIFN0cmluZyxcbiAgICAgIGNvZGVQb2ludHMuc2xpY2UoaSwgaSArPSBNQVhfQVJHVU1FTlRTX0xFTkdUSClcbiAgICApXG4gIH1cbiAgcmV0dXJuIHJlc1xufVxuXG5mdW5jdGlvbiBhc2NpaVNsaWNlIChidWYsIHN0YXJ0LCBlbmQpIHtcbiAgdmFyIHJldCA9ICcnXG4gIGVuZCA9IE1hdGgubWluKGJ1Zi5sZW5ndGgsIGVuZClcblxuICBmb3IgKHZhciBpID0gc3RhcnQ7IGkgPCBlbmQ7ICsraSkge1xuICAgIHJldCArPSBTdHJpbmcuZnJvbUNoYXJDb2RlKGJ1ZltpXSAmIDB4N0YpXG4gIH1cbiAgcmV0dXJuIHJldFxufVxuXG5mdW5jdGlvbiBsYXRpbjFTbGljZSAoYnVmLCBzdGFydCwgZW5kKSB7XG4gIHZhciByZXQgPSAnJ1xuICBlbmQgPSBNYXRoLm1pbihidWYubGVuZ3RoLCBlbmQpXG5cbiAgZm9yICh2YXIgaSA9IHN0YXJ0OyBpIDwgZW5kOyArK2kpIHtcbiAgICByZXQgKz0gU3RyaW5nLmZyb21DaGFyQ29kZShidWZbaV0pXG4gIH1cbiAgcmV0dXJuIHJldFxufVxuXG5mdW5jdGlvbiBoZXhTbGljZSAoYnVmLCBzdGFydCwgZW5kKSB7XG4gIHZhciBsZW4gPSBidWYubGVuZ3RoXG5cbiAgaWYgKCFzdGFydCB8fCBzdGFydCA8IDApIHN0YXJ0ID0gMFxuICBpZiAoIWVuZCB8fCBlbmQgPCAwIHx8IGVuZCA+IGxlbikgZW5kID0gbGVuXG5cbiAgdmFyIG91dCA9ICcnXG4gIGZvciAodmFyIGkgPSBzdGFydDsgaSA8IGVuZDsgKytpKSB7XG4gICAgb3V0ICs9IHRvSGV4KGJ1ZltpXSlcbiAgfVxuICByZXR1cm4gb3V0XG59XG5cbmZ1bmN0aW9uIHV0ZjE2bGVTbGljZSAoYnVmLCBzdGFydCwgZW5kKSB7XG4gIHZhciBieXRlcyA9IGJ1Zi5zbGljZShzdGFydCwgZW5kKVxuICB2YXIgcmVzID0gJydcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBieXRlcy5sZW5ndGg7IGkgKz0gMikge1xuICAgIHJlcyArPSBTdHJpbmcuZnJvbUNoYXJDb2RlKGJ5dGVzW2ldICsgYnl0ZXNbaSArIDFdICogMjU2KVxuICB9XG4gIHJldHVybiByZXNcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5zbGljZSA9IGZ1bmN0aW9uIHNsaWNlIChzdGFydCwgZW5kKSB7XG4gIHZhciBsZW4gPSB0aGlzLmxlbmd0aFxuICBzdGFydCA9IH5+c3RhcnRcbiAgZW5kID0gZW5kID09PSB1bmRlZmluZWQgPyBsZW4gOiB+fmVuZFxuXG4gIGlmIChzdGFydCA8IDApIHtcbiAgICBzdGFydCArPSBsZW5cbiAgICBpZiAoc3RhcnQgPCAwKSBzdGFydCA9IDBcbiAgfSBlbHNlIGlmIChzdGFydCA+IGxlbikge1xuICAgIHN0YXJ0ID0gbGVuXG4gIH1cblxuICBpZiAoZW5kIDwgMCkge1xuICAgIGVuZCArPSBsZW5cbiAgICBpZiAoZW5kIDwgMCkgZW5kID0gMFxuICB9IGVsc2UgaWYgKGVuZCA+IGxlbikge1xuICAgIGVuZCA9IGxlblxuICB9XG5cbiAgaWYgKGVuZCA8IHN0YXJ0KSBlbmQgPSBzdGFydFxuXG4gIHZhciBuZXdCdWZcbiAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgbmV3QnVmID0gdGhpcy5zdWJhcnJheShzdGFydCwgZW5kKVxuICAgIG5ld0J1Zi5fX3Byb3RvX18gPSBCdWZmZXIucHJvdG90eXBlXG4gIH0gZWxzZSB7XG4gICAgdmFyIHNsaWNlTGVuID0gZW5kIC0gc3RhcnRcbiAgICBuZXdCdWYgPSBuZXcgQnVmZmVyKHNsaWNlTGVuLCB1bmRlZmluZWQpXG4gICAgZm9yICh2YXIgaSA9IDA7IGkgPCBzbGljZUxlbjsgKytpKSB7XG4gICAgICBuZXdCdWZbaV0gPSB0aGlzW2kgKyBzdGFydF1cbiAgICB9XG4gIH1cblxuICByZXR1cm4gbmV3QnVmXG59XG5cbi8qXG4gKiBOZWVkIHRvIG1ha2Ugc3VyZSB0aGF0IGJ1ZmZlciBpc24ndCB0cnlpbmcgdG8gd3JpdGUgb3V0IG9mIGJvdW5kcy5cbiAqL1xuZnVuY3Rpb24gY2hlY2tPZmZzZXQgKG9mZnNldCwgZXh0LCBsZW5ndGgpIHtcbiAgaWYgKChvZmZzZXQgJSAxKSAhPT0gMCB8fCBvZmZzZXQgPCAwKSB0aHJvdyBuZXcgUmFuZ2VFcnJvcignb2Zmc2V0IGlzIG5vdCB1aW50JylcbiAgaWYgKG9mZnNldCArIGV4dCA+IGxlbmd0aCkgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ1RyeWluZyB0byBhY2Nlc3MgYmV5b25kIGJ1ZmZlciBsZW5ndGgnKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRVSW50TEUgPSBmdW5jdGlvbiByZWFkVUludExFIChvZmZzZXQsIGJ5dGVMZW5ndGgsIG5vQXNzZXJ0KSB7XG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgYnl0ZUxlbmd0aCA9IGJ5dGVMZW5ndGggfCAwXG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgYnl0ZUxlbmd0aCwgdGhpcy5sZW5ndGgpXG5cbiAgdmFyIHZhbCA9IHRoaXNbb2Zmc2V0XVxuICB2YXIgbXVsID0gMVxuICB2YXIgaSA9IDBcbiAgd2hpbGUgKCsraSA8IGJ5dGVMZW5ndGggJiYgKG11bCAqPSAweDEwMCkpIHtcbiAgICB2YWwgKz0gdGhpc1tvZmZzZXQgKyBpXSAqIG11bFxuICB9XG5cbiAgcmV0dXJuIHZhbFxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRVSW50QkUgPSBmdW5jdGlvbiByZWFkVUludEJFIChvZmZzZXQsIGJ5dGVMZW5ndGgsIG5vQXNzZXJ0KSB7XG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgYnl0ZUxlbmd0aCA9IGJ5dGVMZW5ndGggfCAwXG4gIGlmICghbm9Bc3NlcnQpIHtcbiAgICBjaGVja09mZnNldChvZmZzZXQsIGJ5dGVMZW5ndGgsIHRoaXMubGVuZ3RoKVxuICB9XG5cbiAgdmFyIHZhbCA9IHRoaXNbb2Zmc2V0ICsgLS1ieXRlTGVuZ3RoXVxuICB2YXIgbXVsID0gMVxuICB3aGlsZSAoYnl0ZUxlbmd0aCA+IDAgJiYgKG11bCAqPSAweDEwMCkpIHtcbiAgICB2YWwgKz0gdGhpc1tvZmZzZXQgKyAtLWJ5dGVMZW5ndGhdICogbXVsXG4gIH1cblxuICByZXR1cm4gdmFsXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZFVJbnQ4ID0gZnVuY3Rpb24gcmVhZFVJbnQ4IChvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgMSwgdGhpcy5sZW5ndGgpXG4gIHJldHVybiB0aGlzW29mZnNldF1cbn1cblxuQnVmZmVyLnByb3RvdHlwZS5yZWFkVUludDE2TEUgPSBmdW5jdGlvbiByZWFkVUludDE2TEUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCAyLCB0aGlzLmxlbmd0aClcbiAgcmV0dXJuIHRoaXNbb2Zmc2V0XSB8ICh0aGlzW29mZnNldCArIDFdIDw8IDgpXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZFVJbnQxNkJFID0gZnVuY3Rpb24gcmVhZFVJbnQxNkJFIChvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgMiwgdGhpcy5sZW5ndGgpXG4gIHJldHVybiAodGhpc1tvZmZzZXRdIDw8IDgpIHwgdGhpc1tvZmZzZXQgKyAxXVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRVSW50MzJMRSA9IGZ1bmN0aW9uIHJlYWRVSW50MzJMRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDQsIHRoaXMubGVuZ3RoKVxuXG4gIHJldHVybiAoKHRoaXNbb2Zmc2V0XSkgfFxuICAgICAgKHRoaXNbb2Zmc2V0ICsgMV0gPDwgOCkgfFxuICAgICAgKHRoaXNbb2Zmc2V0ICsgMl0gPDwgMTYpKSArXG4gICAgICAodGhpc1tvZmZzZXQgKyAzXSAqIDB4MTAwMDAwMClcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5yZWFkVUludDMyQkUgPSBmdW5jdGlvbiByZWFkVUludDMyQkUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCA0LCB0aGlzLmxlbmd0aClcblxuICByZXR1cm4gKHRoaXNbb2Zmc2V0XSAqIDB4MTAwMDAwMCkgK1xuICAgICgodGhpc1tvZmZzZXQgKyAxXSA8PCAxNikgfFxuICAgICh0aGlzW29mZnNldCArIDJdIDw8IDgpIHxcbiAgICB0aGlzW29mZnNldCArIDNdKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRJbnRMRSA9IGZ1bmN0aW9uIHJlYWRJbnRMRSAob2Zmc2V0LCBieXRlTGVuZ3RoLCBub0Fzc2VydCkge1xuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGJ5dGVMZW5ndGggPSBieXRlTGVuZ3RoIHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIGJ5dGVMZW5ndGgsIHRoaXMubGVuZ3RoKVxuXG4gIHZhciB2YWwgPSB0aGlzW29mZnNldF1cbiAgdmFyIG11bCA9IDFcbiAgdmFyIGkgPSAwXG4gIHdoaWxlICgrK2kgPCBieXRlTGVuZ3RoICYmIChtdWwgKj0gMHgxMDApKSB7XG4gICAgdmFsICs9IHRoaXNbb2Zmc2V0ICsgaV0gKiBtdWxcbiAgfVxuICBtdWwgKj0gMHg4MFxuXG4gIGlmICh2YWwgPj0gbXVsKSB2YWwgLT0gTWF0aC5wb3coMiwgOCAqIGJ5dGVMZW5ndGgpXG5cbiAgcmV0dXJuIHZhbFxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRJbnRCRSA9IGZ1bmN0aW9uIHJlYWRJbnRCRSAob2Zmc2V0LCBieXRlTGVuZ3RoLCBub0Fzc2VydCkge1xuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGJ5dGVMZW5ndGggPSBieXRlTGVuZ3RoIHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIGJ5dGVMZW5ndGgsIHRoaXMubGVuZ3RoKVxuXG4gIHZhciBpID0gYnl0ZUxlbmd0aFxuICB2YXIgbXVsID0gMVxuICB2YXIgdmFsID0gdGhpc1tvZmZzZXQgKyAtLWldXG4gIHdoaWxlIChpID4gMCAmJiAobXVsICo9IDB4MTAwKSkge1xuICAgIHZhbCArPSB0aGlzW29mZnNldCArIC0taV0gKiBtdWxcbiAgfVxuICBtdWwgKj0gMHg4MFxuXG4gIGlmICh2YWwgPj0gbXVsKSB2YWwgLT0gTWF0aC5wb3coMiwgOCAqIGJ5dGVMZW5ndGgpXG5cbiAgcmV0dXJuIHZhbFxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRJbnQ4ID0gZnVuY3Rpb24gcmVhZEludDggKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCAxLCB0aGlzLmxlbmd0aClcbiAgaWYgKCEodGhpc1tvZmZzZXRdICYgMHg4MCkpIHJldHVybiAodGhpc1tvZmZzZXRdKVxuICByZXR1cm4gKCgweGZmIC0gdGhpc1tvZmZzZXRdICsgMSkgKiAtMSlcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5yZWFkSW50MTZMRSA9IGZ1bmN0aW9uIHJlYWRJbnQxNkxFIChvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgMiwgdGhpcy5sZW5ndGgpXG4gIHZhciB2YWwgPSB0aGlzW29mZnNldF0gfCAodGhpc1tvZmZzZXQgKyAxXSA8PCA4KVxuICByZXR1cm4gKHZhbCAmIDB4ODAwMCkgPyB2YWwgfCAweEZGRkYwMDAwIDogdmFsXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZEludDE2QkUgPSBmdW5jdGlvbiByZWFkSW50MTZCRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDIsIHRoaXMubGVuZ3RoKVxuICB2YXIgdmFsID0gdGhpc1tvZmZzZXQgKyAxXSB8ICh0aGlzW29mZnNldF0gPDwgOClcbiAgcmV0dXJuICh2YWwgJiAweDgwMDApID8gdmFsIHwgMHhGRkZGMDAwMCA6IHZhbFxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRJbnQzMkxFID0gZnVuY3Rpb24gcmVhZEludDMyTEUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCA0LCB0aGlzLmxlbmd0aClcblxuICByZXR1cm4gKHRoaXNbb2Zmc2V0XSkgfFxuICAgICh0aGlzW29mZnNldCArIDFdIDw8IDgpIHxcbiAgICAodGhpc1tvZmZzZXQgKyAyXSA8PCAxNikgfFxuICAgICh0aGlzW29mZnNldCArIDNdIDw8IDI0KVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRJbnQzMkJFID0gZnVuY3Rpb24gcmVhZEludDMyQkUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCA0LCB0aGlzLmxlbmd0aClcblxuICByZXR1cm4gKHRoaXNbb2Zmc2V0XSA8PCAyNCkgfFxuICAgICh0aGlzW29mZnNldCArIDFdIDw8IDE2KSB8XG4gICAgKHRoaXNbb2Zmc2V0ICsgMl0gPDwgOCkgfFxuICAgICh0aGlzW29mZnNldCArIDNdKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRGbG9hdExFID0gZnVuY3Rpb24gcmVhZEZsb2F0TEUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCA0LCB0aGlzLmxlbmd0aClcbiAgcmV0dXJuIGllZWU3NTQucmVhZCh0aGlzLCBvZmZzZXQsIHRydWUsIDIzLCA0KVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRGbG9hdEJFID0gZnVuY3Rpb24gcmVhZEZsb2F0QkUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCA0LCB0aGlzLmxlbmd0aClcbiAgcmV0dXJuIGllZWU3NTQucmVhZCh0aGlzLCBvZmZzZXQsIGZhbHNlLCAyMywgNClcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5yZWFkRG91YmxlTEUgPSBmdW5jdGlvbiByZWFkRG91YmxlTEUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCA4LCB0aGlzLmxlbmd0aClcbiAgcmV0dXJuIGllZWU3NTQucmVhZCh0aGlzLCBvZmZzZXQsIHRydWUsIDUyLCA4KVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWREb3VibGVCRSA9IGZ1bmN0aW9uIHJlYWREb3VibGVCRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDgsIHRoaXMubGVuZ3RoKVxuICByZXR1cm4gaWVlZTc1NC5yZWFkKHRoaXMsIG9mZnNldCwgZmFsc2UsIDUyLCA4KVxufVxuXG5mdW5jdGlvbiBjaGVja0ludCAoYnVmLCB2YWx1ZSwgb2Zmc2V0LCBleHQsIG1heCwgbWluKSB7XG4gIGlmICghQnVmZmVyLmlzQnVmZmVyKGJ1ZikpIHRocm93IG5ldyBUeXBlRXJyb3IoJ1wiYnVmZmVyXCIgYXJndW1lbnQgbXVzdCBiZSBhIEJ1ZmZlciBpbnN0YW5jZScpXG4gIGlmICh2YWx1ZSA+IG1heCB8fCB2YWx1ZSA8IG1pbikgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ1widmFsdWVcIiBhcmd1bWVudCBpcyBvdXQgb2YgYm91bmRzJylcbiAgaWYgKG9mZnNldCArIGV4dCA+IGJ1Zi5sZW5ndGgpIHRocm93IG5ldyBSYW5nZUVycm9yKCdJbmRleCBvdXQgb2YgcmFuZ2UnKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlVUludExFID0gZnVuY3Rpb24gd3JpdGVVSW50TEUgKHZhbHVlLCBvZmZzZXQsIGJ5dGVMZW5ndGgsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgYnl0ZUxlbmd0aCA9IGJ5dGVMZW5ndGggfCAwXG4gIGlmICghbm9Bc3NlcnQpIHtcbiAgICB2YXIgbWF4Qnl0ZXMgPSBNYXRoLnBvdygyLCA4ICogYnl0ZUxlbmd0aCkgLSAxXG4gICAgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgYnl0ZUxlbmd0aCwgbWF4Qnl0ZXMsIDApXG4gIH1cblxuICB2YXIgbXVsID0gMVxuICB2YXIgaSA9IDBcbiAgdGhpc1tvZmZzZXRdID0gdmFsdWUgJiAweEZGXG4gIHdoaWxlICgrK2kgPCBieXRlTGVuZ3RoICYmIChtdWwgKj0gMHgxMDApKSB7XG4gICAgdGhpc1tvZmZzZXQgKyBpXSA9ICh2YWx1ZSAvIG11bCkgJiAweEZGXG4gIH1cblxuICByZXR1cm4gb2Zmc2V0ICsgYnl0ZUxlbmd0aFxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlVUludEJFID0gZnVuY3Rpb24gd3JpdGVVSW50QkUgKHZhbHVlLCBvZmZzZXQsIGJ5dGVMZW5ndGgsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgYnl0ZUxlbmd0aCA9IGJ5dGVMZW5ndGggfCAwXG4gIGlmICghbm9Bc3NlcnQpIHtcbiAgICB2YXIgbWF4Qnl0ZXMgPSBNYXRoLnBvdygyLCA4ICogYnl0ZUxlbmd0aCkgLSAxXG4gICAgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgYnl0ZUxlbmd0aCwgbWF4Qnl0ZXMsIDApXG4gIH1cblxuICB2YXIgaSA9IGJ5dGVMZW5ndGggLSAxXG4gIHZhciBtdWwgPSAxXG4gIHRoaXNbb2Zmc2V0ICsgaV0gPSB2YWx1ZSAmIDB4RkZcbiAgd2hpbGUgKC0taSA+PSAwICYmIChtdWwgKj0gMHgxMDApKSB7XG4gICAgdGhpc1tvZmZzZXQgKyBpXSA9ICh2YWx1ZSAvIG11bCkgJiAweEZGXG4gIH1cblxuICByZXR1cm4gb2Zmc2V0ICsgYnl0ZUxlbmd0aFxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlVUludDggPSBmdW5jdGlvbiB3cml0ZVVJbnQ4ICh2YWx1ZSwgb2Zmc2V0LCBub0Fzc2VydCkge1xuICB2YWx1ZSA9ICt2YWx1ZVxuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrSW50KHRoaXMsIHZhbHVlLCBvZmZzZXQsIDEsIDB4ZmYsIDApXG4gIGlmICghQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHZhbHVlID0gTWF0aC5mbG9vcih2YWx1ZSlcbiAgdGhpc1tvZmZzZXRdID0gKHZhbHVlICYgMHhmZilcbiAgcmV0dXJuIG9mZnNldCArIDFcbn1cblxuZnVuY3Rpb24gb2JqZWN0V3JpdGVVSW50MTYgKGJ1ZiwgdmFsdWUsIG9mZnNldCwgbGl0dGxlRW5kaWFuKSB7XG4gIGlmICh2YWx1ZSA8IDApIHZhbHVlID0gMHhmZmZmICsgdmFsdWUgKyAxXG4gIGZvciAodmFyIGkgPSAwLCBqID0gTWF0aC5taW4oYnVmLmxlbmd0aCAtIG9mZnNldCwgMik7IGkgPCBqOyArK2kpIHtcbiAgICBidWZbb2Zmc2V0ICsgaV0gPSAodmFsdWUgJiAoMHhmZiA8PCAoOCAqIChsaXR0bGVFbmRpYW4gPyBpIDogMSAtIGkpKSkpID4+PlxuICAgICAgKGxpdHRsZUVuZGlhbiA/IGkgOiAxIC0gaSkgKiA4XG4gIH1cbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZVVJbnQxNkxFID0gZnVuY3Rpb24gd3JpdGVVSW50MTZMRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCAyLCAweGZmZmYsIDApXG4gIGlmIChCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkge1xuICAgIHRoaXNbb2Zmc2V0XSA9ICh2YWx1ZSAmIDB4ZmYpXG4gICAgdGhpc1tvZmZzZXQgKyAxXSA9ICh2YWx1ZSA+Pj4gOClcbiAgfSBlbHNlIHtcbiAgICBvYmplY3RXcml0ZVVJbnQxNih0aGlzLCB2YWx1ZSwgb2Zmc2V0LCB0cnVlKVxuICB9XG4gIHJldHVybiBvZmZzZXQgKyAyXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVVSW50MTZCRSA9IGZ1bmN0aW9uIHdyaXRlVUludDE2QkUgKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgMiwgMHhmZmZmLCAwKVxuICBpZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgICB0aGlzW29mZnNldF0gPSAodmFsdWUgPj4+IDgpXG4gICAgdGhpc1tvZmZzZXQgKyAxXSA9ICh2YWx1ZSAmIDB4ZmYpXG4gIH0gZWxzZSB7XG4gICAgb2JqZWN0V3JpdGVVSW50MTYodGhpcywgdmFsdWUsIG9mZnNldCwgZmFsc2UpXG4gIH1cbiAgcmV0dXJuIG9mZnNldCArIDJcbn1cblxuZnVuY3Rpb24gb2JqZWN0V3JpdGVVSW50MzIgKGJ1ZiwgdmFsdWUsIG9mZnNldCwgbGl0dGxlRW5kaWFuKSB7XG4gIGlmICh2YWx1ZSA8IDApIHZhbHVlID0gMHhmZmZmZmZmZiArIHZhbHVlICsgMVxuICBmb3IgKHZhciBpID0gMCwgaiA9IE1hdGgubWluKGJ1Zi5sZW5ndGggLSBvZmZzZXQsIDQpOyBpIDwgajsgKytpKSB7XG4gICAgYnVmW29mZnNldCArIGldID0gKHZhbHVlID4+PiAobGl0dGxlRW5kaWFuID8gaSA6IDMgLSBpKSAqIDgpICYgMHhmZlxuICB9XG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVVSW50MzJMRSA9IGZ1bmN0aW9uIHdyaXRlVUludDMyTEUgKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgNCwgMHhmZmZmZmZmZiwgMClcbiAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgdGhpc1tvZmZzZXQgKyAzXSA9ICh2YWx1ZSA+Pj4gMjQpXG4gICAgdGhpc1tvZmZzZXQgKyAyXSA9ICh2YWx1ZSA+Pj4gMTYpXG4gICAgdGhpc1tvZmZzZXQgKyAxXSA9ICh2YWx1ZSA+Pj4gOClcbiAgICB0aGlzW29mZnNldF0gPSAodmFsdWUgJiAweGZmKVxuICB9IGVsc2Uge1xuICAgIG9iamVjdFdyaXRlVUludDMyKHRoaXMsIHZhbHVlLCBvZmZzZXQsIHRydWUpXG4gIH1cbiAgcmV0dXJuIG9mZnNldCArIDRcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZVVJbnQzMkJFID0gZnVuY3Rpb24gd3JpdGVVSW50MzJCRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCA0LCAweGZmZmZmZmZmLCAwKVxuICBpZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgICB0aGlzW29mZnNldF0gPSAodmFsdWUgPj4+IDI0KVxuICAgIHRoaXNbb2Zmc2V0ICsgMV0gPSAodmFsdWUgPj4+IDE2KVxuICAgIHRoaXNbb2Zmc2V0ICsgMl0gPSAodmFsdWUgPj4+IDgpXG4gICAgdGhpc1tvZmZzZXQgKyAzXSA9ICh2YWx1ZSAmIDB4ZmYpXG4gIH0gZWxzZSB7XG4gICAgb2JqZWN0V3JpdGVVSW50MzIodGhpcywgdmFsdWUsIG9mZnNldCwgZmFsc2UpXG4gIH1cbiAgcmV0dXJuIG9mZnNldCArIDRcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZUludExFID0gZnVuY3Rpb24gd3JpdGVJbnRMRSAodmFsdWUsIG9mZnNldCwgYnl0ZUxlbmd0aCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSB7XG4gICAgdmFyIGxpbWl0ID0gTWF0aC5wb3coMiwgOCAqIGJ5dGVMZW5ndGggLSAxKVxuXG4gICAgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgYnl0ZUxlbmd0aCwgbGltaXQgLSAxLCAtbGltaXQpXG4gIH1cblxuICB2YXIgaSA9IDBcbiAgdmFyIG11bCA9IDFcbiAgdmFyIHN1YiA9IDBcbiAgdGhpc1tvZmZzZXRdID0gdmFsdWUgJiAweEZGXG4gIHdoaWxlICgrK2kgPCBieXRlTGVuZ3RoICYmIChtdWwgKj0gMHgxMDApKSB7XG4gICAgaWYgKHZhbHVlIDwgMCAmJiBzdWIgPT09IDAgJiYgdGhpc1tvZmZzZXQgKyBpIC0gMV0gIT09IDApIHtcbiAgICAgIHN1YiA9IDFcbiAgICB9XG4gICAgdGhpc1tvZmZzZXQgKyBpXSA9ICgodmFsdWUgLyBtdWwpID4+IDApIC0gc3ViICYgMHhGRlxuICB9XG5cbiAgcmV0dXJuIG9mZnNldCArIGJ5dGVMZW5ndGhcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZUludEJFID0gZnVuY3Rpb24gd3JpdGVJbnRCRSAodmFsdWUsIG9mZnNldCwgYnl0ZUxlbmd0aCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSB7XG4gICAgdmFyIGxpbWl0ID0gTWF0aC5wb3coMiwgOCAqIGJ5dGVMZW5ndGggLSAxKVxuXG4gICAgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgYnl0ZUxlbmd0aCwgbGltaXQgLSAxLCAtbGltaXQpXG4gIH1cblxuICB2YXIgaSA9IGJ5dGVMZW5ndGggLSAxXG4gIHZhciBtdWwgPSAxXG4gIHZhciBzdWIgPSAwXG4gIHRoaXNbb2Zmc2V0ICsgaV0gPSB2YWx1ZSAmIDB4RkZcbiAgd2hpbGUgKC0taSA+PSAwICYmIChtdWwgKj0gMHgxMDApKSB7XG4gICAgaWYgKHZhbHVlIDwgMCAmJiBzdWIgPT09IDAgJiYgdGhpc1tvZmZzZXQgKyBpICsgMV0gIT09IDApIHtcbiAgICAgIHN1YiA9IDFcbiAgICB9XG4gICAgdGhpc1tvZmZzZXQgKyBpXSA9ICgodmFsdWUgLyBtdWwpID4+IDApIC0gc3ViICYgMHhGRlxuICB9XG5cbiAgcmV0dXJuIG9mZnNldCArIGJ5dGVMZW5ndGhcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZUludDggPSBmdW5jdGlvbiB3cml0ZUludDggKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgMSwgMHg3ZiwgLTB4ODApXG4gIGlmICghQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHZhbHVlID0gTWF0aC5mbG9vcih2YWx1ZSlcbiAgaWYgKHZhbHVlIDwgMCkgdmFsdWUgPSAweGZmICsgdmFsdWUgKyAxXG4gIHRoaXNbb2Zmc2V0XSA9ICh2YWx1ZSAmIDB4ZmYpXG4gIHJldHVybiBvZmZzZXQgKyAxXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVJbnQxNkxFID0gZnVuY3Rpb24gd3JpdGVJbnQxNkxFICh2YWx1ZSwgb2Zmc2V0LCBub0Fzc2VydCkge1xuICB2YWx1ZSA9ICt2YWx1ZVxuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrSW50KHRoaXMsIHZhbHVlLCBvZmZzZXQsIDIsIDB4N2ZmZiwgLTB4ODAwMClcbiAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgdGhpc1tvZmZzZXRdID0gKHZhbHVlICYgMHhmZilcbiAgICB0aGlzW29mZnNldCArIDFdID0gKHZhbHVlID4+PiA4KVxuICB9IGVsc2Uge1xuICAgIG9iamVjdFdyaXRlVUludDE2KHRoaXMsIHZhbHVlLCBvZmZzZXQsIHRydWUpXG4gIH1cbiAgcmV0dXJuIG9mZnNldCArIDJcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZUludDE2QkUgPSBmdW5jdGlvbiB3cml0ZUludDE2QkUgKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgMiwgMHg3ZmZmLCAtMHg4MDAwKVxuICBpZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgICB0aGlzW29mZnNldF0gPSAodmFsdWUgPj4+IDgpXG4gICAgdGhpc1tvZmZzZXQgKyAxXSA9ICh2YWx1ZSAmIDB4ZmYpXG4gIH0gZWxzZSB7XG4gICAgb2JqZWN0V3JpdGVVSW50MTYodGhpcywgdmFsdWUsIG9mZnNldCwgZmFsc2UpXG4gIH1cbiAgcmV0dXJuIG9mZnNldCArIDJcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZUludDMyTEUgPSBmdW5jdGlvbiB3cml0ZUludDMyTEUgKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgNCwgMHg3ZmZmZmZmZiwgLTB4ODAwMDAwMDApXG4gIGlmIChCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkge1xuICAgIHRoaXNbb2Zmc2V0XSA9ICh2YWx1ZSAmIDB4ZmYpXG4gICAgdGhpc1tvZmZzZXQgKyAxXSA9ICh2YWx1ZSA+Pj4gOClcbiAgICB0aGlzW29mZnNldCArIDJdID0gKHZhbHVlID4+PiAxNilcbiAgICB0aGlzW29mZnNldCArIDNdID0gKHZhbHVlID4+PiAyNClcbiAgfSBlbHNlIHtcbiAgICBvYmplY3RXcml0ZVVJbnQzMih0aGlzLCB2YWx1ZSwgb2Zmc2V0LCB0cnVlKVxuICB9XG4gIHJldHVybiBvZmZzZXQgKyA0XG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVJbnQzMkJFID0gZnVuY3Rpb24gd3JpdGVJbnQzMkJFICh2YWx1ZSwgb2Zmc2V0LCBub0Fzc2VydCkge1xuICB2YWx1ZSA9ICt2YWx1ZVxuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrSW50KHRoaXMsIHZhbHVlLCBvZmZzZXQsIDQsIDB4N2ZmZmZmZmYsIC0weDgwMDAwMDAwKVxuICBpZiAodmFsdWUgPCAwKSB2YWx1ZSA9IDB4ZmZmZmZmZmYgKyB2YWx1ZSArIDFcbiAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgdGhpc1tvZmZzZXRdID0gKHZhbHVlID4+PiAyNClcbiAgICB0aGlzW29mZnNldCArIDFdID0gKHZhbHVlID4+PiAxNilcbiAgICB0aGlzW29mZnNldCArIDJdID0gKHZhbHVlID4+PiA4KVxuICAgIHRoaXNbb2Zmc2V0ICsgM10gPSAodmFsdWUgJiAweGZmKVxuICB9IGVsc2Uge1xuICAgIG9iamVjdFdyaXRlVUludDMyKHRoaXMsIHZhbHVlLCBvZmZzZXQsIGZhbHNlKVxuICB9XG4gIHJldHVybiBvZmZzZXQgKyA0XG59XG5cbmZ1bmN0aW9uIGNoZWNrSUVFRTc1NCAoYnVmLCB2YWx1ZSwgb2Zmc2V0LCBleHQsIG1heCwgbWluKSB7XG4gIGlmIChvZmZzZXQgKyBleHQgPiBidWYubGVuZ3RoKSB0aHJvdyBuZXcgUmFuZ2VFcnJvcignSW5kZXggb3V0IG9mIHJhbmdlJylcbiAgaWYgKG9mZnNldCA8IDApIHRocm93IG5ldyBSYW5nZUVycm9yKCdJbmRleCBvdXQgb2YgcmFuZ2UnKVxufVxuXG5mdW5jdGlvbiB3cml0ZUZsb2F0IChidWYsIHZhbHVlLCBvZmZzZXQsIGxpdHRsZUVuZGlhbiwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkge1xuICAgIGNoZWNrSUVFRTc1NChidWYsIHZhbHVlLCBvZmZzZXQsIDQsIDMuNDAyODIzNDY2Mzg1Mjg4NmUrMzgsIC0zLjQwMjgyMzQ2NjM4NTI4ODZlKzM4KVxuICB9XG4gIGllZWU3NTQud3JpdGUoYnVmLCB2YWx1ZSwgb2Zmc2V0LCBsaXR0bGVFbmRpYW4sIDIzLCA0KVxuICByZXR1cm4gb2Zmc2V0ICsgNFxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlRmxvYXRMRSA9IGZ1bmN0aW9uIHdyaXRlRmxvYXRMRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgcmV0dXJuIHdyaXRlRmxvYXQodGhpcywgdmFsdWUsIG9mZnNldCwgdHJ1ZSwgbm9Bc3NlcnQpXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVGbG9hdEJFID0gZnVuY3Rpb24gd3JpdGVGbG9hdEJFICh2YWx1ZSwgb2Zmc2V0LCBub0Fzc2VydCkge1xuICByZXR1cm4gd3JpdGVGbG9hdCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCBmYWxzZSwgbm9Bc3NlcnQpXG59XG5cbmZ1bmN0aW9uIHdyaXRlRG91YmxlIChidWYsIHZhbHVlLCBvZmZzZXQsIGxpdHRsZUVuZGlhbiwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkge1xuICAgIGNoZWNrSUVFRTc1NChidWYsIHZhbHVlLCBvZmZzZXQsIDgsIDEuNzk3NjkzMTM0ODYyMzE1N0UrMzA4LCAtMS43OTc2OTMxMzQ4NjIzMTU3RSszMDgpXG4gIH1cbiAgaWVlZTc1NC53cml0ZShidWYsIHZhbHVlLCBvZmZzZXQsIGxpdHRsZUVuZGlhbiwgNTIsIDgpXG4gIHJldHVybiBvZmZzZXQgKyA4XG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVEb3VibGVMRSA9IGZ1bmN0aW9uIHdyaXRlRG91YmxlTEUgKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHJldHVybiB3cml0ZURvdWJsZSh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCB0cnVlLCBub0Fzc2VydClcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZURvdWJsZUJFID0gZnVuY3Rpb24gd3JpdGVEb3VibGVCRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgcmV0dXJuIHdyaXRlRG91YmxlKHRoaXMsIHZhbHVlLCBvZmZzZXQsIGZhbHNlLCBub0Fzc2VydClcbn1cblxuLy8gY29weSh0YXJnZXRCdWZmZXIsIHRhcmdldFN0YXJ0PTAsIHNvdXJjZVN0YXJ0PTAsIHNvdXJjZUVuZD1idWZmZXIubGVuZ3RoKVxuQnVmZmVyLnByb3RvdHlwZS5jb3B5ID0gZnVuY3Rpb24gY29weSAodGFyZ2V0LCB0YXJnZXRTdGFydCwgc3RhcnQsIGVuZCkge1xuICBpZiAoIXN0YXJ0KSBzdGFydCA9IDBcbiAgaWYgKCFlbmQgJiYgZW5kICE9PSAwKSBlbmQgPSB0aGlzLmxlbmd0aFxuICBpZiAodGFyZ2V0U3RhcnQgPj0gdGFyZ2V0Lmxlbmd0aCkgdGFyZ2V0U3RhcnQgPSB0YXJnZXQubGVuZ3RoXG4gIGlmICghdGFyZ2V0U3RhcnQpIHRhcmdldFN0YXJ0ID0gMFxuICBpZiAoZW5kID4gMCAmJiBlbmQgPCBzdGFydCkgZW5kID0gc3RhcnRcblxuICAvLyBDb3B5IDAgYnl0ZXM7IHdlJ3JlIGRvbmVcbiAgaWYgKGVuZCA9PT0gc3RhcnQpIHJldHVybiAwXG4gIGlmICh0YXJnZXQubGVuZ3RoID09PSAwIHx8IHRoaXMubGVuZ3RoID09PSAwKSByZXR1cm4gMFxuXG4gIC8vIEZhdGFsIGVycm9yIGNvbmRpdGlvbnNcbiAgaWYgKHRhcmdldFN0YXJ0IDwgMCkge1xuICAgIHRocm93IG5ldyBSYW5nZUVycm9yKCd0YXJnZXRTdGFydCBvdXQgb2YgYm91bmRzJylcbiAgfVxuICBpZiAoc3RhcnQgPCAwIHx8IHN0YXJ0ID49IHRoaXMubGVuZ3RoKSB0aHJvdyBuZXcgUmFuZ2VFcnJvcignc291cmNlU3RhcnQgb3V0IG9mIGJvdW5kcycpXG4gIGlmIChlbmQgPCAwKSB0aHJvdyBuZXcgUmFuZ2VFcnJvcignc291cmNlRW5kIG91dCBvZiBib3VuZHMnKVxuXG4gIC8vIEFyZSB3ZSBvb2I/XG4gIGlmIChlbmQgPiB0aGlzLmxlbmd0aCkgZW5kID0gdGhpcy5sZW5ndGhcbiAgaWYgKHRhcmdldC5sZW5ndGggLSB0YXJnZXRTdGFydCA8IGVuZCAtIHN0YXJ0KSB7XG4gICAgZW5kID0gdGFyZ2V0Lmxlbmd0aCAtIHRhcmdldFN0YXJ0ICsgc3RhcnRcbiAgfVxuXG4gIHZhciBsZW4gPSBlbmQgLSBzdGFydFxuICB2YXIgaVxuXG4gIGlmICh0aGlzID09PSB0YXJnZXQgJiYgc3RhcnQgPCB0YXJnZXRTdGFydCAmJiB0YXJnZXRTdGFydCA8IGVuZCkge1xuICAgIC8vIGRlc2NlbmRpbmcgY29weSBmcm9tIGVuZFxuICAgIGZvciAoaSA9IGxlbiAtIDE7IGkgPj0gMDsgLS1pKSB7XG4gICAgICB0YXJnZXRbaSArIHRhcmdldFN0YXJ0XSA9IHRoaXNbaSArIHN0YXJ0XVxuICAgIH1cbiAgfSBlbHNlIGlmIChsZW4gPCAxMDAwIHx8ICFCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkge1xuICAgIC8vIGFzY2VuZGluZyBjb3B5IGZyb20gc3RhcnRcbiAgICBmb3IgKGkgPSAwOyBpIDwgbGVuOyArK2kpIHtcbiAgICAgIHRhcmdldFtpICsgdGFyZ2V0U3RhcnRdID0gdGhpc1tpICsgc3RhcnRdXG4gICAgfVxuICB9IGVsc2Uge1xuICAgIFVpbnQ4QXJyYXkucHJvdG90eXBlLnNldC5jYWxsKFxuICAgICAgdGFyZ2V0LFxuICAgICAgdGhpcy5zdWJhcnJheShzdGFydCwgc3RhcnQgKyBsZW4pLFxuICAgICAgdGFyZ2V0U3RhcnRcbiAgICApXG4gIH1cblxuICByZXR1cm4gbGVuXG59XG5cbi8vIFVzYWdlOlxuLy8gICAgYnVmZmVyLmZpbGwobnVtYmVyWywgb2Zmc2V0WywgZW5kXV0pXG4vLyAgICBidWZmZXIuZmlsbChidWZmZXJbLCBvZmZzZXRbLCBlbmRdXSlcbi8vICAgIGJ1ZmZlci5maWxsKHN0cmluZ1ssIG9mZnNldFssIGVuZF1dWywgZW5jb2RpbmddKVxuQnVmZmVyLnByb3RvdHlwZS5maWxsID0gZnVuY3Rpb24gZmlsbCAodmFsLCBzdGFydCwgZW5kLCBlbmNvZGluZykge1xuICAvLyBIYW5kbGUgc3RyaW5nIGNhc2VzOlxuICBpZiAodHlwZW9mIHZhbCA9PT0gJ3N0cmluZycpIHtcbiAgICBpZiAodHlwZW9mIHN0YXJ0ID09PSAnc3RyaW5nJykge1xuICAgICAgZW5jb2RpbmcgPSBzdGFydFxuICAgICAgc3RhcnQgPSAwXG4gICAgICBlbmQgPSB0aGlzLmxlbmd0aFxuICAgIH0gZWxzZSBpZiAodHlwZW9mIGVuZCA9PT0gJ3N0cmluZycpIHtcbiAgICAgIGVuY29kaW5nID0gZW5kXG4gICAgICBlbmQgPSB0aGlzLmxlbmd0aFxuICAgIH1cbiAgICBpZiAodmFsLmxlbmd0aCA9PT0gMSkge1xuICAgICAgdmFyIGNvZGUgPSB2YWwuY2hhckNvZGVBdCgwKVxuICAgICAgaWYgKGNvZGUgPCAyNTYpIHtcbiAgICAgICAgdmFsID0gY29kZVxuICAgICAgfVxuICAgIH1cbiAgICBpZiAoZW5jb2RpbmcgIT09IHVuZGVmaW5lZCAmJiB0eXBlb2YgZW5jb2RpbmcgIT09ICdzdHJpbmcnKSB7XG4gICAgICB0aHJvdyBuZXcgVHlwZUVycm9yKCdlbmNvZGluZyBtdXN0IGJlIGEgc3RyaW5nJylcbiAgICB9XG4gICAgaWYgKHR5cGVvZiBlbmNvZGluZyA9PT0gJ3N0cmluZycgJiYgIUJ1ZmZlci5pc0VuY29kaW5nKGVuY29kaW5nKSkge1xuICAgICAgdGhyb3cgbmV3IFR5cGVFcnJvcignVW5rbm93biBlbmNvZGluZzogJyArIGVuY29kaW5nKVxuICAgIH1cbiAgfSBlbHNlIGlmICh0eXBlb2YgdmFsID09PSAnbnVtYmVyJykge1xuICAgIHZhbCA9IHZhbCAmIDI1NVxuICB9XG5cbiAgLy8gSW52YWxpZCByYW5nZXMgYXJlIG5vdCBzZXQgdG8gYSBkZWZhdWx0LCBzbyBjYW4gcmFuZ2UgY2hlY2sgZWFybHkuXG4gIGlmIChzdGFydCA8IDAgfHwgdGhpcy5sZW5ndGggPCBzdGFydCB8fCB0aGlzLmxlbmd0aCA8IGVuZCkge1xuICAgIHRocm93IG5ldyBSYW5nZUVycm9yKCdPdXQgb2YgcmFuZ2UgaW5kZXgnKVxuICB9XG5cbiAgaWYgKGVuZCA8PSBzdGFydCkge1xuICAgIHJldHVybiB0aGlzXG4gIH1cblxuICBzdGFydCA9IHN0YXJ0ID4+PiAwXG4gIGVuZCA9IGVuZCA9PT0gdW5kZWZpbmVkID8gdGhpcy5sZW5ndGggOiBlbmQgPj4+IDBcblxuICBpZiAoIXZhbCkgdmFsID0gMFxuXG4gIHZhciBpXG4gIGlmICh0eXBlb2YgdmFsID09PSAnbnVtYmVyJykge1xuICAgIGZvciAoaSA9IHN0YXJ0OyBpIDwgZW5kOyArK2kpIHtcbiAgICAgIHRoaXNbaV0gPSB2YWxcbiAgICB9XG4gIH0gZWxzZSB7XG4gICAgdmFyIGJ5dGVzID0gQnVmZmVyLmlzQnVmZmVyKHZhbClcbiAgICAgID8gdmFsXG4gICAgICA6IHV0ZjhUb0J5dGVzKG5ldyBCdWZmZXIodmFsLCBlbmNvZGluZykudG9TdHJpbmcoKSlcbiAgICB2YXIgbGVuID0gYnl0ZXMubGVuZ3RoXG4gICAgZm9yIChpID0gMDsgaSA8IGVuZCAtIHN0YXJ0OyArK2kpIHtcbiAgICAgIHRoaXNbaSArIHN0YXJ0XSA9IGJ5dGVzW2kgJSBsZW5dXG4gICAgfVxuICB9XG5cbiAgcmV0dXJuIHRoaXNcbn1cblxuLy8gSEVMUEVSIEZVTkNUSU9OU1xuLy8gPT09PT09PT09PT09PT09PVxuXG52YXIgSU5WQUxJRF9CQVNFNjRfUkUgPSAvW14rXFwvMC05QS1aYS16LV9dL2dcblxuZnVuY3Rpb24gYmFzZTY0Y2xlYW4gKHN0cikge1xuICAvLyBOb2RlIHN0cmlwcyBvdXQgaW52YWxpZCBjaGFyYWN0ZXJzIGxpa2UgXFxuIGFuZCBcXHQgZnJvbSB0aGUgc3RyaW5nLCBiYXNlNjQtanMgZG9lcyBub3RcbiAgc3RyID0gc3RyaW5ndHJpbShzdHIpLnJlcGxhY2UoSU5WQUxJRF9CQVNFNjRfUkUsICcnKVxuICAvLyBOb2RlIGNvbnZlcnRzIHN0cmluZ3Mgd2l0aCBsZW5ndGggPCAyIHRvICcnXG4gIGlmIChzdHIubGVuZ3RoIDwgMikgcmV0dXJuICcnXG4gIC8vIE5vZGUgYWxsb3dzIGZvciBub24tcGFkZGVkIGJhc2U2NCBzdHJpbmdzIChtaXNzaW5nIHRyYWlsaW5nID09PSksIGJhc2U2NC1qcyBkb2VzIG5vdFxuICB3aGlsZSAoc3RyLmxlbmd0aCAlIDQgIT09IDApIHtcbiAgICBzdHIgPSBzdHIgKyAnPSdcbiAgfVxuICByZXR1cm4gc3RyXG59XG5cbmZ1bmN0aW9uIHN0cmluZ3RyaW0gKHN0cikge1xuICBpZiAoc3RyLnRyaW0pIHJldHVybiBzdHIudHJpbSgpXG4gIHJldHVybiBzdHIucmVwbGFjZSgvXlxccyt8XFxzKyQvZywgJycpXG59XG5cbmZ1bmN0aW9uIHRvSGV4IChuKSB7XG4gIGlmIChuIDwgMTYpIHJldHVybiAnMCcgKyBuLnRvU3RyaW5nKDE2KVxuICByZXR1cm4gbi50b1N0cmluZygxNilcbn1cblxuZnVuY3Rpb24gdXRmOFRvQnl0ZXMgKHN0cmluZywgdW5pdHMpIHtcbiAgdW5pdHMgPSB1bml0cyB8fCBJbmZpbml0eVxuICB2YXIgY29kZVBvaW50XG4gIHZhciBsZW5ndGggPSBzdHJpbmcubGVuZ3RoXG4gIHZhciBsZWFkU3Vycm9nYXRlID0gbnVsbFxuICB2YXIgYnl0ZXMgPSBbXVxuXG4gIGZvciAodmFyIGkgPSAwOyBpIDwgbGVuZ3RoOyArK2kpIHtcbiAgICBjb2RlUG9pbnQgPSBzdHJpbmcuY2hhckNvZGVBdChpKVxuXG4gICAgLy8gaXMgc3Vycm9nYXRlIGNvbXBvbmVudFxuICAgIGlmIChjb2RlUG9pbnQgPiAweEQ3RkYgJiYgY29kZVBvaW50IDwgMHhFMDAwKSB7XG4gICAgICAvLyBsYXN0IGNoYXIgd2FzIGEgbGVhZFxuICAgICAgaWYgKCFsZWFkU3Vycm9nYXRlKSB7XG4gICAgICAgIC8vIG5vIGxlYWQgeWV0XG4gICAgICAgIGlmIChjb2RlUG9pbnQgPiAweERCRkYpIHtcbiAgICAgICAgICAvLyB1bmV4cGVjdGVkIHRyYWlsXG4gICAgICAgICAgaWYgKCh1bml0cyAtPSAzKSA+IC0xKSBieXRlcy5wdXNoKDB4RUYsIDB4QkYsIDB4QkQpXG4gICAgICAgICAgY29udGludWVcbiAgICAgICAgfSBlbHNlIGlmIChpICsgMSA9PT0gbGVuZ3RoKSB7XG4gICAgICAgICAgLy8gdW5wYWlyZWQgbGVhZFxuICAgICAgICAgIGlmICgodW5pdHMgLT0gMykgPiAtMSkgYnl0ZXMucHVzaCgweEVGLCAweEJGLCAweEJEKVxuICAgICAgICAgIGNvbnRpbnVlXG4gICAgICAgIH1cblxuICAgICAgICAvLyB2YWxpZCBsZWFkXG4gICAgICAgIGxlYWRTdXJyb2dhdGUgPSBjb2RlUG9pbnRcblxuICAgICAgICBjb250aW51ZVxuICAgICAgfVxuXG4gICAgICAvLyAyIGxlYWRzIGluIGEgcm93XG4gICAgICBpZiAoY29kZVBvaW50IDwgMHhEQzAwKSB7XG4gICAgICAgIGlmICgodW5pdHMgLT0gMykgPiAtMSkgYnl0ZXMucHVzaCgweEVGLCAweEJGLCAweEJEKVxuICAgICAgICBsZWFkU3Vycm9nYXRlID0gY29kZVBvaW50XG4gICAgICAgIGNvbnRpbnVlXG4gICAgICB9XG5cbiAgICAgIC8vIHZhbGlkIHN1cnJvZ2F0ZSBwYWlyXG4gICAgICBjb2RlUG9pbnQgPSAobGVhZFN1cnJvZ2F0ZSAtIDB4RDgwMCA8PCAxMCB8IGNvZGVQb2ludCAtIDB4REMwMCkgKyAweDEwMDAwXG4gICAgfSBlbHNlIGlmIChsZWFkU3Vycm9nYXRlKSB7XG4gICAgICAvLyB2YWxpZCBibXAgY2hhciwgYnV0IGxhc3QgY2hhciB3YXMgYSBsZWFkXG4gICAgICBpZiAoKHVuaXRzIC09IDMpID4gLTEpIGJ5dGVzLnB1c2goMHhFRiwgMHhCRiwgMHhCRClcbiAgICB9XG5cbiAgICBsZWFkU3Vycm9nYXRlID0gbnVsbFxuXG4gICAgLy8gZW5jb2RlIHV0ZjhcbiAgICBpZiAoY29kZVBvaW50IDwgMHg4MCkge1xuICAgICAgaWYgKCh1bml0cyAtPSAxKSA8IDApIGJyZWFrXG4gICAgICBieXRlcy5wdXNoKGNvZGVQb2ludClcbiAgICB9IGVsc2UgaWYgKGNvZGVQb2ludCA8IDB4ODAwKSB7XG4gICAgICBpZiAoKHVuaXRzIC09IDIpIDwgMCkgYnJlYWtcbiAgICAgIGJ5dGVzLnB1c2goXG4gICAgICAgIGNvZGVQb2ludCA+PiAweDYgfCAweEMwLFxuICAgICAgICBjb2RlUG9pbnQgJiAweDNGIHwgMHg4MFxuICAgICAgKVxuICAgIH0gZWxzZSBpZiAoY29kZVBvaW50IDwgMHgxMDAwMCkge1xuICAgICAgaWYgKCh1bml0cyAtPSAzKSA8IDApIGJyZWFrXG4gICAgICBieXRlcy5wdXNoKFxuICAgICAgICBjb2RlUG9pbnQgPj4gMHhDIHwgMHhFMCxcbiAgICAgICAgY29kZVBvaW50ID4+IDB4NiAmIDB4M0YgfCAweDgwLFxuICAgICAgICBjb2RlUG9pbnQgJiAweDNGIHwgMHg4MFxuICAgICAgKVxuICAgIH0gZWxzZSBpZiAoY29kZVBvaW50IDwgMHgxMTAwMDApIHtcbiAgICAgIGlmICgodW5pdHMgLT0gNCkgPCAwKSBicmVha1xuICAgICAgYnl0ZXMucHVzaChcbiAgICAgICAgY29kZVBvaW50ID4+IDB4MTIgfCAweEYwLFxuICAgICAgICBjb2RlUG9pbnQgPj4gMHhDICYgMHgzRiB8IDB4ODAsXG4gICAgICAgIGNvZGVQb2ludCA+PiAweDYgJiAweDNGIHwgMHg4MCxcbiAgICAgICAgY29kZVBvaW50ICYgMHgzRiB8IDB4ODBcbiAgICAgIClcbiAgICB9IGVsc2Uge1xuICAgICAgdGhyb3cgbmV3IEVycm9yKCdJbnZhbGlkIGNvZGUgcG9pbnQnKVxuICAgIH1cbiAgfVxuXG4gIHJldHVybiBieXRlc1xufVxuXG5mdW5jdGlvbiBhc2NpaVRvQnl0ZXMgKHN0cikge1xuICB2YXIgYnl0ZUFycmF5ID0gW11cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBzdHIubGVuZ3RoOyArK2kpIHtcbiAgICAvLyBOb2RlJ3MgY29kZSBzZWVtcyB0byBiZSBkb2luZyB0aGlzIGFuZCBub3QgJiAweDdGLi5cbiAgICBieXRlQXJyYXkucHVzaChzdHIuY2hhckNvZGVBdChpKSAmIDB4RkYpXG4gIH1cbiAgcmV0dXJuIGJ5dGVBcnJheVxufVxuXG5mdW5jdGlvbiB1dGYxNmxlVG9CeXRlcyAoc3RyLCB1bml0cykge1xuICB2YXIgYywgaGksIGxvXG4gIHZhciBieXRlQXJyYXkgPSBbXVxuICBmb3IgKHZhciBpID0gMDsgaSA8IHN0ci5sZW5ndGg7ICsraSkge1xuICAgIGlmICgodW5pdHMgLT0gMikgPCAwKSBicmVha1xuXG4gICAgYyA9IHN0ci5jaGFyQ29kZUF0KGkpXG4gICAgaGkgPSBjID4+IDhcbiAgICBsbyA9IGMgJSAyNTZcbiAgICBieXRlQXJyYXkucHVzaChsbylcbiAgICBieXRlQXJyYXkucHVzaChoaSlcbiAgfVxuXG4gIHJldHVybiBieXRlQXJyYXlcbn1cblxuZnVuY3Rpb24gYmFzZTY0VG9CeXRlcyAoc3RyKSB7XG4gIHJldHVybiBiYXNlNjQudG9CeXRlQXJyYXkoYmFzZTY0Y2xlYW4oc3RyKSlcbn1cblxuZnVuY3Rpb24gYmxpdEJ1ZmZlciAoc3JjLCBkc3QsIG9mZnNldCwgbGVuZ3RoKSB7XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgbGVuZ3RoOyArK2kpIHtcbiAgICBpZiAoKGkgKyBvZmZzZXQgPj0gZHN0Lmxlbmd0aCkgfHwgKGkgPj0gc3JjLmxlbmd0aCkpIGJyZWFrXG4gICAgZHN0W2kgKyBvZmZzZXRdID0gc3JjW2ldXG4gIH1cbiAgcmV0dXJuIGlcbn1cblxuZnVuY3Rpb24gaXNuYW4gKHZhbCkge1xuICByZXR1cm4gdmFsICE9PSB2YWwgLy8gZXNsaW50LWRpc2FibGUtbGluZSBuby1zZWxmLWNvbXBhcmVcbn1cbiIsImV4cG9ydHMucmVhZCA9IGZ1bmN0aW9uIChidWZmZXIsIG9mZnNldCwgaXNMRSwgbUxlbiwgbkJ5dGVzKSB7XG4gIHZhciBlLCBtXG4gIHZhciBlTGVuID0gbkJ5dGVzICogOCAtIG1MZW4gLSAxXG4gIHZhciBlTWF4ID0gKDEgPDwgZUxlbikgLSAxXG4gIHZhciBlQmlhcyA9IGVNYXggPj4gMVxuICB2YXIgbkJpdHMgPSAtN1xuICB2YXIgaSA9IGlzTEUgPyAobkJ5dGVzIC0gMSkgOiAwXG4gIHZhciBkID0gaXNMRSA/IC0xIDogMVxuICB2YXIgcyA9IGJ1ZmZlcltvZmZzZXQgKyBpXVxuXG4gIGkgKz0gZFxuXG4gIGUgPSBzICYgKCgxIDw8ICgtbkJpdHMpKSAtIDEpXG4gIHMgPj49ICgtbkJpdHMpXG4gIG5CaXRzICs9IGVMZW5cbiAgZm9yICg7IG5CaXRzID4gMDsgZSA9IGUgKiAyNTYgKyBidWZmZXJbb2Zmc2V0ICsgaV0sIGkgKz0gZCwgbkJpdHMgLT0gOCkge31cblxuICBtID0gZSAmICgoMSA8PCAoLW5CaXRzKSkgLSAxKVxuICBlID4+PSAoLW5CaXRzKVxuICBuQml0cyArPSBtTGVuXG4gIGZvciAoOyBuQml0cyA+IDA7IG0gPSBtICogMjU2ICsgYnVmZmVyW29mZnNldCArIGldLCBpICs9IGQsIG5CaXRzIC09IDgpIHt9XG5cbiAgaWYgKGUgPT09IDApIHtcbiAgICBlID0gMSAtIGVCaWFzXG4gIH0gZWxzZSBpZiAoZSA9PT0gZU1heCkge1xuICAgIHJldHVybiBtID8gTmFOIDogKChzID8gLTEgOiAxKSAqIEluZmluaXR5KVxuICB9IGVsc2Uge1xuICAgIG0gPSBtICsgTWF0aC5wb3coMiwgbUxlbilcbiAgICBlID0gZSAtIGVCaWFzXG4gIH1cbiAgcmV0dXJuIChzID8gLTEgOiAxKSAqIG0gKiBNYXRoLnBvdygyLCBlIC0gbUxlbilcbn1cblxuZXhwb3J0cy53cml0ZSA9IGZ1bmN0aW9uIChidWZmZXIsIHZhbHVlLCBvZmZzZXQsIGlzTEUsIG1MZW4sIG5CeXRlcykge1xuICB2YXIgZSwgbSwgY1xuICB2YXIgZUxlbiA9IG5CeXRlcyAqIDggLSBtTGVuIC0gMVxuICB2YXIgZU1heCA9ICgxIDw8IGVMZW4pIC0gMVxuICB2YXIgZUJpYXMgPSBlTWF4ID4+IDFcbiAgdmFyIHJ0ID0gKG1MZW4gPT09IDIzID8gTWF0aC5wb3coMiwgLTI0KSAtIE1hdGgucG93KDIsIC03NykgOiAwKVxuICB2YXIgaSA9IGlzTEUgPyAwIDogKG5CeXRlcyAtIDEpXG4gIHZhciBkID0gaXNMRSA/IDEgOiAtMVxuICB2YXIgcyA9IHZhbHVlIDwgMCB8fCAodmFsdWUgPT09IDAgJiYgMSAvIHZhbHVlIDwgMCkgPyAxIDogMFxuXG4gIHZhbHVlID0gTWF0aC5hYnModmFsdWUpXG5cbiAgaWYgKGlzTmFOKHZhbHVlKSB8fCB2YWx1ZSA9PT0gSW5maW5pdHkpIHtcbiAgICBtID0gaXNOYU4odmFsdWUpID8gMSA6IDBcbiAgICBlID0gZU1heFxuICB9IGVsc2Uge1xuICAgIGUgPSBNYXRoLmZsb29yKE1hdGgubG9nKHZhbHVlKSAvIE1hdGguTE4yKVxuICAgIGlmICh2YWx1ZSAqIChjID0gTWF0aC5wb3coMiwgLWUpKSA8IDEpIHtcbiAgICAgIGUtLVxuICAgICAgYyAqPSAyXG4gICAgfVxuICAgIGlmIChlICsgZUJpYXMgPj0gMSkge1xuICAgICAgdmFsdWUgKz0gcnQgLyBjXG4gICAgfSBlbHNlIHtcbiAgICAgIHZhbHVlICs9IHJ0ICogTWF0aC5wb3coMiwgMSAtIGVCaWFzKVxuICAgIH1cbiAgICBpZiAodmFsdWUgKiBjID49IDIpIHtcbiAgICAgIGUrK1xuICAgICAgYyAvPSAyXG4gICAgfVxuXG4gICAgaWYgKGUgKyBlQmlhcyA+PSBlTWF4KSB7XG4gICAgICBtID0gMFxuICAgICAgZSA9IGVNYXhcbiAgICB9IGVsc2UgaWYgKGUgKyBlQmlhcyA+PSAxKSB7XG4gICAgICBtID0gKHZhbHVlICogYyAtIDEpICogTWF0aC5wb3coMiwgbUxlbilcbiAgICAgIGUgPSBlICsgZUJpYXNcbiAgICB9IGVsc2Uge1xuICAgICAgbSA9IHZhbHVlICogTWF0aC5wb3coMiwgZUJpYXMgLSAxKSAqIE1hdGgucG93KDIsIG1MZW4pXG4gICAgICBlID0gMFxuICAgIH1cbiAgfVxuXG4gIGZvciAoOyBtTGVuID49IDg7IGJ1ZmZlcltvZmZzZXQgKyBpXSA9IG0gJiAweGZmLCBpICs9IGQsIG0gLz0gMjU2LCBtTGVuIC09IDgpIHt9XG5cbiAgZSA9IChlIDw8IG1MZW4pIHwgbVxuICBlTGVuICs9IG1MZW5cbiAgZm9yICg7IGVMZW4gPiAwOyBidWZmZXJbb2Zmc2V0ICsgaV0gPSBlICYgMHhmZiwgaSArPSBkLCBlIC89IDI1NiwgZUxlbiAtPSA4KSB7fVxuXG4gIGJ1ZmZlcltvZmZzZXQgKyBpIC0gZF0gfD0gcyAqIDEyOFxufVxuIiwiaWYgKHR5cGVvZiBPYmplY3QuY3JlYXRlID09PSAnZnVuY3Rpb24nKSB7XG4gIC8vIGltcGxlbWVudGF0aW9uIGZyb20gc3RhbmRhcmQgbm9kZS5qcyAndXRpbCcgbW9kdWxlXG4gIG1vZHVsZS5leHBvcnRzID0gZnVuY3Rpb24gaW5oZXJpdHMoY3Rvciwgc3VwZXJDdG9yKSB7XG4gICAgY3Rvci5zdXBlcl8gPSBzdXBlckN0b3JcbiAgICBjdG9yLnByb3RvdHlwZSA9IE9iamVjdC5jcmVhdGUoc3VwZXJDdG9yLnByb3RvdHlwZSwge1xuICAgICAgY29uc3RydWN0b3I6IHtcbiAgICAgICAgdmFsdWU6IGN0b3IsXG4gICAgICAgIGVudW1lcmFibGU6IGZhbHNlLFxuICAgICAgICB3cml0YWJsZTogdHJ1ZSxcbiAgICAgICAgY29uZmlndXJhYmxlOiB0cnVlXG4gICAgICB9XG4gICAgfSk7XG4gIH07XG59IGVsc2Uge1xuICAvLyBvbGQgc2Nob29sIHNoaW0gZm9yIG9sZCBicm93c2Vyc1xuICBtb2R1bGUuZXhwb3J0cyA9IGZ1bmN0aW9uIGluaGVyaXRzKGN0b3IsIHN1cGVyQ3Rvcikge1xuICAgIGN0b3Iuc3VwZXJfID0gc3VwZXJDdG9yXG4gICAgdmFyIFRlbXBDdG9yID0gZnVuY3Rpb24gKCkge31cbiAgICBUZW1wQ3Rvci5wcm90b3R5cGUgPSBzdXBlckN0b3IucHJvdG90eXBlXG4gICAgY3Rvci5wcm90b3R5cGUgPSBuZXcgVGVtcEN0b3IoKVxuICAgIGN0b3IucHJvdG90eXBlLmNvbnN0cnVjdG9yID0gY3RvclxuICB9XG59XG4iLCJ2YXIgdG9TdHJpbmcgPSB7fS50b1N0cmluZztcblxubW9kdWxlLmV4cG9ydHMgPSBBcnJheS5pc0FycmF5IHx8IGZ1bmN0aW9uIChhcnIpIHtcbiAgcmV0dXJuIHRvU3RyaW5nLmNhbGwoYXJyKSA9PSAnW29iamVjdCBBcnJheV0nO1xufTtcbiIsIi8qXG5vYmplY3QtYXNzaWduXG4oYykgU2luZHJlIFNvcmh1c1xuQGxpY2Vuc2UgTUlUXG4qL1xuXG4ndXNlIHN0cmljdCc7XG4vKiBlc2xpbnQtZGlzYWJsZSBuby11bnVzZWQtdmFycyAqL1xudmFyIGdldE93blByb3BlcnR5U3ltYm9scyA9IE9iamVjdC5nZXRPd25Qcm9wZXJ0eVN5bWJvbHM7XG52YXIgaGFzT3duUHJvcGVydHkgPSBPYmplY3QucHJvdG90eXBlLmhhc093blByb3BlcnR5O1xudmFyIHByb3BJc0VudW1lcmFibGUgPSBPYmplY3QucHJvdG90eXBlLnByb3BlcnR5SXNFbnVtZXJhYmxlO1xuXG5mdW5jdGlvbiB0b09iamVjdCh2YWwpIHtcblx0aWYgKHZhbCA9PT0gbnVsbCB8fCB2YWwgPT09IHVuZGVmaW5lZCkge1xuXHRcdHRocm93IG5ldyBUeXBlRXJyb3IoJ09iamVjdC5hc3NpZ24gY2Fubm90IGJlIGNhbGxlZCB3aXRoIG51bGwgb3IgdW5kZWZpbmVkJyk7XG5cdH1cblxuXHRyZXR1cm4gT2JqZWN0KHZhbCk7XG59XG5cbmZ1bmN0aW9uIHNob3VsZFVzZU5hdGl2ZSgpIHtcblx0dHJ5IHtcblx0XHRpZiAoIU9iamVjdC5hc3NpZ24pIHtcblx0XHRcdHJldHVybiBmYWxzZTtcblx0XHR9XG5cblx0XHQvLyBEZXRlY3QgYnVnZ3kgcHJvcGVydHkgZW51bWVyYXRpb24gb3JkZXIgaW4gb2xkZXIgVjggdmVyc2lvbnMuXG5cblx0XHQvLyBodHRwczovL2J1Z3MuY2hyb21pdW0ub3JnL3AvdjgvaXNzdWVzL2RldGFpbD9pZD00MTE4XG5cdFx0dmFyIHRlc3QxID0gbmV3IFN0cmluZygnYWJjJyk7ICAvLyBlc2xpbnQtZGlzYWJsZS1saW5lIG5vLW5ldy13cmFwcGVyc1xuXHRcdHRlc3QxWzVdID0gJ2RlJztcblx0XHRpZiAoT2JqZWN0LmdldE93blByb3BlcnR5TmFtZXModGVzdDEpWzBdID09PSAnNScpIHtcblx0XHRcdHJldHVybiBmYWxzZTtcblx0XHR9XG5cblx0XHQvLyBodHRwczovL2J1Z3MuY2hyb21pdW0ub3JnL3AvdjgvaXNzdWVzL2RldGFpbD9pZD0zMDU2XG5cdFx0dmFyIHRlc3QyID0ge307XG5cdFx0Zm9yICh2YXIgaSA9IDA7IGkgPCAxMDsgaSsrKSB7XG5cdFx0XHR0ZXN0MlsnXycgKyBTdHJpbmcuZnJvbUNoYXJDb2RlKGkpXSA9IGk7XG5cdFx0fVxuXHRcdHZhciBvcmRlcjIgPSBPYmplY3QuZ2V0T3duUHJvcGVydHlOYW1lcyh0ZXN0MikubWFwKGZ1bmN0aW9uIChuKSB7XG5cdFx0XHRyZXR1cm4gdGVzdDJbbl07XG5cdFx0fSk7XG5cdFx0aWYgKG9yZGVyMi5qb2luKCcnKSAhPT0gJzAxMjM0NTY3ODknKSB7XG5cdFx0XHRyZXR1cm4gZmFsc2U7XG5cdFx0fVxuXG5cdFx0Ly8gaHR0cHM6Ly9idWdzLmNocm9taXVtLm9yZy9wL3Y4L2lzc3Vlcy9kZXRhaWw/aWQ9MzA1NlxuXHRcdHZhciB0ZXN0MyA9IHt9O1xuXHRcdCdhYmNkZWZnaGlqa2xtbm9wcXJzdCcuc3BsaXQoJycpLmZvckVhY2goZnVuY3Rpb24gKGxldHRlcikge1xuXHRcdFx0dGVzdDNbbGV0dGVyXSA9IGxldHRlcjtcblx0XHR9KTtcblx0XHRpZiAoT2JqZWN0LmtleXMoT2JqZWN0LmFzc2lnbih7fSwgdGVzdDMpKS5qb2luKCcnKSAhPT1cblx0XHRcdFx0J2FiY2RlZmdoaWprbG1ub3BxcnN0Jykge1xuXHRcdFx0cmV0dXJuIGZhbHNlO1xuXHRcdH1cblxuXHRcdHJldHVybiB0cnVlO1xuXHR9IGNhdGNoIChlcnIpIHtcblx0XHQvLyBXZSBkb24ndCBleHBlY3QgYW55IG9mIHRoZSBhYm92ZSB0byB0aHJvdywgYnV0IGJldHRlciB0byBiZSBzYWZlLlxuXHRcdHJldHVybiBmYWxzZTtcblx0fVxufVxuXG5tb2R1bGUuZXhwb3J0cyA9IHNob3VsZFVzZU5hdGl2ZSgpID8gT2JqZWN0LmFzc2lnbiA6IGZ1bmN0aW9uICh0YXJnZXQsIHNvdXJjZSkge1xuXHR2YXIgZnJvbTtcblx0dmFyIHRvID0gdG9PYmplY3QodGFyZ2V0KTtcblx0dmFyIHN5bWJvbHM7XG5cblx0Zm9yICh2YXIgcyA9IDE7IHMgPCBhcmd1bWVudHMubGVuZ3RoOyBzKyspIHtcblx0XHRmcm9tID0gT2JqZWN0KGFyZ3VtZW50c1tzXSk7XG5cblx0XHRmb3IgKHZhciBrZXkgaW4gZnJvbSkge1xuXHRcdFx0aWYgKGhhc093blByb3BlcnR5LmNhbGwoZnJvbSwga2V5KSkge1xuXHRcdFx0XHR0b1trZXldID0gZnJvbVtrZXldO1xuXHRcdFx0fVxuXHRcdH1cblxuXHRcdGlmIChnZXRPd25Qcm9wZXJ0eVN5bWJvbHMpIHtcblx0XHRcdHN5bWJvbHMgPSBnZXRPd25Qcm9wZXJ0eVN5bWJvbHMoZnJvbSk7XG5cdFx0XHRmb3IgKHZhciBpID0gMDsgaSA8IHN5bWJvbHMubGVuZ3RoOyBpKyspIHtcblx0XHRcdFx0aWYgKHByb3BJc0VudW1lcmFibGUuY2FsbChmcm9tLCBzeW1ib2xzW2ldKSkge1xuXHRcdFx0XHRcdHRvW3N5bWJvbHNbaV1dID0gZnJvbVtzeW1ib2xzW2ldXTtcblx0XHRcdFx0fVxuXHRcdFx0fVxuXHRcdH1cblx0fVxuXG5cdHJldHVybiB0bztcbn07XG4iLCIvLyBwcm90b3R5cGUgY2xhc3MgZm9yIGhhc2ggZnVuY3Rpb25zXG5mdW5jdGlvbiBIYXNoIChibG9ja1NpemUsIGZpbmFsU2l6ZSkge1xuICB0aGlzLl9ibG9jayA9IG5ldyBCdWZmZXIoYmxvY2tTaXplKVxuICB0aGlzLl9maW5hbFNpemUgPSBmaW5hbFNpemVcbiAgdGhpcy5fYmxvY2tTaXplID0gYmxvY2tTaXplXG4gIHRoaXMuX2xlbiA9IDBcbiAgdGhpcy5fcyA9IDBcbn1cblxuSGFzaC5wcm90b3R5cGUudXBkYXRlID0gZnVuY3Rpb24gKGRhdGEsIGVuYykge1xuICBpZiAodHlwZW9mIGRhdGEgPT09ICdzdHJpbmcnKSB7XG4gICAgZW5jID0gZW5jIHx8ICd1dGY4J1xuICAgIGRhdGEgPSBuZXcgQnVmZmVyKGRhdGEsIGVuYylcbiAgfVxuXG4gIHZhciBsID0gdGhpcy5fbGVuICs9IGRhdGEubGVuZ3RoXG4gIHZhciBzID0gdGhpcy5fcyB8fCAwXG4gIHZhciBmID0gMFxuICB2YXIgYnVmZmVyID0gdGhpcy5fYmxvY2tcblxuICB3aGlsZSAocyA8IGwpIHtcbiAgICB2YXIgdCA9IE1hdGgubWluKGRhdGEubGVuZ3RoLCBmICsgdGhpcy5fYmxvY2tTaXplIC0gKHMgJSB0aGlzLl9ibG9ja1NpemUpKVxuICAgIHZhciBjaCA9ICh0IC0gZilcblxuICAgIGZvciAodmFyIGkgPSAwOyBpIDwgY2g7IGkrKykge1xuICAgICAgYnVmZmVyWyhzICUgdGhpcy5fYmxvY2tTaXplKSArIGldID0gZGF0YVtpICsgZl1cbiAgICB9XG5cbiAgICBzICs9IGNoXG4gICAgZiArPSBjaFxuXG4gICAgaWYgKChzICUgdGhpcy5fYmxvY2tTaXplKSA9PT0gMCkge1xuICAgICAgdGhpcy5fdXBkYXRlKGJ1ZmZlcilcbiAgICB9XG4gIH1cbiAgdGhpcy5fcyA9IHNcblxuICByZXR1cm4gdGhpc1xufVxuXG5IYXNoLnByb3RvdHlwZS5kaWdlc3QgPSBmdW5jdGlvbiAoZW5jKSB7XG4gIC8vIFN1cHBvc2UgdGhlIGxlbmd0aCBvZiB0aGUgbWVzc2FnZSBNLCBpbiBiaXRzLCBpcyBsXG4gIHZhciBsID0gdGhpcy5fbGVuICogOFxuXG4gIC8vIEFwcGVuZCB0aGUgYml0IDEgdG8gdGhlIGVuZCBvZiB0aGUgbWVzc2FnZVxuICB0aGlzLl9ibG9ja1t0aGlzLl9sZW4gJSB0aGlzLl9ibG9ja1NpemVdID0gMHg4MFxuXG4gIC8vIGFuZCB0aGVuIGsgemVybyBiaXRzLCB3aGVyZSBrIGlzIHRoZSBzbWFsbGVzdCBub24tbmVnYXRpdmUgc29sdXRpb24gdG8gdGhlIGVxdWF0aW9uIChsICsgMSArIGspID09PSBmaW5hbFNpemUgbW9kIGJsb2NrU2l6ZVxuICB0aGlzLl9ibG9jay5maWxsKDAsIHRoaXMuX2xlbiAlIHRoaXMuX2Jsb2NrU2l6ZSArIDEpXG5cbiAgaWYgKGwgJSAodGhpcy5fYmxvY2tTaXplICogOCkgPj0gdGhpcy5fZmluYWxTaXplICogOCkge1xuICAgIHRoaXMuX3VwZGF0ZSh0aGlzLl9ibG9jaylcbiAgICB0aGlzLl9ibG9jay5maWxsKDApXG4gIH1cblxuICAvLyB0byB0aGlzIGFwcGVuZCB0aGUgYmxvY2sgd2hpY2ggaXMgZXF1YWwgdG8gdGhlIG51bWJlciBsIHdyaXR0ZW4gaW4gYmluYXJ5XG4gIC8vIFRPRE86IGhhbmRsZSBjYXNlIHdoZXJlIGwgaXMgPiBNYXRoLnBvdygyLCAyOSlcbiAgdGhpcy5fYmxvY2sud3JpdGVJbnQzMkJFKGwsIHRoaXMuX2Jsb2NrU2l6ZSAtIDQpXG5cbiAgdmFyIGhhc2ggPSB0aGlzLl91cGRhdGUodGhpcy5fYmxvY2spIHx8IHRoaXMuX2hhc2goKVxuXG4gIHJldHVybiBlbmMgPyBoYXNoLnRvU3RyaW5nKGVuYykgOiBoYXNoXG59XG5cbkhhc2gucHJvdG90eXBlLl91cGRhdGUgPSBmdW5jdGlvbiAoKSB7XG4gIHRocm93IG5ldyBFcnJvcignX3VwZGF0ZSBtdXN0IGJlIGltcGxlbWVudGVkIGJ5IHN1YmNsYXNzJylcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBIYXNoXG4iLCJ2YXIgZXhwb3J0cyA9IG1vZHVsZS5leHBvcnRzID0gZnVuY3Rpb24gU0hBIChhbGdvcml0aG0pIHtcbiAgYWxnb3JpdGhtID0gYWxnb3JpdGhtLnRvTG93ZXJDYXNlKClcblxuICB2YXIgQWxnb3JpdGhtID0gZXhwb3J0c1thbGdvcml0aG1dXG4gIGlmICghQWxnb3JpdGhtKSB0aHJvdyBuZXcgRXJyb3IoYWxnb3JpdGhtICsgJyBpcyBub3Qgc3VwcG9ydGVkICh3ZSBhY2NlcHQgcHVsbCByZXF1ZXN0cyknKVxuXG4gIHJldHVybiBuZXcgQWxnb3JpdGhtKClcbn1cblxuZXhwb3J0cy5zaGEgPSByZXF1aXJlKCcuL3NoYScpXG5leHBvcnRzLnNoYTEgPSByZXF1aXJlKCcuL3NoYTEnKVxuZXhwb3J0cy5zaGEyMjQgPSByZXF1aXJlKCcuL3NoYTIyNCcpXG5leHBvcnRzLnNoYTI1NiA9IHJlcXVpcmUoJy4vc2hhMjU2JylcbmV4cG9ydHMuc2hhMzg0ID0gcmVxdWlyZSgnLi9zaGEzODQnKVxuZXhwb3J0cy5zaGE1MTIgPSByZXF1aXJlKCcuL3NoYTUxMicpXG4iLCIvKlxuICogQSBKYXZhU2NyaXB0IGltcGxlbWVudGF0aW9uIG9mIHRoZSBTZWN1cmUgSGFzaCBBbGdvcml0aG0sIFNIQS0wLCBhcyBkZWZpbmVkXG4gKiBpbiBGSVBTIFBVQiAxODAtMVxuICogVGhpcyBzb3VyY2UgY29kZSBpcyBkZXJpdmVkIGZyb20gc2hhMS5qcyBvZiB0aGUgc2FtZSByZXBvc2l0b3J5LlxuICogVGhlIGRpZmZlcmVuY2UgYmV0d2VlbiBTSEEtMCBhbmQgU0hBLTEgaXMganVzdCBhIGJpdHdpc2Ugcm90YXRlIGxlZnRcbiAqIG9wZXJhdGlvbiB3YXMgYWRkZWQuXG4gKi9cblxudmFyIGluaGVyaXRzID0gcmVxdWlyZSgnaW5oZXJpdHMnKVxudmFyIEhhc2ggPSByZXF1aXJlKCcuL2hhc2gnKVxuXG52YXIgSyA9IFtcbiAgMHg1YTgyNzk5OSwgMHg2ZWQ5ZWJhMSwgMHg4ZjFiYmNkYyB8IDAsIDB4Y2E2MmMxZDYgfCAwXG5dXG5cbnZhciBXID0gbmV3IEFycmF5KDgwKVxuXG5mdW5jdGlvbiBTaGEgKCkge1xuICB0aGlzLmluaXQoKVxuICB0aGlzLl93ID0gV1xuXG4gIEhhc2guY2FsbCh0aGlzLCA2NCwgNTYpXG59XG5cbmluaGVyaXRzKFNoYSwgSGFzaClcblxuU2hhLnByb3RvdHlwZS5pbml0ID0gZnVuY3Rpb24gKCkge1xuICB0aGlzLl9hID0gMHg2NzQ1MjMwMVxuICB0aGlzLl9iID0gMHhlZmNkYWI4OVxuICB0aGlzLl9jID0gMHg5OGJhZGNmZVxuICB0aGlzLl9kID0gMHgxMDMyNTQ3NlxuICB0aGlzLl9lID0gMHhjM2QyZTFmMFxuXG4gIHJldHVybiB0aGlzXG59XG5cbmZ1bmN0aW9uIHJvdGw1IChudW0pIHtcbiAgcmV0dXJuIChudW0gPDwgNSkgfCAobnVtID4+PiAyNylcbn1cblxuZnVuY3Rpb24gcm90bDMwIChudW0pIHtcbiAgcmV0dXJuIChudW0gPDwgMzApIHwgKG51bSA+Pj4gMilcbn1cblxuZnVuY3Rpb24gZnQgKHMsIGIsIGMsIGQpIHtcbiAgaWYgKHMgPT09IDApIHJldHVybiAoYiAmIGMpIHwgKCh+YikgJiBkKVxuICBpZiAocyA9PT0gMikgcmV0dXJuIChiICYgYykgfCAoYiAmIGQpIHwgKGMgJiBkKVxuICByZXR1cm4gYiBeIGMgXiBkXG59XG5cblNoYS5wcm90b3R5cGUuX3VwZGF0ZSA9IGZ1bmN0aW9uIChNKSB7XG4gIHZhciBXID0gdGhpcy5fd1xuXG4gIHZhciBhID0gdGhpcy5fYSB8IDBcbiAgdmFyIGIgPSB0aGlzLl9iIHwgMFxuICB2YXIgYyA9IHRoaXMuX2MgfCAwXG4gIHZhciBkID0gdGhpcy5fZCB8IDBcbiAgdmFyIGUgPSB0aGlzLl9lIHwgMFxuXG4gIGZvciAodmFyIGkgPSAwOyBpIDwgMTY7ICsraSkgV1tpXSA9IE0ucmVhZEludDMyQkUoaSAqIDQpXG4gIGZvciAoOyBpIDwgODA7ICsraSkgV1tpXSA9IFdbaSAtIDNdIF4gV1tpIC0gOF0gXiBXW2kgLSAxNF0gXiBXW2kgLSAxNl1cblxuICBmb3IgKHZhciBqID0gMDsgaiA8IDgwOyArK2opIHtcbiAgICB2YXIgcyA9IH5+KGogLyAyMClcbiAgICB2YXIgdCA9IChyb3RsNShhKSArIGZ0KHMsIGIsIGMsIGQpICsgZSArIFdbal0gKyBLW3NdKSB8IDBcblxuICAgIGUgPSBkXG4gICAgZCA9IGNcbiAgICBjID0gcm90bDMwKGIpXG4gICAgYiA9IGFcbiAgICBhID0gdFxuICB9XG5cbiAgdGhpcy5fYSA9IChhICsgdGhpcy5fYSkgfCAwXG4gIHRoaXMuX2IgPSAoYiArIHRoaXMuX2IpIHwgMFxuICB0aGlzLl9jID0gKGMgKyB0aGlzLl9jKSB8IDBcbiAgdGhpcy5fZCA9IChkICsgdGhpcy5fZCkgfCAwXG4gIHRoaXMuX2UgPSAoZSArIHRoaXMuX2UpIHwgMFxufVxuXG5TaGEucHJvdG90eXBlLl9oYXNoID0gZnVuY3Rpb24gKCkge1xuICB2YXIgSCA9IG5ldyBCdWZmZXIoMjApXG5cbiAgSC53cml0ZUludDMyQkUodGhpcy5fYSB8IDAsIDApXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2IgfCAwLCA0KVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9jIHwgMCwgOClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fZCB8IDAsIDEyKVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9lIHwgMCwgMTYpXG5cbiAgcmV0dXJuIEhcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBTaGFcbiIsIi8qXG4gKiBBIEphdmFTY3JpcHQgaW1wbGVtZW50YXRpb24gb2YgdGhlIFNlY3VyZSBIYXNoIEFsZ29yaXRobSwgU0hBLTEsIGFzIGRlZmluZWRcbiAqIGluIEZJUFMgUFVCIDE4MC0xXG4gKiBWZXJzaW9uIDIuMWEgQ29weXJpZ2h0IFBhdWwgSm9obnN0b24gMjAwMCAtIDIwMDIuXG4gKiBPdGhlciBjb250cmlidXRvcnM6IEdyZWcgSG9sdCwgQW5kcmV3IEtlcGVydCwgWWRuYXIsIExvc3RpbmV0XG4gKiBEaXN0cmlidXRlZCB1bmRlciB0aGUgQlNEIExpY2Vuc2VcbiAqIFNlZSBodHRwOi8vcGFqaG9tZS5vcmcudWsvY3J5cHQvbWQ1IGZvciBkZXRhaWxzLlxuICovXG5cbnZhciBpbmhlcml0cyA9IHJlcXVpcmUoJ2luaGVyaXRzJylcbnZhciBIYXNoID0gcmVxdWlyZSgnLi9oYXNoJylcblxudmFyIEsgPSBbXG4gIDB4NWE4Mjc5OTksIDB4NmVkOWViYTEsIDB4OGYxYmJjZGMgfCAwLCAweGNhNjJjMWQ2IHwgMFxuXVxuXG52YXIgVyA9IG5ldyBBcnJheSg4MClcblxuZnVuY3Rpb24gU2hhMSAoKSB7XG4gIHRoaXMuaW5pdCgpXG4gIHRoaXMuX3cgPSBXXG5cbiAgSGFzaC5jYWxsKHRoaXMsIDY0LCA1Nilcbn1cblxuaW5oZXJpdHMoU2hhMSwgSGFzaClcblxuU2hhMS5wcm90b3R5cGUuaW5pdCA9IGZ1bmN0aW9uICgpIHtcbiAgdGhpcy5fYSA9IDB4Njc0NTIzMDFcbiAgdGhpcy5fYiA9IDB4ZWZjZGFiODlcbiAgdGhpcy5fYyA9IDB4OThiYWRjZmVcbiAgdGhpcy5fZCA9IDB4MTAzMjU0NzZcbiAgdGhpcy5fZSA9IDB4YzNkMmUxZjBcblxuICByZXR1cm4gdGhpc1xufVxuXG5mdW5jdGlvbiByb3RsMSAobnVtKSB7XG4gIHJldHVybiAobnVtIDw8IDEpIHwgKG51bSA+Pj4gMzEpXG59XG5cbmZ1bmN0aW9uIHJvdGw1IChudW0pIHtcbiAgcmV0dXJuIChudW0gPDwgNSkgfCAobnVtID4+PiAyNylcbn1cblxuZnVuY3Rpb24gcm90bDMwIChudW0pIHtcbiAgcmV0dXJuIChudW0gPDwgMzApIHwgKG51bSA+Pj4gMilcbn1cblxuZnVuY3Rpb24gZnQgKHMsIGIsIGMsIGQpIHtcbiAgaWYgKHMgPT09IDApIHJldHVybiAoYiAmIGMpIHwgKCh+YikgJiBkKVxuICBpZiAocyA9PT0gMikgcmV0dXJuIChiICYgYykgfCAoYiAmIGQpIHwgKGMgJiBkKVxuICByZXR1cm4gYiBeIGMgXiBkXG59XG5cblNoYTEucHJvdG90eXBlLl91cGRhdGUgPSBmdW5jdGlvbiAoTSkge1xuICB2YXIgVyA9IHRoaXMuX3dcblxuICB2YXIgYSA9IHRoaXMuX2EgfCAwXG4gIHZhciBiID0gdGhpcy5fYiB8IDBcbiAgdmFyIGMgPSB0aGlzLl9jIHwgMFxuICB2YXIgZCA9IHRoaXMuX2QgfCAwXG4gIHZhciBlID0gdGhpcy5fZSB8IDBcblxuICBmb3IgKHZhciBpID0gMDsgaSA8IDE2OyArK2kpIFdbaV0gPSBNLnJlYWRJbnQzMkJFKGkgKiA0KVxuICBmb3IgKDsgaSA8IDgwOyArK2kpIFdbaV0gPSByb3RsMShXW2kgLSAzXSBeIFdbaSAtIDhdIF4gV1tpIC0gMTRdIF4gV1tpIC0gMTZdKVxuXG4gIGZvciAodmFyIGogPSAwOyBqIDwgODA7ICsraikge1xuICAgIHZhciBzID0gfn4oaiAvIDIwKVxuICAgIHZhciB0ID0gKHJvdGw1KGEpICsgZnQocywgYiwgYywgZCkgKyBlICsgV1tqXSArIEtbc10pIHwgMFxuXG4gICAgZSA9IGRcbiAgICBkID0gY1xuICAgIGMgPSByb3RsMzAoYilcbiAgICBiID0gYVxuICAgIGEgPSB0XG4gIH1cblxuICB0aGlzLl9hID0gKGEgKyB0aGlzLl9hKSB8IDBcbiAgdGhpcy5fYiA9IChiICsgdGhpcy5fYikgfCAwXG4gIHRoaXMuX2MgPSAoYyArIHRoaXMuX2MpIHwgMFxuICB0aGlzLl9kID0gKGQgKyB0aGlzLl9kKSB8IDBcbiAgdGhpcy5fZSA9IChlICsgdGhpcy5fZSkgfCAwXG59XG5cblNoYTEucHJvdG90eXBlLl9oYXNoID0gZnVuY3Rpb24gKCkge1xuICB2YXIgSCA9IG5ldyBCdWZmZXIoMjApXG5cbiAgSC53cml0ZUludDMyQkUodGhpcy5fYSB8IDAsIDApXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2IgfCAwLCA0KVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9jIHwgMCwgOClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fZCB8IDAsIDEyKVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9lIHwgMCwgMTYpXG5cbiAgcmV0dXJuIEhcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBTaGExXG4iLCIvKipcbiAqIEEgSmF2YVNjcmlwdCBpbXBsZW1lbnRhdGlvbiBvZiB0aGUgU2VjdXJlIEhhc2ggQWxnb3JpdGhtLCBTSEEtMjU2LCBhcyBkZWZpbmVkXG4gKiBpbiBGSVBTIDE4MC0yXG4gKiBWZXJzaW9uIDIuMi1iZXRhIENvcHlyaWdodCBBbmdlbCBNYXJpbiwgUGF1bCBKb2huc3RvbiAyMDAwIC0gMjAwOS5cbiAqIE90aGVyIGNvbnRyaWJ1dG9yczogR3JlZyBIb2x0LCBBbmRyZXcgS2VwZXJ0LCBZZG5hciwgTG9zdGluZXRcbiAqXG4gKi9cblxudmFyIGluaGVyaXRzID0gcmVxdWlyZSgnaW5oZXJpdHMnKVxudmFyIFNoYTI1NiA9IHJlcXVpcmUoJy4vc2hhMjU2JylcbnZhciBIYXNoID0gcmVxdWlyZSgnLi9oYXNoJylcblxudmFyIFcgPSBuZXcgQXJyYXkoNjQpXG5cbmZ1bmN0aW9uIFNoYTIyNCAoKSB7XG4gIHRoaXMuaW5pdCgpXG5cbiAgdGhpcy5fdyA9IFcgLy8gbmV3IEFycmF5KDY0KVxuXG4gIEhhc2guY2FsbCh0aGlzLCA2NCwgNTYpXG59XG5cbmluaGVyaXRzKFNoYTIyNCwgU2hhMjU2KVxuXG5TaGEyMjQucHJvdG90eXBlLmluaXQgPSBmdW5jdGlvbiAoKSB7XG4gIHRoaXMuX2EgPSAweGMxMDU5ZWQ4XG4gIHRoaXMuX2IgPSAweDM2N2NkNTA3XG4gIHRoaXMuX2MgPSAweDMwNzBkZDE3XG4gIHRoaXMuX2QgPSAweGY3MGU1OTM5XG4gIHRoaXMuX2UgPSAweGZmYzAwYjMxXG4gIHRoaXMuX2YgPSAweDY4NTgxNTExXG4gIHRoaXMuX2cgPSAweDY0Zjk4ZmE3XG4gIHRoaXMuX2ggPSAweGJlZmE0ZmE0XG5cbiAgcmV0dXJuIHRoaXNcbn1cblxuU2hhMjI0LnByb3RvdHlwZS5faGFzaCA9IGZ1bmN0aW9uICgpIHtcbiAgdmFyIEggPSBuZXcgQnVmZmVyKDI4KVxuXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2EsIDApXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2IsIDQpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2MsIDgpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2QsIDEyKVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9lLCAxNilcbiAgSC53cml0ZUludDMyQkUodGhpcy5fZiwgMjApXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2csIDI0KVxuXG4gIHJldHVybiBIXG59XG5cbm1vZHVsZS5leHBvcnRzID0gU2hhMjI0XG4iLCIvKipcbiAqIEEgSmF2YVNjcmlwdCBpbXBsZW1lbnRhdGlvbiBvZiB0aGUgU2VjdXJlIEhhc2ggQWxnb3JpdGhtLCBTSEEtMjU2LCBhcyBkZWZpbmVkXG4gKiBpbiBGSVBTIDE4MC0yXG4gKiBWZXJzaW9uIDIuMi1iZXRhIENvcHlyaWdodCBBbmdlbCBNYXJpbiwgUGF1bCBKb2huc3RvbiAyMDAwIC0gMjAwOS5cbiAqIE90aGVyIGNvbnRyaWJ1dG9yczogR3JlZyBIb2x0LCBBbmRyZXcgS2VwZXJ0LCBZZG5hciwgTG9zdGluZXRcbiAqXG4gKi9cblxudmFyIGluaGVyaXRzID0gcmVxdWlyZSgnaW5oZXJpdHMnKVxudmFyIEhhc2ggPSByZXF1aXJlKCcuL2hhc2gnKVxuXG52YXIgSyA9IFtcbiAgMHg0MjhBMkY5OCwgMHg3MTM3NDQ5MSwgMHhCNUMwRkJDRiwgMHhFOUI1REJBNSxcbiAgMHgzOTU2QzI1QiwgMHg1OUYxMTFGMSwgMHg5MjNGODJBNCwgMHhBQjFDNUVENSxcbiAgMHhEODA3QUE5OCwgMHgxMjgzNUIwMSwgMHgyNDMxODVCRSwgMHg1NTBDN0RDMyxcbiAgMHg3MkJFNUQ3NCwgMHg4MERFQjFGRSwgMHg5QkRDMDZBNywgMHhDMTlCRjE3NCxcbiAgMHhFNDlCNjlDMSwgMHhFRkJFNDc4NiwgMHgwRkMxOURDNiwgMHgyNDBDQTFDQyxcbiAgMHgyREU5MkM2RiwgMHg0QTc0ODRBQSwgMHg1Q0IwQTlEQywgMHg3NkY5ODhEQSxcbiAgMHg5ODNFNTE1MiwgMHhBODMxQzY2RCwgMHhCMDAzMjdDOCwgMHhCRjU5N0ZDNyxcbiAgMHhDNkUwMEJGMywgMHhENUE3OTE0NywgMHgwNkNBNjM1MSwgMHgxNDI5Mjk2NyxcbiAgMHgyN0I3MEE4NSwgMHgyRTFCMjEzOCwgMHg0RDJDNkRGQywgMHg1MzM4MEQxMyxcbiAgMHg2NTBBNzM1NCwgMHg3NjZBMEFCQiwgMHg4MUMyQzkyRSwgMHg5MjcyMkM4NSxcbiAgMHhBMkJGRThBMSwgMHhBODFBNjY0QiwgMHhDMjRCOEI3MCwgMHhDNzZDNTFBMyxcbiAgMHhEMTkyRTgxOSwgMHhENjk5MDYyNCwgMHhGNDBFMzU4NSwgMHgxMDZBQTA3MCxcbiAgMHgxOUE0QzExNiwgMHgxRTM3NkMwOCwgMHgyNzQ4Nzc0QywgMHgzNEIwQkNCNSxcbiAgMHgzOTFDMENCMywgMHg0RUQ4QUE0QSwgMHg1QjlDQ0E0RiwgMHg2ODJFNkZGMyxcbiAgMHg3NDhGODJFRSwgMHg3OEE1NjM2RiwgMHg4NEM4NzgxNCwgMHg4Q0M3MDIwOCxcbiAgMHg5MEJFRkZGQSwgMHhBNDUwNkNFQiwgMHhCRUY5QTNGNywgMHhDNjcxNzhGMlxuXVxuXG52YXIgVyA9IG5ldyBBcnJheSg2NClcblxuZnVuY3Rpb24gU2hhMjU2ICgpIHtcbiAgdGhpcy5pbml0KClcblxuICB0aGlzLl93ID0gVyAvLyBuZXcgQXJyYXkoNjQpXG5cbiAgSGFzaC5jYWxsKHRoaXMsIDY0LCA1Nilcbn1cblxuaW5oZXJpdHMoU2hhMjU2LCBIYXNoKVxuXG5TaGEyNTYucHJvdG90eXBlLmluaXQgPSBmdW5jdGlvbiAoKSB7XG4gIHRoaXMuX2EgPSAweDZhMDllNjY3XG4gIHRoaXMuX2IgPSAweGJiNjdhZTg1XG4gIHRoaXMuX2MgPSAweDNjNmVmMzcyXG4gIHRoaXMuX2QgPSAweGE1NGZmNTNhXG4gIHRoaXMuX2UgPSAweDUxMGU1MjdmXG4gIHRoaXMuX2YgPSAweDliMDU2ODhjXG4gIHRoaXMuX2cgPSAweDFmODNkOWFiXG4gIHRoaXMuX2ggPSAweDViZTBjZDE5XG5cbiAgcmV0dXJuIHRoaXNcbn1cblxuZnVuY3Rpb24gY2ggKHgsIHksIHopIHtcbiAgcmV0dXJuIHogXiAoeCAmICh5IF4geikpXG59XG5cbmZ1bmN0aW9uIG1haiAoeCwgeSwgeikge1xuICByZXR1cm4gKHggJiB5KSB8ICh6ICYgKHggfCB5KSlcbn1cblxuZnVuY3Rpb24gc2lnbWEwICh4KSB7XG4gIHJldHVybiAoeCA+Pj4gMiB8IHggPDwgMzApIF4gKHggPj4+IDEzIHwgeCA8PCAxOSkgXiAoeCA+Pj4gMjIgfCB4IDw8IDEwKVxufVxuXG5mdW5jdGlvbiBzaWdtYTEgKHgpIHtcbiAgcmV0dXJuICh4ID4+PiA2IHwgeCA8PCAyNikgXiAoeCA+Pj4gMTEgfCB4IDw8IDIxKSBeICh4ID4+PiAyNSB8IHggPDwgNylcbn1cblxuZnVuY3Rpb24gZ2FtbWEwICh4KSB7XG4gIHJldHVybiAoeCA+Pj4gNyB8IHggPDwgMjUpIF4gKHggPj4+IDE4IHwgeCA8PCAxNCkgXiAoeCA+Pj4gMylcbn1cblxuZnVuY3Rpb24gZ2FtbWExICh4KSB7XG4gIHJldHVybiAoeCA+Pj4gMTcgfCB4IDw8IDE1KSBeICh4ID4+PiAxOSB8IHggPDwgMTMpIF4gKHggPj4+IDEwKVxufVxuXG5TaGEyNTYucHJvdG90eXBlLl91cGRhdGUgPSBmdW5jdGlvbiAoTSkge1xuICB2YXIgVyA9IHRoaXMuX3dcblxuICB2YXIgYSA9IHRoaXMuX2EgfCAwXG4gIHZhciBiID0gdGhpcy5fYiB8IDBcbiAgdmFyIGMgPSB0aGlzLl9jIHwgMFxuICB2YXIgZCA9IHRoaXMuX2QgfCAwXG4gIHZhciBlID0gdGhpcy5fZSB8IDBcbiAgdmFyIGYgPSB0aGlzLl9mIHwgMFxuICB2YXIgZyA9IHRoaXMuX2cgfCAwXG4gIHZhciBoID0gdGhpcy5faCB8IDBcblxuICBmb3IgKHZhciBpID0gMDsgaSA8IDE2OyArK2kpIFdbaV0gPSBNLnJlYWRJbnQzMkJFKGkgKiA0KVxuICBmb3IgKDsgaSA8IDY0OyArK2kpIFdbaV0gPSAoZ2FtbWExKFdbaSAtIDJdKSArIFdbaSAtIDddICsgZ2FtbWEwKFdbaSAtIDE1XSkgKyBXW2kgLSAxNl0pIHwgMFxuXG4gIGZvciAodmFyIGogPSAwOyBqIDwgNjQ7ICsraikge1xuICAgIHZhciBUMSA9IChoICsgc2lnbWExKGUpICsgY2goZSwgZiwgZykgKyBLW2pdICsgV1tqXSkgfCAwXG4gICAgdmFyIFQyID0gKHNpZ21hMChhKSArIG1haihhLCBiLCBjKSkgfCAwXG5cbiAgICBoID0gZ1xuICAgIGcgPSBmXG4gICAgZiA9IGVcbiAgICBlID0gKGQgKyBUMSkgfCAwXG4gICAgZCA9IGNcbiAgICBjID0gYlxuICAgIGIgPSBhXG4gICAgYSA9IChUMSArIFQyKSB8IDBcbiAgfVxuXG4gIHRoaXMuX2EgPSAoYSArIHRoaXMuX2EpIHwgMFxuICB0aGlzLl9iID0gKGIgKyB0aGlzLl9iKSB8IDBcbiAgdGhpcy5fYyA9IChjICsgdGhpcy5fYykgfCAwXG4gIHRoaXMuX2QgPSAoZCArIHRoaXMuX2QpIHwgMFxuICB0aGlzLl9lID0gKGUgKyB0aGlzLl9lKSB8IDBcbiAgdGhpcy5fZiA9IChmICsgdGhpcy5fZikgfCAwXG4gIHRoaXMuX2cgPSAoZyArIHRoaXMuX2cpIHwgMFxuICB0aGlzLl9oID0gKGggKyB0aGlzLl9oKSB8IDBcbn1cblxuU2hhMjU2LnByb3RvdHlwZS5faGFzaCA9IGZ1bmN0aW9uICgpIHtcbiAgdmFyIEggPSBuZXcgQnVmZmVyKDMyKVxuXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2EsIDApXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2IsIDQpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2MsIDgpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2QsIDEyKVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9lLCAxNilcbiAgSC53cml0ZUludDMyQkUodGhpcy5fZiwgMjApXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2csIDI0KVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9oLCAyOClcblxuICByZXR1cm4gSFxufVxuXG5tb2R1bGUuZXhwb3J0cyA9IFNoYTI1NlxuIiwidmFyIGluaGVyaXRzID0gcmVxdWlyZSgnaW5oZXJpdHMnKVxudmFyIFNIQTUxMiA9IHJlcXVpcmUoJy4vc2hhNTEyJylcbnZhciBIYXNoID0gcmVxdWlyZSgnLi9oYXNoJylcblxudmFyIFcgPSBuZXcgQXJyYXkoMTYwKVxuXG5mdW5jdGlvbiBTaGEzODQgKCkge1xuICB0aGlzLmluaXQoKVxuICB0aGlzLl93ID0gV1xuXG4gIEhhc2guY2FsbCh0aGlzLCAxMjgsIDExMilcbn1cblxuaW5oZXJpdHMoU2hhMzg0LCBTSEE1MTIpXG5cblNoYTM4NC5wcm90b3R5cGUuaW5pdCA9IGZ1bmN0aW9uICgpIHtcbiAgdGhpcy5fYWggPSAweGNiYmI5ZDVkXG4gIHRoaXMuX2JoID0gMHg2MjlhMjkyYVxuICB0aGlzLl9jaCA9IDB4OTE1OTAxNWFcbiAgdGhpcy5fZGggPSAweDE1MmZlY2Q4XG4gIHRoaXMuX2VoID0gMHg2NzMzMjY2N1xuICB0aGlzLl9maCA9IDB4OGViNDRhODdcbiAgdGhpcy5fZ2ggPSAweGRiMGMyZTBkXG4gIHRoaXMuX2hoID0gMHg0N2I1NDgxZFxuXG4gIHRoaXMuX2FsID0gMHhjMTA1OWVkOFxuICB0aGlzLl9ibCA9IDB4MzY3Y2Q1MDdcbiAgdGhpcy5fY2wgPSAweDMwNzBkZDE3XG4gIHRoaXMuX2RsID0gMHhmNzBlNTkzOVxuICB0aGlzLl9lbCA9IDB4ZmZjMDBiMzFcbiAgdGhpcy5fZmwgPSAweDY4NTgxNTExXG4gIHRoaXMuX2dsID0gMHg2NGY5OGZhN1xuICB0aGlzLl9obCA9IDB4YmVmYTRmYTRcblxuICByZXR1cm4gdGhpc1xufVxuXG5TaGEzODQucHJvdG90eXBlLl9oYXNoID0gZnVuY3Rpb24gKCkge1xuICB2YXIgSCA9IG5ldyBCdWZmZXIoNDgpXG5cbiAgZnVuY3Rpb24gd3JpdGVJbnQ2NEJFIChoLCBsLCBvZmZzZXQpIHtcbiAgICBILndyaXRlSW50MzJCRShoLCBvZmZzZXQpXG4gICAgSC53cml0ZUludDMyQkUobCwgb2Zmc2V0ICsgNClcbiAgfVxuXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9haCwgdGhpcy5fYWwsIDApXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9iaCwgdGhpcy5fYmwsIDgpXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9jaCwgdGhpcy5fY2wsIDE2KVxuICB3cml0ZUludDY0QkUodGhpcy5fZGgsIHRoaXMuX2RsLCAyNClcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2VoLCB0aGlzLl9lbCwgMzIpXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9maCwgdGhpcy5fZmwsIDQwKVxuXG4gIHJldHVybiBIXG59XG5cbm1vZHVsZS5leHBvcnRzID0gU2hhMzg0XG4iLCJ2YXIgaW5oZXJpdHMgPSByZXF1aXJlKCdpbmhlcml0cycpXG52YXIgSGFzaCA9IHJlcXVpcmUoJy4vaGFzaCcpXG5cbnZhciBLID0gW1xuICAweDQyOGEyZjk4LCAweGQ3MjhhZTIyLCAweDcxMzc0NDkxLCAweDIzZWY2NWNkLFxuICAweGI1YzBmYmNmLCAweGVjNGQzYjJmLCAweGU5YjVkYmE1LCAweDgxODlkYmJjLFxuICAweDM5NTZjMjViLCAweGYzNDhiNTM4LCAweDU5ZjExMWYxLCAweGI2MDVkMDE5LFxuICAweDkyM2Y4MmE0LCAweGFmMTk0ZjliLCAweGFiMWM1ZWQ1LCAweGRhNmQ4MTE4LFxuICAweGQ4MDdhYTk4LCAweGEzMDMwMjQyLCAweDEyODM1YjAxLCAweDQ1NzA2ZmJlLFxuICAweDI0MzE4NWJlLCAweDRlZTRiMjhjLCAweDU1MGM3ZGMzLCAweGQ1ZmZiNGUyLFxuICAweDcyYmU1ZDc0LCAweGYyN2I4OTZmLCAweDgwZGViMWZlLCAweDNiMTY5NmIxLFxuICAweDliZGMwNmE3LCAweDI1YzcxMjM1LCAweGMxOWJmMTc0LCAweGNmNjkyNjk0LFxuICAweGU0OWI2OWMxLCAweDllZjE0YWQyLCAweGVmYmU0Nzg2LCAweDM4NGYyNWUzLFxuICAweDBmYzE5ZGM2LCAweDhiOGNkNWI1LCAweDI0MGNhMWNjLCAweDc3YWM5YzY1LFxuICAweDJkZTkyYzZmLCAweDU5MmIwMjc1LCAweDRhNzQ4NGFhLCAweDZlYTZlNDgzLFxuICAweDVjYjBhOWRjLCAweGJkNDFmYmQ0LCAweDc2Zjk4OGRhLCAweDgzMTE1M2I1LFxuICAweDk4M2U1MTUyLCAweGVlNjZkZmFiLCAweGE4MzFjNjZkLCAweDJkYjQzMjEwLFxuICAweGIwMDMyN2M4LCAweDk4ZmIyMTNmLCAweGJmNTk3ZmM3LCAweGJlZWYwZWU0LFxuICAweGM2ZTAwYmYzLCAweDNkYTg4ZmMyLCAweGQ1YTc5MTQ3LCAweDkzMGFhNzI1LFxuICAweDA2Y2E2MzUxLCAweGUwMDM4MjZmLCAweDE0MjkyOTY3LCAweDBhMGU2ZTcwLFxuICAweDI3YjcwYTg1LCAweDQ2ZDIyZmZjLCAweDJlMWIyMTM4LCAweDVjMjZjOTI2LFxuICAweDRkMmM2ZGZjLCAweDVhYzQyYWVkLCAweDUzMzgwZDEzLCAweDlkOTViM2RmLFxuICAweDY1MGE3MzU0LCAweDhiYWY2M2RlLCAweDc2NmEwYWJiLCAweDNjNzdiMmE4LFxuICAweDgxYzJjOTJlLCAweDQ3ZWRhZWU2LCAweDkyNzIyYzg1LCAweDE0ODIzNTNiLFxuICAweGEyYmZlOGExLCAweDRjZjEwMzY0LCAweGE4MWE2NjRiLCAweGJjNDIzMDAxLFxuICAweGMyNGI4YjcwLCAweGQwZjg5NzkxLCAweGM3NmM1MWEzLCAweDA2NTRiZTMwLFxuICAweGQxOTJlODE5LCAweGQ2ZWY1MjE4LCAweGQ2OTkwNjI0LCAweDU1NjVhOTEwLFxuICAweGY0MGUzNTg1LCAweDU3NzEyMDJhLCAweDEwNmFhMDcwLCAweDMyYmJkMWI4LFxuICAweDE5YTRjMTE2LCAweGI4ZDJkMGM4LCAweDFlMzc2YzA4LCAweDUxNDFhYjUzLFxuICAweDI3NDg3NzRjLCAweGRmOGVlYjk5LCAweDM0YjBiY2I1LCAweGUxOWI0OGE4LFxuICAweDM5MWMwY2IzLCAweGM1Yzk1YTYzLCAweDRlZDhhYTRhLCAweGUzNDE4YWNiLFxuICAweDViOWNjYTRmLCAweDc3NjNlMzczLCAweDY4MmU2ZmYzLCAweGQ2YjJiOGEzLFxuICAweDc0OGY4MmVlLCAweDVkZWZiMmZjLCAweDc4YTU2MzZmLCAweDQzMTcyZjYwLFxuICAweDg0Yzg3ODE0LCAweGExZjBhYjcyLCAweDhjYzcwMjA4LCAweDFhNjQzOWVjLFxuICAweDkwYmVmZmZhLCAweDIzNjMxZTI4LCAweGE0NTA2Y2ViLCAweGRlODJiZGU5LFxuICAweGJlZjlhM2Y3LCAweGIyYzY3OTE1LCAweGM2NzE3OGYyLCAweGUzNzI1MzJiLFxuICAweGNhMjczZWNlLCAweGVhMjY2MTljLCAweGQxODZiOGM3LCAweDIxYzBjMjA3LFxuICAweGVhZGE3ZGQ2LCAweGNkZTBlYjFlLCAweGY1N2Q0ZjdmLCAweGVlNmVkMTc4LFxuICAweDA2ZjA2N2FhLCAweDcyMTc2ZmJhLCAweDBhNjM3ZGM1LCAweGEyYzg5OGE2LFxuICAweDExM2Y5ODA0LCAweGJlZjkwZGFlLCAweDFiNzEwYjM1LCAweDEzMWM0NzFiLFxuICAweDI4ZGI3N2Y1LCAweDIzMDQ3ZDg0LCAweDMyY2FhYjdiLCAweDQwYzcyNDkzLFxuICAweDNjOWViZTBhLCAweDE1YzliZWJjLCAweDQzMWQ2N2M0LCAweDljMTAwZDRjLFxuICAweDRjYzVkNGJlLCAweGNiM2U0MmI2LCAweDU5N2YyOTljLCAweGZjNjU3ZTJhLFxuICAweDVmY2I2ZmFiLCAweDNhZDZmYWVjLCAweDZjNDQxOThjLCAweDRhNDc1ODE3XG5dXG5cbnZhciBXID0gbmV3IEFycmF5KDE2MClcblxuZnVuY3Rpb24gU2hhNTEyICgpIHtcbiAgdGhpcy5pbml0KClcbiAgdGhpcy5fdyA9IFdcblxuICBIYXNoLmNhbGwodGhpcywgMTI4LCAxMTIpXG59XG5cbmluaGVyaXRzKFNoYTUxMiwgSGFzaClcblxuU2hhNTEyLnByb3RvdHlwZS5pbml0ID0gZnVuY3Rpb24gKCkge1xuICB0aGlzLl9haCA9IDB4NmEwOWU2NjdcbiAgdGhpcy5fYmggPSAweGJiNjdhZTg1XG4gIHRoaXMuX2NoID0gMHgzYzZlZjM3MlxuICB0aGlzLl9kaCA9IDB4YTU0ZmY1M2FcbiAgdGhpcy5fZWggPSAweDUxMGU1MjdmXG4gIHRoaXMuX2ZoID0gMHg5YjA1Njg4Y1xuICB0aGlzLl9naCA9IDB4MWY4M2Q5YWJcbiAgdGhpcy5faGggPSAweDViZTBjZDE5XG5cbiAgdGhpcy5fYWwgPSAweGYzYmNjOTA4XG4gIHRoaXMuX2JsID0gMHg4NGNhYTczYlxuICB0aGlzLl9jbCA9IDB4ZmU5NGY4MmJcbiAgdGhpcy5fZGwgPSAweDVmMWQzNmYxXG4gIHRoaXMuX2VsID0gMHhhZGU2ODJkMVxuICB0aGlzLl9mbCA9IDB4MmIzZTZjMWZcbiAgdGhpcy5fZ2wgPSAweGZiNDFiZDZiXG4gIHRoaXMuX2hsID0gMHgxMzdlMjE3OVxuXG4gIHJldHVybiB0aGlzXG59XG5cbmZ1bmN0aW9uIENoICh4LCB5LCB6KSB7XG4gIHJldHVybiB6IF4gKHggJiAoeSBeIHopKVxufVxuXG5mdW5jdGlvbiBtYWogKHgsIHksIHopIHtcbiAgcmV0dXJuICh4ICYgeSkgfCAoeiAmICh4IHwgeSkpXG59XG5cbmZ1bmN0aW9uIHNpZ21hMCAoeCwgeGwpIHtcbiAgcmV0dXJuICh4ID4+PiAyOCB8IHhsIDw8IDQpIF4gKHhsID4+PiAyIHwgeCA8PCAzMCkgXiAoeGwgPj4+IDcgfCB4IDw8IDI1KVxufVxuXG5mdW5jdGlvbiBzaWdtYTEgKHgsIHhsKSB7XG4gIHJldHVybiAoeCA+Pj4gMTQgfCB4bCA8PCAxOCkgXiAoeCA+Pj4gMTggfCB4bCA8PCAxNCkgXiAoeGwgPj4+IDkgfCB4IDw8IDIzKVxufVxuXG5mdW5jdGlvbiBHYW1tYTAgKHgsIHhsKSB7XG4gIHJldHVybiAoeCA+Pj4gMSB8IHhsIDw8IDMxKSBeICh4ID4+PiA4IHwgeGwgPDwgMjQpIF4gKHggPj4+IDcpXG59XG5cbmZ1bmN0aW9uIEdhbW1hMGwgKHgsIHhsKSB7XG4gIHJldHVybiAoeCA+Pj4gMSB8IHhsIDw8IDMxKSBeICh4ID4+PiA4IHwgeGwgPDwgMjQpIF4gKHggPj4+IDcgfCB4bCA8PCAyNSlcbn1cblxuZnVuY3Rpb24gR2FtbWExICh4LCB4bCkge1xuICByZXR1cm4gKHggPj4+IDE5IHwgeGwgPDwgMTMpIF4gKHhsID4+PiAyOSB8IHggPDwgMykgXiAoeCA+Pj4gNilcbn1cblxuZnVuY3Rpb24gR2FtbWExbCAoeCwgeGwpIHtcbiAgcmV0dXJuICh4ID4+PiAxOSB8IHhsIDw8IDEzKSBeICh4bCA+Pj4gMjkgfCB4IDw8IDMpIF4gKHggPj4+IDYgfCB4bCA8PCAyNilcbn1cblxuZnVuY3Rpb24gZ2V0Q2FycnkgKGEsIGIpIHtcbiAgcmV0dXJuIChhID4+PiAwKSA8IChiID4+PiAwKSA/IDEgOiAwXG59XG5cblNoYTUxMi5wcm90b3R5cGUuX3VwZGF0ZSA9IGZ1bmN0aW9uIChNKSB7XG4gIHZhciBXID0gdGhpcy5fd1xuXG4gIHZhciBhaCA9IHRoaXMuX2FoIHwgMFxuICB2YXIgYmggPSB0aGlzLl9iaCB8IDBcbiAgdmFyIGNoID0gdGhpcy5fY2ggfCAwXG4gIHZhciBkaCA9IHRoaXMuX2RoIHwgMFxuICB2YXIgZWggPSB0aGlzLl9laCB8IDBcbiAgdmFyIGZoID0gdGhpcy5fZmggfCAwXG4gIHZhciBnaCA9IHRoaXMuX2doIHwgMFxuICB2YXIgaGggPSB0aGlzLl9oaCB8IDBcblxuICB2YXIgYWwgPSB0aGlzLl9hbCB8IDBcbiAgdmFyIGJsID0gdGhpcy5fYmwgfCAwXG4gIHZhciBjbCA9IHRoaXMuX2NsIHwgMFxuICB2YXIgZGwgPSB0aGlzLl9kbCB8IDBcbiAgdmFyIGVsID0gdGhpcy5fZWwgfCAwXG4gIHZhciBmbCA9IHRoaXMuX2ZsIHwgMFxuICB2YXIgZ2wgPSB0aGlzLl9nbCB8IDBcbiAgdmFyIGhsID0gdGhpcy5faGwgfCAwXG5cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCAzMjsgaSArPSAyKSB7XG4gICAgV1tpXSA9IE0ucmVhZEludDMyQkUoaSAqIDQpXG4gICAgV1tpICsgMV0gPSBNLnJlYWRJbnQzMkJFKGkgKiA0ICsgNClcbiAgfVxuICBmb3IgKDsgaSA8IDE2MDsgaSArPSAyKSB7XG4gICAgdmFyIHhoID0gV1tpIC0gMTUgKiAyXVxuICAgIHZhciB4bCA9IFdbaSAtIDE1ICogMiArIDFdXG4gICAgdmFyIGdhbW1hMCA9IEdhbW1hMCh4aCwgeGwpXG4gICAgdmFyIGdhbW1hMGwgPSBHYW1tYTBsKHhsLCB4aClcblxuICAgIHhoID0gV1tpIC0gMiAqIDJdXG4gICAgeGwgPSBXW2kgLSAyICogMiArIDFdXG4gICAgdmFyIGdhbW1hMSA9IEdhbW1hMSh4aCwgeGwpXG4gICAgdmFyIGdhbW1hMWwgPSBHYW1tYTFsKHhsLCB4aClcblxuICAgIC8vIFdbaV0gPSBnYW1tYTAgKyBXW2kgLSA3XSArIGdhbW1hMSArIFdbaSAtIDE2XVxuICAgIHZhciBXaTdoID0gV1tpIC0gNyAqIDJdXG4gICAgdmFyIFdpN2wgPSBXW2kgLSA3ICogMiArIDFdXG5cbiAgICB2YXIgV2kxNmggPSBXW2kgLSAxNiAqIDJdXG4gICAgdmFyIFdpMTZsID0gV1tpIC0gMTYgKiAyICsgMV1cblxuICAgIHZhciBXaWwgPSAoZ2FtbWEwbCArIFdpN2wpIHwgMFxuICAgIHZhciBXaWggPSAoZ2FtbWEwICsgV2k3aCArIGdldENhcnJ5KFdpbCwgZ2FtbWEwbCkpIHwgMFxuICAgIFdpbCA9IChXaWwgKyBnYW1tYTFsKSB8IDBcbiAgICBXaWggPSAoV2loICsgZ2FtbWExICsgZ2V0Q2FycnkoV2lsLCBnYW1tYTFsKSkgfCAwXG4gICAgV2lsID0gKFdpbCArIFdpMTZsKSB8IDBcbiAgICBXaWggPSAoV2loICsgV2kxNmggKyBnZXRDYXJyeShXaWwsIFdpMTZsKSkgfCAwXG5cbiAgICBXW2ldID0gV2loXG4gICAgV1tpICsgMV0gPSBXaWxcbiAgfVxuXG4gIGZvciAodmFyIGogPSAwOyBqIDwgMTYwOyBqICs9IDIpIHtcbiAgICBXaWggPSBXW2pdXG4gICAgV2lsID0gV1tqICsgMV1cblxuICAgIHZhciBtYWpoID0gbWFqKGFoLCBiaCwgY2gpXG4gICAgdmFyIG1hamwgPSBtYWooYWwsIGJsLCBjbClcblxuICAgIHZhciBzaWdtYTBoID0gc2lnbWEwKGFoLCBhbClcbiAgICB2YXIgc2lnbWEwbCA9IHNpZ21hMChhbCwgYWgpXG4gICAgdmFyIHNpZ21hMWggPSBzaWdtYTEoZWgsIGVsKVxuICAgIHZhciBzaWdtYTFsID0gc2lnbWExKGVsLCBlaClcblxuICAgIC8vIHQxID0gaCArIHNpZ21hMSArIGNoICsgS1tqXSArIFdbal1cbiAgICB2YXIgS2loID0gS1tqXVxuICAgIHZhciBLaWwgPSBLW2ogKyAxXVxuXG4gICAgdmFyIGNoaCA9IENoKGVoLCBmaCwgZ2gpXG4gICAgdmFyIGNobCA9IENoKGVsLCBmbCwgZ2wpXG5cbiAgICB2YXIgdDFsID0gKGhsICsgc2lnbWExbCkgfCAwXG4gICAgdmFyIHQxaCA9IChoaCArIHNpZ21hMWggKyBnZXRDYXJyeSh0MWwsIGhsKSkgfCAwXG4gICAgdDFsID0gKHQxbCArIGNobCkgfCAwXG4gICAgdDFoID0gKHQxaCArIGNoaCArIGdldENhcnJ5KHQxbCwgY2hsKSkgfCAwXG4gICAgdDFsID0gKHQxbCArIEtpbCkgfCAwXG4gICAgdDFoID0gKHQxaCArIEtpaCArIGdldENhcnJ5KHQxbCwgS2lsKSkgfCAwXG4gICAgdDFsID0gKHQxbCArIFdpbCkgfCAwXG4gICAgdDFoID0gKHQxaCArIFdpaCArIGdldENhcnJ5KHQxbCwgV2lsKSkgfCAwXG5cbiAgICAvLyB0MiA9IHNpZ21hMCArIG1halxuICAgIHZhciB0MmwgPSAoc2lnbWEwbCArIG1hamwpIHwgMFxuICAgIHZhciB0MmggPSAoc2lnbWEwaCArIG1hamggKyBnZXRDYXJyeSh0MmwsIHNpZ21hMGwpKSB8IDBcblxuICAgIGhoID0gZ2hcbiAgICBobCA9IGdsXG4gICAgZ2ggPSBmaFxuICAgIGdsID0gZmxcbiAgICBmaCA9IGVoXG4gICAgZmwgPSBlbFxuICAgIGVsID0gKGRsICsgdDFsKSB8IDBcbiAgICBlaCA9IChkaCArIHQxaCArIGdldENhcnJ5KGVsLCBkbCkpIHwgMFxuICAgIGRoID0gY2hcbiAgICBkbCA9IGNsXG4gICAgY2ggPSBiaFxuICAgIGNsID0gYmxcbiAgICBiaCA9IGFoXG4gICAgYmwgPSBhbFxuICAgIGFsID0gKHQxbCArIHQybCkgfCAwXG4gICAgYWggPSAodDFoICsgdDJoICsgZ2V0Q2FycnkoYWwsIHQxbCkpIHwgMFxuICB9XG5cbiAgdGhpcy5fYWwgPSAodGhpcy5fYWwgKyBhbCkgfCAwXG4gIHRoaXMuX2JsID0gKHRoaXMuX2JsICsgYmwpIHwgMFxuICB0aGlzLl9jbCA9ICh0aGlzLl9jbCArIGNsKSB8IDBcbiAgdGhpcy5fZGwgPSAodGhpcy5fZGwgKyBkbCkgfCAwXG4gIHRoaXMuX2VsID0gKHRoaXMuX2VsICsgZWwpIHwgMFxuICB0aGlzLl9mbCA9ICh0aGlzLl9mbCArIGZsKSB8IDBcbiAgdGhpcy5fZ2wgPSAodGhpcy5fZ2wgKyBnbCkgfCAwXG4gIHRoaXMuX2hsID0gKHRoaXMuX2hsICsgaGwpIHwgMFxuXG4gIHRoaXMuX2FoID0gKHRoaXMuX2FoICsgYWggKyBnZXRDYXJyeSh0aGlzLl9hbCwgYWwpKSB8IDBcbiAgdGhpcy5fYmggPSAodGhpcy5fYmggKyBiaCArIGdldENhcnJ5KHRoaXMuX2JsLCBibCkpIHwgMFxuICB0aGlzLl9jaCA9ICh0aGlzLl9jaCArIGNoICsgZ2V0Q2FycnkodGhpcy5fY2wsIGNsKSkgfCAwXG4gIHRoaXMuX2RoID0gKHRoaXMuX2RoICsgZGggKyBnZXRDYXJyeSh0aGlzLl9kbCwgZGwpKSB8IDBcbiAgdGhpcy5fZWggPSAodGhpcy5fZWggKyBlaCArIGdldENhcnJ5KHRoaXMuX2VsLCBlbCkpIHwgMFxuICB0aGlzLl9maCA9ICh0aGlzLl9maCArIGZoICsgZ2V0Q2FycnkodGhpcy5fZmwsIGZsKSkgfCAwXG4gIHRoaXMuX2doID0gKHRoaXMuX2doICsgZ2ggKyBnZXRDYXJyeSh0aGlzLl9nbCwgZ2wpKSB8IDBcbiAgdGhpcy5faGggPSAodGhpcy5faGggKyBoaCArIGdldENhcnJ5KHRoaXMuX2hsLCBobCkpIHwgMFxufVxuXG5TaGE1MTIucHJvdG90eXBlLl9oYXNoID0gZnVuY3Rpb24gKCkge1xuICB2YXIgSCA9IG5ldyBCdWZmZXIoNjQpXG5cbiAgZnVuY3Rpb24gd3JpdGVJbnQ2NEJFIChoLCBsLCBvZmZzZXQpIHtcbiAgICBILndyaXRlSW50MzJCRShoLCBvZmZzZXQpXG4gICAgSC53cml0ZUludDMyQkUobCwgb2Zmc2V0ICsgNClcbiAgfVxuXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9haCwgdGhpcy5fYWwsIDApXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9iaCwgdGhpcy5fYmwsIDgpXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9jaCwgdGhpcy5fY2wsIDE2KVxuICB3cml0ZUludDY0QkUodGhpcy5fZGgsIHRoaXMuX2RsLCAyNClcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2VoLCB0aGlzLl9lbCwgMzIpXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9maCwgdGhpcy5fZmwsIDQwKVxuICB3cml0ZUludDY0QkUodGhpcy5fZ2gsIHRoaXMuX2dsLCA0OClcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2hoLCB0aGlzLl9obCwgNTYpXG5cbiAgcmV0dXJuIEhcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBTaGE1MTJcbiIsIihmdW5jdGlvbihuYWNsKSB7XG4ndXNlIHN0cmljdCc7XG5cbi8vIFBvcnRlZCBpbiAyMDE0IGJ5IERtaXRyeSBDaGVzdG55a2ggYW5kIERldmkgTWFuZGlyaS5cbi8vIFB1YmxpYyBkb21haW4uXG4vL1xuLy8gSW1wbGVtZW50YXRpb24gZGVyaXZlZCBmcm9tIFR3ZWV0TmFDbCB2ZXJzaW9uIDIwMTQwNDI3LlxuLy8gU2VlIGZvciBkZXRhaWxzOiBodHRwOi8vdHdlZXRuYWNsLmNyLnlwLnRvL1xuXG52YXIgZ2YgPSBmdW5jdGlvbihpbml0KSB7XG4gIHZhciBpLCByID0gbmV3IEZsb2F0NjRBcnJheSgxNik7XG4gIGlmIChpbml0KSBmb3IgKGkgPSAwOyBpIDwgaW5pdC5sZW5ndGg7IGkrKykgcltpXSA9IGluaXRbaV07XG4gIHJldHVybiByO1xufTtcblxuLy8gIFBsdWdnYWJsZSwgaW5pdGlhbGl6ZWQgaW4gaGlnaC1sZXZlbCBBUEkgYmVsb3cuXG52YXIgcmFuZG9tYnl0ZXMgPSBmdW5jdGlvbigvKiB4LCBuICovKSB7IHRocm93IG5ldyBFcnJvcignbm8gUFJORycpOyB9O1xuXG52YXIgXzAgPSBuZXcgVWludDhBcnJheSgxNik7XG52YXIgXzkgPSBuZXcgVWludDhBcnJheSgzMik7IF85WzBdID0gOTtcblxudmFyIGdmMCA9IGdmKCksXG4gICAgZ2YxID0gZ2YoWzFdKSxcbiAgICBfMTIxNjY1ID0gZ2YoWzB4ZGI0MSwgMV0pLFxuICAgIEQgPSBnZihbMHg3OGEzLCAweDEzNTksIDB4NGRjYSwgMHg3NWViLCAweGQ4YWIsIDB4NDE0MSwgMHgwYTRkLCAweDAwNzAsIDB4ZTg5OCwgMHg3Nzc5LCAweDQwNzksIDB4OGNjNywgMHhmZTczLCAweDJiNmYsIDB4NmNlZSwgMHg1MjAzXSksXG4gICAgRDIgPSBnZihbMHhmMTU5LCAweDI2YjIsIDB4OWI5NCwgMHhlYmQ2LCAweGIxNTYsIDB4ODI4MywgMHgxNDlhLCAweDAwZTAsIDB4ZDEzMCwgMHhlZWYzLCAweDgwZjIsIDB4MTk4ZSwgMHhmY2U3LCAweDU2ZGYsIDB4ZDlkYywgMHgyNDA2XSksXG4gICAgWCA9IGdmKFsweGQ1MWEsIDB4OGYyNSwgMHgyZDYwLCAweGM5NTYsIDB4YTdiMiwgMHg5NTI1LCAweGM3NjAsIDB4NjkyYywgMHhkYzVjLCAweGZkZDYsIDB4ZTIzMSwgMHhjMGE0LCAweDUzZmUsIDB4Y2Q2ZSwgMHgzNmQzLCAweDIxNjldKSxcbiAgICBZID0gZ2YoWzB4NjY1OCwgMHg2NjY2LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2LCAweDY2NjYsIDB4NjY2Nl0pLFxuICAgIEkgPSBnZihbMHhhMGIwLCAweDRhMGUsIDB4MWIyNywgMHhjNGVlLCAweGU0NzgsIDB4YWQyZiwgMHgxODA2LCAweDJmNDMsIDB4ZDdhNywgMHgzZGZiLCAweDAwOTksIDB4MmI0ZCwgMHhkZjBiLCAweDRmYzEsIDB4MjQ4MCwgMHgyYjgzXSk7XG5cbmZ1bmN0aW9uIHRzNjQoeCwgaSwgaCwgbCkge1xuICB4W2ldICAgPSAoaCA+PiAyNCkgJiAweGZmO1xuICB4W2krMV0gPSAoaCA+PiAxNikgJiAweGZmO1xuICB4W2krMl0gPSAoaCA+PiAgOCkgJiAweGZmO1xuICB4W2krM10gPSBoICYgMHhmZjtcbiAgeFtpKzRdID0gKGwgPj4gMjQpICAmIDB4ZmY7XG4gIHhbaSs1XSA9IChsID4+IDE2KSAgJiAweGZmO1xuICB4W2krNl0gPSAobCA+PiAgOCkgICYgMHhmZjtcbiAgeFtpKzddID0gbCAmIDB4ZmY7XG59XG5cbmZ1bmN0aW9uIHZuKHgsIHhpLCB5LCB5aSwgbikge1xuICB2YXIgaSxkID0gMDtcbiAgZm9yIChpID0gMDsgaSA8IG47IGkrKykgZCB8PSB4W3hpK2ldXnlbeWkraV07XG4gIHJldHVybiAoMSAmICgoZCAtIDEpID4+PiA4KSkgLSAxO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fdmVyaWZ5XzE2KHgsIHhpLCB5LCB5aSkge1xuICByZXR1cm4gdm4oeCx4aSx5LHlpLDE2KTtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX3ZlcmlmeV8zMih4LCB4aSwgeSwgeWkpIHtcbiAgcmV0dXJuIHZuKHgseGkseSx5aSwzMik7XG59XG5cbmZ1bmN0aW9uIGNvcmVfc2Fsc2EyMChvLCBwLCBrLCBjKSB7XG4gIHZhciBqMCAgPSBjWyAwXSAmIDB4ZmYgfCAoY1sgMV0gJiAweGZmKTw8OCB8IChjWyAyXSAmIDB4ZmYpPDwxNiB8IChjWyAzXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxICA9IGtbIDBdICYgMHhmZiB8IChrWyAxXSAmIDB4ZmYpPDw4IHwgKGtbIDJdICYgMHhmZik8PDE2IHwgKGtbIDNdICYgMHhmZik8PDI0LFxuICAgICAgajIgID0ga1sgNF0gJiAweGZmIHwgKGtbIDVdICYgMHhmZik8PDggfCAoa1sgNl0gJiAweGZmKTw8MTYgfCAoa1sgN10gJiAweGZmKTw8MjQsXG4gICAgICBqMyAgPSBrWyA4XSAmIDB4ZmYgfCAoa1sgOV0gJiAweGZmKTw8OCB8IChrWzEwXSAmIDB4ZmYpPDwxNiB8IChrWzExXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGo0ICA9IGtbMTJdICYgMHhmZiB8IChrWzEzXSAmIDB4ZmYpPDw4IHwgKGtbMTRdICYgMHhmZik8PDE2IHwgKGtbMTVdICYgMHhmZik8PDI0LFxuICAgICAgajUgID0gY1sgNF0gJiAweGZmIHwgKGNbIDVdICYgMHhmZik8PDggfCAoY1sgNl0gJiAweGZmKTw8MTYgfCAoY1sgN10gJiAweGZmKTw8MjQsXG4gICAgICBqNiAgPSBwWyAwXSAmIDB4ZmYgfCAocFsgMV0gJiAweGZmKTw8OCB8IChwWyAyXSAmIDB4ZmYpPDwxNiB8IChwWyAzXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGo3ICA9IHBbIDRdICYgMHhmZiB8IChwWyA1XSAmIDB4ZmYpPDw4IHwgKHBbIDZdICYgMHhmZik8PDE2IHwgKHBbIDddICYgMHhmZik8PDI0LFxuICAgICAgajggID0gcFsgOF0gJiAweGZmIHwgKHBbIDldICYgMHhmZik8PDggfCAocFsxMF0gJiAweGZmKTw8MTYgfCAocFsxMV0gJiAweGZmKTw8MjQsXG4gICAgICBqOSAgPSBwWzEyXSAmIDB4ZmYgfCAocFsxM10gJiAweGZmKTw8OCB8IChwWzE0XSAmIDB4ZmYpPDwxNiB8IChwWzE1XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxMCA9IGNbIDhdICYgMHhmZiB8IChjWyA5XSAmIDB4ZmYpPDw4IHwgKGNbMTBdICYgMHhmZik8PDE2IHwgKGNbMTFdICYgMHhmZik8PDI0LFxuICAgICAgajExID0ga1sxNl0gJiAweGZmIHwgKGtbMTddICYgMHhmZik8PDggfCAoa1sxOF0gJiAweGZmKTw8MTYgfCAoa1sxOV0gJiAweGZmKTw8MjQsXG4gICAgICBqMTIgPSBrWzIwXSAmIDB4ZmYgfCAoa1syMV0gJiAweGZmKTw8OCB8IChrWzIyXSAmIDB4ZmYpPDwxNiB8IChrWzIzXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxMyA9IGtbMjRdICYgMHhmZiB8IChrWzI1XSAmIDB4ZmYpPDw4IHwgKGtbMjZdICYgMHhmZik8PDE2IHwgKGtbMjddICYgMHhmZik8PDI0LFxuICAgICAgajE0ID0ga1syOF0gJiAweGZmIHwgKGtbMjldICYgMHhmZik8PDggfCAoa1szMF0gJiAweGZmKTw8MTYgfCAoa1szMV0gJiAweGZmKTw8MjQsXG4gICAgICBqMTUgPSBjWzEyXSAmIDB4ZmYgfCAoY1sxM10gJiAweGZmKTw8OCB8IChjWzE0XSAmIDB4ZmYpPDwxNiB8IChjWzE1XSAmIDB4ZmYpPDwyNDtcblxuICB2YXIgeDAgPSBqMCwgeDEgPSBqMSwgeDIgPSBqMiwgeDMgPSBqMywgeDQgPSBqNCwgeDUgPSBqNSwgeDYgPSBqNiwgeDcgPSBqNyxcbiAgICAgIHg4ID0gajgsIHg5ID0gajksIHgxMCA9IGoxMCwgeDExID0gajExLCB4MTIgPSBqMTIsIHgxMyA9IGoxMywgeDE0ID0gajE0LFxuICAgICAgeDE1ID0gajE1LCB1O1xuXG4gIGZvciAodmFyIGkgPSAwOyBpIDwgMjA7IGkgKz0gMikge1xuICAgIHUgPSB4MCArIHgxMiB8IDA7XG4gICAgeDQgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHg0ICsgeDAgfCAwO1xuICAgIHg4IF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4OCArIHg0IHwgMDtcbiAgICB4MTIgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDEyICsgeDggfCAwO1xuICAgIHgwIF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDUgKyB4MSB8IDA7XG4gICAgeDkgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHg5ICsgeDUgfCAwO1xuICAgIHgxMyBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDEzICsgeDkgfCAwO1xuICAgIHgxIF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHgxICsgeDEzIHwgMDtcbiAgICB4NSBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHgxMCArIHg2IHwgMDtcbiAgICB4MTQgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHgxNCArIHgxMCB8IDA7XG4gICAgeDIgXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHgyICsgeDE0IHwgMDtcbiAgICB4NiBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4NiArIHgyIHwgMDtcbiAgICB4MTAgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4MTUgKyB4MTEgfCAwO1xuICAgIHgzIF49IHU8PDcgfCB1Pj4+KDMyLTcpO1xuICAgIHUgPSB4MyArIHgxNSB8IDA7XG4gICAgeDcgXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHg3ICsgeDMgfCAwO1xuICAgIHgxMSBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4MTEgKyB4NyB8IDA7XG4gICAgeDE1IF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDAgKyB4MyB8IDA7XG4gICAgeDEgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHgxICsgeDAgfCAwO1xuICAgIHgyIF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4MiArIHgxIHwgMDtcbiAgICB4MyBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4MyArIHgyIHwgMDtcbiAgICB4MCBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHg1ICsgeDQgfCAwO1xuICAgIHg2IF49IHU8PDcgfCB1Pj4+KDMyLTcpO1xuICAgIHUgPSB4NiArIHg1IHwgMDtcbiAgICB4NyBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDcgKyB4NiB8IDA7XG4gICAgeDQgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDQgKyB4NyB8IDA7XG4gICAgeDUgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4MTAgKyB4OSB8IDA7XG4gICAgeDExIF49IHU8PDcgfCB1Pj4+KDMyLTcpO1xuICAgIHUgPSB4MTEgKyB4MTAgfCAwO1xuICAgIHg4IF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4OCArIHgxMSB8IDA7XG4gICAgeDkgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDkgKyB4OCB8IDA7XG4gICAgeDEwIF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDE1ICsgeDE0IHwgMDtcbiAgICB4MTIgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHgxMiArIHgxNSB8IDA7XG4gICAgeDEzIF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4MTMgKyB4MTIgfCAwO1xuICAgIHgxNCBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4MTQgKyB4MTMgfCAwO1xuICAgIHgxNSBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuICB9XG4gICB4MCA9ICB4MCArICBqMCB8IDA7XG4gICB4MSA9ICB4MSArICBqMSB8IDA7XG4gICB4MiA9ICB4MiArICBqMiB8IDA7XG4gICB4MyA9ICB4MyArICBqMyB8IDA7XG4gICB4NCA9ICB4NCArICBqNCB8IDA7XG4gICB4NSA9ICB4NSArICBqNSB8IDA7XG4gICB4NiA9ICB4NiArICBqNiB8IDA7XG4gICB4NyA9ICB4NyArICBqNyB8IDA7XG4gICB4OCA9ICB4OCArICBqOCB8IDA7XG4gICB4OSA9ICB4OSArICBqOSB8IDA7XG4gIHgxMCA9IHgxMCArIGoxMCB8IDA7XG4gIHgxMSA9IHgxMSArIGoxMSB8IDA7XG4gIHgxMiA9IHgxMiArIGoxMiB8IDA7XG4gIHgxMyA9IHgxMyArIGoxMyB8IDA7XG4gIHgxNCA9IHgxNCArIGoxNCB8IDA7XG4gIHgxNSA9IHgxNSArIGoxNSB8IDA7XG5cbiAgb1sgMF0gPSB4MCA+Pj4gIDAgJiAweGZmO1xuICBvWyAxXSA9IHgwID4+PiAgOCAmIDB4ZmY7XG4gIG9bIDJdID0geDAgPj4+IDE2ICYgMHhmZjtcbiAgb1sgM10gPSB4MCA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bIDRdID0geDEgPj4+ICAwICYgMHhmZjtcbiAgb1sgNV0gPSB4MSA+Pj4gIDggJiAweGZmO1xuICBvWyA2XSA9IHgxID4+PiAxNiAmIDB4ZmY7XG4gIG9bIDddID0geDEgPj4+IDI0ICYgMHhmZjtcblxuICBvWyA4XSA9IHgyID4+PiAgMCAmIDB4ZmY7XG4gIG9bIDldID0geDIgPj4+ICA4ICYgMHhmZjtcbiAgb1sxMF0gPSB4MiA+Pj4gMTYgJiAweGZmO1xuICBvWzExXSA9IHgyID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1sxMl0gPSB4MyA+Pj4gIDAgJiAweGZmO1xuICBvWzEzXSA9IHgzID4+PiAgOCAmIDB4ZmY7XG4gIG9bMTRdID0geDMgPj4+IDE2ICYgMHhmZjtcbiAgb1sxNV0gPSB4MyA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bMTZdID0geDQgPj4+ICAwICYgMHhmZjtcbiAgb1sxN10gPSB4NCA+Pj4gIDggJiAweGZmO1xuICBvWzE4XSA9IHg0ID4+PiAxNiAmIDB4ZmY7XG4gIG9bMTldID0geDQgPj4+IDI0ICYgMHhmZjtcblxuICBvWzIwXSA9IHg1ID4+PiAgMCAmIDB4ZmY7XG4gIG9bMjFdID0geDUgPj4+ICA4ICYgMHhmZjtcbiAgb1syMl0gPSB4NSA+Pj4gMTYgJiAweGZmO1xuICBvWzIzXSA9IHg1ID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1syNF0gPSB4NiA+Pj4gIDAgJiAweGZmO1xuICBvWzI1XSA9IHg2ID4+PiAgOCAmIDB4ZmY7XG4gIG9bMjZdID0geDYgPj4+IDE2ICYgMHhmZjtcbiAgb1syN10gPSB4NiA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bMjhdID0geDcgPj4+ICAwICYgMHhmZjtcbiAgb1syOV0gPSB4NyA+Pj4gIDggJiAweGZmO1xuICBvWzMwXSA9IHg3ID4+PiAxNiAmIDB4ZmY7XG4gIG9bMzFdID0geDcgPj4+IDI0ICYgMHhmZjtcblxuICBvWzMyXSA9IHg4ID4+PiAgMCAmIDB4ZmY7XG4gIG9bMzNdID0geDggPj4+ICA4ICYgMHhmZjtcbiAgb1szNF0gPSB4OCA+Pj4gMTYgJiAweGZmO1xuICBvWzM1XSA9IHg4ID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1szNl0gPSB4OSA+Pj4gIDAgJiAweGZmO1xuICBvWzM3XSA9IHg5ID4+PiAgOCAmIDB4ZmY7XG4gIG9bMzhdID0geDkgPj4+IDE2ICYgMHhmZjtcbiAgb1szOV0gPSB4OSA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bNDBdID0geDEwID4+PiAgMCAmIDB4ZmY7XG4gIG9bNDFdID0geDEwID4+PiAgOCAmIDB4ZmY7XG4gIG9bNDJdID0geDEwID4+PiAxNiAmIDB4ZmY7XG4gIG9bNDNdID0geDEwID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1s0NF0gPSB4MTEgPj4+ICAwICYgMHhmZjtcbiAgb1s0NV0gPSB4MTEgPj4+ICA4ICYgMHhmZjtcbiAgb1s0Nl0gPSB4MTEgPj4+IDE2ICYgMHhmZjtcbiAgb1s0N10gPSB4MTEgPj4+IDI0ICYgMHhmZjtcblxuICBvWzQ4XSA9IHgxMiA+Pj4gIDAgJiAweGZmO1xuICBvWzQ5XSA9IHgxMiA+Pj4gIDggJiAweGZmO1xuICBvWzUwXSA9IHgxMiA+Pj4gMTYgJiAweGZmO1xuICBvWzUxXSA9IHgxMiA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bNTJdID0geDEzID4+PiAgMCAmIDB4ZmY7XG4gIG9bNTNdID0geDEzID4+PiAgOCAmIDB4ZmY7XG4gIG9bNTRdID0geDEzID4+PiAxNiAmIDB4ZmY7XG4gIG9bNTVdID0geDEzID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1s1Nl0gPSB4MTQgPj4+ICAwICYgMHhmZjtcbiAgb1s1N10gPSB4MTQgPj4+ICA4ICYgMHhmZjtcbiAgb1s1OF0gPSB4MTQgPj4+IDE2ICYgMHhmZjtcbiAgb1s1OV0gPSB4MTQgPj4+IDI0ICYgMHhmZjtcblxuICBvWzYwXSA9IHgxNSA+Pj4gIDAgJiAweGZmO1xuICBvWzYxXSA9IHgxNSA+Pj4gIDggJiAweGZmO1xuICBvWzYyXSA9IHgxNSA+Pj4gMTYgJiAweGZmO1xuICBvWzYzXSA9IHgxNSA+Pj4gMjQgJiAweGZmO1xufVxuXG5mdW5jdGlvbiBjb3JlX2hzYWxzYTIwKG8scCxrLGMpIHtcbiAgdmFyIGowICA9IGNbIDBdICYgMHhmZiB8IChjWyAxXSAmIDB4ZmYpPDw4IHwgKGNbIDJdICYgMHhmZik8PDE2IHwgKGNbIDNdICYgMHhmZik8PDI0LFxuICAgICAgajEgID0ga1sgMF0gJiAweGZmIHwgKGtbIDFdICYgMHhmZik8PDggfCAoa1sgMl0gJiAweGZmKTw8MTYgfCAoa1sgM10gJiAweGZmKTw8MjQsXG4gICAgICBqMiAgPSBrWyA0XSAmIDB4ZmYgfCAoa1sgNV0gJiAweGZmKTw8OCB8IChrWyA2XSAmIDB4ZmYpPDwxNiB8IChrWyA3XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGozICA9IGtbIDhdICYgMHhmZiB8IChrWyA5XSAmIDB4ZmYpPDw4IHwgKGtbMTBdICYgMHhmZik8PDE2IHwgKGtbMTFdICYgMHhmZik8PDI0LFxuICAgICAgajQgID0ga1sxMl0gJiAweGZmIHwgKGtbMTNdICYgMHhmZik8PDggfCAoa1sxNF0gJiAweGZmKTw8MTYgfCAoa1sxNV0gJiAweGZmKTw8MjQsXG4gICAgICBqNSAgPSBjWyA0XSAmIDB4ZmYgfCAoY1sgNV0gJiAweGZmKTw8OCB8IChjWyA2XSAmIDB4ZmYpPDwxNiB8IChjWyA3XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGo2ICA9IHBbIDBdICYgMHhmZiB8IChwWyAxXSAmIDB4ZmYpPDw4IHwgKHBbIDJdICYgMHhmZik8PDE2IHwgKHBbIDNdICYgMHhmZik8PDI0LFxuICAgICAgajcgID0gcFsgNF0gJiAweGZmIHwgKHBbIDVdICYgMHhmZik8PDggfCAocFsgNl0gJiAweGZmKTw8MTYgfCAocFsgN10gJiAweGZmKTw8MjQsXG4gICAgICBqOCAgPSBwWyA4XSAmIDB4ZmYgfCAocFsgOV0gJiAweGZmKTw8OCB8IChwWzEwXSAmIDB4ZmYpPDwxNiB8IChwWzExXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGo5ICA9IHBbMTJdICYgMHhmZiB8IChwWzEzXSAmIDB4ZmYpPDw4IHwgKHBbMTRdICYgMHhmZik8PDE2IHwgKHBbMTVdICYgMHhmZik8PDI0LFxuICAgICAgajEwID0gY1sgOF0gJiAweGZmIHwgKGNbIDldICYgMHhmZik8PDggfCAoY1sxMF0gJiAweGZmKTw8MTYgfCAoY1sxMV0gJiAweGZmKTw8MjQsXG4gICAgICBqMTEgPSBrWzE2XSAmIDB4ZmYgfCAoa1sxN10gJiAweGZmKTw8OCB8IChrWzE4XSAmIDB4ZmYpPDwxNiB8IChrWzE5XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxMiA9IGtbMjBdICYgMHhmZiB8IChrWzIxXSAmIDB4ZmYpPDw4IHwgKGtbMjJdICYgMHhmZik8PDE2IHwgKGtbMjNdICYgMHhmZik8PDI0LFxuICAgICAgajEzID0ga1syNF0gJiAweGZmIHwgKGtbMjVdICYgMHhmZik8PDggfCAoa1syNl0gJiAweGZmKTw8MTYgfCAoa1syN10gJiAweGZmKTw8MjQsXG4gICAgICBqMTQgPSBrWzI4XSAmIDB4ZmYgfCAoa1syOV0gJiAweGZmKTw8OCB8IChrWzMwXSAmIDB4ZmYpPDwxNiB8IChrWzMxXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxNSA9IGNbMTJdICYgMHhmZiB8IChjWzEzXSAmIDB4ZmYpPDw4IHwgKGNbMTRdICYgMHhmZik8PDE2IHwgKGNbMTVdICYgMHhmZik8PDI0O1xuXG4gIHZhciB4MCA9IGowLCB4MSA9IGoxLCB4MiA9IGoyLCB4MyA9IGozLCB4NCA9IGo0LCB4NSA9IGo1LCB4NiA9IGo2LCB4NyA9IGo3LFxuICAgICAgeDggPSBqOCwgeDkgPSBqOSwgeDEwID0gajEwLCB4MTEgPSBqMTEsIHgxMiA9IGoxMiwgeDEzID0gajEzLCB4MTQgPSBqMTQsXG4gICAgICB4MTUgPSBqMTUsIHU7XG5cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCAyMDsgaSArPSAyKSB7XG4gICAgdSA9IHgwICsgeDEyIHwgMDtcbiAgICB4NCBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDQgKyB4MCB8IDA7XG4gICAgeDggXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHg4ICsgeDQgfCAwO1xuICAgIHgxMiBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4MTIgKyB4OCB8IDA7XG4gICAgeDAgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4NSArIHgxIHwgMDtcbiAgICB4OSBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDkgKyB4NSB8IDA7XG4gICAgeDEzIF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4MTMgKyB4OSB8IDA7XG4gICAgeDEgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDEgKyB4MTMgfCAwO1xuICAgIHg1IF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDEwICsgeDYgfCAwO1xuICAgIHgxNCBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDE0ICsgeDEwIHwgMDtcbiAgICB4MiBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDIgKyB4MTQgfCAwO1xuICAgIHg2IF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHg2ICsgeDIgfCAwO1xuICAgIHgxMCBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHgxNSArIHgxMSB8IDA7XG4gICAgeDMgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHgzICsgeDE1IHwgMDtcbiAgICB4NyBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDcgKyB4MyB8IDA7XG4gICAgeDExIF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHgxMSArIHg3IHwgMDtcbiAgICB4MTUgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4MCArIHgzIHwgMDtcbiAgICB4MSBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDEgKyB4MCB8IDA7XG4gICAgeDIgXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHgyICsgeDEgfCAwO1xuICAgIHgzIF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHgzICsgeDIgfCAwO1xuICAgIHgwIF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDUgKyB4NCB8IDA7XG4gICAgeDYgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHg2ICsgeDUgfCAwO1xuICAgIHg3IF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4NyArIHg2IHwgMDtcbiAgICB4NCBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4NCArIHg3IHwgMDtcbiAgICB4NSBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHgxMCArIHg5IHwgMDtcbiAgICB4MTEgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHgxMSArIHgxMCB8IDA7XG4gICAgeDggXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHg4ICsgeDExIHwgMDtcbiAgICB4OSBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4OSArIHg4IHwgMDtcbiAgICB4MTAgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4MTUgKyB4MTQgfCAwO1xuICAgIHgxMiBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDEyICsgeDE1IHwgMDtcbiAgICB4MTMgXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHgxMyArIHgxMiB8IDA7XG4gICAgeDE0IF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHgxNCArIHgxMyB8IDA7XG4gICAgeDE1IF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG4gIH1cblxuICBvWyAwXSA9IHgwID4+PiAgMCAmIDB4ZmY7XG4gIG9bIDFdID0geDAgPj4+ICA4ICYgMHhmZjtcbiAgb1sgMl0gPSB4MCA+Pj4gMTYgJiAweGZmO1xuICBvWyAzXSA9IHgwID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1sgNF0gPSB4NSA+Pj4gIDAgJiAweGZmO1xuICBvWyA1XSA9IHg1ID4+PiAgOCAmIDB4ZmY7XG4gIG9bIDZdID0geDUgPj4+IDE2ICYgMHhmZjtcbiAgb1sgN10gPSB4NSA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bIDhdID0geDEwID4+PiAgMCAmIDB4ZmY7XG4gIG9bIDldID0geDEwID4+PiAgOCAmIDB4ZmY7XG4gIG9bMTBdID0geDEwID4+PiAxNiAmIDB4ZmY7XG4gIG9bMTFdID0geDEwID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1sxMl0gPSB4MTUgPj4+ICAwICYgMHhmZjtcbiAgb1sxM10gPSB4MTUgPj4+ICA4ICYgMHhmZjtcbiAgb1sxNF0gPSB4MTUgPj4+IDE2ICYgMHhmZjtcbiAgb1sxNV0gPSB4MTUgPj4+IDI0ICYgMHhmZjtcblxuICBvWzE2XSA9IHg2ID4+PiAgMCAmIDB4ZmY7XG4gIG9bMTddID0geDYgPj4+ICA4ICYgMHhmZjtcbiAgb1sxOF0gPSB4NiA+Pj4gMTYgJiAweGZmO1xuICBvWzE5XSA9IHg2ID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1syMF0gPSB4NyA+Pj4gIDAgJiAweGZmO1xuICBvWzIxXSA9IHg3ID4+PiAgOCAmIDB4ZmY7XG4gIG9bMjJdID0geDcgPj4+IDE2ICYgMHhmZjtcbiAgb1syM10gPSB4NyA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bMjRdID0geDggPj4+ICAwICYgMHhmZjtcbiAgb1syNV0gPSB4OCA+Pj4gIDggJiAweGZmO1xuICBvWzI2XSA9IHg4ID4+PiAxNiAmIDB4ZmY7XG4gIG9bMjddID0geDggPj4+IDI0ICYgMHhmZjtcblxuICBvWzI4XSA9IHg5ID4+PiAgMCAmIDB4ZmY7XG4gIG9bMjldID0geDkgPj4+ICA4ICYgMHhmZjtcbiAgb1szMF0gPSB4OSA+Pj4gMTYgJiAweGZmO1xuICBvWzMxXSA9IHg5ID4+PiAyNCAmIDB4ZmY7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19jb3JlX3NhbHNhMjAob3V0LGlucCxrLGMpIHtcbiAgY29yZV9zYWxzYTIwKG91dCxpbnAsayxjKTtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX2NvcmVfaHNhbHNhMjAob3V0LGlucCxrLGMpIHtcbiAgY29yZV9oc2Fsc2EyMChvdXQsaW5wLGssYyk7XG59XG5cbnZhciBzaWdtYSA9IG5ldyBVaW50OEFycmF5KFsxMDEsIDEyMCwgMTEyLCA5NywgMTEwLCAxMDAsIDMyLCA1MSwgNTAsIDQ1LCA5OCwgMTIxLCAxMTYsIDEwMSwgMzIsIDEwN10pO1xuICAgICAgICAgICAgLy8gXCJleHBhbmQgMzItYnl0ZSBrXCJcblxuZnVuY3Rpb24gY3J5cHRvX3N0cmVhbV9zYWxzYTIwX3hvcihjLGNwb3MsbSxtcG9zLGIsbixrKSB7XG4gIHZhciB6ID0gbmV3IFVpbnQ4QXJyYXkoMTYpLCB4ID0gbmV3IFVpbnQ4QXJyYXkoNjQpO1xuICB2YXIgdSwgaTtcbiAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIHpbaV0gPSAwO1xuICBmb3IgKGkgPSAwOyBpIDwgODsgaSsrKSB6W2ldID0gbltpXTtcbiAgd2hpbGUgKGIgPj0gNjQpIHtcbiAgICBjcnlwdG9fY29yZV9zYWxzYTIwKHgseixrLHNpZ21hKTtcbiAgICBmb3IgKGkgPSAwOyBpIDwgNjQ7IGkrKykgY1tjcG9zK2ldID0gbVttcG9zK2ldIF4geFtpXTtcbiAgICB1ID0gMTtcbiAgICBmb3IgKGkgPSA4OyBpIDwgMTY7IGkrKykge1xuICAgICAgdSA9IHUgKyAoeltpXSAmIDB4ZmYpIHwgMDtcbiAgICAgIHpbaV0gPSB1ICYgMHhmZjtcbiAgICAgIHUgPj4+PSA4O1xuICAgIH1cbiAgICBiIC09IDY0O1xuICAgIGNwb3MgKz0gNjQ7XG4gICAgbXBvcyArPSA2NDtcbiAgfVxuICBpZiAoYiA+IDApIHtcbiAgICBjcnlwdG9fY29yZV9zYWxzYTIwKHgseixrLHNpZ21hKTtcbiAgICBmb3IgKGkgPSAwOyBpIDwgYjsgaSsrKSBjW2Nwb3MraV0gPSBtW21wb3MraV0gXiB4W2ldO1xuICB9XG4gIHJldHVybiAwO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fc3RyZWFtX3NhbHNhMjAoYyxjcG9zLGIsbixrKSB7XG4gIHZhciB6ID0gbmV3IFVpbnQ4QXJyYXkoMTYpLCB4ID0gbmV3IFVpbnQ4QXJyYXkoNjQpO1xuICB2YXIgdSwgaTtcbiAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIHpbaV0gPSAwO1xuICBmb3IgKGkgPSAwOyBpIDwgODsgaSsrKSB6W2ldID0gbltpXTtcbiAgd2hpbGUgKGIgPj0gNjQpIHtcbiAgICBjcnlwdG9fY29yZV9zYWxzYTIwKHgseixrLHNpZ21hKTtcbiAgICBmb3IgKGkgPSAwOyBpIDwgNjQ7IGkrKykgY1tjcG9zK2ldID0geFtpXTtcbiAgICB1ID0gMTtcbiAgICBmb3IgKGkgPSA4OyBpIDwgMTY7IGkrKykge1xuICAgICAgdSA9IHUgKyAoeltpXSAmIDB4ZmYpIHwgMDtcbiAgICAgIHpbaV0gPSB1ICYgMHhmZjtcbiAgICAgIHUgPj4+PSA4O1xuICAgIH1cbiAgICBiIC09IDY0O1xuICAgIGNwb3MgKz0gNjQ7XG4gIH1cbiAgaWYgKGIgPiAwKSB7XG4gICAgY3J5cHRvX2NvcmVfc2Fsc2EyMCh4LHosayxzaWdtYSk7XG4gICAgZm9yIChpID0gMDsgaSA8IGI7IGkrKykgY1tjcG9zK2ldID0geFtpXTtcbiAgfVxuICByZXR1cm4gMDtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX3N0cmVhbShjLGNwb3MsZCxuLGspIHtcbiAgdmFyIHMgPSBuZXcgVWludDhBcnJheSgzMik7XG4gIGNyeXB0b19jb3JlX2hzYWxzYTIwKHMsbixrLHNpZ21hKTtcbiAgdmFyIHNuID0gbmV3IFVpbnQ4QXJyYXkoOCk7XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgODsgaSsrKSBzbltpXSA9IG5baSsxNl07XG4gIHJldHVybiBjcnlwdG9fc3RyZWFtX3NhbHNhMjAoYyxjcG9zLGQsc24scyk7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19zdHJlYW1feG9yKGMsY3BvcyxtLG1wb3MsZCxuLGspIHtcbiAgdmFyIHMgPSBuZXcgVWludDhBcnJheSgzMik7XG4gIGNyeXB0b19jb3JlX2hzYWxzYTIwKHMsbixrLHNpZ21hKTtcbiAgdmFyIHNuID0gbmV3IFVpbnQ4QXJyYXkoOCk7XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgODsgaSsrKSBzbltpXSA9IG5baSsxNl07XG4gIHJldHVybiBjcnlwdG9fc3RyZWFtX3NhbHNhMjBfeG9yKGMsY3BvcyxtLG1wb3MsZCxzbixzKTtcbn1cblxuLypcbiogUG9ydCBvZiBBbmRyZXcgTW9vbidzIFBvbHkxMzA1LWRvbm5hLTE2LiBQdWJsaWMgZG9tYWluLlxuKiBodHRwczovL2dpdGh1Yi5jb20vZmxvb2R5YmVycnkvcG9seTEzMDUtZG9ubmFcbiovXG5cbnZhciBwb2x5MTMwNSA9IGZ1bmN0aW9uKGtleSkge1xuICB0aGlzLmJ1ZmZlciA9IG5ldyBVaW50OEFycmF5KDE2KTtcbiAgdGhpcy5yID0gbmV3IFVpbnQxNkFycmF5KDEwKTtcbiAgdGhpcy5oID0gbmV3IFVpbnQxNkFycmF5KDEwKTtcbiAgdGhpcy5wYWQgPSBuZXcgVWludDE2QXJyYXkoOCk7XG4gIHRoaXMubGVmdG92ZXIgPSAwO1xuICB0aGlzLmZpbiA9IDA7XG5cbiAgdmFyIHQwLCB0MSwgdDIsIHQzLCB0NCwgdDUsIHQ2LCB0NztcblxuICB0MCA9IGtleVsgMF0gJiAweGZmIHwgKGtleVsgMV0gJiAweGZmKSA8PCA4OyB0aGlzLnJbMF0gPSAoIHQwICAgICAgICAgICAgICAgICAgICAgKSAmIDB4MWZmZjtcbiAgdDEgPSBrZXlbIDJdICYgMHhmZiB8IChrZXlbIDNdICYgMHhmZikgPDwgODsgdGhpcy5yWzFdID0gKCh0MCA+Pj4gMTMpIHwgKHQxIDw8ICAzKSkgJiAweDFmZmY7XG4gIHQyID0ga2V5WyA0XSAmIDB4ZmYgfCAoa2V5WyA1XSAmIDB4ZmYpIDw8IDg7IHRoaXMuclsyXSA9ICgodDEgPj4+IDEwKSB8ICh0MiA8PCAgNikpICYgMHgxZjAzO1xuICB0MyA9IGtleVsgNl0gJiAweGZmIHwgKGtleVsgN10gJiAweGZmKSA8PCA4OyB0aGlzLnJbM10gPSAoKHQyID4+PiAgNykgfCAodDMgPDwgIDkpKSAmIDB4MWZmZjtcbiAgdDQgPSBrZXlbIDhdICYgMHhmZiB8IChrZXlbIDldICYgMHhmZikgPDwgODsgdGhpcy5yWzRdID0gKCh0MyA+Pj4gIDQpIHwgKHQ0IDw8IDEyKSkgJiAweDAwZmY7XG4gIHRoaXMucls1XSA9ICgodDQgPj4+ICAxKSkgJiAweDFmZmU7XG4gIHQ1ID0ga2V5WzEwXSAmIDB4ZmYgfCAoa2V5WzExXSAmIDB4ZmYpIDw8IDg7IHRoaXMucls2XSA9ICgodDQgPj4+IDE0KSB8ICh0NSA8PCAgMikpICYgMHgxZmZmO1xuICB0NiA9IGtleVsxMl0gJiAweGZmIHwgKGtleVsxM10gJiAweGZmKSA8PCA4OyB0aGlzLnJbN10gPSAoKHQ1ID4+PiAxMSkgfCAodDYgPDwgIDUpKSAmIDB4MWY4MTtcbiAgdDcgPSBrZXlbMTRdICYgMHhmZiB8IChrZXlbMTVdICYgMHhmZikgPDwgODsgdGhpcy5yWzhdID0gKCh0NiA+Pj4gIDgpIHwgKHQ3IDw8ICA4KSkgJiAweDFmZmY7XG4gIHRoaXMucls5XSA9ICgodDcgPj4+ICA1KSkgJiAweDAwN2Y7XG5cbiAgdGhpcy5wYWRbMF0gPSBrZXlbMTZdICYgMHhmZiB8IChrZXlbMTddICYgMHhmZikgPDwgODtcbiAgdGhpcy5wYWRbMV0gPSBrZXlbMThdICYgMHhmZiB8IChrZXlbMTldICYgMHhmZikgPDwgODtcbiAgdGhpcy5wYWRbMl0gPSBrZXlbMjBdICYgMHhmZiB8IChrZXlbMjFdICYgMHhmZikgPDwgODtcbiAgdGhpcy5wYWRbM10gPSBrZXlbMjJdICYgMHhmZiB8IChrZXlbMjNdICYgMHhmZikgPDwgODtcbiAgdGhpcy5wYWRbNF0gPSBrZXlbMjRdICYgMHhmZiB8IChrZXlbMjVdICYgMHhmZikgPDwgODtcbiAgdGhpcy5wYWRbNV0gPSBrZXlbMjZdICYgMHhmZiB8IChrZXlbMjddICYgMHhmZikgPDwgODtcbiAgdGhpcy5wYWRbNl0gPSBrZXlbMjhdICYgMHhmZiB8IChrZXlbMjldICYgMHhmZikgPDwgODtcbiAgdGhpcy5wYWRbN10gPSBrZXlbMzBdICYgMHhmZiB8IChrZXlbMzFdICYgMHhmZikgPDwgODtcbn07XG5cbnBvbHkxMzA1LnByb3RvdHlwZS5ibG9ja3MgPSBmdW5jdGlvbihtLCBtcG9zLCBieXRlcykge1xuICB2YXIgaGliaXQgPSB0aGlzLmZpbiA/IDAgOiAoMSA8PCAxMSk7XG4gIHZhciB0MCwgdDEsIHQyLCB0MywgdDQsIHQ1LCB0NiwgdDcsIGM7XG4gIHZhciBkMCwgZDEsIGQyLCBkMywgZDQsIGQ1LCBkNiwgZDcsIGQ4LCBkOTtcblxuICB2YXIgaDAgPSB0aGlzLmhbMF0sXG4gICAgICBoMSA9IHRoaXMuaFsxXSxcbiAgICAgIGgyID0gdGhpcy5oWzJdLFxuICAgICAgaDMgPSB0aGlzLmhbM10sXG4gICAgICBoNCA9IHRoaXMuaFs0XSxcbiAgICAgIGg1ID0gdGhpcy5oWzVdLFxuICAgICAgaDYgPSB0aGlzLmhbNl0sXG4gICAgICBoNyA9IHRoaXMuaFs3XSxcbiAgICAgIGg4ID0gdGhpcy5oWzhdLFxuICAgICAgaDkgPSB0aGlzLmhbOV07XG5cbiAgdmFyIHIwID0gdGhpcy5yWzBdLFxuICAgICAgcjEgPSB0aGlzLnJbMV0sXG4gICAgICByMiA9IHRoaXMuclsyXSxcbiAgICAgIHIzID0gdGhpcy5yWzNdLFxuICAgICAgcjQgPSB0aGlzLnJbNF0sXG4gICAgICByNSA9IHRoaXMucls1XSxcbiAgICAgIHI2ID0gdGhpcy5yWzZdLFxuICAgICAgcjcgPSB0aGlzLnJbN10sXG4gICAgICByOCA9IHRoaXMucls4XSxcbiAgICAgIHI5ID0gdGhpcy5yWzldO1xuXG4gIHdoaWxlIChieXRlcyA+PSAxNikge1xuICAgIHQwID0gbVttcG9zKyAwXSAmIDB4ZmYgfCAobVttcG9zKyAxXSAmIDB4ZmYpIDw8IDg7IGgwICs9ICggdDAgICAgICAgICAgICAgICAgICAgICApICYgMHgxZmZmO1xuICAgIHQxID0gbVttcG9zKyAyXSAmIDB4ZmYgfCAobVttcG9zKyAzXSAmIDB4ZmYpIDw8IDg7IGgxICs9ICgodDAgPj4+IDEzKSB8ICh0MSA8PCAgMykpICYgMHgxZmZmO1xuICAgIHQyID0gbVttcG9zKyA0XSAmIDB4ZmYgfCAobVttcG9zKyA1XSAmIDB4ZmYpIDw8IDg7IGgyICs9ICgodDEgPj4+IDEwKSB8ICh0MiA8PCAgNikpICYgMHgxZmZmO1xuICAgIHQzID0gbVttcG9zKyA2XSAmIDB4ZmYgfCAobVttcG9zKyA3XSAmIDB4ZmYpIDw8IDg7IGgzICs9ICgodDIgPj4+ICA3KSB8ICh0MyA8PCAgOSkpICYgMHgxZmZmO1xuICAgIHQ0ID0gbVttcG9zKyA4XSAmIDB4ZmYgfCAobVttcG9zKyA5XSAmIDB4ZmYpIDw8IDg7IGg0ICs9ICgodDMgPj4+ICA0KSB8ICh0NCA8PCAxMikpICYgMHgxZmZmO1xuICAgIGg1ICs9ICgodDQgPj4+ICAxKSkgJiAweDFmZmY7XG4gICAgdDUgPSBtW21wb3MrMTBdICYgMHhmZiB8IChtW21wb3MrMTFdICYgMHhmZikgPDwgODsgaDYgKz0gKCh0NCA+Pj4gMTQpIHwgKHQ1IDw8ICAyKSkgJiAweDFmZmY7XG4gICAgdDYgPSBtW21wb3MrMTJdICYgMHhmZiB8IChtW21wb3MrMTNdICYgMHhmZikgPDwgODsgaDcgKz0gKCh0NSA+Pj4gMTEpIHwgKHQ2IDw8ICA1KSkgJiAweDFmZmY7XG4gICAgdDcgPSBtW21wb3MrMTRdICYgMHhmZiB8IChtW21wb3MrMTVdICYgMHhmZikgPDwgODsgaDggKz0gKCh0NiA+Pj4gIDgpIHwgKHQ3IDw8ICA4KSkgJiAweDFmZmY7XG4gICAgaDkgKz0gKCh0NyA+Pj4gNSkpIHwgaGliaXQ7XG5cbiAgICBjID0gMDtcblxuICAgIGQwID0gYztcbiAgICBkMCArPSBoMCAqIHIwO1xuICAgIGQwICs9IGgxICogKDUgKiByOSk7XG4gICAgZDAgKz0gaDIgKiAoNSAqIHI4KTtcbiAgICBkMCArPSBoMyAqICg1ICogcjcpO1xuICAgIGQwICs9IGg0ICogKDUgKiByNik7XG4gICAgYyA9IChkMCA+Pj4gMTMpOyBkMCAmPSAweDFmZmY7XG4gICAgZDAgKz0gaDUgKiAoNSAqIHI1KTtcbiAgICBkMCArPSBoNiAqICg1ICogcjQpO1xuICAgIGQwICs9IGg3ICogKDUgKiByMyk7XG4gICAgZDAgKz0gaDggKiAoNSAqIHIyKTtcbiAgICBkMCArPSBoOSAqICg1ICogcjEpO1xuICAgIGMgKz0gKGQwID4+PiAxMyk7IGQwICY9IDB4MWZmZjtcblxuICAgIGQxID0gYztcbiAgICBkMSArPSBoMCAqIHIxO1xuICAgIGQxICs9IGgxICogcjA7XG4gICAgZDEgKz0gaDIgKiAoNSAqIHI5KTtcbiAgICBkMSArPSBoMyAqICg1ICogcjgpO1xuICAgIGQxICs9IGg0ICogKDUgKiByNyk7XG4gICAgYyA9IChkMSA+Pj4gMTMpOyBkMSAmPSAweDFmZmY7XG4gICAgZDEgKz0gaDUgKiAoNSAqIHI2KTtcbiAgICBkMSArPSBoNiAqICg1ICogcjUpO1xuICAgIGQxICs9IGg3ICogKDUgKiByNCk7XG4gICAgZDEgKz0gaDggKiAoNSAqIHIzKTtcbiAgICBkMSArPSBoOSAqICg1ICogcjIpO1xuICAgIGMgKz0gKGQxID4+PiAxMyk7IGQxICY9IDB4MWZmZjtcblxuICAgIGQyID0gYztcbiAgICBkMiArPSBoMCAqIHIyO1xuICAgIGQyICs9IGgxICogcjE7XG4gICAgZDIgKz0gaDIgKiByMDtcbiAgICBkMiArPSBoMyAqICg1ICogcjkpO1xuICAgIGQyICs9IGg0ICogKDUgKiByOCk7XG4gICAgYyA9IChkMiA+Pj4gMTMpOyBkMiAmPSAweDFmZmY7XG4gICAgZDIgKz0gaDUgKiAoNSAqIHI3KTtcbiAgICBkMiArPSBoNiAqICg1ICogcjYpO1xuICAgIGQyICs9IGg3ICogKDUgKiByNSk7XG4gICAgZDIgKz0gaDggKiAoNSAqIHI0KTtcbiAgICBkMiArPSBoOSAqICg1ICogcjMpO1xuICAgIGMgKz0gKGQyID4+PiAxMyk7IGQyICY9IDB4MWZmZjtcblxuICAgIGQzID0gYztcbiAgICBkMyArPSBoMCAqIHIzO1xuICAgIGQzICs9IGgxICogcjI7XG4gICAgZDMgKz0gaDIgKiByMTtcbiAgICBkMyArPSBoMyAqIHIwO1xuICAgIGQzICs9IGg0ICogKDUgKiByOSk7XG4gICAgYyA9IChkMyA+Pj4gMTMpOyBkMyAmPSAweDFmZmY7XG4gICAgZDMgKz0gaDUgKiAoNSAqIHI4KTtcbiAgICBkMyArPSBoNiAqICg1ICogcjcpO1xuICAgIGQzICs9IGg3ICogKDUgKiByNik7XG4gICAgZDMgKz0gaDggKiAoNSAqIHI1KTtcbiAgICBkMyArPSBoOSAqICg1ICogcjQpO1xuICAgIGMgKz0gKGQzID4+PiAxMyk7IGQzICY9IDB4MWZmZjtcblxuICAgIGQ0ID0gYztcbiAgICBkNCArPSBoMCAqIHI0O1xuICAgIGQ0ICs9IGgxICogcjM7XG4gICAgZDQgKz0gaDIgKiByMjtcbiAgICBkNCArPSBoMyAqIHIxO1xuICAgIGQ0ICs9IGg0ICogcjA7XG4gICAgYyA9IChkNCA+Pj4gMTMpOyBkNCAmPSAweDFmZmY7XG4gICAgZDQgKz0gaDUgKiAoNSAqIHI5KTtcbiAgICBkNCArPSBoNiAqICg1ICogcjgpO1xuICAgIGQ0ICs9IGg3ICogKDUgKiByNyk7XG4gICAgZDQgKz0gaDggKiAoNSAqIHI2KTtcbiAgICBkNCArPSBoOSAqICg1ICogcjUpO1xuICAgIGMgKz0gKGQ0ID4+PiAxMyk7IGQ0ICY9IDB4MWZmZjtcblxuICAgIGQ1ID0gYztcbiAgICBkNSArPSBoMCAqIHI1O1xuICAgIGQ1ICs9IGgxICogcjQ7XG4gICAgZDUgKz0gaDIgKiByMztcbiAgICBkNSArPSBoMyAqIHIyO1xuICAgIGQ1ICs9IGg0ICogcjE7XG4gICAgYyA9IChkNSA+Pj4gMTMpOyBkNSAmPSAweDFmZmY7XG4gICAgZDUgKz0gaDUgKiByMDtcbiAgICBkNSArPSBoNiAqICg1ICogcjkpO1xuICAgIGQ1ICs9IGg3ICogKDUgKiByOCk7XG4gICAgZDUgKz0gaDggKiAoNSAqIHI3KTtcbiAgICBkNSArPSBoOSAqICg1ICogcjYpO1xuICAgIGMgKz0gKGQ1ID4+PiAxMyk7IGQ1ICY9IDB4MWZmZjtcblxuICAgIGQ2ID0gYztcbiAgICBkNiArPSBoMCAqIHI2O1xuICAgIGQ2ICs9IGgxICogcjU7XG4gICAgZDYgKz0gaDIgKiByNDtcbiAgICBkNiArPSBoMyAqIHIzO1xuICAgIGQ2ICs9IGg0ICogcjI7XG4gICAgYyA9IChkNiA+Pj4gMTMpOyBkNiAmPSAweDFmZmY7XG4gICAgZDYgKz0gaDUgKiByMTtcbiAgICBkNiArPSBoNiAqIHIwO1xuICAgIGQ2ICs9IGg3ICogKDUgKiByOSk7XG4gICAgZDYgKz0gaDggKiAoNSAqIHI4KTtcbiAgICBkNiArPSBoOSAqICg1ICogcjcpO1xuICAgIGMgKz0gKGQ2ID4+PiAxMyk7IGQ2ICY9IDB4MWZmZjtcblxuICAgIGQ3ID0gYztcbiAgICBkNyArPSBoMCAqIHI3O1xuICAgIGQ3ICs9IGgxICogcjY7XG4gICAgZDcgKz0gaDIgKiByNTtcbiAgICBkNyArPSBoMyAqIHI0O1xuICAgIGQ3ICs9IGg0ICogcjM7XG4gICAgYyA9IChkNyA+Pj4gMTMpOyBkNyAmPSAweDFmZmY7XG4gICAgZDcgKz0gaDUgKiByMjtcbiAgICBkNyArPSBoNiAqIHIxO1xuICAgIGQ3ICs9IGg3ICogcjA7XG4gICAgZDcgKz0gaDggKiAoNSAqIHI5KTtcbiAgICBkNyArPSBoOSAqICg1ICogcjgpO1xuICAgIGMgKz0gKGQ3ID4+PiAxMyk7IGQ3ICY9IDB4MWZmZjtcblxuICAgIGQ4ID0gYztcbiAgICBkOCArPSBoMCAqIHI4O1xuICAgIGQ4ICs9IGgxICogcjc7XG4gICAgZDggKz0gaDIgKiByNjtcbiAgICBkOCArPSBoMyAqIHI1O1xuICAgIGQ4ICs9IGg0ICogcjQ7XG4gICAgYyA9IChkOCA+Pj4gMTMpOyBkOCAmPSAweDFmZmY7XG4gICAgZDggKz0gaDUgKiByMztcbiAgICBkOCArPSBoNiAqIHIyO1xuICAgIGQ4ICs9IGg3ICogcjE7XG4gICAgZDggKz0gaDggKiByMDtcbiAgICBkOCArPSBoOSAqICg1ICogcjkpO1xuICAgIGMgKz0gKGQ4ID4+PiAxMyk7IGQ4ICY9IDB4MWZmZjtcblxuICAgIGQ5ID0gYztcbiAgICBkOSArPSBoMCAqIHI5O1xuICAgIGQ5ICs9IGgxICogcjg7XG4gICAgZDkgKz0gaDIgKiByNztcbiAgICBkOSArPSBoMyAqIHI2O1xuICAgIGQ5ICs9IGg0ICogcjU7XG4gICAgYyA9IChkOSA+Pj4gMTMpOyBkOSAmPSAweDFmZmY7XG4gICAgZDkgKz0gaDUgKiByNDtcbiAgICBkOSArPSBoNiAqIHIzO1xuICAgIGQ5ICs9IGg3ICogcjI7XG4gICAgZDkgKz0gaDggKiByMTtcbiAgICBkOSArPSBoOSAqIHIwO1xuICAgIGMgKz0gKGQ5ID4+PiAxMyk7IGQ5ICY9IDB4MWZmZjtcblxuICAgIGMgPSAoKChjIDw8IDIpICsgYykpIHwgMDtcbiAgICBjID0gKGMgKyBkMCkgfCAwO1xuICAgIGQwID0gYyAmIDB4MWZmZjtcbiAgICBjID0gKGMgPj4+IDEzKTtcbiAgICBkMSArPSBjO1xuXG4gICAgaDAgPSBkMDtcbiAgICBoMSA9IGQxO1xuICAgIGgyID0gZDI7XG4gICAgaDMgPSBkMztcbiAgICBoNCA9IGQ0O1xuICAgIGg1ID0gZDU7XG4gICAgaDYgPSBkNjtcbiAgICBoNyA9IGQ3O1xuICAgIGg4ID0gZDg7XG4gICAgaDkgPSBkOTtcblxuICAgIG1wb3MgKz0gMTY7XG4gICAgYnl0ZXMgLT0gMTY7XG4gIH1cbiAgdGhpcy5oWzBdID0gaDA7XG4gIHRoaXMuaFsxXSA9IGgxO1xuICB0aGlzLmhbMl0gPSBoMjtcbiAgdGhpcy5oWzNdID0gaDM7XG4gIHRoaXMuaFs0XSA9IGg0O1xuICB0aGlzLmhbNV0gPSBoNTtcbiAgdGhpcy5oWzZdID0gaDY7XG4gIHRoaXMuaFs3XSA9IGg3O1xuICB0aGlzLmhbOF0gPSBoODtcbiAgdGhpcy5oWzldID0gaDk7XG59O1xuXG5wb2x5MTMwNS5wcm90b3R5cGUuZmluaXNoID0gZnVuY3Rpb24obWFjLCBtYWNwb3MpIHtcbiAgdmFyIGcgPSBuZXcgVWludDE2QXJyYXkoMTApO1xuICB2YXIgYywgbWFzaywgZiwgaTtcblxuICBpZiAodGhpcy5sZWZ0b3Zlcikge1xuICAgIGkgPSB0aGlzLmxlZnRvdmVyO1xuICAgIHRoaXMuYnVmZmVyW2krK10gPSAxO1xuICAgIGZvciAoOyBpIDwgMTY7IGkrKykgdGhpcy5idWZmZXJbaV0gPSAwO1xuICAgIHRoaXMuZmluID0gMTtcbiAgICB0aGlzLmJsb2Nrcyh0aGlzLmJ1ZmZlciwgMCwgMTYpO1xuICB9XG5cbiAgYyA9IHRoaXMuaFsxXSA+Pj4gMTM7XG4gIHRoaXMuaFsxXSAmPSAweDFmZmY7XG4gIGZvciAoaSA9IDI7IGkgPCAxMDsgaSsrKSB7XG4gICAgdGhpcy5oW2ldICs9IGM7XG4gICAgYyA9IHRoaXMuaFtpXSA+Pj4gMTM7XG4gICAgdGhpcy5oW2ldICY9IDB4MWZmZjtcbiAgfVxuICB0aGlzLmhbMF0gKz0gKGMgKiA1KTtcbiAgYyA9IHRoaXMuaFswXSA+Pj4gMTM7XG4gIHRoaXMuaFswXSAmPSAweDFmZmY7XG4gIHRoaXMuaFsxXSArPSBjO1xuICBjID0gdGhpcy5oWzFdID4+PiAxMztcbiAgdGhpcy5oWzFdICY9IDB4MWZmZjtcbiAgdGhpcy5oWzJdICs9IGM7XG5cbiAgZ1swXSA9IHRoaXMuaFswXSArIDU7XG4gIGMgPSBnWzBdID4+PiAxMztcbiAgZ1swXSAmPSAweDFmZmY7XG4gIGZvciAoaSA9IDE7IGkgPCAxMDsgaSsrKSB7XG4gICAgZ1tpXSA9IHRoaXMuaFtpXSArIGM7XG4gICAgYyA9IGdbaV0gPj4+IDEzO1xuICAgIGdbaV0gJj0gMHgxZmZmO1xuICB9XG4gIGdbOV0gLT0gKDEgPDwgMTMpO1xuXG4gIG1hc2sgPSAoYyBeIDEpIC0gMTtcbiAgZm9yIChpID0gMDsgaSA8IDEwOyBpKyspIGdbaV0gJj0gbWFzaztcbiAgbWFzayA9IH5tYXNrO1xuICBmb3IgKGkgPSAwOyBpIDwgMTA7IGkrKykgdGhpcy5oW2ldID0gKHRoaXMuaFtpXSAmIG1hc2spIHwgZ1tpXTtcblxuICB0aGlzLmhbMF0gPSAoKHRoaXMuaFswXSAgICAgICApIHwgKHRoaXMuaFsxXSA8PCAxMykgICAgICAgICAgICAgICAgICAgICkgJiAweGZmZmY7XG4gIHRoaXMuaFsxXSA9ICgodGhpcy5oWzFdID4+PiAgMykgfCAodGhpcy5oWzJdIDw8IDEwKSAgICAgICAgICAgICAgICAgICAgKSAmIDB4ZmZmZjtcbiAgdGhpcy5oWzJdID0gKCh0aGlzLmhbMl0gPj4+ICA2KSB8ICh0aGlzLmhbM10gPDwgIDcpICAgICAgICAgICAgICAgICAgICApICYgMHhmZmZmO1xuICB0aGlzLmhbM10gPSAoKHRoaXMuaFszXSA+Pj4gIDkpIHwgKHRoaXMuaFs0XSA8PCAgNCkgICAgICAgICAgICAgICAgICAgICkgJiAweGZmZmY7XG4gIHRoaXMuaFs0XSA9ICgodGhpcy5oWzRdID4+PiAxMikgfCAodGhpcy5oWzVdIDw8ICAxKSB8ICh0aGlzLmhbNl0gPDwgMTQpKSAmIDB4ZmZmZjtcbiAgdGhpcy5oWzVdID0gKCh0aGlzLmhbNl0gPj4+ICAyKSB8ICh0aGlzLmhbN10gPDwgMTEpICAgICAgICAgICAgICAgICAgICApICYgMHhmZmZmO1xuICB0aGlzLmhbNl0gPSAoKHRoaXMuaFs3XSA+Pj4gIDUpIHwgKHRoaXMuaFs4XSA8PCAgOCkgICAgICAgICAgICAgICAgICAgICkgJiAweGZmZmY7XG4gIHRoaXMuaFs3XSA9ICgodGhpcy5oWzhdID4+PiAgOCkgfCAodGhpcy5oWzldIDw8ICA1KSAgICAgICAgICAgICAgICAgICAgKSAmIDB4ZmZmZjtcblxuICBmID0gdGhpcy5oWzBdICsgdGhpcy5wYWRbMF07XG4gIHRoaXMuaFswXSA9IGYgJiAweGZmZmY7XG4gIGZvciAoaSA9IDE7IGkgPCA4OyBpKyspIHtcbiAgICBmID0gKCgodGhpcy5oW2ldICsgdGhpcy5wYWRbaV0pIHwgMCkgKyAoZiA+Pj4gMTYpKSB8IDA7XG4gICAgdGhpcy5oW2ldID0gZiAmIDB4ZmZmZjtcbiAgfVxuXG4gIG1hY1ttYWNwb3MrIDBdID0gKHRoaXMuaFswXSA+Pj4gMCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKyAxXSA9ICh0aGlzLmhbMF0gPj4+IDgpICYgMHhmZjtcbiAgbWFjW21hY3BvcysgMl0gPSAodGhpcy5oWzFdID4+PiAwKSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrIDNdID0gKHRoaXMuaFsxXSA+Pj4gOCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKyA0XSA9ICh0aGlzLmhbMl0gPj4+IDApICYgMHhmZjtcbiAgbWFjW21hY3BvcysgNV0gPSAodGhpcy5oWzJdID4+PiA4KSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrIDZdID0gKHRoaXMuaFszXSA+Pj4gMCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKyA3XSA9ICh0aGlzLmhbM10gPj4+IDgpICYgMHhmZjtcbiAgbWFjW21hY3BvcysgOF0gPSAodGhpcy5oWzRdID4+PiAwKSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrIDldID0gKHRoaXMuaFs0XSA+Pj4gOCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKzEwXSA9ICh0aGlzLmhbNV0gPj4+IDApICYgMHhmZjtcbiAgbWFjW21hY3BvcysxMV0gPSAodGhpcy5oWzVdID4+PiA4KSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrMTJdID0gKHRoaXMuaFs2XSA+Pj4gMCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKzEzXSA9ICh0aGlzLmhbNl0gPj4+IDgpICYgMHhmZjtcbiAgbWFjW21hY3BvcysxNF0gPSAodGhpcy5oWzddID4+PiAwKSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrMTVdID0gKHRoaXMuaFs3XSA+Pj4gOCkgJiAweGZmO1xufTtcblxucG9seTEzMDUucHJvdG90eXBlLnVwZGF0ZSA9IGZ1bmN0aW9uKG0sIG1wb3MsIGJ5dGVzKSB7XG4gIHZhciBpLCB3YW50O1xuXG4gIGlmICh0aGlzLmxlZnRvdmVyKSB7XG4gICAgd2FudCA9ICgxNiAtIHRoaXMubGVmdG92ZXIpO1xuICAgIGlmICh3YW50ID4gYnl0ZXMpXG4gICAgICB3YW50ID0gYnl0ZXM7XG4gICAgZm9yIChpID0gMDsgaSA8IHdhbnQ7IGkrKylcbiAgICAgIHRoaXMuYnVmZmVyW3RoaXMubGVmdG92ZXIgKyBpXSA9IG1bbXBvcytpXTtcbiAgICBieXRlcyAtPSB3YW50O1xuICAgIG1wb3MgKz0gd2FudDtcbiAgICB0aGlzLmxlZnRvdmVyICs9IHdhbnQ7XG4gICAgaWYgKHRoaXMubGVmdG92ZXIgPCAxNilcbiAgICAgIHJldHVybjtcbiAgICB0aGlzLmJsb2Nrcyh0aGlzLmJ1ZmZlciwgMCwgMTYpO1xuICAgIHRoaXMubGVmdG92ZXIgPSAwO1xuICB9XG5cbiAgaWYgKGJ5dGVzID49IDE2KSB7XG4gICAgd2FudCA9IGJ5dGVzIC0gKGJ5dGVzICUgMTYpO1xuICAgIHRoaXMuYmxvY2tzKG0sIG1wb3MsIHdhbnQpO1xuICAgIG1wb3MgKz0gd2FudDtcbiAgICBieXRlcyAtPSB3YW50O1xuICB9XG5cbiAgaWYgKGJ5dGVzKSB7XG4gICAgZm9yIChpID0gMDsgaSA8IGJ5dGVzOyBpKyspXG4gICAgICB0aGlzLmJ1ZmZlclt0aGlzLmxlZnRvdmVyICsgaV0gPSBtW21wb3MraV07XG4gICAgdGhpcy5sZWZ0b3ZlciArPSBieXRlcztcbiAgfVxufTtcblxuZnVuY3Rpb24gY3J5cHRvX29uZXRpbWVhdXRoKG91dCwgb3V0cG9zLCBtLCBtcG9zLCBuLCBrKSB7XG4gIHZhciBzID0gbmV3IHBvbHkxMzA1KGspO1xuICBzLnVwZGF0ZShtLCBtcG9zLCBuKTtcbiAgcy5maW5pc2gob3V0LCBvdXRwb3MpO1xuICByZXR1cm4gMDtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX29uZXRpbWVhdXRoX3ZlcmlmeShoLCBocG9zLCBtLCBtcG9zLCBuLCBrKSB7XG4gIHZhciB4ID0gbmV3IFVpbnQ4QXJyYXkoMTYpO1xuICBjcnlwdG9fb25ldGltZWF1dGgoeCwwLG0sbXBvcyxuLGspO1xuICByZXR1cm4gY3J5cHRvX3ZlcmlmeV8xNihoLGhwb3MseCwwKTtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX3NlY3JldGJveChjLG0sZCxuLGspIHtcbiAgdmFyIGk7XG4gIGlmIChkIDwgMzIpIHJldHVybiAtMTtcbiAgY3J5cHRvX3N0cmVhbV94b3IoYywwLG0sMCxkLG4sayk7XG4gIGNyeXB0b19vbmV0aW1lYXV0aChjLCAxNiwgYywgMzIsIGQgLSAzMiwgYyk7XG4gIGZvciAoaSA9IDA7IGkgPCAxNjsgaSsrKSBjW2ldID0gMDtcbiAgcmV0dXJuIDA7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19zZWNyZXRib3hfb3BlbihtLGMsZCxuLGspIHtcbiAgdmFyIGk7XG4gIHZhciB4ID0gbmV3IFVpbnQ4QXJyYXkoMzIpO1xuICBpZiAoZCA8IDMyKSByZXR1cm4gLTE7XG4gIGNyeXB0b19zdHJlYW0oeCwwLDMyLG4sayk7XG4gIGlmIChjcnlwdG9fb25ldGltZWF1dGhfdmVyaWZ5KGMsIDE2LGMsIDMyLGQgLSAzMix4KSAhPT0gMCkgcmV0dXJuIC0xO1xuICBjcnlwdG9fc3RyZWFtX3hvcihtLDAsYywwLGQsbixrKTtcbiAgZm9yIChpID0gMDsgaSA8IDMyOyBpKyspIG1baV0gPSAwO1xuICByZXR1cm4gMDtcbn1cblxuZnVuY3Rpb24gc2V0MjU1MTkociwgYSkge1xuICB2YXIgaTtcbiAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIHJbaV0gPSBhW2ldfDA7XG59XG5cbmZ1bmN0aW9uIGNhcjI1NTE5KG8pIHtcbiAgdmFyIGksIHYsIGMgPSAxO1xuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykge1xuICAgIHYgPSBvW2ldICsgYyArIDY1NTM1O1xuICAgIGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7XG4gICAgb1tpXSA9IHYgLSBjICogNjU1MzY7XG4gIH1cbiAgb1swXSArPSBjLTEgKyAzNyAqIChjLTEpO1xufVxuXG5mdW5jdGlvbiBzZWwyNTUxOShwLCBxLCBiKSB7XG4gIHZhciB0LCBjID0gfihiLTEpO1xuICBmb3IgKHZhciBpID0gMDsgaSA8IDE2OyBpKyspIHtcbiAgICB0ID0gYyAmIChwW2ldIF4gcVtpXSk7XG4gICAgcFtpXSBePSB0O1xuICAgIHFbaV0gXj0gdDtcbiAgfVxufVxuXG5mdW5jdGlvbiBwYWNrMjU1MTkobywgbikge1xuICB2YXIgaSwgaiwgYjtcbiAgdmFyIG0gPSBnZigpLCB0ID0gZ2YoKTtcbiAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIHRbaV0gPSBuW2ldO1xuICBjYXIyNTUxOSh0KTtcbiAgY2FyMjU1MTkodCk7XG4gIGNhcjI1NTE5KHQpO1xuICBmb3IgKGogPSAwOyBqIDwgMjsgaisrKSB7XG4gICAgbVswXSA9IHRbMF0gLSAweGZmZWQ7XG4gICAgZm9yIChpID0gMTsgaSA8IDE1OyBpKyspIHtcbiAgICAgIG1baV0gPSB0W2ldIC0gMHhmZmZmIC0gKChtW2ktMV0+PjE2KSAmIDEpO1xuICAgICAgbVtpLTFdICY9IDB4ZmZmZjtcbiAgICB9XG4gICAgbVsxNV0gPSB0WzE1XSAtIDB4N2ZmZiAtICgobVsxNF0+PjE2KSAmIDEpO1xuICAgIGIgPSAobVsxNV0+PjE2KSAmIDE7XG4gICAgbVsxNF0gJj0gMHhmZmZmO1xuICAgIHNlbDI1NTE5KHQsIG0sIDEtYik7XG4gIH1cbiAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIHtcbiAgICBvWzIqaV0gPSB0W2ldICYgMHhmZjtcbiAgICBvWzIqaSsxXSA9IHRbaV0+Pjg7XG4gIH1cbn1cblxuZnVuY3Rpb24gbmVxMjU1MTkoYSwgYikge1xuICB2YXIgYyA9IG5ldyBVaW50OEFycmF5KDMyKSwgZCA9IG5ldyBVaW50OEFycmF5KDMyKTtcbiAgcGFjazI1NTE5KGMsIGEpO1xuICBwYWNrMjU1MTkoZCwgYik7XG4gIHJldHVybiBjcnlwdG9fdmVyaWZ5XzMyKGMsIDAsIGQsIDApO1xufVxuXG5mdW5jdGlvbiBwYXIyNTUxOShhKSB7XG4gIHZhciBkID0gbmV3IFVpbnQ4QXJyYXkoMzIpO1xuICBwYWNrMjU1MTkoZCwgYSk7XG4gIHJldHVybiBkWzBdICYgMTtcbn1cblxuZnVuY3Rpb24gdW5wYWNrMjU1MTkobywgbikge1xuICB2YXIgaTtcbiAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIG9baV0gPSBuWzIqaV0gKyAoblsyKmkrMV0gPDwgOCk7XG4gIG9bMTVdICY9IDB4N2ZmZjtcbn1cblxuZnVuY3Rpb24gQShvLCBhLCBiKSB7XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgMTY7IGkrKykgb1tpXSA9IGFbaV0gKyBiW2ldO1xufVxuXG5mdW5jdGlvbiBaKG8sIGEsIGIpIHtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCAxNjsgaSsrKSBvW2ldID0gYVtpXSAtIGJbaV07XG59XG5cbmZ1bmN0aW9uIE0obywgYSwgYikge1xuICB2YXIgdiwgYyxcbiAgICAgdDAgPSAwLCAgdDEgPSAwLCAgdDIgPSAwLCAgdDMgPSAwLCAgdDQgPSAwLCAgdDUgPSAwLCAgdDYgPSAwLCAgdDcgPSAwLFxuICAgICB0OCA9IDAsICB0OSA9IDAsIHQxMCA9IDAsIHQxMSA9IDAsIHQxMiA9IDAsIHQxMyA9IDAsIHQxNCA9IDAsIHQxNSA9IDAsXG4gICAgdDE2ID0gMCwgdDE3ID0gMCwgdDE4ID0gMCwgdDE5ID0gMCwgdDIwID0gMCwgdDIxID0gMCwgdDIyID0gMCwgdDIzID0gMCxcbiAgICB0MjQgPSAwLCB0MjUgPSAwLCB0MjYgPSAwLCB0MjcgPSAwLCB0MjggPSAwLCB0MjkgPSAwLCB0MzAgPSAwLFxuICAgIGIwID0gYlswXSxcbiAgICBiMSA9IGJbMV0sXG4gICAgYjIgPSBiWzJdLFxuICAgIGIzID0gYlszXSxcbiAgICBiNCA9IGJbNF0sXG4gICAgYjUgPSBiWzVdLFxuICAgIGI2ID0gYls2XSxcbiAgICBiNyA9IGJbN10sXG4gICAgYjggPSBiWzhdLFxuICAgIGI5ID0gYls5XSxcbiAgICBiMTAgPSBiWzEwXSxcbiAgICBiMTEgPSBiWzExXSxcbiAgICBiMTIgPSBiWzEyXSxcbiAgICBiMTMgPSBiWzEzXSxcbiAgICBiMTQgPSBiWzE0XSxcbiAgICBiMTUgPSBiWzE1XTtcblxuICB2ID0gYVswXTtcbiAgdDAgKz0gdiAqIGIwO1xuICB0MSArPSB2ICogYjE7XG4gIHQyICs9IHYgKiBiMjtcbiAgdDMgKz0gdiAqIGIzO1xuICB0NCArPSB2ICogYjQ7XG4gIHQ1ICs9IHYgKiBiNTtcbiAgdDYgKz0gdiAqIGI2O1xuICB0NyArPSB2ICogYjc7XG4gIHQ4ICs9IHYgKiBiODtcbiAgdDkgKz0gdiAqIGI5O1xuICB0MTAgKz0gdiAqIGIxMDtcbiAgdDExICs9IHYgKiBiMTE7XG4gIHQxMiArPSB2ICogYjEyO1xuICB0MTMgKz0gdiAqIGIxMztcbiAgdDE0ICs9IHYgKiBiMTQ7XG4gIHQxNSArPSB2ICogYjE1O1xuICB2ID0gYVsxXTtcbiAgdDEgKz0gdiAqIGIwO1xuICB0MiArPSB2ICogYjE7XG4gIHQzICs9IHYgKiBiMjtcbiAgdDQgKz0gdiAqIGIzO1xuICB0NSArPSB2ICogYjQ7XG4gIHQ2ICs9IHYgKiBiNTtcbiAgdDcgKz0gdiAqIGI2O1xuICB0OCArPSB2ICogYjc7XG4gIHQ5ICs9IHYgKiBiODtcbiAgdDEwICs9IHYgKiBiOTtcbiAgdDExICs9IHYgKiBiMTA7XG4gIHQxMiArPSB2ICogYjExO1xuICB0MTMgKz0gdiAqIGIxMjtcbiAgdDE0ICs9IHYgKiBiMTM7XG4gIHQxNSArPSB2ICogYjE0O1xuICB0MTYgKz0gdiAqIGIxNTtcbiAgdiA9IGFbMl07XG4gIHQyICs9IHYgKiBiMDtcbiAgdDMgKz0gdiAqIGIxO1xuICB0NCArPSB2ICogYjI7XG4gIHQ1ICs9IHYgKiBiMztcbiAgdDYgKz0gdiAqIGI0O1xuICB0NyArPSB2ICogYjU7XG4gIHQ4ICs9IHYgKiBiNjtcbiAgdDkgKz0gdiAqIGI3O1xuICB0MTAgKz0gdiAqIGI4O1xuICB0MTEgKz0gdiAqIGI5O1xuICB0MTIgKz0gdiAqIGIxMDtcbiAgdDEzICs9IHYgKiBiMTE7XG4gIHQxNCArPSB2ICogYjEyO1xuICB0MTUgKz0gdiAqIGIxMztcbiAgdDE2ICs9IHYgKiBiMTQ7XG4gIHQxNyArPSB2ICogYjE1O1xuICB2ID0gYVszXTtcbiAgdDMgKz0gdiAqIGIwO1xuICB0NCArPSB2ICogYjE7XG4gIHQ1ICs9IHYgKiBiMjtcbiAgdDYgKz0gdiAqIGIzO1xuICB0NyArPSB2ICogYjQ7XG4gIHQ4ICs9IHYgKiBiNTtcbiAgdDkgKz0gdiAqIGI2O1xuICB0MTAgKz0gdiAqIGI3O1xuICB0MTEgKz0gdiAqIGI4O1xuICB0MTIgKz0gdiAqIGI5O1xuICB0MTMgKz0gdiAqIGIxMDtcbiAgdDE0ICs9IHYgKiBiMTE7XG4gIHQxNSArPSB2ICogYjEyO1xuICB0MTYgKz0gdiAqIGIxMztcbiAgdDE3ICs9IHYgKiBiMTQ7XG4gIHQxOCArPSB2ICogYjE1O1xuICB2ID0gYVs0XTtcbiAgdDQgKz0gdiAqIGIwO1xuICB0NSArPSB2ICogYjE7XG4gIHQ2ICs9IHYgKiBiMjtcbiAgdDcgKz0gdiAqIGIzO1xuICB0OCArPSB2ICogYjQ7XG4gIHQ5ICs9IHYgKiBiNTtcbiAgdDEwICs9IHYgKiBiNjtcbiAgdDExICs9IHYgKiBiNztcbiAgdDEyICs9IHYgKiBiODtcbiAgdDEzICs9IHYgKiBiOTtcbiAgdDE0ICs9IHYgKiBiMTA7XG4gIHQxNSArPSB2ICogYjExO1xuICB0MTYgKz0gdiAqIGIxMjtcbiAgdDE3ICs9IHYgKiBiMTM7XG4gIHQxOCArPSB2ICogYjE0O1xuICB0MTkgKz0gdiAqIGIxNTtcbiAgdiA9IGFbNV07XG4gIHQ1ICs9IHYgKiBiMDtcbiAgdDYgKz0gdiAqIGIxO1xuICB0NyArPSB2ICogYjI7XG4gIHQ4ICs9IHYgKiBiMztcbiAgdDkgKz0gdiAqIGI0O1xuICB0MTAgKz0gdiAqIGI1O1xuICB0MTEgKz0gdiAqIGI2O1xuICB0MTIgKz0gdiAqIGI3O1xuICB0MTMgKz0gdiAqIGI4O1xuICB0MTQgKz0gdiAqIGI5O1xuICB0MTUgKz0gdiAqIGIxMDtcbiAgdDE2ICs9IHYgKiBiMTE7XG4gIHQxNyArPSB2ICogYjEyO1xuICB0MTggKz0gdiAqIGIxMztcbiAgdDE5ICs9IHYgKiBiMTQ7XG4gIHQyMCArPSB2ICogYjE1O1xuICB2ID0gYVs2XTtcbiAgdDYgKz0gdiAqIGIwO1xuICB0NyArPSB2ICogYjE7XG4gIHQ4ICs9IHYgKiBiMjtcbiAgdDkgKz0gdiAqIGIzO1xuICB0MTAgKz0gdiAqIGI0O1xuICB0MTEgKz0gdiAqIGI1O1xuICB0MTIgKz0gdiAqIGI2O1xuICB0MTMgKz0gdiAqIGI3O1xuICB0MTQgKz0gdiAqIGI4O1xuICB0MTUgKz0gdiAqIGI5O1xuICB0MTYgKz0gdiAqIGIxMDtcbiAgdDE3ICs9IHYgKiBiMTE7XG4gIHQxOCArPSB2ICogYjEyO1xuICB0MTkgKz0gdiAqIGIxMztcbiAgdDIwICs9IHYgKiBiMTQ7XG4gIHQyMSArPSB2ICogYjE1O1xuICB2ID0gYVs3XTtcbiAgdDcgKz0gdiAqIGIwO1xuICB0OCArPSB2ICogYjE7XG4gIHQ5ICs9IHYgKiBiMjtcbiAgdDEwICs9IHYgKiBiMztcbiAgdDExICs9IHYgKiBiNDtcbiAgdDEyICs9IHYgKiBiNTtcbiAgdDEzICs9IHYgKiBiNjtcbiAgdDE0ICs9IHYgKiBiNztcbiAgdDE1ICs9IHYgKiBiODtcbiAgdDE2ICs9IHYgKiBiOTtcbiAgdDE3ICs9IHYgKiBiMTA7XG4gIHQxOCArPSB2ICogYjExO1xuICB0MTkgKz0gdiAqIGIxMjtcbiAgdDIwICs9IHYgKiBiMTM7XG4gIHQyMSArPSB2ICogYjE0O1xuICB0MjIgKz0gdiAqIGIxNTtcbiAgdiA9IGFbOF07XG4gIHQ4ICs9IHYgKiBiMDtcbiAgdDkgKz0gdiAqIGIxO1xuICB0MTAgKz0gdiAqIGIyO1xuICB0MTEgKz0gdiAqIGIzO1xuICB0MTIgKz0gdiAqIGI0O1xuICB0MTMgKz0gdiAqIGI1O1xuICB0MTQgKz0gdiAqIGI2O1xuICB0MTUgKz0gdiAqIGI3O1xuICB0MTYgKz0gdiAqIGI4O1xuICB0MTcgKz0gdiAqIGI5O1xuICB0MTggKz0gdiAqIGIxMDtcbiAgdDE5ICs9IHYgKiBiMTE7XG4gIHQyMCArPSB2ICogYjEyO1xuICB0MjEgKz0gdiAqIGIxMztcbiAgdDIyICs9IHYgKiBiMTQ7XG4gIHQyMyArPSB2ICogYjE1O1xuICB2ID0gYVs5XTtcbiAgdDkgKz0gdiAqIGIwO1xuICB0MTAgKz0gdiAqIGIxO1xuICB0MTEgKz0gdiAqIGIyO1xuICB0MTIgKz0gdiAqIGIzO1xuICB0MTMgKz0gdiAqIGI0O1xuICB0MTQgKz0gdiAqIGI1O1xuICB0MTUgKz0gdiAqIGI2O1xuICB0MTYgKz0gdiAqIGI3O1xuICB0MTcgKz0gdiAqIGI4O1xuICB0MTggKz0gdiAqIGI5O1xuICB0MTkgKz0gdiAqIGIxMDtcbiAgdDIwICs9IHYgKiBiMTE7XG4gIHQyMSArPSB2ICogYjEyO1xuICB0MjIgKz0gdiAqIGIxMztcbiAgdDIzICs9IHYgKiBiMTQ7XG4gIHQyNCArPSB2ICogYjE1O1xuICB2ID0gYVsxMF07XG4gIHQxMCArPSB2ICogYjA7XG4gIHQxMSArPSB2ICogYjE7XG4gIHQxMiArPSB2ICogYjI7XG4gIHQxMyArPSB2ICogYjM7XG4gIHQxNCArPSB2ICogYjQ7XG4gIHQxNSArPSB2ICogYjU7XG4gIHQxNiArPSB2ICogYjY7XG4gIHQxNyArPSB2ICogYjc7XG4gIHQxOCArPSB2ICogYjg7XG4gIHQxOSArPSB2ICogYjk7XG4gIHQyMCArPSB2ICogYjEwO1xuICB0MjEgKz0gdiAqIGIxMTtcbiAgdDIyICs9IHYgKiBiMTI7XG4gIHQyMyArPSB2ICogYjEzO1xuICB0MjQgKz0gdiAqIGIxNDtcbiAgdDI1ICs9IHYgKiBiMTU7XG4gIHYgPSBhWzExXTtcbiAgdDExICs9IHYgKiBiMDtcbiAgdDEyICs9IHYgKiBiMTtcbiAgdDEzICs9IHYgKiBiMjtcbiAgdDE0ICs9IHYgKiBiMztcbiAgdDE1ICs9IHYgKiBiNDtcbiAgdDE2ICs9IHYgKiBiNTtcbiAgdDE3ICs9IHYgKiBiNjtcbiAgdDE4ICs9IHYgKiBiNztcbiAgdDE5ICs9IHYgKiBiODtcbiAgdDIwICs9IHYgKiBiOTtcbiAgdDIxICs9IHYgKiBiMTA7XG4gIHQyMiArPSB2ICogYjExO1xuICB0MjMgKz0gdiAqIGIxMjtcbiAgdDI0ICs9IHYgKiBiMTM7XG4gIHQyNSArPSB2ICogYjE0O1xuICB0MjYgKz0gdiAqIGIxNTtcbiAgdiA9IGFbMTJdO1xuICB0MTIgKz0gdiAqIGIwO1xuICB0MTMgKz0gdiAqIGIxO1xuICB0MTQgKz0gdiAqIGIyO1xuICB0MTUgKz0gdiAqIGIzO1xuICB0MTYgKz0gdiAqIGI0O1xuICB0MTcgKz0gdiAqIGI1O1xuICB0MTggKz0gdiAqIGI2O1xuICB0MTkgKz0gdiAqIGI3O1xuICB0MjAgKz0gdiAqIGI4O1xuICB0MjEgKz0gdiAqIGI5O1xuICB0MjIgKz0gdiAqIGIxMDtcbiAgdDIzICs9IHYgKiBiMTE7XG4gIHQyNCArPSB2ICogYjEyO1xuICB0MjUgKz0gdiAqIGIxMztcbiAgdDI2ICs9IHYgKiBiMTQ7XG4gIHQyNyArPSB2ICogYjE1O1xuICB2ID0gYVsxM107XG4gIHQxMyArPSB2ICogYjA7XG4gIHQxNCArPSB2ICogYjE7XG4gIHQxNSArPSB2ICogYjI7XG4gIHQxNiArPSB2ICogYjM7XG4gIHQxNyArPSB2ICogYjQ7XG4gIHQxOCArPSB2ICogYjU7XG4gIHQxOSArPSB2ICogYjY7XG4gIHQyMCArPSB2ICogYjc7XG4gIHQyMSArPSB2ICogYjg7XG4gIHQyMiArPSB2ICogYjk7XG4gIHQyMyArPSB2ICogYjEwO1xuICB0MjQgKz0gdiAqIGIxMTtcbiAgdDI1ICs9IHYgKiBiMTI7XG4gIHQyNiArPSB2ICogYjEzO1xuICB0MjcgKz0gdiAqIGIxNDtcbiAgdDI4ICs9IHYgKiBiMTU7XG4gIHYgPSBhWzE0XTtcbiAgdDE0ICs9IHYgKiBiMDtcbiAgdDE1ICs9IHYgKiBiMTtcbiAgdDE2ICs9IHYgKiBiMjtcbiAgdDE3ICs9IHYgKiBiMztcbiAgdDE4ICs9IHYgKiBiNDtcbiAgdDE5ICs9IHYgKiBiNTtcbiAgdDIwICs9IHYgKiBiNjtcbiAgdDIxICs9IHYgKiBiNztcbiAgdDIyICs9IHYgKiBiODtcbiAgdDIzICs9IHYgKiBiOTtcbiAgdDI0ICs9IHYgKiBiMTA7XG4gIHQyNSArPSB2ICogYjExO1xuICB0MjYgKz0gdiAqIGIxMjtcbiAgdDI3ICs9IHYgKiBiMTM7XG4gIHQyOCArPSB2ICogYjE0O1xuICB0MjkgKz0gdiAqIGIxNTtcbiAgdiA9IGFbMTVdO1xuICB0MTUgKz0gdiAqIGIwO1xuICB0MTYgKz0gdiAqIGIxO1xuICB0MTcgKz0gdiAqIGIyO1xuICB0MTggKz0gdiAqIGIzO1xuICB0MTkgKz0gdiAqIGI0O1xuICB0MjAgKz0gdiAqIGI1O1xuICB0MjEgKz0gdiAqIGI2O1xuICB0MjIgKz0gdiAqIGI3O1xuICB0MjMgKz0gdiAqIGI4O1xuICB0MjQgKz0gdiAqIGI5O1xuICB0MjUgKz0gdiAqIGIxMDtcbiAgdDI2ICs9IHYgKiBiMTE7XG4gIHQyNyArPSB2ICogYjEyO1xuICB0MjggKz0gdiAqIGIxMztcbiAgdDI5ICs9IHYgKiBiMTQ7XG4gIHQzMCArPSB2ICogYjE1O1xuXG4gIHQwICArPSAzOCAqIHQxNjtcbiAgdDEgICs9IDM4ICogdDE3O1xuICB0MiAgKz0gMzggKiB0MTg7XG4gIHQzICArPSAzOCAqIHQxOTtcbiAgdDQgICs9IDM4ICogdDIwO1xuICB0NSAgKz0gMzggKiB0MjE7XG4gIHQ2ICArPSAzOCAqIHQyMjtcbiAgdDcgICs9IDM4ICogdDIzO1xuICB0OCAgKz0gMzggKiB0MjQ7XG4gIHQ5ICArPSAzOCAqIHQyNTtcbiAgdDEwICs9IDM4ICogdDI2O1xuICB0MTEgKz0gMzggKiB0Mjc7XG4gIHQxMiArPSAzOCAqIHQyODtcbiAgdDEzICs9IDM4ICogdDI5O1xuICB0MTQgKz0gMzggKiB0MzA7XG4gIC8vIHQxNSBsZWZ0IGFzIGlzXG5cbiAgLy8gZmlyc3QgY2FyXG4gIGMgPSAxO1xuICB2ID0gIHQwICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDAgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQxICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDEgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQyICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDIgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQzICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDMgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQ0ICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDQgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQ1ICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDUgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQ2ICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDYgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQ3ICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDcgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQ4ICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDggPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gIHQ5ICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyAgdDkgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gdDEwICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyB0MTAgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gdDExICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyB0MTEgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gdDEyICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyB0MTIgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gdDEzICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyB0MTMgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gdDE0ICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyB0MTQgPSB2IC0gYyAqIDY1NTM2O1xuICB2ID0gdDE1ICsgYyArIDY1NTM1OyBjID0gTWF0aC5mbG9vcih2IC8gNjU1MzYpOyB0MTUgPSB2IC0gYyAqIDY1NTM2O1xuICB0MCArPSBjLTEgKyAzNyAqIChjLTEpO1xuXG4gIC8vIHNlY29uZCBjYXJcbiAgYyA9IDE7XG4gIHYgPSAgdDAgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0MCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDEgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0MSA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDIgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0MiA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDMgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0MyA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDQgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0NCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDUgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0NSA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDYgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0NiA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDcgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0NyA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDggKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0OCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDkgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0OSA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTAgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxMCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTEgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxMSA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTIgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxMiA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTMgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxMyA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTQgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxNCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTUgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxNSA9IHYgLSBjICogNjU1MzY7XG4gIHQwICs9IGMtMSArIDM3ICogKGMtMSk7XG5cbiAgb1sgMF0gPSB0MDtcbiAgb1sgMV0gPSB0MTtcbiAgb1sgMl0gPSB0MjtcbiAgb1sgM10gPSB0MztcbiAgb1sgNF0gPSB0NDtcbiAgb1sgNV0gPSB0NTtcbiAgb1sgNl0gPSB0NjtcbiAgb1sgN10gPSB0NztcbiAgb1sgOF0gPSB0ODtcbiAgb1sgOV0gPSB0OTtcbiAgb1sxMF0gPSB0MTA7XG4gIG9bMTFdID0gdDExO1xuICBvWzEyXSA9IHQxMjtcbiAgb1sxM10gPSB0MTM7XG4gIG9bMTRdID0gdDE0O1xuICBvWzE1XSA9IHQxNTtcbn1cblxuZnVuY3Rpb24gUyhvLCBhKSB7XG4gIE0obywgYSwgYSk7XG59XG5cbmZ1bmN0aW9uIGludjI1NTE5KG8sIGkpIHtcbiAgdmFyIGMgPSBnZigpO1xuICB2YXIgYTtcbiAgZm9yIChhID0gMDsgYSA8IDE2OyBhKyspIGNbYV0gPSBpW2FdO1xuICBmb3IgKGEgPSAyNTM7IGEgPj0gMDsgYS0tKSB7XG4gICAgUyhjLCBjKTtcbiAgICBpZihhICE9PSAyICYmIGEgIT09IDQpIE0oYywgYywgaSk7XG4gIH1cbiAgZm9yIChhID0gMDsgYSA8IDE2OyBhKyspIG9bYV0gPSBjW2FdO1xufVxuXG5mdW5jdGlvbiBwb3cyNTIzKG8sIGkpIHtcbiAgdmFyIGMgPSBnZigpO1xuICB2YXIgYTtcbiAgZm9yIChhID0gMDsgYSA8IDE2OyBhKyspIGNbYV0gPSBpW2FdO1xuICBmb3IgKGEgPSAyNTA7IGEgPj0gMDsgYS0tKSB7XG4gICAgICBTKGMsIGMpO1xuICAgICAgaWYoYSAhPT0gMSkgTShjLCBjLCBpKTtcbiAgfVxuICBmb3IgKGEgPSAwOyBhIDwgMTY7IGErKykgb1thXSA9IGNbYV07XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19zY2FsYXJtdWx0KHEsIG4sIHApIHtcbiAgdmFyIHogPSBuZXcgVWludDhBcnJheSgzMik7XG4gIHZhciB4ID0gbmV3IEZsb2F0NjRBcnJheSg4MCksIHIsIGk7XG4gIHZhciBhID0gZ2YoKSwgYiA9IGdmKCksIGMgPSBnZigpLFxuICAgICAgZCA9IGdmKCksIGUgPSBnZigpLCBmID0gZ2YoKTtcbiAgZm9yIChpID0gMDsgaSA8IDMxOyBpKyspIHpbaV0gPSBuW2ldO1xuICB6WzMxXT0oblszMV0mMTI3KXw2NDtcbiAgelswXSY9MjQ4O1xuICB1bnBhY2syNTUxOSh4LHApO1xuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykge1xuICAgIGJbaV09eFtpXTtcbiAgICBkW2ldPWFbaV09Y1tpXT0wO1xuICB9XG4gIGFbMF09ZFswXT0xO1xuICBmb3IgKGk9MjU0OyBpPj0wOyAtLWkpIHtcbiAgICByPSh6W2k+Pj4zXT4+PihpJjcpKSYxO1xuICAgIHNlbDI1NTE5KGEsYixyKTtcbiAgICBzZWwyNTUxOShjLGQscik7XG4gICAgQShlLGEsYyk7XG4gICAgWihhLGEsYyk7XG4gICAgQShjLGIsZCk7XG4gICAgWihiLGIsZCk7XG4gICAgUyhkLGUpO1xuICAgIFMoZixhKTtcbiAgICBNKGEsYyxhKTtcbiAgICBNKGMsYixlKTtcbiAgICBBKGUsYSxjKTtcbiAgICBaKGEsYSxjKTtcbiAgICBTKGIsYSk7XG4gICAgWihjLGQsZik7XG4gICAgTShhLGMsXzEyMTY2NSk7XG4gICAgQShhLGEsZCk7XG4gICAgTShjLGMsYSk7XG4gICAgTShhLGQsZik7XG4gICAgTShkLGIseCk7XG4gICAgUyhiLGUpO1xuICAgIHNlbDI1NTE5KGEsYixyKTtcbiAgICBzZWwyNTUxOShjLGQscik7XG4gIH1cbiAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIHtcbiAgICB4W2krMTZdPWFbaV07XG4gICAgeFtpKzMyXT1jW2ldO1xuICAgIHhbaSs0OF09YltpXTtcbiAgICB4W2krNjRdPWRbaV07XG4gIH1cbiAgdmFyIHgzMiA9IHguc3ViYXJyYXkoMzIpO1xuICB2YXIgeDE2ID0geC5zdWJhcnJheSgxNik7XG4gIGludjI1NTE5KHgzMix4MzIpO1xuICBNKHgxNix4MTYseDMyKTtcbiAgcGFjazI1NTE5KHEseDE2KTtcbiAgcmV0dXJuIDA7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19zY2FsYXJtdWx0X2Jhc2UocSwgbikge1xuICByZXR1cm4gY3J5cHRvX3NjYWxhcm11bHQocSwgbiwgXzkpO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fYm94X2tleXBhaXIoeSwgeCkge1xuICByYW5kb21ieXRlcyh4LCAzMik7XG4gIHJldHVybiBjcnlwdG9fc2NhbGFybXVsdF9iYXNlKHksIHgpO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fYm94X2JlZm9yZW5tKGssIHksIHgpIHtcbiAgdmFyIHMgPSBuZXcgVWludDhBcnJheSgzMik7XG4gIGNyeXB0b19zY2FsYXJtdWx0KHMsIHgsIHkpO1xuICByZXR1cm4gY3J5cHRvX2NvcmVfaHNhbHNhMjAoaywgXzAsIHMsIHNpZ21hKTtcbn1cblxudmFyIGNyeXB0b19ib3hfYWZ0ZXJubSA9IGNyeXB0b19zZWNyZXRib3g7XG52YXIgY3J5cHRvX2JveF9vcGVuX2FmdGVybm0gPSBjcnlwdG9fc2VjcmV0Ym94X29wZW47XG5cbmZ1bmN0aW9uIGNyeXB0b19ib3goYywgbSwgZCwgbiwgeSwgeCkge1xuICB2YXIgayA9IG5ldyBVaW50OEFycmF5KDMyKTtcbiAgY3J5cHRvX2JveF9iZWZvcmVubShrLCB5LCB4KTtcbiAgcmV0dXJuIGNyeXB0b19ib3hfYWZ0ZXJubShjLCBtLCBkLCBuLCBrKTtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX2JveF9vcGVuKG0sIGMsIGQsIG4sIHksIHgpIHtcbiAgdmFyIGsgPSBuZXcgVWludDhBcnJheSgzMik7XG4gIGNyeXB0b19ib3hfYmVmb3Jlbm0oaywgeSwgeCk7XG4gIHJldHVybiBjcnlwdG9fYm94X29wZW5fYWZ0ZXJubShtLCBjLCBkLCBuLCBrKTtcbn1cblxudmFyIEsgPSBbXG4gIDB4NDI4YTJmOTgsIDB4ZDcyOGFlMjIsIDB4NzEzNzQ0OTEsIDB4MjNlZjY1Y2QsXG4gIDB4YjVjMGZiY2YsIDB4ZWM0ZDNiMmYsIDB4ZTliNWRiYTUsIDB4ODE4OWRiYmMsXG4gIDB4Mzk1NmMyNWIsIDB4ZjM0OGI1MzgsIDB4NTlmMTExZjEsIDB4YjYwNWQwMTksXG4gIDB4OTIzZjgyYTQsIDB4YWYxOTRmOWIsIDB4YWIxYzVlZDUsIDB4ZGE2ZDgxMTgsXG4gIDB4ZDgwN2FhOTgsIDB4YTMwMzAyNDIsIDB4MTI4MzViMDEsIDB4NDU3MDZmYmUsXG4gIDB4MjQzMTg1YmUsIDB4NGVlNGIyOGMsIDB4NTUwYzdkYzMsIDB4ZDVmZmI0ZTIsXG4gIDB4NzJiZTVkNzQsIDB4ZjI3Yjg5NmYsIDB4ODBkZWIxZmUsIDB4M2IxNjk2YjEsXG4gIDB4OWJkYzA2YTcsIDB4MjVjNzEyMzUsIDB4YzE5YmYxNzQsIDB4Y2Y2OTI2OTQsXG4gIDB4ZTQ5YjY5YzEsIDB4OWVmMTRhZDIsIDB4ZWZiZTQ3ODYsIDB4Mzg0ZjI1ZTMsXG4gIDB4MGZjMTlkYzYsIDB4OGI4Y2Q1YjUsIDB4MjQwY2ExY2MsIDB4NzdhYzljNjUsXG4gIDB4MmRlOTJjNmYsIDB4NTkyYjAyNzUsIDB4NGE3NDg0YWEsIDB4NmVhNmU0ODMsXG4gIDB4NWNiMGE5ZGMsIDB4YmQ0MWZiZDQsIDB4NzZmOTg4ZGEsIDB4ODMxMTUzYjUsXG4gIDB4OTgzZTUxNTIsIDB4ZWU2NmRmYWIsIDB4YTgzMWM2NmQsIDB4MmRiNDMyMTAsXG4gIDB4YjAwMzI3YzgsIDB4OThmYjIxM2YsIDB4YmY1OTdmYzcsIDB4YmVlZjBlZTQsXG4gIDB4YzZlMDBiZjMsIDB4M2RhODhmYzIsIDB4ZDVhNzkxNDcsIDB4OTMwYWE3MjUsXG4gIDB4MDZjYTYzNTEsIDB4ZTAwMzgyNmYsIDB4MTQyOTI5NjcsIDB4MGEwZTZlNzAsXG4gIDB4MjdiNzBhODUsIDB4NDZkMjJmZmMsIDB4MmUxYjIxMzgsIDB4NWMyNmM5MjYsXG4gIDB4NGQyYzZkZmMsIDB4NWFjNDJhZWQsIDB4NTMzODBkMTMsIDB4OWQ5NWIzZGYsXG4gIDB4NjUwYTczNTQsIDB4OGJhZjYzZGUsIDB4NzY2YTBhYmIsIDB4M2M3N2IyYTgsXG4gIDB4ODFjMmM5MmUsIDB4NDdlZGFlZTYsIDB4OTI3MjJjODUsIDB4MTQ4MjM1M2IsXG4gIDB4YTJiZmU4YTEsIDB4NGNmMTAzNjQsIDB4YTgxYTY2NGIsIDB4YmM0MjMwMDEsXG4gIDB4YzI0YjhiNzAsIDB4ZDBmODk3OTEsIDB4Yzc2YzUxYTMsIDB4MDY1NGJlMzAsXG4gIDB4ZDE5MmU4MTksIDB4ZDZlZjUyMTgsIDB4ZDY5OTA2MjQsIDB4NTU2NWE5MTAsXG4gIDB4ZjQwZTM1ODUsIDB4NTc3MTIwMmEsIDB4MTA2YWEwNzAsIDB4MzJiYmQxYjgsXG4gIDB4MTlhNGMxMTYsIDB4YjhkMmQwYzgsIDB4MWUzNzZjMDgsIDB4NTE0MWFiNTMsXG4gIDB4Mjc0ODc3NGMsIDB4ZGY4ZWViOTksIDB4MzRiMGJjYjUsIDB4ZTE5YjQ4YTgsXG4gIDB4MzkxYzBjYjMsIDB4YzVjOTVhNjMsIDB4NGVkOGFhNGEsIDB4ZTM0MThhY2IsXG4gIDB4NWI5Y2NhNGYsIDB4Nzc2M2UzNzMsIDB4NjgyZTZmZjMsIDB4ZDZiMmI4YTMsXG4gIDB4NzQ4ZjgyZWUsIDB4NWRlZmIyZmMsIDB4NzhhNTYzNmYsIDB4NDMxNzJmNjAsXG4gIDB4ODRjODc4MTQsIDB4YTFmMGFiNzIsIDB4OGNjNzAyMDgsIDB4MWE2NDM5ZWMsXG4gIDB4OTBiZWZmZmEsIDB4MjM2MzFlMjgsIDB4YTQ1MDZjZWIsIDB4ZGU4MmJkZTksXG4gIDB4YmVmOWEzZjcsIDB4YjJjNjc5MTUsIDB4YzY3MTc4ZjIsIDB4ZTM3MjUzMmIsXG4gIDB4Y2EyNzNlY2UsIDB4ZWEyNjYxOWMsIDB4ZDE4NmI4YzcsIDB4MjFjMGMyMDcsXG4gIDB4ZWFkYTdkZDYsIDB4Y2RlMGViMWUsIDB4ZjU3ZDRmN2YsIDB4ZWU2ZWQxNzgsXG4gIDB4MDZmMDY3YWEsIDB4NzIxNzZmYmEsIDB4MGE2MzdkYzUsIDB4YTJjODk4YTYsXG4gIDB4MTEzZjk4MDQsIDB4YmVmOTBkYWUsIDB4MWI3MTBiMzUsIDB4MTMxYzQ3MWIsXG4gIDB4MjhkYjc3ZjUsIDB4MjMwNDdkODQsIDB4MzJjYWFiN2IsIDB4NDBjNzI0OTMsXG4gIDB4M2M5ZWJlMGEsIDB4MTVjOWJlYmMsIDB4NDMxZDY3YzQsIDB4OWMxMDBkNGMsXG4gIDB4NGNjNWQ0YmUsIDB4Y2IzZTQyYjYsIDB4NTk3ZjI5OWMsIDB4ZmM2NTdlMmEsXG4gIDB4NWZjYjZmYWIsIDB4M2FkNmZhZWMsIDB4NmM0NDE5OGMsIDB4NGE0NzU4MTdcbl07XG5cbmZ1bmN0aW9uIGNyeXB0b19oYXNoYmxvY2tzX2hsKGhoLCBobCwgbSwgbikge1xuICB2YXIgd2ggPSBuZXcgSW50MzJBcnJheSgxNiksIHdsID0gbmV3IEludDMyQXJyYXkoMTYpLFxuICAgICAgYmgwLCBiaDEsIGJoMiwgYmgzLCBiaDQsIGJoNSwgYmg2LCBiaDcsXG4gICAgICBibDAsIGJsMSwgYmwyLCBibDMsIGJsNCwgYmw1LCBibDYsIGJsNyxcbiAgICAgIHRoLCB0bCwgaSwgaiwgaCwgbCwgYSwgYiwgYywgZDtcblxuICB2YXIgYWgwID0gaGhbMF0sXG4gICAgICBhaDEgPSBoaFsxXSxcbiAgICAgIGFoMiA9IGhoWzJdLFxuICAgICAgYWgzID0gaGhbM10sXG4gICAgICBhaDQgPSBoaFs0XSxcbiAgICAgIGFoNSA9IGhoWzVdLFxuICAgICAgYWg2ID0gaGhbNl0sXG4gICAgICBhaDcgPSBoaFs3XSxcblxuICAgICAgYWwwID0gaGxbMF0sXG4gICAgICBhbDEgPSBobFsxXSxcbiAgICAgIGFsMiA9IGhsWzJdLFxuICAgICAgYWwzID0gaGxbM10sXG4gICAgICBhbDQgPSBobFs0XSxcbiAgICAgIGFsNSA9IGhsWzVdLFxuICAgICAgYWw2ID0gaGxbNl0sXG4gICAgICBhbDcgPSBobFs3XTtcblxuICB2YXIgcG9zID0gMDtcbiAgd2hpbGUgKG4gPj0gMTI4KSB7XG4gICAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIHtcbiAgICAgIGogPSA4ICogaSArIHBvcztcbiAgICAgIHdoW2ldID0gKG1baiswXSA8PCAyNCkgfCAobVtqKzFdIDw8IDE2KSB8IChtW2orMl0gPDwgOCkgfCBtW2orM107XG4gICAgICB3bFtpXSA9IChtW2orNF0gPDwgMjQpIHwgKG1bais1XSA8PCAxNikgfCAobVtqKzZdIDw8IDgpIHwgbVtqKzddO1xuICAgIH1cbiAgICBmb3IgKGkgPSAwOyBpIDwgODA7IGkrKykge1xuICAgICAgYmgwID0gYWgwO1xuICAgICAgYmgxID0gYWgxO1xuICAgICAgYmgyID0gYWgyO1xuICAgICAgYmgzID0gYWgzO1xuICAgICAgYmg0ID0gYWg0O1xuICAgICAgYmg1ID0gYWg1O1xuICAgICAgYmg2ID0gYWg2O1xuICAgICAgYmg3ID0gYWg3O1xuXG4gICAgICBibDAgPSBhbDA7XG4gICAgICBibDEgPSBhbDE7XG4gICAgICBibDIgPSBhbDI7XG4gICAgICBibDMgPSBhbDM7XG4gICAgICBibDQgPSBhbDQ7XG4gICAgICBibDUgPSBhbDU7XG4gICAgICBibDYgPSBhbDY7XG4gICAgICBibDcgPSBhbDc7XG5cbiAgICAgIC8vIGFkZFxuICAgICAgaCA9IGFoNztcbiAgICAgIGwgPSBhbDc7XG5cbiAgICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgICBjID0gaCAmIDB4ZmZmZjsgZCA9IGggPj4+IDE2O1xuXG4gICAgICAvLyBTaWdtYTFcbiAgICAgIGggPSAoKGFoNCA+Pj4gMTQpIHwgKGFsNCA8PCAoMzItMTQpKSkgXiAoKGFoNCA+Pj4gMTgpIHwgKGFsNCA8PCAoMzItMTgpKSkgXiAoKGFsNCA+Pj4gKDQxLTMyKSkgfCAoYWg0IDw8ICgzMi0oNDEtMzIpKSkpO1xuICAgICAgbCA9ICgoYWw0ID4+PiAxNCkgfCAoYWg0IDw8ICgzMi0xNCkpKSBeICgoYWw0ID4+PiAxOCkgfCAoYWg0IDw8ICgzMi0xOCkpKSBeICgoYWg0ID4+PiAoNDEtMzIpKSB8IChhbDQgPDwgKDMyLSg0MS0zMikpKSk7XG5cbiAgICAgIGEgKz0gbCAmIDB4ZmZmZjsgYiArPSBsID4+PiAxNjtcbiAgICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgICAgLy8gQ2hcbiAgICAgIGggPSAoYWg0ICYgYWg1KSBeICh+YWg0ICYgYWg2KTtcbiAgICAgIGwgPSAoYWw0ICYgYWw1KSBeICh+YWw0ICYgYWw2KTtcblxuICAgICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgICAvLyBLXG4gICAgICBoID0gS1tpKjJdO1xuICAgICAgbCA9IEtbaSoyKzFdO1xuXG4gICAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgICBjICs9IGggJiAweGZmZmY7IGQgKz0gaCA+Pj4gMTY7XG5cbiAgICAgIC8vIHdcbiAgICAgIGggPSB3aFtpJTE2XTtcbiAgICAgIGwgPSB3bFtpJTE2XTtcblxuICAgICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgICBiICs9IGEgPj4+IDE2O1xuICAgICAgYyArPSBiID4+PiAxNjtcbiAgICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICAgIHRoID0gYyAmIDB4ZmZmZiB8IGQgPDwgMTY7XG4gICAgICB0bCA9IGEgJiAweGZmZmYgfCBiIDw8IDE2O1xuXG4gICAgICAvLyBhZGRcbiAgICAgIGggPSB0aDtcbiAgICAgIGwgPSB0bDtcblxuICAgICAgYSA9IGwgJiAweGZmZmY7IGIgPSBsID4+PiAxNjtcbiAgICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICAgIC8vIFNpZ21hMFxuICAgICAgaCA9ICgoYWgwID4+PiAyOCkgfCAoYWwwIDw8ICgzMi0yOCkpKSBeICgoYWwwID4+PiAoMzQtMzIpKSB8IChhaDAgPDwgKDMyLSgzNC0zMikpKSkgXiAoKGFsMCA+Pj4gKDM5LTMyKSkgfCAoYWgwIDw8ICgzMi0oMzktMzIpKSkpO1xuICAgICAgbCA9ICgoYWwwID4+PiAyOCkgfCAoYWgwIDw8ICgzMi0yOCkpKSBeICgoYWgwID4+PiAoMzQtMzIpKSB8IChhbDAgPDwgKDMyLSgzNC0zMikpKSkgXiAoKGFoMCA+Pj4gKDM5LTMyKSkgfCAoYWwwIDw8ICgzMi0oMzktMzIpKSkpO1xuXG4gICAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgICBjICs9IGggJiAweGZmZmY7IGQgKz0gaCA+Pj4gMTY7XG5cbiAgICAgIC8vIE1halxuICAgICAgaCA9IChhaDAgJiBhaDEpIF4gKGFoMCAmIGFoMikgXiAoYWgxICYgYWgyKTtcbiAgICAgIGwgPSAoYWwwICYgYWwxKSBeIChhbDAgJiBhbDIpIF4gKGFsMSAmIGFsMik7XG5cbiAgICAgIGEgKz0gbCAmIDB4ZmZmZjsgYiArPSBsID4+PiAxNjtcbiAgICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgICAgYiArPSBhID4+PiAxNjtcbiAgICAgIGMgKz0gYiA+Pj4gMTY7XG4gICAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgICBiaDcgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgICBibDcgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICAgIC8vIGFkZFxuICAgICAgaCA9IGJoMztcbiAgICAgIGwgPSBibDM7XG5cbiAgICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgICBjID0gaCAmIDB4ZmZmZjsgZCA9IGggPj4+IDE2O1xuXG4gICAgICBoID0gdGg7XG4gICAgICBsID0gdGw7XG5cbiAgICAgIGEgKz0gbCAmIDB4ZmZmZjsgYiArPSBsID4+PiAxNjtcbiAgICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgICAgYiArPSBhID4+PiAxNjtcbiAgICAgIGMgKz0gYiA+Pj4gMTY7XG4gICAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgICBiaDMgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgICBibDMgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICAgIGFoMSA9IGJoMDtcbiAgICAgIGFoMiA9IGJoMTtcbiAgICAgIGFoMyA9IGJoMjtcbiAgICAgIGFoNCA9IGJoMztcbiAgICAgIGFoNSA9IGJoNDtcbiAgICAgIGFoNiA9IGJoNTtcbiAgICAgIGFoNyA9IGJoNjtcbiAgICAgIGFoMCA9IGJoNztcblxuICAgICAgYWwxID0gYmwwO1xuICAgICAgYWwyID0gYmwxO1xuICAgICAgYWwzID0gYmwyO1xuICAgICAgYWw0ID0gYmwzO1xuICAgICAgYWw1ID0gYmw0O1xuICAgICAgYWw2ID0gYmw1O1xuICAgICAgYWw3ID0gYmw2O1xuICAgICAgYWwwID0gYmw3O1xuXG4gICAgICBpZiAoaSUxNiA9PT0gMTUpIHtcbiAgICAgICAgZm9yIChqID0gMDsgaiA8IDE2OyBqKyspIHtcbiAgICAgICAgICAvLyBhZGRcbiAgICAgICAgICBoID0gd2hbal07XG4gICAgICAgICAgbCA9IHdsW2pdO1xuXG4gICAgICAgICAgYSA9IGwgJiAweGZmZmY7IGIgPSBsID4+PiAxNjtcbiAgICAgICAgICBjID0gaCAmIDB4ZmZmZjsgZCA9IGggPj4+IDE2O1xuXG4gICAgICAgICAgaCA9IHdoWyhqKzkpJTE2XTtcbiAgICAgICAgICBsID0gd2xbKGorOSklMTZdO1xuXG4gICAgICAgICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgICAgICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgICAgICAgIC8vIHNpZ21hMFxuICAgICAgICAgIHRoID0gd2hbKGorMSklMTZdO1xuICAgICAgICAgIHRsID0gd2xbKGorMSklMTZdO1xuICAgICAgICAgIGggPSAoKHRoID4+PiAxKSB8ICh0bCA8PCAoMzItMSkpKSBeICgodGggPj4+IDgpIHwgKHRsIDw8ICgzMi04KSkpIF4gKHRoID4+PiA3KTtcbiAgICAgICAgICBsID0gKCh0bCA+Pj4gMSkgfCAodGggPDwgKDMyLTEpKSkgXiAoKHRsID4+PiA4KSB8ICh0aCA8PCAoMzItOCkpKSBeICgodGwgPj4+IDcpIHwgKHRoIDw8ICgzMi03KSkpO1xuXG4gICAgICAgICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgICAgICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgICAgICAgIC8vIHNpZ21hMVxuICAgICAgICAgIHRoID0gd2hbKGorMTQpJTE2XTtcbiAgICAgICAgICB0bCA9IHdsWyhqKzE0KSUxNl07XG4gICAgICAgICAgaCA9ICgodGggPj4+IDE5KSB8ICh0bCA8PCAoMzItMTkpKSkgXiAoKHRsID4+PiAoNjEtMzIpKSB8ICh0aCA8PCAoMzItKDYxLTMyKSkpKSBeICh0aCA+Pj4gNik7XG4gICAgICAgICAgbCA9ICgodGwgPj4+IDE5KSB8ICh0aCA8PCAoMzItMTkpKSkgXiAoKHRoID4+PiAoNjEtMzIpKSB8ICh0bCA8PCAoMzItKDYxLTMyKSkpKSBeICgodGwgPj4+IDYpIHwgKHRoIDw8ICgzMi02KSkpO1xuXG4gICAgICAgICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgICAgICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgICAgICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgICAgICAgYyArPSBiID4+PiAxNjtcbiAgICAgICAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgICAgICAgd2hbal0gPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgICAgICAgd2xbal0gPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG4gICAgICAgIH1cbiAgICAgIH1cbiAgICB9XG5cbiAgICAvLyBhZGRcbiAgICBoID0gYWgwO1xuICAgIGwgPSBhbDA7XG5cbiAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICBoID0gaGhbMF07XG4gICAgbCA9IGhsWzBdO1xuXG4gICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgYyArPSBiID4+PiAxNjtcbiAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgaGhbMF0gPSBhaDAgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgaGxbMF0gPSBhbDAgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICBoID0gYWgxO1xuICAgIGwgPSBhbDE7XG5cbiAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICBoID0gaGhbMV07XG4gICAgbCA9IGhsWzFdO1xuXG4gICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgYyArPSBiID4+PiAxNjtcbiAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgaGhbMV0gPSBhaDEgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgaGxbMV0gPSBhbDEgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICBoID0gYWgyO1xuICAgIGwgPSBhbDI7XG5cbiAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICBoID0gaGhbMl07XG4gICAgbCA9IGhsWzJdO1xuXG4gICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgYyArPSBiID4+PiAxNjtcbiAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgaGhbMl0gPSBhaDIgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgaGxbMl0gPSBhbDIgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICBoID0gYWgzO1xuICAgIGwgPSBhbDM7XG5cbiAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICBoID0gaGhbM107XG4gICAgbCA9IGhsWzNdO1xuXG4gICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgYyArPSBiID4+PiAxNjtcbiAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgaGhbM10gPSBhaDMgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgaGxbM10gPSBhbDMgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICBoID0gYWg0O1xuICAgIGwgPSBhbDQ7XG5cbiAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICBoID0gaGhbNF07XG4gICAgbCA9IGhsWzRdO1xuXG4gICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgYyArPSBiID4+PiAxNjtcbiAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgaGhbNF0gPSBhaDQgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgaGxbNF0gPSBhbDQgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICBoID0gYWg1O1xuICAgIGwgPSBhbDU7XG5cbiAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICBoID0gaGhbNV07XG4gICAgbCA9IGhsWzVdO1xuXG4gICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgYyArPSBiID4+PiAxNjtcbiAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgaGhbNV0gPSBhaDUgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgaGxbNV0gPSBhbDUgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICBoID0gYWg2O1xuICAgIGwgPSBhbDY7XG5cbiAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICBoID0gaGhbNl07XG4gICAgbCA9IGhsWzZdO1xuXG4gICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgYyArPSBiID4+PiAxNjtcbiAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgaGhbNl0gPSBhaDYgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgaGxbNl0gPSBhbDYgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICBoID0gYWg3O1xuICAgIGwgPSBhbDc7XG5cbiAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICBoID0gaGhbN107XG4gICAgbCA9IGhsWzddO1xuXG4gICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgYyArPSBiID4+PiAxNjtcbiAgICBkICs9IGMgPj4+IDE2O1xuXG4gICAgaGhbN10gPSBhaDcgPSAoYyAmIDB4ZmZmZikgfCAoZCA8PCAxNik7XG4gICAgaGxbN10gPSBhbDcgPSAoYSAmIDB4ZmZmZikgfCAoYiA8PCAxNik7XG5cbiAgICBwb3MgKz0gMTI4O1xuICAgIG4gLT0gMTI4O1xuICB9XG5cbiAgcmV0dXJuIG47XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19oYXNoKG91dCwgbSwgbikge1xuICB2YXIgaGggPSBuZXcgSW50MzJBcnJheSg4KSxcbiAgICAgIGhsID0gbmV3IEludDMyQXJyYXkoOCksXG4gICAgICB4ID0gbmV3IFVpbnQ4QXJyYXkoMjU2KSxcbiAgICAgIGksIGIgPSBuO1xuXG4gIGhoWzBdID0gMHg2YTA5ZTY2NztcbiAgaGhbMV0gPSAweGJiNjdhZTg1O1xuICBoaFsyXSA9IDB4M2M2ZWYzNzI7XG4gIGhoWzNdID0gMHhhNTRmZjUzYTtcbiAgaGhbNF0gPSAweDUxMGU1MjdmO1xuICBoaFs1XSA9IDB4OWIwNTY4OGM7XG4gIGhoWzZdID0gMHgxZjgzZDlhYjtcbiAgaGhbN10gPSAweDViZTBjZDE5O1xuXG4gIGhsWzBdID0gMHhmM2JjYzkwODtcbiAgaGxbMV0gPSAweDg0Y2FhNzNiO1xuICBobFsyXSA9IDB4ZmU5NGY4MmI7XG4gIGhsWzNdID0gMHg1ZjFkMzZmMTtcbiAgaGxbNF0gPSAweGFkZTY4MmQxO1xuICBobFs1XSA9IDB4MmIzZTZjMWY7XG4gIGhsWzZdID0gMHhmYjQxYmQ2YjtcbiAgaGxbN10gPSAweDEzN2UyMTc5O1xuXG4gIGNyeXB0b19oYXNoYmxvY2tzX2hsKGhoLCBobCwgbSwgbik7XG4gIG4gJT0gMTI4O1xuXG4gIGZvciAoaSA9IDA7IGkgPCBuOyBpKyspIHhbaV0gPSBtW2ItbitpXTtcbiAgeFtuXSA9IDEyODtcblxuICBuID0gMjU2LTEyOCoobjwxMTI/MTowKTtcbiAgeFtuLTldID0gMDtcbiAgdHM2NCh4LCBuLTgsICAoYiAvIDB4MjAwMDAwMDApIHwgMCwgYiA8PCAzKTtcbiAgY3J5cHRvX2hhc2hibG9ja3NfaGwoaGgsIGhsLCB4LCBuKTtcblxuICBmb3IgKGkgPSAwOyBpIDwgODsgaSsrKSB0czY0KG91dCwgOCppLCBoaFtpXSwgaGxbaV0pO1xuXG4gIHJldHVybiAwO1xufVxuXG5mdW5jdGlvbiBhZGQocCwgcSkge1xuICB2YXIgYSA9IGdmKCksIGIgPSBnZigpLCBjID0gZ2YoKSxcbiAgICAgIGQgPSBnZigpLCBlID0gZ2YoKSwgZiA9IGdmKCksXG4gICAgICBnID0gZ2YoKSwgaCA9IGdmKCksIHQgPSBnZigpO1xuXG4gIFooYSwgcFsxXSwgcFswXSk7XG4gIFoodCwgcVsxXSwgcVswXSk7XG4gIE0oYSwgYSwgdCk7XG4gIEEoYiwgcFswXSwgcFsxXSk7XG4gIEEodCwgcVswXSwgcVsxXSk7XG4gIE0oYiwgYiwgdCk7XG4gIE0oYywgcFszXSwgcVszXSk7XG4gIE0oYywgYywgRDIpO1xuICBNKGQsIHBbMl0sIHFbMl0pO1xuICBBKGQsIGQsIGQpO1xuICBaKGUsIGIsIGEpO1xuICBaKGYsIGQsIGMpO1xuICBBKGcsIGQsIGMpO1xuICBBKGgsIGIsIGEpO1xuXG4gIE0ocFswXSwgZSwgZik7XG4gIE0ocFsxXSwgaCwgZyk7XG4gIE0ocFsyXSwgZywgZik7XG4gIE0ocFszXSwgZSwgaCk7XG59XG5cbmZ1bmN0aW9uIGNzd2FwKHAsIHEsIGIpIHtcbiAgdmFyIGk7XG4gIGZvciAoaSA9IDA7IGkgPCA0OyBpKyspIHtcbiAgICBzZWwyNTUxOShwW2ldLCBxW2ldLCBiKTtcbiAgfVxufVxuXG5mdW5jdGlvbiBwYWNrKHIsIHApIHtcbiAgdmFyIHR4ID0gZ2YoKSwgdHkgPSBnZigpLCB6aSA9IGdmKCk7XG4gIGludjI1NTE5KHppLCBwWzJdKTtcbiAgTSh0eCwgcFswXSwgemkpO1xuICBNKHR5LCBwWzFdLCB6aSk7XG4gIHBhY2syNTUxOShyLCB0eSk7XG4gIHJbMzFdIF49IHBhcjI1NTE5KHR4KSA8PCA3O1xufVxuXG5mdW5jdGlvbiBzY2FsYXJtdWx0KHAsIHEsIHMpIHtcbiAgdmFyIGIsIGk7XG4gIHNldDI1NTE5KHBbMF0sIGdmMCk7XG4gIHNldDI1NTE5KHBbMV0sIGdmMSk7XG4gIHNldDI1NTE5KHBbMl0sIGdmMSk7XG4gIHNldDI1NTE5KHBbM10sIGdmMCk7XG4gIGZvciAoaSA9IDI1NTsgaSA+PSAwOyAtLWkpIHtcbiAgICBiID0gKHNbKGkvOCl8MF0gPj4gKGkmNykpICYgMTtcbiAgICBjc3dhcChwLCBxLCBiKTtcbiAgICBhZGQocSwgcCk7XG4gICAgYWRkKHAsIHApO1xuICAgIGNzd2FwKHAsIHEsIGIpO1xuICB9XG59XG5cbmZ1bmN0aW9uIHNjYWxhcmJhc2UocCwgcykge1xuICB2YXIgcSA9IFtnZigpLCBnZigpLCBnZigpLCBnZigpXTtcbiAgc2V0MjU1MTkocVswXSwgWCk7XG4gIHNldDI1NTE5KHFbMV0sIFkpO1xuICBzZXQyNTUxOShxWzJdLCBnZjEpO1xuICBNKHFbM10sIFgsIFkpO1xuICBzY2FsYXJtdWx0KHAsIHEsIHMpO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fc2lnbl9rZXlwYWlyKHBrLCBzaywgc2VlZGVkKSB7XG4gIHZhciBkID0gbmV3IFVpbnQ4QXJyYXkoNjQpO1xuICB2YXIgcCA9IFtnZigpLCBnZigpLCBnZigpLCBnZigpXTtcbiAgdmFyIGk7XG5cbiAgaWYgKCFzZWVkZWQpIHJhbmRvbWJ5dGVzKHNrLCAzMik7XG4gIGNyeXB0b19oYXNoKGQsIHNrLCAzMik7XG4gIGRbMF0gJj0gMjQ4O1xuICBkWzMxXSAmPSAxMjc7XG4gIGRbMzFdIHw9IDY0O1xuXG4gIHNjYWxhcmJhc2UocCwgZCk7XG4gIHBhY2socGssIHApO1xuXG4gIGZvciAoaSA9IDA7IGkgPCAzMjsgaSsrKSBza1tpKzMyXSA9IHBrW2ldO1xuICByZXR1cm4gMDtcbn1cblxudmFyIEwgPSBuZXcgRmxvYXQ2NEFycmF5KFsweGVkLCAweGQzLCAweGY1LCAweDVjLCAweDFhLCAweDYzLCAweDEyLCAweDU4LCAweGQ2LCAweDljLCAweGY3LCAweGEyLCAweGRlLCAweGY5LCAweGRlLCAweDE0LCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAwLCAweDEwXSk7XG5cbmZ1bmN0aW9uIG1vZEwociwgeCkge1xuICB2YXIgY2FycnksIGksIGosIGs7XG4gIGZvciAoaSA9IDYzOyBpID49IDMyOyAtLWkpIHtcbiAgICBjYXJyeSA9IDA7XG4gICAgZm9yIChqID0gaSAtIDMyLCBrID0gaSAtIDEyOyBqIDwgazsgKytqKSB7XG4gICAgICB4W2pdICs9IGNhcnJ5IC0gMTYgKiB4W2ldICogTFtqIC0gKGkgLSAzMildO1xuICAgICAgY2FycnkgPSAoeFtqXSArIDEyOCkgPj4gODtcbiAgICAgIHhbal0gLT0gY2FycnkgKiAyNTY7XG4gICAgfVxuICAgIHhbal0gKz0gY2Fycnk7XG4gICAgeFtpXSA9IDA7XG4gIH1cbiAgY2FycnkgPSAwO1xuICBmb3IgKGogPSAwOyBqIDwgMzI7IGorKykge1xuICAgIHhbal0gKz0gY2FycnkgLSAoeFszMV0gPj4gNCkgKiBMW2pdO1xuICAgIGNhcnJ5ID0geFtqXSA+PiA4O1xuICAgIHhbal0gJj0gMjU1O1xuICB9XG4gIGZvciAoaiA9IDA7IGogPCAzMjsgaisrKSB4W2pdIC09IGNhcnJ5ICogTFtqXTtcbiAgZm9yIChpID0gMDsgaSA8IDMyOyBpKyspIHtcbiAgICB4W2krMV0gKz0geFtpXSA+PiA4O1xuICAgIHJbaV0gPSB4W2ldICYgMjU1O1xuICB9XG59XG5cbmZ1bmN0aW9uIHJlZHVjZShyKSB7XG4gIHZhciB4ID0gbmV3IEZsb2F0NjRBcnJheSg2NCksIGk7XG4gIGZvciAoaSA9IDA7IGkgPCA2NDsgaSsrKSB4W2ldID0gcltpXTtcbiAgZm9yIChpID0gMDsgaSA8IDY0OyBpKyspIHJbaV0gPSAwO1xuICBtb2RMKHIsIHgpO1xufVxuXG4vLyBOb3RlOiBkaWZmZXJlbmNlIGZyb20gQyAtIHNtbGVuIHJldHVybmVkLCBub3QgcGFzc2VkIGFzIGFyZ3VtZW50LlxuZnVuY3Rpb24gY3J5cHRvX3NpZ24oc20sIG0sIG4sIHNrKSB7XG4gIHZhciBkID0gbmV3IFVpbnQ4QXJyYXkoNjQpLCBoID0gbmV3IFVpbnQ4QXJyYXkoNjQpLCByID0gbmV3IFVpbnQ4QXJyYXkoNjQpO1xuICB2YXIgaSwgaiwgeCA9IG5ldyBGbG9hdDY0QXJyYXkoNjQpO1xuICB2YXIgcCA9IFtnZigpLCBnZigpLCBnZigpLCBnZigpXTtcblxuICBjcnlwdG9faGFzaChkLCBzaywgMzIpO1xuICBkWzBdICY9IDI0ODtcbiAgZFszMV0gJj0gMTI3O1xuICBkWzMxXSB8PSA2NDtcblxuICB2YXIgc21sZW4gPSBuICsgNjQ7XG4gIGZvciAoaSA9IDA7IGkgPCBuOyBpKyspIHNtWzY0ICsgaV0gPSBtW2ldO1xuICBmb3IgKGkgPSAwOyBpIDwgMzI7IGkrKykgc21bMzIgKyBpXSA9IGRbMzIgKyBpXTtcblxuICBjcnlwdG9faGFzaChyLCBzbS5zdWJhcnJheSgzMiksIG4rMzIpO1xuICByZWR1Y2Uocik7XG4gIHNjYWxhcmJhc2UocCwgcik7XG4gIHBhY2soc20sIHApO1xuXG4gIGZvciAoaSA9IDMyOyBpIDwgNjQ7IGkrKykgc21baV0gPSBza1tpXTtcbiAgY3J5cHRvX2hhc2goaCwgc20sIG4gKyA2NCk7XG4gIHJlZHVjZShoKTtcblxuICBmb3IgKGkgPSAwOyBpIDwgNjQ7IGkrKykgeFtpXSA9IDA7XG4gIGZvciAoaSA9IDA7IGkgPCAzMjsgaSsrKSB4W2ldID0gcltpXTtcbiAgZm9yIChpID0gMDsgaSA8IDMyOyBpKyspIHtcbiAgICBmb3IgKGogPSAwOyBqIDwgMzI7IGorKykge1xuICAgICAgeFtpK2pdICs9IGhbaV0gKiBkW2pdO1xuICAgIH1cbiAgfVxuXG4gIG1vZEwoc20uc3ViYXJyYXkoMzIpLCB4KTtcbiAgcmV0dXJuIHNtbGVuO1xufVxuXG5mdW5jdGlvbiB1bnBhY2tuZWcociwgcCkge1xuICB2YXIgdCA9IGdmKCksIGNoayA9IGdmKCksIG51bSA9IGdmKCksXG4gICAgICBkZW4gPSBnZigpLCBkZW4yID0gZ2YoKSwgZGVuNCA9IGdmKCksXG4gICAgICBkZW42ID0gZ2YoKTtcblxuICBzZXQyNTUxOShyWzJdLCBnZjEpO1xuICB1bnBhY2syNTUxOShyWzFdLCBwKTtcbiAgUyhudW0sIHJbMV0pO1xuICBNKGRlbiwgbnVtLCBEKTtcbiAgWihudW0sIG51bSwgclsyXSk7XG4gIEEoZGVuLCByWzJdLCBkZW4pO1xuXG4gIFMoZGVuMiwgZGVuKTtcbiAgUyhkZW40LCBkZW4yKTtcbiAgTShkZW42LCBkZW40LCBkZW4yKTtcbiAgTSh0LCBkZW42LCBudW0pO1xuICBNKHQsIHQsIGRlbik7XG5cbiAgcG93MjUyMyh0LCB0KTtcbiAgTSh0LCB0LCBudW0pO1xuICBNKHQsIHQsIGRlbik7XG4gIE0odCwgdCwgZGVuKTtcbiAgTShyWzBdLCB0LCBkZW4pO1xuXG4gIFMoY2hrLCByWzBdKTtcbiAgTShjaGssIGNoaywgZGVuKTtcbiAgaWYgKG5lcTI1NTE5KGNoaywgbnVtKSkgTShyWzBdLCByWzBdLCBJKTtcblxuICBTKGNoaywgclswXSk7XG4gIE0oY2hrLCBjaGssIGRlbik7XG4gIGlmIChuZXEyNTUxOShjaGssIG51bSkpIHJldHVybiAtMTtcblxuICBpZiAocGFyMjU1MTkoclswXSkgPT09IChwWzMxXT4+NykpIFooclswXSwgZ2YwLCByWzBdKTtcblxuICBNKHJbM10sIHJbMF0sIHJbMV0pO1xuICByZXR1cm4gMDtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX3NpZ25fb3BlbihtLCBzbSwgbiwgcGspIHtcbiAgdmFyIGksIG1sZW47XG4gIHZhciB0ID0gbmV3IFVpbnQ4QXJyYXkoMzIpLCBoID0gbmV3IFVpbnQ4QXJyYXkoNjQpO1xuICB2YXIgcCA9IFtnZigpLCBnZigpLCBnZigpLCBnZigpXSxcbiAgICAgIHEgPSBbZ2YoKSwgZ2YoKSwgZ2YoKSwgZ2YoKV07XG5cbiAgbWxlbiA9IC0xO1xuICBpZiAobiA8IDY0KSByZXR1cm4gLTE7XG5cbiAgaWYgKHVucGFja25lZyhxLCBwaykpIHJldHVybiAtMTtcblxuICBmb3IgKGkgPSAwOyBpIDwgbjsgaSsrKSBtW2ldID0gc21baV07XG4gIGZvciAoaSA9IDA7IGkgPCAzMjsgaSsrKSBtW2krMzJdID0gcGtbaV07XG4gIGNyeXB0b19oYXNoKGgsIG0sIG4pO1xuICByZWR1Y2UoaCk7XG4gIHNjYWxhcm11bHQocCwgcSwgaCk7XG5cbiAgc2NhbGFyYmFzZShxLCBzbS5zdWJhcnJheSgzMikpO1xuICBhZGQocCwgcSk7XG4gIHBhY2sodCwgcCk7XG5cbiAgbiAtPSA2NDtcbiAgaWYgKGNyeXB0b192ZXJpZnlfMzIoc20sIDAsIHQsIDApKSB7XG4gICAgZm9yIChpID0gMDsgaSA8IG47IGkrKykgbVtpXSA9IDA7XG4gICAgcmV0dXJuIC0xO1xuICB9XG5cbiAgZm9yIChpID0gMDsgaSA8IG47IGkrKykgbVtpXSA9IHNtW2kgKyA2NF07XG4gIG1sZW4gPSBuO1xuICByZXR1cm4gbWxlbjtcbn1cblxudmFyIGNyeXB0b19zZWNyZXRib3hfS0VZQllURVMgPSAzMixcbiAgICBjcnlwdG9fc2VjcmV0Ym94X05PTkNFQllURVMgPSAyNCxcbiAgICBjcnlwdG9fc2VjcmV0Ym94X1pFUk9CWVRFUyA9IDMyLFxuICAgIGNyeXB0b19zZWNyZXRib3hfQk9YWkVST0JZVEVTID0gMTYsXG4gICAgY3J5cHRvX3NjYWxhcm11bHRfQllURVMgPSAzMixcbiAgICBjcnlwdG9fc2NhbGFybXVsdF9TQ0FMQVJCWVRFUyA9IDMyLFxuICAgIGNyeXB0b19ib3hfUFVCTElDS0VZQllURVMgPSAzMixcbiAgICBjcnlwdG9fYm94X1NFQ1JFVEtFWUJZVEVTID0gMzIsXG4gICAgY3J5cHRvX2JveF9CRUZPUkVOTUJZVEVTID0gMzIsXG4gICAgY3J5cHRvX2JveF9OT05DRUJZVEVTID0gY3J5cHRvX3NlY3JldGJveF9OT05DRUJZVEVTLFxuICAgIGNyeXB0b19ib3hfWkVST0JZVEVTID0gY3J5cHRvX3NlY3JldGJveF9aRVJPQllURVMsXG4gICAgY3J5cHRvX2JveF9CT1haRVJPQllURVMgPSBjcnlwdG9fc2VjcmV0Ym94X0JPWFpFUk9CWVRFUyxcbiAgICBjcnlwdG9fc2lnbl9CWVRFUyA9IDY0LFxuICAgIGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTID0gMzIsXG4gICAgY3J5cHRvX3NpZ25fU0VDUkVUS0VZQllURVMgPSA2NCxcbiAgICBjcnlwdG9fc2lnbl9TRUVEQllURVMgPSAzMixcbiAgICBjcnlwdG9faGFzaF9CWVRFUyA9IDY0O1xuXG5uYWNsLmxvd2xldmVsID0ge1xuICBjcnlwdG9fY29yZV9oc2Fsc2EyMDogY3J5cHRvX2NvcmVfaHNhbHNhMjAsXG4gIGNyeXB0b19zdHJlYW1feG9yOiBjcnlwdG9fc3RyZWFtX3hvcixcbiAgY3J5cHRvX3N0cmVhbTogY3J5cHRvX3N0cmVhbSxcbiAgY3J5cHRvX3N0cmVhbV9zYWxzYTIwX3hvcjogY3J5cHRvX3N0cmVhbV9zYWxzYTIwX3hvcixcbiAgY3J5cHRvX3N0cmVhbV9zYWxzYTIwOiBjcnlwdG9fc3RyZWFtX3NhbHNhMjAsXG4gIGNyeXB0b19vbmV0aW1lYXV0aDogY3J5cHRvX29uZXRpbWVhdXRoLFxuICBjcnlwdG9fb25ldGltZWF1dGhfdmVyaWZ5OiBjcnlwdG9fb25ldGltZWF1dGhfdmVyaWZ5LFxuICBjcnlwdG9fdmVyaWZ5XzE2OiBjcnlwdG9fdmVyaWZ5XzE2LFxuICBjcnlwdG9fdmVyaWZ5XzMyOiBjcnlwdG9fdmVyaWZ5XzMyLFxuICBjcnlwdG9fc2VjcmV0Ym94OiBjcnlwdG9fc2VjcmV0Ym94LFxuICBjcnlwdG9fc2VjcmV0Ym94X29wZW46IGNyeXB0b19zZWNyZXRib3hfb3BlbixcbiAgY3J5cHRvX3NjYWxhcm11bHQ6IGNyeXB0b19zY2FsYXJtdWx0LFxuICBjcnlwdG9fc2NhbGFybXVsdF9iYXNlOiBjcnlwdG9fc2NhbGFybXVsdF9iYXNlLFxuICBjcnlwdG9fYm94X2JlZm9yZW5tOiBjcnlwdG9fYm94X2JlZm9yZW5tLFxuICBjcnlwdG9fYm94X2FmdGVybm06IGNyeXB0b19ib3hfYWZ0ZXJubSxcbiAgY3J5cHRvX2JveDogY3J5cHRvX2JveCxcbiAgY3J5cHRvX2JveF9vcGVuOiBjcnlwdG9fYm94X29wZW4sXG4gIGNyeXB0b19ib3hfa2V5cGFpcjogY3J5cHRvX2JveF9rZXlwYWlyLFxuICBjcnlwdG9faGFzaDogY3J5cHRvX2hhc2gsXG4gIGNyeXB0b19zaWduOiBjcnlwdG9fc2lnbixcbiAgY3J5cHRvX3NpZ25fa2V5cGFpcjogY3J5cHRvX3NpZ25fa2V5cGFpcixcbiAgY3J5cHRvX3NpZ25fb3BlbjogY3J5cHRvX3NpZ25fb3BlbixcblxuICBjcnlwdG9fc2VjcmV0Ym94X0tFWUJZVEVTOiBjcnlwdG9fc2VjcmV0Ym94X0tFWUJZVEVTLFxuICBjcnlwdG9fc2VjcmV0Ym94X05PTkNFQllURVM6IGNyeXB0b19zZWNyZXRib3hfTk9OQ0VCWVRFUyxcbiAgY3J5cHRvX3NlY3JldGJveF9aRVJPQllURVM6IGNyeXB0b19zZWNyZXRib3hfWkVST0JZVEVTLFxuICBjcnlwdG9fc2VjcmV0Ym94X0JPWFpFUk9CWVRFUzogY3J5cHRvX3NlY3JldGJveF9CT1haRVJPQllURVMsXG4gIGNyeXB0b19zY2FsYXJtdWx0X0JZVEVTOiBjcnlwdG9fc2NhbGFybXVsdF9CWVRFUyxcbiAgY3J5cHRvX3NjYWxhcm11bHRfU0NBTEFSQllURVM6IGNyeXB0b19zY2FsYXJtdWx0X1NDQUxBUkJZVEVTLFxuICBjcnlwdG9fYm94X1BVQkxJQ0tFWUJZVEVTOiBjcnlwdG9fYm94X1BVQkxJQ0tFWUJZVEVTLFxuICBjcnlwdG9fYm94X1NFQ1JFVEtFWUJZVEVTOiBjcnlwdG9fYm94X1NFQ1JFVEtFWUJZVEVTLFxuICBjcnlwdG9fYm94X0JFRk9SRU5NQllURVM6IGNyeXB0b19ib3hfQkVGT1JFTk1CWVRFUyxcbiAgY3J5cHRvX2JveF9OT05DRUJZVEVTOiBjcnlwdG9fYm94X05PTkNFQllURVMsXG4gIGNyeXB0b19ib3hfWkVST0JZVEVTOiBjcnlwdG9fYm94X1pFUk9CWVRFUyxcbiAgY3J5cHRvX2JveF9CT1haRVJPQllURVM6IGNyeXB0b19ib3hfQk9YWkVST0JZVEVTLFxuICBjcnlwdG9fc2lnbl9CWVRFUzogY3J5cHRvX3NpZ25fQllURVMsXG4gIGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTOiBjcnlwdG9fc2lnbl9QVUJMSUNLRVlCWVRFUyxcbiAgY3J5cHRvX3NpZ25fU0VDUkVUS0VZQllURVM6IGNyeXB0b19zaWduX1NFQ1JFVEtFWUJZVEVTLFxuICBjcnlwdG9fc2lnbl9TRUVEQllURVM6IGNyeXB0b19zaWduX1NFRURCWVRFUyxcbiAgY3J5cHRvX2hhc2hfQllURVM6IGNyeXB0b19oYXNoX0JZVEVTXG59O1xuXG4vKiBIaWdoLWxldmVsIEFQSSAqL1xuXG5mdW5jdGlvbiBjaGVja0xlbmd0aHMoaywgbikge1xuICBpZiAoay5sZW5ndGggIT09IGNyeXB0b19zZWNyZXRib3hfS0VZQllURVMpIHRocm93IG5ldyBFcnJvcignYmFkIGtleSBzaXplJyk7XG4gIGlmIChuLmxlbmd0aCAhPT0gY3J5cHRvX3NlY3JldGJveF9OT05DRUJZVEVTKSB0aHJvdyBuZXcgRXJyb3IoJ2JhZCBub25jZSBzaXplJyk7XG59XG5cbmZ1bmN0aW9uIGNoZWNrQm94TGVuZ3Rocyhwaywgc2spIHtcbiAgaWYgKHBrLmxlbmd0aCAhPT0gY3J5cHRvX2JveF9QVUJMSUNLRVlCWVRFUykgdGhyb3cgbmV3IEVycm9yKCdiYWQgcHVibGljIGtleSBzaXplJyk7XG4gIGlmIChzay5sZW5ndGggIT09IGNyeXB0b19ib3hfU0VDUkVUS0VZQllURVMpIHRocm93IG5ldyBFcnJvcignYmFkIHNlY3JldCBrZXkgc2l6ZScpO1xufVxuXG5mdW5jdGlvbiBjaGVja0FycmF5VHlwZXMoKSB7XG4gIHZhciB0LCBpO1xuICBmb3IgKGkgPSAwOyBpIDwgYXJndW1lbnRzLmxlbmd0aDsgaSsrKSB7XG4gICAgIGlmICgodCA9IE9iamVjdC5wcm90b3R5cGUudG9TdHJpbmcuY2FsbChhcmd1bWVudHNbaV0pKSAhPT0gJ1tvYmplY3QgVWludDhBcnJheV0nKVxuICAgICAgIHRocm93IG5ldyBUeXBlRXJyb3IoJ3VuZXhwZWN0ZWQgdHlwZSAnICsgdCArICcsIHVzZSBVaW50OEFycmF5Jyk7XG4gIH1cbn1cblxuZnVuY3Rpb24gY2xlYW51cChhcnIpIHtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBhcnIubGVuZ3RoOyBpKyspIGFycltpXSA9IDA7XG59XG5cbi8vIFRPRE86IENvbXBsZXRlbHkgcmVtb3ZlIHRoaXMgaW4gdjAuMTUuXG5pZiAoIW5hY2wudXRpbCkge1xuICBuYWNsLnV0aWwgPSB7fTtcbiAgbmFjbC51dGlsLmRlY29kZVVURjggPSBuYWNsLnV0aWwuZW5jb2RlVVRGOCA9IG5hY2wudXRpbC5lbmNvZGVCYXNlNjQgPSBuYWNsLnV0aWwuZGVjb2RlQmFzZTY0ID0gZnVuY3Rpb24oKSB7XG4gICAgdGhyb3cgbmV3IEVycm9yKCduYWNsLnV0aWwgbW92ZWQgaW50byBzZXBhcmF0ZSBwYWNrYWdlOiBodHRwczovL2dpdGh1Yi5jb20vZGNoZXN0L3R3ZWV0bmFjbC11dGlsLWpzJyk7XG4gIH07XG59XG5cbm5hY2wucmFuZG9tQnl0ZXMgPSBmdW5jdGlvbihuKSB7XG4gIHZhciBiID0gbmV3IFVpbnQ4QXJyYXkobik7XG4gIHJhbmRvbWJ5dGVzKGIsIG4pO1xuICByZXR1cm4gYjtcbn07XG5cbm5hY2wuc2VjcmV0Ym94ID0gZnVuY3Rpb24obXNnLCBub25jZSwga2V5KSB7XG4gIGNoZWNrQXJyYXlUeXBlcyhtc2csIG5vbmNlLCBrZXkpO1xuICBjaGVja0xlbmd0aHMoa2V5LCBub25jZSk7XG4gIHZhciBtID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NlY3JldGJveF9aRVJPQllURVMgKyBtc2cubGVuZ3RoKTtcbiAgdmFyIGMgPSBuZXcgVWludDhBcnJheShtLmxlbmd0aCk7XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgbXNnLmxlbmd0aDsgaSsrKSBtW2krY3J5cHRvX3NlY3JldGJveF9aRVJPQllURVNdID0gbXNnW2ldO1xuICBjcnlwdG9fc2VjcmV0Ym94KGMsIG0sIG0ubGVuZ3RoLCBub25jZSwga2V5KTtcbiAgcmV0dXJuIGMuc3ViYXJyYXkoY3J5cHRvX3NlY3JldGJveF9CT1haRVJPQllURVMpO1xufTtcblxubmFjbC5zZWNyZXRib3gub3BlbiA9IGZ1bmN0aW9uKGJveCwgbm9uY2UsIGtleSkge1xuICBjaGVja0FycmF5VHlwZXMoYm94LCBub25jZSwga2V5KTtcbiAgY2hlY2tMZW5ndGhzKGtleSwgbm9uY2UpO1xuICB2YXIgYyA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zZWNyZXRib3hfQk9YWkVST0JZVEVTICsgYm94Lmxlbmd0aCk7XG4gIHZhciBtID0gbmV3IFVpbnQ4QXJyYXkoYy5sZW5ndGgpO1xuICBmb3IgKHZhciBpID0gMDsgaSA8IGJveC5sZW5ndGg7IGkrKykgY1tpK2NyeXB0b19zZWNyZXRib3hfQk9YWkVST0JZVEVTXSA9IGJveFtpXTtcbiAgaWYgKGMubGVuZ3RoIDwgMzIpIHJldHVybiBmYWxzZTtcbiAgaWYgKGNyeXB0b19zZWNyZXRib3hfb3BlbihtLCBjLCBjLmxlbmd0aCwgbm9uY2UsIGtleSkgIT09IDApIHJldHVybiBmYWxzZTtcbiAgcmV0dXJuIG0uc3ViYXJyYXkoY3J5cHRvX3NlY3JldGJveF9aRVJPQllURVMpO1xufTtcblxubmFjbC5zZWNyZXRib3gua2V5TGVuZ3RoID0gY3J5cHRvX3NlY3JldGJveF9LRVlCWVRFUztcbm5hY2wuc2VjcmV0Ym94Lm5vbmNlTGVuZ3RoID0gY3J5cHRvX3NlY3JldGJveF9OT05DRUJZVEVTO1xubmFjbC5zZWNyZXRib3gub3ZlcmhlYWRMZW5ndGggPSBjcnlwdG9fc2VjcmV0Ym94X0JPWFpFUk9CWVRFUztcblxubmFjbC5zY2FsYXJNdWx0ID0gZnVuY3Rpb24obiwgcCkge1xuICBjaGVja0FycmF5VHlwZXMobiwgcCk7XG4gIGlmIChuLmxlbmd0aCAhPT0gY3J5cHRvX3NjYWxhcm11bHRfU0NBTEFSQllURVMpIHRocm93IG5ldyBFcnJvcignYmFkIG4gc2l6ZScpO1xuICBpZiAocC5sZW5ndGggIT09IGNyeXB0b19zY2FsYXJtdWx0X0JZVEVTKSB0aHJvdyBuZXcgRXJyb3IoJ2JhZCBwIHNpemUnKTtcbiAgdmFyIHEgPSBuZXcgVWludDhBcnJheShjcnlwdG9fc2NhbGFybXVsdF9CWVRFUyk7XG4gIGNyeXB0b19zY2FsYXJtdWx0KHEsIG4sIHApO1xuICByZXR1cm4gcTtcbn07XG5cbm5hY2wuc2NhbGFyTXVsdC5iYXNlID0gZnVuY3Rpb24obikge1xuICBjaGVja0FycmF5VHlwZXMobik7XG4gIGlmIChuLmxlbmd0aCAhPT0gY3J5cHRvX3NjYWxhcm11bHRfU0NBTEFSQllURVMpIHRocm93IG5ldyBFcnJvcignYmFkIG4gc2l6ZScpO1xuICB2YXIgcSA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zY2FsYXJtdWx0X0JZVEVTKTtcbiAgY3J5cHRvX3NjYWxhcm11bHRfYmFzZShxLCBuKTtcbiAgcmV0dXJuIHE7XG59O1xuXG5uYWNsLnNjYWxhck11bHQuc2NhbGFyTGVuZ3RoID0gY3J5cHRvX3NjYWxhcm11bHRfU0NBTEFSQllURVM7XG5uYWNsLnNjYWxhck11bHQuZ3JvdXBFbGVtZW50TGVuZ3RoID0gY3J5cHRvX3NjYWxhcm11bHRfQllURVM7XG5cbm5hY2wuYm94ID0gZnVuY3Rpb24obXNnLCBub25jZSwgcHVibGljS2V5LCBzZWNyZXRLZXkpIHtcbiAgdmFyIGsgPSBuYWNsLmJveC5iZWZvcmUocHVibGljS2V5LCBzZWNyZXRLZXkpO1xuICByZXR1cm4gbmFjbC5zZWNyZXRib3gobXNnLCBub25jZSwgayk7XG59O1xuXG5uYWNsLmJveC5iZWZvcmUgPSBmdW5jdGlvbihwdWJsaWNLZXksIHNlY3JldEtleSkge1xuICBjaGVja0FycmF5VHlwZXMocHVibGljS2V5LCBzZWNyZXRLZXkpO1xuICBjaGVja0JveExlbmd0aHMocHVibGljS2V5LCBzZWNyZXRLZXkpO1xuICB2YXIgayA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19ib3hfQkVGT1JFTk1CWVRFUyk7XG4gIGNyeXB0b19ib3hfYmVmb3Jlbm0oaywgcHVibGljS2V5LCBzZWNyZXRLZXkpO1xuICByZXR1cm4gaztcbn07XG5cbm5hY2wuYm94LmFmdGVyID0gbmFjbC5zZWNyZXRib3g7XG5cbm5hY2wuYm94Lm9wZW4gPSBmdW5jdGlvbihtc2csIG5vbmNlLCBwdWJsaWNLZXksIHNlY3JldEtleSkge1xuICB2YXIgayA9IG5hY2wuYm94LmJlZm9yZShwdWJsaWNLZXksIHNlY3JldEtleSk7XG4gIHJldHVybiBuYWNsLnNlY3JldGJveC5vcGVuKG1zZywgbm9uY2UsIGspO1xufTtcblxubmFjbC5ib3gub3Blbi5hZnRlciA9IG5hY2wuc2VjcmV0Ym94Lm9wZW47XG5cbm5hY2wuYm94LmtleVBhaXIgPSBmdW5jdGlvbigpIHtcbiAgdmFyIHBrID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX2JveF9QVUJMSUNLRVlCWVRFUyk7XG4gIHZhciBzayA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19ib3hfU0VDUkVUS0VZQllURVMpO1xuICBjcnlwdG9fYm94X2tleXBhaXIocGssIHNrKTtcbiAgcmV0dXJuIHtwdWJsaWNLZXk6IHBrLCBzZWNyZXRLZXk6IHNrfTtcbn07XG5cbm5hY2wuYm94LmtleVBhaXIuZnJvbVNlY3JldEtleSA9IGZ1bmN0aW9uKHNlY3JldEtleSkge1xuICBjaGVja0FycmF5VHlwZXMoc2VjcmV0S2V5KTtcbiAgaWYgKHNlY3JldEtleS5sZW5ndGggIT09IGNyeXB0b19ib3hfU0VDUkVUS0VZQllURVMpXG4gICAgdGhyb3cgbmV3IEVycm9yKCdiYWQgc2VjcmV0IGtleSBzaXplJyk7XG4gIHZhciBwayA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19ib3hfUFVCTElDS0VZQllURVMpO1xuICBjcnlwdG9fc2NhbGFybXVsdF9iYXNlKHBrLCBzZWNyZXRLZXkpO1xuICByZXR1cm4ge3B1YmxpY0tleTogcGssIHNlY3JldEtleTogbmV3IFVpbnQ4QXJyYXkoc2VjcmV0S2V5KX07XG59O1xuXG5uYWNsLmJveC5wdWJsaWNLZXlMZW5ndGggPSBjcnlwdG9fYm94X1BVQkxJQ0tFWUJZVEVTO1xubmFjbC5ib3guc2VjcmV0S2V5TGVuZ3RoID0gY3J5cHRvX2JveF9TRUNSRVRLRVlCWVRFUztcbm5hY2wuYm94LnNoYXJlZEtleUxlbmd0aCA9IGNyeXB0b19ib3hfQkVGT1JFTk1CWVRFUztcbm5hY2wuYm94Lm5vbmNlTGVuZ3RoID0gY3J5cHRvX2JveF9OT05DRUJZVEVTO1xubmFjbC5ib3gub3ZlcmhlYWRMZW5ndGggPSBuYWNsLnNlY3JldGJveC5vdmVyaGVhZExlbmd0aDtcblxubmFjbC5zaWduID0gZnVuY3Rpb24obXNnLCBzZWNyZXRLZXkpIHtcbiAgY2hlY2tBcnJheVR5cGVzKG1zZywgc2VjcmV0S2V5KTtcbiAgaWYgKHNlY3JldEtleS5sZW5ndGggIT09IGNyeXB0b19zaWduX1NFQ1JFVEtFWUJZVEVTKVxuICAgIHRocm93IG5ldyBFcnJvcignYmFkIHNlY3JldCBrZXkgc2l6ZScpO1xuICB2YXIgc2lnbmVkTXNnID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NpZ25fQllURVMrbXNnLmxlbmd0aCk7XG4gIGNyeXB0b19zaWduKHNpZ25lZE1zZywgbXNnLCBtc2cubGVuZ3RoLCBzZWNyZXRLZXkpO1xuICByZXR1cm4gc2lnbmVkTXNnO1xufTtcblxubmFjbC5zaWduLm9wZW4gPSBmdW5jdGlvbihzaWduZWRNc2csIHB1YmxpY0tleSkge1xuICBpZiAoYXJndW1lbnRzLmxlbmd0aCAhPT0gMilcbiAgICB0aHJvdyBuZXcgRXJyb3IoJ25hY2wuc2lnbi5vcGVuIGFjY2VwdHMgMiBhcmd1bWVudHM7IGRpZCB5b3UgbWVhbiB0byB1c2UgbmFjbC5zaWduLmRldGFjaGVkLnZlcmlmeT8nKTtcbiAgY2hlY2tBcnJheVR5cGVzKHNpZ25lZE1zZywgcHVibGljS2V5KTtcbiAgaWYgKHB1YmxpY0tleS5sZW5ndGggIT09IGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTKVxuICAgIHRocm93IG5ldyBFcnJvcignYmFkIHB1YmxpYyBrZXkgc2l6ZScpO1xuICB2YXIgdG1wID0gbmV3IFVpbnQ4QXJyYXkoc2lnbmVkTXNnLmxlbmd0aCk7XG4gIHZhciBtbGVuID0gY3J5cHRvX3NpZ25fb3Blbih0bXAsIHNpZ25lZE1zZywgc2lnbmVkTXNnLmxlbmd0aCwgcHVibGljS2V5KTtcbiAgaWYgKG1sZW4gPCAwKSByZXR1cm4gbnVsbDtcbiAgdmFyIG0gPSBuZXcgVWludDhBcnJheShtbGVuKTtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBtLmxlbmd0aDsgaSsrKSBtW2ldID0gdG1wW2ldO1xuICByZXR1cm4gbTtcbn07XG5cbm5hY2wuc2lnbi5kZXRhY2hlZCA9IGZ1bmN0aW9uKG1zZywgc2VjcmV0S2V5KSB7XG4gIHZhciBzaWduZWRNc2cgPSBuYWNsLnNpZ24obXNnLCBzZWNyZXRLZXkpO1xuICB2YXIgc2lnID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NpZ25fQllURVMpO1xuICBmb3IgKHZhciBpID0gMDsgaSA8IHNpZy5sZW5ndGg7IGkrKykgc2lnW2ldID0gc2lnbmVkTXNnW2ldO1xuICByZXR1cm4gc2lnO1xufTtcblxubmFjbC5zaWduLmRldGFjaGVkLnZlcmlmeSA9IGZ1bmN0aW9uKG1zZywgc2lnLCBwdWJsaWNLZXkpIHtcbiAgY2hlY2tBcnJheVR5cGVzKG1zZywgc2lnLCBwdWJsaWNLZXkpO1xuICBpZiAoc2lnLmxlbmd0aCAhPT0gY3J5cHRvX3NpZ25fQllURVMpXG4gICAgdGhyb3cgbmV3IEVycm9yKCdiYWQgc2lnbmF0dXJlIHNpemUnKTtcbiAgaWYgKHB1YmxpY0tleS5sZW5ndGggIT09IGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTKVxuICAgIHRocm93IG5ldyBFcnJvcignYmFkIHB1YmxpYyBrZXkgc2l6ZScpO1xuICB2YXIgc20gPSBuZXcgVWludDhBcnJheShjcnlwdG9fc2lnbl9CWVRFUyArIG1zZy5sZW5ndGgpO1xuICB2YXIgbSA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zaWduX0JZVEVTICsgbXNnLmxlbmd0aCk7XG4gIHZhciBpO1xuICBmb3IgKGkgPSAwOyBpIDwgY3J5cHRvX3NpZ25fQllURVM7IGkrKykgc21baV0gPSBzaWdbaV07XG4gIGZvciAoaSA9IDA7IGkgPCBtc2cubGVuZ3RoOyBpKyspIHNtW2krY3J5cHRvX3NpZ25fQllURVNdID0gbXNnW2ldO1xuICByZXR1cm4gKGNyeXB0b19zaWduX29wZW4obSwgc20sIHNtLmxlbmd0aCwgcHVibGljS2V5KSA+PSAwKTtcbn07XG5cbm5hY2wuc2lnbi5rZXlQYWlyID0gZnVuY3Rpb24oKSB7XG4gIHZhciBwayA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTKTtcbiAgdmFyIHNrID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NpZ25fU0VDUkVUS0VZQllURVMpO1xuICBjcnlwdG9fc2lnbl9rZXlwYWlyKHBrLCBzayk7XG4gIHJldHVybiB7cHVibGljS2V5OiBwaywgc2VjcmV0S2V5OiBza307XG59O1xuXG5uYWNsLnNpZ24ua2V5UGFpci5mcm9tU2VjcmV0S2V5ID0gZnVuY3Rpb24oc2VjcmV0S2V5KSB7XG4gIGNoZWNrQXJyYXlUeXBlcyhzZWNyZXRLZXkpO1xuICBpZiAoc2VjcmV0S2V5Lmxlbmd0aCAhPT0gY3J5cHRvX3NpZ25fU0VDUkVUS0VZQllURVMpXG4gICAgdGhyb3cgbmV3IEVycm9yKCdiYWQgc2VjcmV0IGtleSBzaXplJyk7XG4gIHZhciBwayA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTKTtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBway5sZW5ndGg7IGkrKykgcGtbaV0gPSBzZWNyZXRLZXlbMzIraV07XG4gIHJldHVybiB7cHVibGljS2V5OiBwaywgc2VjcmV0S2V5OiBuZXcgVWludDhBcnJheShzZWNyZXRLZXkpfTtcbn07XG5cbm5hY2wuc2lnbi5rZXlQYWlyLmZyb21TZWVkID0gZnVuY3Rpb24oc2VlZCkge1xuICBjaGVja0FycmF5VHlwZXMoc2VlZCk7XG4gIGlmIChzZWVkLmxlbmd0aCAhPT0gY3J5cHRvX3NpZ25fU0VFREJZVEVTKVxuICAgIHRocm93IG5ldyBFcnJvcignYmFkIHNlZWQgc2l6ZScpO1xuICB2YXIgcGsgPSBuZXcgVWludDhBcnJheShjcnlwdG9fc2lnbl9QVUJMSUNLRVlCWVRFUyk7XG4gIHZhciBzayA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zaWduX1NFQ1JFVEtFWUJZVEVTKTtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCAzMjsgaSsrKSBza1tpXSA9IHNlZWRbaV07XG4gIGNyeXB0b19zaWduX2tleXBhaXIocGssIHNrLCB0cnVlKTtcbiAgcmV0dXJuIHtwdWJsaWNLZXk6IHBrLCBzZWNyZXRLZXk6IHNrfTtcbn07XG5cbm5hY2wuc2lnbi5wdWJsaWNLZXlMZW5ndGggPSBjcnlwdG9fc2lnbl9QVUJMSUNLRVlCWVRFUztcbm5hY2wuc2lnbi5zZWNyZXRLZXlMZW5ndGggPSBjcnlwdG9fc2lnbl9TRUNSRVRLRVlCWVRFUztcbm5hY2wuc2lnbi5zZWVkTGVuZ3RoID0gY3J5cHRvX3NpZ25fU0VFREJZVEVTO1xubmFjbC5zaWduLnNpZ25hdHVyZUxlbmd0aCA9IGNyeXB0b19zaWduX0JZVEVTO1xuXG5uYWNsLmhhc2ggPSBmdW5jdGlvbihtc2cpIHtcbiAgY2hlY2tBcnJheVR5cGVzKG1zZyk7XG4gIHZhciBoID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX2hhc2hfQllURVMpO1xuICBjcnlwdG9faGFzaChoLCBtc2csIG1zZy5sZW5ndGgpO1xuICByZXR1cm4gaDtcbn07XG5cbm5hY2wuaGFzaC5oYXNoTGVuZ3RoID0gY3J5cHRvX2hhc2hfQllURVM7XG5cbm5hY2wudmVyaWZ5ID0gZnVuY3Rpb24oeCwgeSkge1xuICBjaGVja0FycmF5VHlwZXMoeCwgeSk7XG4gIC8vIFplcm8gbGVuZ3RoIGFyZ3VtZW50cyBhcmUgY29uc2lkZXJlZCBub3QgZXF1YWwuXG4gIGlmICh4Lmxlbmd0aCA9PT0gMCB8fCB5Lmxlbmd0aCA9PT0gMCkgcmV0dXJuIGZhbHNlO1xuICBpZiAoeC5sZW5ndGggIT09IHkubGVuZ3RoKSByZXR1cm4gZmFsc2U7XG4gIHJldHVybiAodm4oeCwgMCwgeSwgMCwgeC5sZW5ndGgpID09PSAwKSA/IHRydWUgOiBmYWxzZTtcbn07XG5cbm5hY2wuc2V0UFJORyA9IGZ1bmN0aW9uKGZuKSB7XG4gIHJhbmRvbWJ5dGVzID0gZm47XG59O1xuXG4oZnVuY3Rpb24oKSB7XG4gIC8vIEluaXRpYWxpemUgUFJORyBpZiBlbnZpcm9ubWVudCBwcm92aWRlcyBDU1BSTkcuXG4gIC8vIElmIG5vdCwgbWV0aG9kcyBjYWxsaW5nIHJhbmRvbWJ5dGVzIHdpbGwgdGhyb3cuXG4gIHZhciBjcnlwdG8gPSB0eXBlb2Ygc2VsZiAhPT0gJ3VuZGVmaW5lZCcgPyAoc2VsZi5jcnlwdG8gfHwgc2VsZi5tc0NyeXB0bykgOiBudWxsO1xuICBpZiAoY3J5cHRvICYmIGNyeXB0by5nZXRSYW5kb21WYWx1ZXMpIHtcbiAgICAvLyBCcm93c2Vycy5cbiAgICB2YXIgUVVPVEEgPSA2NTUzNjtcbiAgICBuYWNsLnNldFBSTkcoZnVuY3Rpb24oeCwgbikge1xuICAgICAgdmFyIGksIHYgPSBuZXcgVWludDhBcnJheShuKTtcbiAgICAgIGZvciAoaSA9IDA7IGkgPCBuOyBpICs9IFFVT1RBKSB7XG4gICAgICAgIGNyeXB0by5nZXRSYW5kb21WYWx1ZXModi5zdWJhcnJheShpLCBpICsgTWF0aC5taW4obiAtIGksIFFVT1RBKSkpO1xuICAgICAgfVxuICAgICAgZm9yIChpID0gMDsgaSA8IG47IGkrKykgeFtpXSA9IHZbaV07XG4gICAgICBjbGVhbnVwKHYpO1xuICAgIH0pO1xuICB9IGVsc2UgaWYgKHR5cGVvZiByZXF1aXJlICE9PSAndW5kZWZpbmVkJykge1xuICAgIC8vIE5vZGUuanMuXG4gICAgY3J5cHRvID0gcmVxdWlyZSgnY3J5cHRvJyk7XG4gICAgaWYgKGNyeXB0byAmJiBjcnlwdG8ucmFuZG9tQnl0ZXMpIHtcbiAgICAgIG5hY2wuc2V0UFJORyhmdW5jdGlvbih4LCBuKSB7XG4gICAgICAgIHZhciBpLCB2ID0gY3J5cHRvLnJhbmRvbUJ5dGVzKG4pO1xuICAgICAgICBmb3IgKGkgPSAwOyBpIDwgbjsgaSsrKSB4W2ldID0gdltpXTtcbiAgICAgICAgY2xlYW51cCh2KTtcbiAgICAgIH0pO1xuICAgIH1cbiAgfVxufSkoKTtcblxufSkodHlwZW9mIG1vZHVsZSAhPT0gJ3VuZGVmaW5lZCcgJiYgbW9kdWxlLmV4cG9ydHMgPyBtb2R1bGUuZXhwb3J0cyA6IChzZWxmLm5hY2wgPSBzZWxmLm5hY2wgfHwge30pKTtcbiIsInZhciBFeG9udW0gPSByZXF1aXJlKCcuL2luZGV4LmpzJyk7XG5cbndpbmRvdy5FeG9udW0gPSB3aW5kb3cuRXhvbnVtIHx8IEV4b251bTtcbiIsIlwidXNlIHN0cmljdFwiO1xuXG52YXIgc2hhID0gcmVxdWlyZSgnc2hhLmpzJyk7XG52YXIgbmFjbCA9IHJlcXVpcmUoJ3R3ZWV0bmFjbCcpO1xudmFyIG9iamVjdEFzc2lnbiA9IHJlcXVpcmUoJ29iamVjdC1hc3NpZ24nKTtcbnZhciBmcyA9IHJlcXVpcmUoJ2ZzJyk7XG52YXIgYmlnSW50ID0gcmVxdWlyZShcImJpZy1pbnRlZ2VyXCIpO1xuXG52YXIgRXhvbnVtQ2xpZW50ID0gKGZ1bmN0aW9uKCkge1xuXG4gICAgdmFyIENPTlNUID0ge1xuICAgICAgICBNSU5fSU5UODogLTEyOCxcbiAgICAgICAgTUFYX0lOVDg6IDEyNyxcbiAgICAgICAgTUlOX0lOVDE2OiAtMzI3NjgsXG4gICAgICAgIE1BWF9JTlQxNjogMzI3NjcsXG4gICAgICAgIE1JTl9JTlQzMjogLTIxNDc0ODM2NDgsXG4gICAgICAgIE1BWF9JTlQzMjogMjE0NzQ4MzY0NyxcbiAgICAgICAgTUlOX0lOVDY0OiAnLTkyMjMzNzIwMzY4NTQ3NzU4MDgnLFxuICAgICAgICBNQVhfSU5UNjQ6ICc5MjIzMzcyMDM2ODU0Nzc1ODA3JyxcbiAgICAgICAgTUFYX1VJTlQ4OiAyNTUsXG4gICAgICAgIE1BWF9VSU5UMTY6IDY1NTM1LFxuICAgICAgICBNQVhfVUlOVDMyOiA0Mjk0OTY3Mjk1LFxuICAgICAgICBNQVhfVUlOVDY0OiAnMTg0NDY3NDQwNzM3MDk1NTE2MTUnLFxuICAgICAgICBNRVJLTEVfUEFUUklDSUFfS0VZX0xFTkdUSDogMzIsXG4gICAgICAgIFNJR05BVFVSRV9MRU5HVEg6IDY0LFxuICAgICAgICBORVRXT1JLX0lEOiAwXG4gICAgfTtcbiAgICB2YXIgREJLZXkgPSBjcmVhdGVOZXdUeXBlKHtcbiAgICAgICAgc2l6ZTogMzQsXG4gICAgICAgIGZpZWxkczoge1xuICAgICAgICAgICAgdmFyaWFudDoge3R5cGU6IFVpbnQ4LCBzaXplOiAxLCBmcm9tOiAwLCB0bzogMX0sXG4gICAgICAgICAgICBrZXk6IHt0eXBlOiBIYXNoLCBzaXplOiAzMiwgZnJvbTogMSwgdG86IDMzfSxcbiAgICAgICAgICAgIGxlbmd0aDoge3R5cGU6IFVpbnQ4LCBzaXplOiAxLCBmcm9tOiAzMywgdG86IDM0fVxuICAgICAgICB9XG4gICAgfSk7XG4gICAgdmFyIEJyYW5jaCA9IGNyZWF0ZU5ld1R5cGUoe1xuICAgICAgICBzaXplOiAxMzIsXG4gICAgICAgIGZpZWxkczoge1xuICAgICAgICAgICAgbGVmdF9oYXNoOiB7dHlwZTogSGFzaCwgc2l6ZTogMzIsIGZyb206IDAsIHRvOiAzMn0sXG4gICAgICAgICAgICByaWdodF9oYXNoOiB7dHlwZTogSGFzaCwgc2l6ZTogMzIsIGZyb206IDMyLCB0bzogNjR9LFxuICAgICAgICAgICAgbGVmdF9rZXk6IHt0eXBlOiBEQktleSwgc2l6ZTogMzQsIGZyb206IDY0LCB0bzogOTh9LFxuICAgICAgICAgICAgcmlnaHRfa2V5OiB7dHlwZTogREJLZXksIHNpemU6IDM0LCBmcm9tOiA5OCwgdG86IDEzMn1cbiAgICAgICAgfVxuICAgIH0pO1xuICAgIHZhciBSb290QnJhbmNoID0gY3JlYXRlTmV3VHlwZSh7XG4gICAgICAgIHNpemU6IDY2LFxuICAgICAgICBmaWVsZHM6IHtcbiAgICAgICAgICAgIGtleToge3R5cGU6IERCS2V5LCBzaXplOiAzNCwgZnJvbTogMCwgdG86IDM0fSxcbiAgICAgICAgICAgIGhhc2g6IHt0eXBlOiBIYXNoLCBzaXplOiAzMiwgZnJvbTogMzQsIHRvOiA2Nn1cbiAgICAgICAgfVxuICAgIH0pO1xuICAgIHZhciBCbG9jayA9IGNyZWF0ZU5ld1R5cGUoe1xuICAgICAgICBzaXplOiAxMTYsXG4gICAgICAgIGZpZWxkczoge1xuICAgICAgICAgICAgaGVpZ2h0OiB7dHlwZTogVWludDY0LCBzaXplOiA4LCBmcm9tOiAwLCB0bzogOH0sXG4gICAgICAgICAgICBwcm9wb3NlX3JvdW5kOiB7dHlwZTogVWludDMyLCBzaXplOiA0LCBmcm9tOiA4LCB0bzogMTJ9LFxuICAgICAgICAgICAgdGltZToge3R5cGU6IFRpbWVzcGVjLCBzaXplOiA4LCBmcm9tOiAxMiwgdG86IDIwfSxcbiAgICAgICAgICAgIHByZXZfaGFzaDoge3R5cGU6IEhhc2gsIHNpemU6IDMyLCBmcm9tOiAyMCwgdG86IDUyfSxcbiAgICAgICAgICAgIHR4X2hhc2g6IHt0eXBlOiBIYXNoLCBzaXplOiAzMiwgZnJvbTogNTIsIHRvOiA4NH0sXG4gICAgICAgICAgICBzdGF0ZV9oYXNoOiB7dHlwZTogSGFzaCwgc2l6ZTogMzIsIGZyb206IDg0LCB0bzogMTE2fVxuICAgICAgICB9XG4gICAgfSk7XG4gICAgdmFyIE1lc3NhZ2VIZWFkID0gY3JlYXRlTmV3VHlwZSh7XG4gICAgICAgIHNpemU6IDEwLFxuICAgICAgICBmaWVsZHM6IHtcbiAgICAgICAgICAgIG5ldHdvcmtfaWQ6IHt0eXBlOiBVaW50OCwgc2l6ZTogMSwgZnJvbTogMCwgdG86IDF9LFxuICAgICAgICAgICAgdmVyc2lvbjoge3R5cGU6IFVpbnQ4LCBzaXplOiAxLCBmcm9tOiAxLCB0bzogMn0sXG4gICAgICAgICAgICBtZXNzYWdlX2lkOiB7dHlwZTogVWludDE2LCBzaXplOiAyLCBmcm9tOiAyLCB0bzogNH0sXG4gICAgICAgICAgICBzZXJ2aWNlX2lkOiB7dHlwZTogVWludDE2LCBzaXplOiAyLCBmcm9tOiA0LCB0bzogNn0sXG4gICAgICAgICAgICBwYXlsb2FkOiB7dHlwZTogVWludDMyLCBzaXplOiA0LCBmcm9tOiA2LCB0bzogMTB9XG4gICAgICAgIH1cbiAgICB9KTtcbiAgICB2YXIgUHJlY29tbWl0ID0gY3JlYXRlTmV3TWVzc2FnZSh7XG4gICAgICAgIHNpemU6IDg0LFxuICAgICAgICBzZXJ2aWNlX2lkOiAwLFxuICAgICAgICBtZXNzYWdlX2lkOiA0LFxuICAgICAgICBmaWVsZHM6IHtcbiAgICAgICAgICAgIHZhbGlkYXRvcjoge3R5cGU6IFVpbnQzMiwgc2l6ZTogNCwgZnJvbTogMCwgdG86IDR9LFxuICAgICAgICAgICAgaGVpZ2h0OiB7dHlwZTogVWludDY0LCBzaXplOiA4LCBmcm9tOiA4LCB0bzogMTZ9LFxuICAgICAgICAgICAgcm91bmQ6IHt0eXBlOiBVaW50MzIsIHNpemU6IDQsIGZyb206IDE2LCB0bzogMjB9LFxuICAgICAgICAgICAgcHJvcG9zZV9oYXNoOiB7dHlwZTogSGFzaCwgc2l6ZTogMzIsIGZyb206IDIwLCB0bzogNTJ9LFxuICAgICAgICAgICAgYmxvY2tfaGFzaDoge3R5cGU6IEhhc2gsIHNpemU6IDMyLCBmcm9tOiA1MiwgdG86IDg0fVxuICAgICAgICB9XG4gICAgfSk7XG5cbiAgICAvKipcbiAgICAgKiBAY29uc3RydWN0b3JcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gdHlwZVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIE5ld1R5cGUodHlwZSkge1xuICAgICAgICB0aGlzLnNpemUgPSB0eXBlLnNpemU7XG4gICAgICAgIHRoaXMuZmllbGRzID0gdHlwZS5maWVsZHM7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gbWV0aG9kIHRvIHNlcmlhbGl6ZSBkYXRhIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnNcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gZGF0YVxuICAgICAqIEByZXR1cm5zIHtBcnJheX1cbiAgICAgKi9cbiAgICBOZXdUeXBlLnByb3RvdHlwZS5zZXJpYWxpemUgPSBmdW5jdGlvbihkYXRhKSB7XG4gICAgICAgIHJldHVybiBzZXJpYWxpemUoW10sIDAsIGRhdGEsIHRoaXMpO1xuICAgIH07XG5cbiAgICAvKipcbiAgICAgKiBDcmVhdGUgZWxlbWVudCBvZiBOZXdUeXBlIGNsYXNzXG4gICAgICogQHBhcmFtIHtPYmplY3R9IHR5cGVcbiAgICAgKiBAcmV0dXJucyB7TmV3VHlwZX1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBjcmVhdGVOZXdUeXBlKHR5cGUpIHtcbiAgICAgICAgcmV0dXJuIG5ldyBOZXdUeXBlKHR5cGUpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBjb25zdHJ1Y3RvclxuICAgICAqIEBwYXJhbSB7T2JqZWN0fSB0eXBlXG4gICAgICovXG4gICAgZnVuY3Rpb24gTmV3TWVzc2FnZSh0eXBlKSB7XG4gICAgICAgIHRoaXMuc2l6ZSA9IHR5cGUuc2l6ZTtcbiAgICAgICAgdGhpcy5tZXNzYWdlX2lkID0gdHlwZS5tZXNzYWdlX2lkO1xuICAgICAgICB0aGlzLnNlcnZpY2VfaWQgPSB0eXBlLnNlcnZpY2VfaWQ7XG4gICAgICAgIHRoaXMuc2lnbmF0dXJlID0gdHlwZS5zaWduYXR1cmU7XG4gICAgICAgIHRoaXMuZmllbGRzID0gdHlwZS5maWVsZHM7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gbWV0aG9kIHRvIHNlcmlhbGl6ZSBkYXRhIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnNcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gZGF0YVxuICAgICAqIEBwYXJhbSB7Qm9vbGVhbn0gY3V0U2lnbmF0dXJlXG4gICAgICogQHJldHVybnMge0FycmF5fVxuICAgICAqL1xuICAgIE5ld01lc3NhZ2UucHJvdG90eXBlLnNlcmlhbGl6ZSA9IGZ1bmN0aW9uKGRhdGEsIGN1dFNpZ25hdHVyZSkge1xuICAgICAgICB2YXIgYnVmZmVyID0gTWVzc2FnZUhlYWQuc2VyaWFsaXplKHtcbiAgICAgICAgICAgIG5ldHdvcmtfaWQ6IENPTlNULk5FVFdPUktfSUQsXG4gICAgICAgICAgICB2ZXJzaW9uOiAwLFxuICAgICAgICAgICAgbWVzc2FnZV9pZDogdGhpcy5tZXNzYWdlX2lkLFxuICAgICAgICAgICAgc2VydmljZV9pZDogdGhpcy5zZXJ2aWNlX2lkXG4gICAgICAgIH0pO1xuXG4gICAgICAgIC8vIHNlcmlhbGl6ZSBhbmQgYXBwZW5kIG1lc3NhZ2UgYm9keVxuICAgICAgICBidWZmZXIgPSBzZXJpYWxpemUoYnVmZmVyLCBNZXNzYWdlSGVhZC5zaXplLCBkYXRhLCB0aGlzKTtcbiAgICAgICAgaWYgKHR5cGVvZiBidWZmZXIgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICAvLyBjYWxjdWxhdGUgcGF5bG9hZCBhbmQgaW5zZXJ0IGl0IGludG8gYnVmZmVyXG4gICAgICAgIFVpbnQzMihidWZmZXIubGVuZ3RoICsgQ09OU1QuU0lHTkFUVVJFX0xFTkdUSCwgYnVmZmVyLCBNZXNzYWdlSGVhZC5maWVsZHMucGF5bG9hZC5mcm9tLCBNZXNzYWdlSGVhZC5maWVsZHMucGF5bG9hZC50byk7XG5cbiAgICAgICAgaWYgKGN1dFNpZ25hdHVyZSAhPT0gdHJ1ZSkge1xuICAgICAgICAgICAgLy8gYXBwZW5kIHNpZ25hdHVyZVxuICAgICAgICAgICAgRGlnZXN0KHRoaXMuc2lnbmF0dXJlLCBidWZmZXIsIGJ1ZmZlci5sZW5ndGgsIGJ1ZmZlci5sZW5ndGggKyBDT05TVC5TSUdOQVRVUkVfTEVOR1RIKTtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfTtcblxuICAgIC8qKlxuICAgICAqIENyZWF0ZSBlbGVtZW50IG9mIE5ld01lc3NhZ2UgY2xhc3NcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gdHlwZVxuICAgICAqIEByZXR1cm5zIHtOZXdNZXNzYWdlfVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIGNyZWF0ZU5ld01lc3NhZ2UodHlwZSkge1xuICAgICAgICByZXR1cm4gbmV3IE5ld01lc3NhZ2UodHlwZSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtOdW1iZXIgfHwgU3RyaW5nfSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gSW50OCh2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAoIXZhbGlkYXRlSW50ZWdlcih2YWx1ZSwgQ09OU1QuTUlOX0lOVDgsIENPTlNULk1BWF9JTlQ4LCBmcm9tLCB0bywgMSkpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGlmICh2YWx1ZSA8IDApIHtcbiAgICAgICAgICAgIHZhbHVlID0gQ09OU1QuTUFYX1VJTlQ4ICsgdmFsdWUgKyAxXG4gICAgICAgIH1cblxuICAgICAgICBpbnNlcnRJbnRlZ2VyVG9CeXRlQXJyYXkodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtOdW1iZXIgfHwgU3RyaW5nfSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gSW50MTYodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgaWYgKCF2YWxpZGF0ZUludGVnZXIodmFsdWUsIENPTlNULk1JTl9JTlQxNiwgQ09OU1QuTUFYX0lOVDE2LCBmcm9tLCB0bywgMikpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGlmICh2YWx1ZSA8IDApIHtcbiAgICAgICAgICAgIHZhbHVlID0gQ09OU1QuTUFYX1VJTlQxNiArIHZhbHVlICsgMTtcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEludGVnZXJUb0J5dGVBcnJheSh2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0byk7XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge051bWJlciB8fCBTdHJpbmd9IHZhbHVlXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBJbnQzMih2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAoIXZhbGlkYXRlSW50ZWdlcih2YWx1ZSwgQ09OU1QuTUlOX0lOVDMyLCBDT05TVC5NQVhfSU5UMzIsIGZyb20sIHRvLCA0KSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaWYgKHZhbHVlIDwgMCkge1xuICAgICAgICAgICAgdmFsdWUgPSBDT05TVC5NQVhfVUlOVDMyICsgdmFsdWUgKyAxO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0SW50ZWdlclRvQnl0ZUFycmF5KHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKTtcblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIGRhdGEgdHlwZVxuICAgICAqIEBwYXJhbSB7TnVtYmVyIHx8IFN0cmluZ30gdmFsdWVcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIEludDY0KHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIHZhciB2YWwgPSB2YWxpZGF0ZUJpZ0ludGVnZXIodmFsdWUsIENPTlNULk1JTl9JTlQ2NCwgQ09OU1QuTUFYX0lOVDY0LCBmcm9tLCB0bywgOCk7XG5cbiAgICAgICAgaWYgKHZhbCA9PT0gZmFsc2UpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfSBlbHNlIGlmICghYmlnSW50LmlzSW5zdGFuY2UodmFsKSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaWYgKHZhbC5pc05lZ2F0aXZlKCkpIHtcbiAgICAgICAgICAgIHZhbCA9IGJpZ0ludChDT05TVC5NQVhfVUlOVDY0KS5wbHVzKDEpLnBsdXModmFsKTtcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEJpZ0ludGVnZXJUb0J5dGVBcnJheSh2YWwsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtOdW1iZXIgfHwgU3RyaW5nfSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gVWludDgodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgaWYgKCF2YWxpZGF0ZUludGVnZXIodmFsdWUsIDAsIENPTlNULk1BWF9VSU5UOCwgZnJvbSwgdG8sIDEpKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICBpbnNlcnRJbnRlZ2VyVG9CeXRlQXJyYXkodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtOdW1iZXIgfHwgU3RyaW5nfSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gVWludDE2KHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIGlmICghdmFsaWRhdGVJbnRlZ2VyKHZhbHVlLCAwLCBDT05TVC5NQVhfVUlOVDE2LCBmcm9tLCB0bywgMikpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEludGVnZXJUb0J5dGVBcnJheSh2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0byk7XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge051bWJlciB8fCBTdHJpbmd9IHZhbHVlXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBVaW50MzIodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgaWYgKCF2YWxpZGF0ZUludGVnZXIodmFsdWUsIDAsIENPTlNULk1BWF9VSU5UMzIsIGZyb20sIHRvLCA0KSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0SW50ZWdlclRvQnl0ZUFycmF5KHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKTtcblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIGRhdGEgdHlwZVxuICAgICAqIEBwYXJhbSB7TnVtYmVyIHx8IFN0cmluZ30gdmFsdWVcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIFVpbnQ2NCh2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICB2YXIgdmFsID0gdmFsaWRhdGVCaWdJbnRlZ2VyKHZhbHVlLCAwLCBDT05TVC5NQVhfVUlOVDY0LCBmcm9tLCB0bywgOCk7XG5cbiAgICAgICAgaWYgKHZhbCA9PT0gZmFsc2UpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfSBlbHNlIGlmICghYmlnSW50LmlzSW5zdGFuY2UodmFsKSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0QmlnSW50ZWdlclRvQnl0ZUFycmF5KHZhbCwgYnVmZmVyLCBmcm9tLCB0byk7XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gc3RyaW5nXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBTdHJpbmcoc3RyaW5nLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIGlmICh0eXBlb2Ygc3RyaW5nICE9PSAnc3RyaW5nJykge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgZGF0YSB0eXBlIGlzIHBhc3NlZCBhcyBTdHJpbmcuIFN0cmluZyBpcyByZXF1aXJlZCcpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9IGVsc2UgaWYgKCh0byAtIGZyb20pICE9PSA4KSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdTdHJpbmcgc2VnbWVudCBpcyBvZiB3cm9uZyBsZW5ndGguIDggYnl0ZXMgbG9uZyBpcyByZXF1aXJlZCB0byBzdG9yZSB0cmFuc21pdHRlZCB2YWx1ZS4nKTtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIHZhciBidWZmZXJMZW5ndGggPSBidWZmZXIubGVuZ3RoO1xuICAgICAgICBVaW50MzIoYnVmZmVyTGVuZ3RoLCBidWZmZXIsIGZyb20sIGZyb20gKyA0KTsgLy8gaW5kZXggd2hlcmUgc3RyaW5nIGNvbnRlbnQgc3RhcnRzIGluIGJ1ZmZlclxuICAgICAgICBpbnNlcnRTdHJpbmdUb0J5dGVBcnJheShzdHJpbmcsIGJ1ZmZlciwgYnVmZmVyTGVuZ3RoKTsgLy8gc3RyaW5nIGNvbnRlbnRcbiAgICAgICAgVWludDMyKGJ1ZmZlci5sZW5ndGggLSBidWZmZXJMZW5ndGgsIGJ1ZmZlciwgZnJvbSArIDQsIGZyb20gKyA4KTsgLy8gc3RyaW5nIGxlbmd0aFxuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IGhhc2hcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIEhhc2goaGFzaCwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAoIXZhbGlkYXRlSGV4SGFzaChoYXNoKSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9IGVsc2UgaWYgKCh0byAtIGZyb20pICE9PSAzMikge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSGFzaCBzZWdtZW50IGlzIG9mIHdyb25nIGxlbmd0aC4gMzIgYnl0ZXMgbG9uZyBpcyByZXF1aXJlZCB0byBzdG9yZSB0cmFuc21pdHRlZCB2YWx1ZS4nKTtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEhleGFkZWNpbWFsVG9BcnJheShoYXNoLCBidWZmZXIsIGZyb20sIHRvKTtcblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIGRhdGEgdHlwZVxuICAgICAqIEBwYXJhbSB7U3RyaW5nfSBkaWdlc3RcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIERpZ2VzdChkaWdlc3QsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2goZGlnZXN0LCA2NCkpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfSBlbHNlIGlmICgodG8gLSBmcm9tKSAhPT0gNjQpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0RpZ2VzdCBzZWdtZW50IGlzIG9mIHdyb25nIGxlbmd0aC4gNjQgYnl0ZXMgbG9uZyBpcyByZXF1aXJlZCB0byBzdG9yZSB0cmFuc21pdHRlZCB2YWx1ZS4nKTtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEhleGFkZWNpbWFsVG9BcnJheShkaWdlc3QsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IHB1YmxpY0tleVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gUHVibGljS2V5KHB1YmxpY0tleSwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAoIXZhbGlkYXRlSGV4SGFzaChwdWJsaWNLZXkpKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH0gZWxzZSBpZiAoKHRvIC0gZnJvbSkgIT09IDMyKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdQdWJsaWNLZXkgc2VnbWVudCBpcyBvZiB3cm9uZyBsZW5ndGguIDMyIGJ5dGVzIGxvbmcgaXMgcmVxdWlyZWQgdG8gc3RvcmUgdHJhbnNtaXR0ZWQgdmFsdWUuJyk7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICBpbnNlcnRIZXhhZGVjaW1hbFRvQXJyYXkocHVibGljS2V5LCBidWZmZXIsIGZyb20sIHRvKTtcblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIGRhdGEgdHlwZVxuICAgICAqIEBwYXJhbSB7TnVtYmVyIHx8IFN0cmluZ30gbmFub3NlY29uZHNcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIFRpbWVzcGVjKG5hbm9zZWNvbmRzLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIHZhciB2YWwgPSB2YWxpZGF0ZUJpZ0ludGVnZXIobmFub3NlY29uZHMsIDAsIENPTlNULk1BWF9VSU5UNjQsIGZyb20sIHRvLCA4KTtcblxuICAgICAgICBpZiAodmFsID09PSBmYWxzZSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9IGVsc2UgaWYgKCFiaWdJbnQuaXNJbnN0YW5jZSh2YWwpKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICBpbnNlcnRCaWdJbnRlZ2VyVG9CeXRlQXJyYXkodmFsLCBidWZmZXIsIGZyb20sIHRvKTtcblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIGRhdGEgdHlwZVxuICAgICAqIEBwYXJhbSB7Ym9vbGVhbn0gdmFsdWVcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIEJvb2wodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgaWYgKHR5cGVvZiB2YWx1ZSAhPT0gJ2Jvb2xlYW4nKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBkYXRhIHR5cGUgaXMgcGFzc2VkIGFzIEJvb2xlYW4uIEJvb2xlYW4gaXMgcmVxdWlyZWQnKTtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfSBlbHNlIGlmICgodG8gLSBmcm9tKSAhPT0gMSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignQm9vbCBzZWdtZW50IGlzIG9mIHdyb25nIGxlbmd0aC4gMSBieXRlcyBsb25nIGlzIHJlcXVpcmVkIHRvIHN0b3JlIHRyYW5zbWl0dGVkIHZhbHVlLicpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0SW50ZWdlclRvQnl0ZUFycmF5KHZhbHVlID8gMSA6IDAsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ2hlY2sgaW50ZWdlciBudW1iZXIgdmFsaWRpdHlcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdmFsdWVcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gbWluXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IG1heFxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGxlbmd0aFxuICAgICAqIEByZXR1cm5zIHtCb29sZWFufVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIHZhbGlkYXRlSW50ZWdlcih2YWx1ZSwgbWluLCBtYXgsIGZyb20sIHRvLCBsZW5ndGgpIHtcbiAgICAgICAgaWYgKHR5cGVvZiB2YWx1ZSAhPT0gJ251bWJlcicpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIGRhdGEgdHlwZSBpcyBwYXNzZWQgYXMgbnVtYmVyLiBTaG91bGQgYmUgb2YgdHlwZSBOdW1iZXIuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH0gZWxzZSBpZiAodmFsdWUgPCBtaW4pIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ051bWJlciBzaG91bGQgYmUgbW9yZSBvciBlcXVhbCB0byAnICsgbWluICsgJy4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfSBlbHNlIGlmICh2YWx1ZSA+IG1heCkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignTnVtYmVyIHNob3VsZCBiZSBsZXNzIG9yIGVxdWFsIHRvICcgKyBtYXggKyAnLicpO1xuICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICB9IGVsc2UgaWYgKCh0byAtIGZyb20pICE9PSBsZW5ndGgpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ051bWJlciBzZWdtZW50IGlzIG9mIHdyb25nIGxlbmd0aC4gJyArIGxlbmd0aCArICcgYnl0ZXMgbG9uZyBpcyByZXF1aXJlZCB0byBzdG9yZSB0cmFuc21pdHRlZCB2YWx1ZS4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiB0cnVlO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENoZWNrIGJpdCBpbnRlZ2VyIG51bWJlciB2YWxpZGl0eVxuICAgICAqIEBwYXJhbSB7TnVtYmVyIHx8IFN0cmluZ30gdmFsdWVcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gbWluXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IG1heFxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGxlbmd0aFxuICAgICAqIEByZXR1cm5zIHtiaWdJbnQgfHwgQm9vbGVhbn1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiB2YWxpZGF0ZUJpZ0ludGVnZXIodmFsdWUsIG1pbiwgbWF4LCBmcm9tLCB0bywgbGVuZ3RoKSB7XG4gICAgICAgIHZhciB2YWw7XG5cbiAgICAgICAgaWYgKCEodHlwZW9mIHZhbHVlID09PSAnbnVtYmVyJyB8fCB0eXBlb2YgdmFsdWUgPT09ICdzdHJpbmcnKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgZGF0YSB0eXBlIGlzIHBhc3NlZCBhcyBudW1iZXIuIFNob3VsZCBiZSBvZiB0eXBlIE51bWJlciBvciBTdHJpbmcuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH0gZWxzZSBpZiAoKHRvIC0gZnJvbSkgIT09IGxlbmd0aCkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignTnVtYmVyIHNlZ21lbnQgaXMgb2Ygd3JvbmcgbGVuZ3RoLiAnICsgbGVuZ3RoICsgJyBieXRlcyBsb25nIGlzIHJlcXVpcmVkIHRvIHN0b3JlIHRyYW5zbWl0dGVkIHZhbHVlLicpO1xuICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICB9XG5cbiAgICAgICAgdHJ5IHtcbiAgICAgICAgICAgIHZhbCA9IGJpZ0ludCh2YWx1ZSk7XG5cbiAgICAgICAgICAgIGlmICh2YWwubHQobWluKSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ051bWJlciBzaG91bGQgYmUgbW9yZSBvciBlcXVhbCB0byAnICsgbWluICsgJy4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9IGVsc2UgaWYgKHZhbC5ndChtYXgpKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignTnVtYmVyIHNob3VsZCBiZSBsZXNzIG9yIGVxdWFsIHRvICcgKyBtYXggKyAnLicpO1xuICAgICAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgcmV0dXJuIHZhbDtcbiAgICAgICAgfSBjYXRjaCAoZSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgZGF0YSB0eXBlIGlzIHBhc3NlZCBhcyBudW1iZXIuIFNob3VsZCBiZSBvZiB0eXBlIE51bWJlciBvciBTdHJpbmcuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH1cbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDaGVjayBoYXNoIHZhbGlkaXR5XG4gICAgICogQHBhcmFtIHtTdHJpbmd9IGhhc2hcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gYnl0ZXNcbiAgICAgKiBAcmV0dXJucyB7Qm9vbGVhbn1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiB2YWxpZGF0ZUhleEhhc2goaGFzaCwgYnl0ZXMpIHtcbiAgICAgICAgdmFyIGJ5dGVzID0gYnl0ZXMgfHwgMzI7XG5cbiAgICAgICAgaWYgKHR5cGVvZiBoYXNoICE9PSAnc3RyaW5nJykge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgZGF0YSB0eXBlIGlzIHBhc3NlZCBhcyBoZXhhZGVjaW1hbCBzdHJpbmcuIFN0cmluZyBpcyByZXF1aXJlZCcpO1xuICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICB9IGVsc2UgaWYgKGhhc2gubGVuZ3RoICE9PSBieXRlcyAqIDIpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0hleGFkZWNpbWFsIHN0cmluZyBpcyBvZiB3cm9uZyBsZW5ndGguICcgKyBieXRlcyAqIDIgKyAnIGNoYXIgc3ltYm9scyBsb25nIGlzIHJlcXVpcmVkLiAnICsgaGFzaC5sZW5ndGggKyAnIGlzIHBhc3NlZC4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfVxuXG4gICAgICAgIGZvciAodmFyIGkgPSAwLCBsZW4gPSBoYXNoLmxlbmd0aDsgaSA8IGxlbjsgaSsrKSB7XG4gICAgICAgICAgICBpZiAoaXNOYU4ocGFyc2VJbnQoaGFzaFtpXSwgMTYpKSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgc3ltYm9sIGluIGhleGFkZWNpbWFsIHN0cmluZy4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9XG4gICAgICAgIH1cblxuICAgICAgICByZXR1cm4gdHJ1ZTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDaGVjayBhcnJheSBvZiA4LWJpdCBpbnRlZ2VycyB2YWxpZGl0eVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGFyclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBieXRlc1xuICAgICAqIEByZXR1cm5zIHtCb29sZWFufVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIHZhbGlkYXRlQnl0ZXNBcnJheShhcnIsIGJ5dGVzKSB7XG4gICAgICAgIGlmIChieXRlcyAmJiBhcnIubGVuZ3RoICE9PSBieXRlcykge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignQXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMgdmFsaWRpdHkgaXMgb2Ygd3JvbmcgbGVuZ3RoLiAnICsgYnl0ZXMgKiAyICsgJyBjaGFyIHN5bWJvbHMgbG9uZyBpcyByZXF1aXJlZC4gJyArIGhhc2gubGVuZ3RoICsgJyBpcyBwYXNzZWQuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH1cblxuICAgICAgICBmb3IgKHZhciBpID0gMCwgbGVuID0gYXJyLmxlbmd0aDsgaSA8IGxlbjsgaSsrKSB7XG4gICAgICAgICAgICBpZiAodHlwZW9mIGFycltpXSAhPT0gJ251bWJlcicpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBkYXRhIHR5cGUgaXMgcGFzc2VkIGFzIGJ5dGUuIE51bWJlciBpcyByZXF1aXJlZCcpO1xuICAgICAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgICAgIH0gZWxzZSBpZiAoYXJyW2ldIDwgMCB8fCBhcnJbaV0gPiAyNTUpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdCeXRlIHNob3VsZCBiZSBpbiBbMC4uMjU1XSByYW5nZS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9XG4gICAgICAgIH1cblxuICAgICAgICByZXR1cm4gdHJ1ZTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDaGVjayBiaW5hcnkgc3RyaW5nIHZhbGlkaXR5XG4gICAgICogQHBhcmFtIHtTdHJpbmd9IHN0clxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBiaXRzXG4gICAgICogQHJldHVybnMge0Jvb2xlYW59XG4gICAgICovXG4gICAgZnVuY3Rpb24gdmFsaWRhdGVCaW5hcnlTdHJpbmcoc3RyLCBiaXRzKSB7XG4gICAgICAgIGlmICh0eXBlb2YgYml0cyAhPT0gJ3VuZGVmaW5lZCcgJiYgc3RyLmxlbmd0aCAhPT0gYml0cykge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignQmluYXJ5IHN0cmluZyBpcyBvZiB3cm9uZyBsZW5ndGguJyk7XG4gICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgfVxuXG4gICAgICAgIHZhciBiaXQ7XG4gICAgICAgIGZvciAodmFyIGkgPSAwOyBpIDwgc3RyLmxlbmd0aDsgaSsrKSB7XG4gICAgICAgICAgICBiaXQgPSBwYXJzZUludChzdHJbaV0pO1xuICAgICAgICAgICAgaWYgKGlzTmFOKGJpdCkpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBiaXQgaXMgcGFzc2VkIGluIGJpbmFyeSBzdHJpbmcuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfSBlbHNlIGlmIChiaXQgPiAxKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgYml0IGlzIHBhc3NlZCBpbiBiaW5hcnkgc3RyaW5nLiBCaXQgc2hvdWxkIGJlIDAgb3IgMS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9XG4gICAgICAgIH1cblxuICAgICAgICByZXR1cm4gdHJ1ZTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBTZXJpYWxpemUgZGF0YSBpbnRvIGFycmF5IG9mIDgtYml0IGludGVnZXJzIGFuZCBpbnNlcnQgaW50byBidWZmZXJcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gc2hpZnQgLSB0aGUgaW5kZXggdG8gc3RhcnQgd3JpdGUgaW50byBidWZmZXJcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gZGF0YVxuICAgICAqIEBwYXJhbSB0eXBlIC0gY2FuIGJlIHtOZXdUeXBlfSBvciBvbmUgb2YgYnVpbHQtaW4gdHlwZXNcbiAgICAgKi9cbiAgICBmdW5jdGlvbiBzZXJpYWxpemUoYnVmZmVyLCBzaGlmdCwgZGF0YSwgdHlwZSkge1xuICAgICAgICBmb3IgKHZhciBpID0gMCwgbGVuID0gdHlwZS5zaXplOyBpIDwgbGVuOyBpKyspIHtcbiAgICAgICAgICAgIGJ1ZmZlcltzaGlmdCArIGldID0gMDtcbiAgICAgICAgfVxuXG4gICAgICAgIGZvciAodmFyIGZpZWxkTmFtZSBpbiBkYXRhKSB7XG4gICAgICAgICAgICBpZiAoIWRhdGEuaGFzT3duUHJvcGVydHkoZmllbGROYW1lKSkge1xuICAgICAgICAgICAgICAgIGNvbnRpbnVlO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICB2YXIgZmllbGRUeXBlID0gdHlwZS5maWVsZHNbZmllbGROYW1lXTtcblxuICAgICAgICAgICAgaWYgKHR5cGVvZiBmaWVsZFR5cGUgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcihmaWVsZE5hbWUgKyAnIGZpZWxkIHdhcyBub3QgZm91bmQgaW4gY29uZmlndXJhdGlvbiBvZiB0eXBlLicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgdmFyIGZpZWxkRGF0YSA9IGRhdGFbZmllbGROYW1lXTtcbiAgICAgICAgICAgIHZhciBmcm9tID0gc2hpZnQgKyBmaWVsZFR5cGUuZnJvbTtcblxuICAgICAgICAgICAgaWYgKGZpZWxkVHlwZS50eXBlIGluc3RhbmNlb2YgTmV3VHlwZSkge1xuICAgICAgICAgICAgICAgIHZhciBpc0ZpeGVkID0gKGZ1bmN0aW9uIGNoZWNrSWZJc0ZpeGVkKGZpZWxkcykge1xuICAgICAgICAgICAgICAgICAgICBmb3IgKHZhciBmaWVsZE5hbWUgaW4gZmllbGRzKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBpZiAoIWZpZWxkcy5oYXNPd25Qcm9wZXJ0eShmaWVsZE5hbWUpKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgY29udGludWU7XG4gICAgICAgICAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICAgICAgICAgIGlmIChmaWVsZHNbZmllbGROYW1lXS50eXBlIGluc3RhbmNlb2YgTmV3VHlwZSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIGNoZWNrSWZJc0ZpeGVkKGZpZWxkc1tmaWVsZE5hbWVdLnR5cGUuZmllbGRzKTtcbiAgICAgICAgICAgICAgICAgICAgICAgIH0gZWxzZSBpZiAoZmllbGRzW2ZpZWxkTmFtZV0udHlwZSA9PT0gU3RyaW5nKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgICAgIHJldHVybiB0cnVlO1xuICAgICAgICAgICAgICAgIH0pKGZpZWxkVHlwZS50eXBlLmZpZWxkcyk7XG5cbiAgICAgICAgICAgICAgICBpZiAoaXNGaXhlZCA9PT0gdHJ1ZSkge1xuICAgICAgICAgICAgICAgICAgICBzZXJpYWxpemUoYnVmZmVyLCBmcm9tLCBmaWVsZERhdGEsIGZpZWxkVHlwZS50eXBlKTtcbiAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICB2YXIgZW5kID0gYnVmZmVyLmxlbmd0aDtcbiAgICAgICAgICAgICAgICAgICAgVWludDMyKGVuZCwgYnVmZmVyLCBmcm9tLCBmcm9tICsgNCk7XG4gICAgICAgICAgICAgICAgICAgIHNlcmlhbGl6ZShidWZmZXIsIGVuZCwgZmllbGREYXRhLCBmaWVsZFR5cGUudHlwZSk7XG4gICAgICAgICAgICAgICAgICAgIFVpbnQzMihidWZmZXIubGVuZ3RoIC0gZW5kLCBidWZmZXIsIGZyb20gKyA0LCBmcm9tICsgOCk7XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICBidWZmZXIgPSBmaWVsZFR5cGUudHlwZShmaWVsZERhdGEsIGJ1ZmZlciwgZnJvbSwgc2hpZnQgKyBmaWVsZFR5cGUudG8pO1xuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgYnVmZmVyID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfVxuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDb252ZXJ0IGhleGFkZWNpbWFsIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMgYW5kIGluc2VydCBpbnRvIGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7U3RyaW5nfSBzdHJcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIGluc2VydEhleGFkZWNpbWFsVG9BcnJheShzdHIsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgZm9yICh2YXIgaSA9IDAsIGxlbiA9IHN0ci5sZW5ndGg7IGkgPCBsZW47IGkgKz0gMikge1xuICAgICAgICAgICAgYnVmZmVyW2Zyb21dID0gcGFyc2VJbnQoc3RyLnN1YnN0cihpLCAyKSwgMTYpO1xuICAgICAgICAgICAgZnJvbSsrO1xuXG4gICAgICAgICAgICBpZiAoZnJvbSA+IHRvKSB7XG4gICAgICAgICAgICAgICAgYnJlYWs7XG4gICAgICAgICAgICB9XG4gICAgICAgIH1cbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDb252ZXJ0IGludGVnZXIgaW50byBhcnJheSBvZiA4LWJpdCBpbnRlZ2VycyBhbmQgaW5zZXJ0IGludG8gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IG51bWJlclxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gaW5zZXJ0SW50ZWdlclRvQnl0ZUFycmF5KG51bWJlciwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICB2YXIgc3RyID0gbnVtYmVyLnRvU3RyaW5nKDE2KTtcblxuICAgICAgICBpbnNlcnROdW1iZXJJbkhleFRvQnl0ZUFycmF5KHN0ciwgYnVmZmVyLCBmcm9tLCB0byk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ29udmVydCBiaWcgaW50ZWdlciBpbnRvIGFycmF5IG9mIDgtYml0IGludGVnZXJzIGFuZCBpbnNlcnQgaW50byBidWZmZXJcbiAgICAgKiBAcGFyYW0ge2JpZ0ludH0gbnVtYmVyXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBpbnNlcnRCaWdJbnRlZ2VyVG9CeXRlQXJyYXkobnVtYmVyLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIHZhciBzdHIgPSBudW1iZXIudG9TdHJpbmcoMTYpO1xuXG4gICAgICAgIGluc2VydE51bWJlckluSGV4VG9CeXRlQXJyYXkoc3RyLCBidWZmZXIsIGZyb20sIHRvKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBJbnNlcnQgbnVtYmVyIGluIGhleCBpbnRvIGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7U3RyaW5nfSBudW1iZXJcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIGluc2VydE51bWJlckluSGV4VG9CeXRlQXJyYXkobnVtYmVyLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIC8vIHN0b3JlIE51bWJlciBhcyBsaXR0bGUtZW5kaWFuXG4gICAgICAgIGlmIChudW1iZXIubGVuZ3RoIDwgMykge1xuICAgICAgICAgICAgYnVmZmVyW2Zyb21dID0gcGFyc2VJbnQobnVtYmVyLCAxNik7XG4gICAgICAgICAgICByZXR1cm4gdHJ1ZTtcbiAgICAgICAgfVxuXG4gICAgICAgIGZvciAodmFyIGkgPSBudW1iZXIubGVuZ3RoOyBpID4gMDsgaSAtPSAyKSB7XG4gICAgICAgICAgICBpZiAoaSA+IDEpIHtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbV0gPSBwYXJzZUludChudW1iZXIuc3Vic3RyKGkgLSAyLCAyKSwgMTYpO1xuICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbV0gPSBwYXJzZUludChudW1iZXIuc3Vic3RyKDAsIDEpLCAxNik7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGZyb20rKztcblxuICAgICAgICAgICAgaWYgKGZyb20gPj0gdG8pIHtcbiAgICAgICAgICAgICAgICBicmVhaztcbiAgICAgICAgICAgIH1cbiAgICAgICAgfVxuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENvbnZlcnQgU3RyaW5nIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMgYW5kIGluc2VydCBpbnRvIGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7U3RyaW5nfSBzdHJcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIGluc2VydFN0cmluZ1RvQnl0ZUFycmF5KHN0ciwgYnVmZmVyLCBmcm9tKSB7XG4gICAgICAgIGZvciAodmFyIGkgPSAwOyBpIDwgc3RyLmxlbmd0aDsgaSsrKSB7XG4gICAgICAgICAgICB2YXIgYyA9IHN0ci5jaGFyQ29kZUF0KGkpO1xuXG4gICAgICAgICAgICBpZiAoYyA8IDEyOCkge1xuICAgICAgICAgICAgICAgIGJ1ZmZlcltmcm9tKytdID0gYztcbiAgICAgICAgICAgIH0gZWxzZSBpZiAoYyA8IDIwNDgpIHtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbSsrXSA9IChjID4+IDYpIHwgMTkyO1xuICAgICAgICAgICAgICAgIGJ1ZmZlcltmcm9tKytdID0gKGMgJiA2MykgfCAxMjg7XG4gICAgICAgICAgICB9IGVsc2UgaWYgKCgoYyAmIDB4RkMwMCkgPT0gMHhEODAwKSAmJiAoaSArIDEpIDwgc3RyLmxlbmd0aCAmJiAoKHN0ci5jaGFyQ29kZUF0KGkgKyAxKSAmIDB4RkMwMCkgPT0gMHhEQzAwKSkge1xuICAgICAgICAgICAgICAgIC8vIHN1cnJvZ2F0ZSBwYWlyXG4gICAgICAgICAgICAgICAgYyA9IDB4MTAwMDAgKyAoKGMgJiAweDAzRkYpIDw8IDEwKSArIChzdHIuY2hhckNvZGVBdCgrK2kpICYgMHgwM0ZGKTtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbSsrXSA9IChjID4+IDE4KSB8IDI0MDtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbSsrXSA9ICgoYyA+PiAxMikgJiA2MykgfCAxMjg7XG4gICAgICAgICAgICAgICAgYnVmZmVyW2Zyb20rK10gPSAoKGMgPj4gNikgJiA2MykgfCAxMjg7XG4gICAgICAgICAgICAgICAgYnVmZmVyW2Zyb20rK10gPSAoYyAmIDYzKSB8IDEyODtcbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgYnVmZmVyW2Zyb20rK10gPSAoYyA+PiAxMikgfCAyMjQ7XG4gICAgICAgICAgICAgICAgYnVmZmVyW2Zyb20rK10gPSAoKGMgPj4gNikgJiA2MykgfCAxMjg7XG4gICAgICAgICAgICAgICAgYnVmZmVyW2Zyb20rK10gPSAoYyAmIDYzKSB8IDEyODtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfVxuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENvbnZlcnQgaGV4YWRlY2ltYWwgc3RyaW5nIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnNcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gc3RyXG4gICAgICogIGhleGFkZWNpbWFsXG4gICAgICogQHJldHVybnMge1VpbnQ4QXJyYXl9XG4gICAgICovXG4gICAgZnVuY3Rpb24gaGV4YWRlY2ltYWxUb1VpbnQ4QXJyYXkoc3RyKSB7XG4gICAgICAgIGlmICh0eXBlb2Ygc3RyICE9PSAnc3RyaW5nJykge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgZGF0YSB0eXBlIG9mIGhleGFkZWNpbWFsIHN0cmluZycpO1xuICAgICAgICAgICAgcmV0dXJuIG5ldyBVaW50OEFycmF5KFtdKTtcbiAgICAgICAgfVxuXG4gICAgICAgIHZhciBhcnJheSA9IFtdO1xuICAgICAgICBmb3IgKHZhciBpID0gMCwgbGVuID0gc3RyLmxlbmd0aDsgaSA8IGxlbjsgaSArPSAyKSB7XG4gICAgICAgICAgICBhcnJheS5wdXNoKHBhcnNlSW50KHN0ci5zdWJzdHIoaSwgMiksIDE2KSk7XG4gICAgICAgIH1cblxuICAgICAgICByZXR1cm4gbmV3IFVpbnQ4QXJyYXkoYXJyYXkpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENvbnZlcnQgaGV4YWRlY2ltYWwgc3RyaW5nIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnNcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gc3RyXG4gICAgICogIGhleGFkZWNpbWFsXG4gICAgICogQHJldHVybnMge1N0cmluZ31cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBoZXhhZGVjaW1hbFRvQmluYXJ5U3RyaW5nKHN0cikge1xuICAgICAgICB2YXIgYmluYXJ5U3RyID0gJyc7XG4gICAgICAgIGZvciAodmFyIGkgPSAwLCBsZW4gPSBzdHIubGVuZ3RoOyBpIDwgbGVuOyBpICs9IDIpIHtcbiAgICAgICAgICAgIGJpbmFyeVN0ciArPSBwYXJzZUludChzdHIuc3Vic3RyKGksIDIpLCAxNikudG9TdHJpbmcoMik7XG4gICAgICAgIH1cbiAgICAgICAgcmV0dXJuIGJpbmFyeVN0cjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDb252ZXJ0IGFycmF5IG9mIDgtYml0IGludGVnZXJzIGludG8gaGV4YWRlY2ltYWwgc3RyaW5nXG4gICAgICogQHBhcmFtIHtVaW50OEFycmF5fSB1aW50OGFyclxuICAgICAqIEByZXR1cm5zIHtTdHJpbmd9XG4gICAgICovXG4gICAgZnVuY3Rpb24gdWludDhBcnJheVRvSGV4YWRlY2ltYWwodWludDhhcnIpIHtcbiAgICAgICAgaWYgKCEodWludDhhcnIgaW5zdGFuY2VvZiBVaW50OEFycmF5KSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgZGF0YSB0eXBlIG9mIGFycmF5IG9mIDgtYml0IGludGVnZXJzJyk7XG4gICAgICAgICAgICByZXR1cm4gJyc7XG4gICAgICAgIH1cblxuICAgICAgICB2YXIgc3RyID0gJyc7XG4gICAgICAgIGZvciAodmFyIGkgPSAwLCBsZW4gPSB1aW50OGFyci5sZW5ndGg7IGkgPCBsZW47IGkrKykge1xuICAgICAgICAgICAgdmFyIGhleCA9ICh1aW50OGFycltpXSkudG9TdHJpbmcoMTYpO1xuICAgICAgICAgICAgaGV4ID0gKGhleC5sZW5ndGggPT09IDEpID8gJzAnICsgaGV4IDogaGV4O1xuICAgICAgICAgICAgc3RyICs9IGhleDtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBzdHIudG9Mb3dlckNhc2UoKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDb252ZXJ0IGJpbmFyeSBzdHJpbmcgaW50byBhcnJheSBvZiA4LWJpdCBpbnRlZ2Vyc1xuICAgICAqIEBwYXJhbSB7U3RyaW5nfSBiaW5hcnlTdHJcbiAgICAgKiBAcmV0dXJucyB7VWludDhBcnJheX1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBiaW5hcnlTdHJpbmdUb1VpbnQ4QXJyYXkoYmluYXJ5U3RyKSB7XG4gICAgICAgIHZhciBhcnJheSA9IFtdO1xuICAgICAgICBmb3IgKHZhciBpID0gMCwgbGVuID0gYmluYXJ5U3RyLmxlbmd0aDsgaSA8IGxlbjsgaSArPSA4KSB7XG4gICAgICAgICAgICBhcnJheS5wdXNoKHBhcnNlSW50KGJpbmFyeVN0ci5zdWJzdHIoaSwgOCksIDIpKTtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBuZXcgVWludDhBcnJheShhcnJheSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ29udmVydCBiaW5hcnkgc3RyaW5nIGludG8gaGV4YWRlY2ltYWwgc3RyaW5nXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IGJpbmFyeVN0clxuICAgICAqIEByZXR1cm5zIHtzdHJpbmd9XG4gICAgICovXG4gICAgZnVuY3Rpb24gYmluYXJ5U3RyaW5nVG9IZXhhZGVjaW1hbChiaW5hcnlTdHIpIHtcbiAgICAgICAgdmFyIHN0ciA9ICcnO1xuICAgICAgICBmb3IgKHZhciBpID0gMCwgbGVuID0gYmluYXJ5U3RyLmxlbmd0aDsgaSA8IGxlbjsgaSArPSA4KSB7XG4gICAgICAgICAgICB2YXIgaGV4ID0gKHBhcnNlSW50KGJpbmFyeVN0ci5zdWJzdHIoaSwgOCksIDIpKS50b1N0cmluZygxNik7XG4gICAgICAgICAgICBoZXggPSAoaGV4Lmxlbmd0aCA9PT0gMSkgPyAnMCcgKyBoZXggOiBoZXg7XG4gICAgICAgICAgICBzdHIgKz0gaGV4O1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIHN0ci50b0xvd2VyQ2FzZSgpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENoZWNrIGlmIGVsZW1lbnQgaXMgb2YgdHlwZSB7T2JqZWN0fVxuICAgICAqIEBwYXJhbSBvYmpcbiAgICAgKiBAcmV0dXJucyB7Ym9vbGVhbn1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBpc09iamVjdChvYmopIHtcbiAgICAgICAgcmV0dXJuICh0eXBlb2Ygb2JqID09PSAnb2JqZWN0JyAmJiAhQXJyYXkuaXNBcnJheShvYmopICYmIG9iaiAhPT0gbnVsbCAmJiAhKG9iaiBpbnN0YW5jZW9mIERhdGUpKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBHZXQgbGVuZ3RoIG9mIG9iamVjdFxuICAgICAqIEBwYXJhbSB7T2JqZWN0fSBvYmpcbiAgICAgKiBAcmV0dXJucyB7TnVtYmVyfVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIGdldE9iamVjdExlbmd0aChvYmopIHtcbiAgICAgICAgdmFyIGwgPSAwO1xuICAgICAgICBmb3IgKHZhciBwcm9wIGluIG9iaikge1xuICAgICAgICAgICAgaWYgKG9iai5oYXNPd25Qcm9wZXJ0eShwcm9wKSkge1xuICAgICAgICAgICAgICAgIGwrKztcbiAgICAgICAgICAgIH1cbiAgICAgICAgfVxuICAgICAgICByZXR1cm4gbDtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBHZXQgU0hBMjU2IGhhc2hcbiAgICAgKiBNZXRob2Qgd2l0aCBvdmVybG9hZGluZy4gQWNjZXB0IHR3byBjb21iaW5hdGlvbnMgb2YgYXJndW1lbnRzOlxuICAgICAqIDEpIHtPYmplY3R9IGRhdGEsIHtOZXdUeXBlfSB0eXBlXG4gICAgICogMikge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcmV0dXJuIHtTdHJpbmd9XG4gICAgICovXG4gICAgZnVuY3Rpb24gaGFzaChkYXRhLCB0eXBlKSB7XG4gICAgICAgIHZhciBidWZmZXI7XG4gICAgICAgIGlmICh0eXBlIGluc3RhbmNlb2YgTmV3VHlwZSkge1xuICAgICAgICAgICAgaWYgKGlzT2JqZWN0KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgYnVmZmVyID0gdHlwZS5zZXJpYWxpemUoZGF0YSk7XG4gICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBidWZmZXIgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgZGF0YSBwYXJhbWV0ZXIuIEluc3RhbmNlIG9mIE5ld1R5cGUgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIHR5cGUgb2YgZGF0YS4gU2hvdWxkIGJlIG9iamVjdC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9XG4gICAgICAgIH0gZWxzZSBpZiAodHlwZSBpbnN0YW5jZW9mIE5ld01lc3NhZ2UpIHtcbiAgICAgICAgICAgIGlmIChpc09iamVjdChkYXRhKSkge1xuICAgICAgICAgICAgICAgIGJ1ZmZlciA9IHR5cGUuc2VyaWFsaXplKGRhdGEpO1xuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgYnVmZmVyID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGRhdGEgcGFyYW1ldGVyLiBJbnN0YW5jZSBvZiBOZXdNZXNzYWdlIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyB0eXBlIG9mIGRhdGEuIFNob3VsZCBiZSBvYmplY3QuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgfVxuICAgICAgICB9IGVsc2UgaWYgKGRhdGEgaW5zdGFuY2VvZiBVaW50OEFycmF5KSB7XG4gICAgICAgICAgICBidWZmZXIgPSBkYXRhO1xuICAgICAgICB9IGVsc2UgaWYgKEFycmF5LmlzQXJyYXkoZGF0YSkpIHtcbiAgICAgICAgICAgIGJ1ZmZlciA9IG5ldyBVaW50OEFycmF5KGRhdGEpO1xuICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB0eXBlIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBzaGEoJ3NoYTI1NicpLnVwZGF0ZShidWZmZXIsICd1dGY4JykuZGlnZXN0KCdoZXgnKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBHZXQgRUQyNTUxOSBzaWduYXR1cmVcbiAgICAgKiBNZXRob2Qgd2l0aCBvdmVybG9hZGluZy4gQWNjZXB0IHR3byBjb21iaW5hdGlvbnMgb2YgYXJndW1lbnRzOlxuICAgICAqIDEpIHtPYmplY3R9IGRhdGEsIHtOZXdUeXBlIHx8IE5ld01lc3NhZ2V9IHR5cGUsIHtTdHJpbmd9IHNlY3JldEtleVxuICAgICAqIDIpIHtBcnJheX0gYnVmZmVyLCB7U3RyaW5nfSBzZWNyZXRLZXlcbiAgICAgKiBAcmV0dXJuIHtTdHJpbmd9XG4gICAgICovXG4gICAgZnVuY3Rpb24gc2lnbihkYXRhLCB0eXBlLCBzZWNyZXRLZXkpIHtcbiAgICAgICAgdmFyIGJ1ZmZlcjtcbiAgICAgICAgdmFyIHNlY3JldEtleSA9IHNlY3JldEtleTtcbiAgICAgICAgdmFyIHNpZ25hdHVyZTtcblxuICAgICAgICBpZiAodHlwZW9mIHNlY3JldEtleSAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgIGlmICh0eXBlIGluc3RhbmNlb2YgTmV3VHlwZSkge1xuICAgICAgICAgICAgICAgIGJ1ZmZlciA9IHR5cGUuc2VyaWFsaXplKGRhdGEpO1xuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgYnVmZmVyID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGRhdGEgcGFyYW1ldGVyLiBJbnN0YW5jZSBvZiBOZXdUeXBlIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIGlmICh0eXBlIGluc3RhbmNlb2YgTmV3TWVzc2FnZSkge1xuICAgICAgICAgICAgICAgIGJ1ZmZlciA9IHR5cGUuc2VyaWFsaXplKGRhdGEsIHRydWUpO1xuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgYnVmZmVyID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGRhdGEgcGFyYW1ldGVyLiBJbnN0YW5jZSBvZiBOZXdNZXNzYWdlIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgIGlmICghdmFsaWRhdGVCeXRlc0FycmF5KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBkYXRhIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGJ1ZmZlciA9IGRhdGE7XG4gICAgICAgICAgICBzZWNyZXRLZXkgPSB0eXBlO1xuICAgICAgICB9XG5cbiAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2goc2VjcmV0S2V5LCA2NCkpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgc2VjcmV0S2V5IHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGJ1ZmZlciA9IG5ldyBVaW50OEFycmF5KGJ1ZmZlcik7XG4gICAgICAgIHNlY3JldEtleSA9IGhleGFkZWNpbWFsVG9VaW50OEFycmF5KHNlY3JldEtleSk7XG4gICAgICAgIHNpZ25hdHVyZSA9IG5hY2wuc2lnbi5kZXRhY2hlZChidWZmZXIsIHNlY3JldEtleSk7XG5cbiAgICAgICAgcmV0dXJuIHVpbnQ4QXJyYXlUb0hleGFkZWNpbWFsKHNpZ25hdHVyZSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogVmVyaWZpZXMgRUQyNTUxOSBzaWduYXR1cmVcbiAgICAgKiBNZXRob2Qgd2l0aCBvdmVybG9hZGluZy4gQWNjZXB0IHR3byBjb21iaW5hdGlvbnMgb2YgYXJndW1lbnRzOlxuICAgICAqIDEpIHtPYmplY3R9IGRhdGEsIHtOZXdUeXBlIHx8IE5ld01lc3NhZ2V9IHR5cGUsIHtTdHJpbmd9IHNpZ25hdHVyZSwge1N0cmluZ30gcHVibGljS2V5XG4gICAgICogMikge0FycmF5fSBidWZmZXIsIHtTdHJpbmd9IHNpZ25hdHVyZSwge1N0cmluZ30gcHVibGljS2V5XG4gICAgICogQHJldHVybiB7Qm9vbGVhbn1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiB2ZXJpZnlTaWduYXR1cmUoZGF0YSwgdHlwZSwgc2lnbmF0dXJlLCBwdWJsaWNLZXkpIHtcbiAgICAgICAgdmFyIGJ1ZmZlcjtcbiAgICAgICAgdmFyIHNpZ25hdHVyZSA9IHNpZ25hdHVyZTtcbiAgICAgICAgdmFyIHB1YmxpY0tleSA9IHB1YmxpY0tleTtcblxuICAgICAgICBpZiAodHlwZW9mIHB1YmxpY0tleSAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgIGlmICh0eXBlIGluc3RhbmNlb2YgTmV3VHlwZSkge1xuICAgICAgICAgICAgICAgIGJ1ZmZlciA9IHR5cGUuc2VyaWFsaXplKGRhdGEpO1xuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgYnVmZmVyID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGRhdGEgcGFyYW1ldGVyLiBJbnN0YW5jZSBvZiBOZXdUeXBlIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIGlmICh0eXBlIGluc3RhbmNlb2YgTmV3TWVzc2FnZSkge1xuICAgICAgICAgICAgICAgIGJ1ZmZlciA9IHR5cGUuc2VyaWFsaXplKGRhdGEsIHRydWUpO1xuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgYnVmZmVyID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGRhdGEgcGFyYW1ldGVyLiBJbnN0YW5jZSBvZiBOZXdNZXNzYWdlIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgIGlmICghdmFsaWRhdGVCeXRlc0FycmF5KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBkYXRhIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGJ1ZmZlciA9IGRhdGE7XG4gICAgICAgICAgICBwdWJsaWNLZXkgPSBzaWduYXR1cmU7XG4gICAgICAgICAgICBzaWduYXR1cmUgPSB0eXBlO1xuICAgICAgICB9XG5cbiAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2gocHVibGljS2V5KSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBwdWJsaWNLZXkgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9IGVsc2UgaWYgKCF2YWxpZGF0ZUhleEhhc2goc2lnbmF0dXJlLCA2NCkpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgc2lnbmF0dXJlIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGJ1ZmZlciA9IG5ldyBVaW50OEFycmF5KGJ1ZmZlcik7XG4gICAgICAgIHNpZ25hdHVyZSA9IGhleGFkZWNpbWFsVG9VaW50OEFycmF5KHNpZ25hdHVyZSk7XG4gICAgICAgIHB1YmxpY0tleSA9IGhleGFkZWNpbWFsVG9VaW50OEFycmF5KHB1YmxpY0tleSk7XG5cbiAgICAgICAgcmV0dXJuIG5hY2wuc2lnbi5kZXRhY2hlZC52ZXJpZnkoYnVmZmVyLCBzaWduYXR1cmUsIHB1YmxpY0tleSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ2hlY2sgTWVya2xlIHRyZWUgcHJvb2YgYW5kIHJldHVybiBhcnJheSBvZiBlbGVtZW50c1xuICAgICAqIEBwYXJhbSB7U3RyaW5nfSByb290SGFzaFxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBjb3VudFxuICAgICAqIEBwYXJhbSB7T2JqZWN0fSBwcm9vZk5vZGVcbiAgICAgKiBAcGFyYW0ge0FycmF5fSByYW5nZVxuICAgICAqIEBwYXJhbSB7TmV3VHlwZX0gdHlwZSAob3B0aW9uYWwpXG4gICAgICogQHJldHVybiB7QXJyYXl9XG4gICAgICovXG4gICAgZnVuY3Rpb24gbWVya2xlUHJvb2Yocm9vdEhhc2gsIGNvdW50LCBwcm9vZk5vZGUsIHJhbmdlLCB0eXBlKSB7XG4gICAgICAgIC8qKlxuICAgICAgICAgKiBDYWxjdWxhdGUgaGVpZ2h0IG9mIG1lcmtsZSB0cmVlXG4gICAgICAgICAqIEBwYXJhbSB7YmlnSW50fSBjb3VudFxuICAgICAgICAgKiBAcmV0dXJuIHtOdW1iZXJ9XG4gICAgICAgICAqL1xuICAgICAgICBmdW5jdGlvbiBjYWxjSGVpZ2h0KGNvdW50KSB7XG4gICAgICAgICAgICB2YXIgaSA9IDA7XG4gICAgICAgICAgICB3aGlsZSAoYmlnSW50KDIpLnBvdyhpKS5sdChjb3VudCkpIHtcbiAgICAgICAgICAgICAgICBpKys7XG4gICAgICAgICAgICB9XG4gICAgICAgICAgICByZXR1cm4gaTtcbiAgICAgICAgfVxuXG4gICAgICAgIC8qKlxuICAgICAgICAgKiBHZXQgdmFsdWUgZnJvbSBub2RlLCBpbnNlcnQgaW50byBlbGVtZW50cyBhcnJheSBhbmQgcmV0dXJuIGl0cyBoYXNoXG4gICAgICAgICAqIEBwYXJhbSBkYXRhXG4gICAgICAgICAqIEBwYXJhbSB7TnVtYmVyfSBkZXB0aFxuICAgICAgICAgKiBAcGFyYW0ge051bWJlcn0gaW5kZXhcbiAgICAgICAgICogQHJldHVybnMge1N0cmluZ31cbiAgICAgICAgICovXG4gICAgICAgIGZ1bmN0aW9uIGdldEhhc2goZGF0YSwgZGVwdGgsIGluZGV4KSB7XG4gICAgICAgICAgICB2YXIgZWxlbWVudDtcbiAgICAgICAgICAgIHZhciBlbGVtZW50c0hhc2g7XG5cbiAgICAgICAgICAgIGlmIChkZXB0aCAhPT0gMCAmJiAoZGVwdGggKyAxKSAhPT0gaGVpZ2h0KSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignVmFsdWUgbm9kZSBpcyBvbiB3cm9uZyBoZWlnaHQgaW4gdHJlZS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9IGVsc2UgaWYgKHN0YXJ0Lmd0KGluZGV4KSAgfHwgZW5kLmx0KGluZGV4KSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIGluZGV4IG9mIHZhbHVlIG5vZGUuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgfSBlbHNlIGlmIChzdGFydC5wbHVzKGVsZW1lbnRzLmxlbmd0aCkubmVxKGluZGV4KSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1ZhbHVlIG5vZGUgaXMgb24gd3JvbmcgcG9zaXRpb24gaW4gdHJlZS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmICh0eXBlb2YgZGF0YSA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgICAgICBpZiAodmFsaWRhdGVIZXhIYXNoKGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgICAgIGVsZW1lbnQgPSBkYXRhO1xuICAgICAgICAgICAgICAgICAgICBlbGVtZW50c0hhc2ggPSBoYXNoKGhleGFkZWNpbWFsVG9VaW50OEFycmF5KGVsZW1lbnQpKTtcbiAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGhleGFkZWNpbWFsIHN0cmluZyBpcyBwYXNzZWQgYXMgdmFsdWUgaW4gdHJlZS4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgIH0gZWxzZSBpZiAoQXJyYXkuaXNBcnJheShkYXRhKSkge1xuICAgICAgICAgICAgICAgIGlmICh2YWxpZGF0ZUJ5dGVzQXJyYXkoZGF0YSkpIHtcbiAgICAgICAgICAgICAgICAgICAgZWxlbWVudCA9IGRhdGEuc2xpY2UoMCk7IC8vIGNsb25lIGFycmF5IG9mIDgtYml0IGludGVnZXJzXG4gICAgICAgICAgICAgICAgICAgIGVsZW1lbnRzSGFzaCA9IGhhc2goZWxlbWVudCk7XG4gICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBhcnJheSBvZiA4LWJpdCBpbnRlZ2VycyBpbiB0cmVlLicpO1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIGlmIChpc09iamVjdChkYXRhKSkge1xuICAgICAgICAgICAgICAgIGlmIChOZXdUeXBlLnByb3RvdHlwZS5pc1Byb3RvdHlwZU9mKHR5cGUpKSB7XG4gICAgICAgICAgICAgICAgICAgIGVsZW1lbnQgPSBvYmplY3RBc3NpZ24oZGF0YSk7IC8vIGRlZXAgY29weVxuICAgICAgICAgICAgICAgICAgICBlbGVtZW50c0hhc2ggPSBoYXNoKGVsZW1lbnQsIHR5cGUpO1xuICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdHlwZSBvZiB0eXBlIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB2YWx1ZSBvZiBkYXRhIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmICh0eXBlb2YgZWxlbWVudHNIYXNoID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgZWxlbWVudHMucHVzaChlbGVtZW50KTtcbiAgICAgICAgICAgIHJldHVybiBlbGVtZW50c0hhc2g7XG4gICAgICAgIH1cblxuICAgICAgICAvKipcbiAgICAgICAgICogUmVjdXJzaXZlIHRyZWUgdHJhdmVyc2FsIGZ1bmN0aW9uXG4gICAgICAgICAqIEBwYXJhbSB7T2JqZWN0fSBub2RlXG4gICAgICAgICAqIEBwYXJhbSB7TnVtYmVyfSBkZXB0aFxuICAgICAgICAgKiBAcGFyYW0ge051bWJlcn0gaW5kZXhcbiAgICAgICAgICogQHJldHVybnMge1N0cmluZ31cbiAgICAgICAgICovXG4gICAgICAgIGZ1bmN0aW9uIHJlY3Vyc2l2ZShub2RlLCBkZXB0aCwgaW5kZXgpIHtcbiAgICAgICAgICAgIHZhciBoYXNoTGVmdDtcbiAgICAgICAgICAgIHZhciBoYXNoUmlnaHQ7XG4gICAgICAgICAgICB2YXIgc3VtbWluZ0J1ZmZlcjtcblxuICAgICAgICAgICAgLy8gY2FzZSB3aXRoIHNpbmdsZSBub2RlIGluIHRyZWVcbiAgICAgICAgICAgIGlmIChkZXB0aCA9PT0gMCAmJiB0eXBlb2Ygbm9kZS52YWwgIT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGdldEhhc2gobm9kZS52YWwsIGRlcHRoLCBpbmRleCAqIDIpO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICBpZiAodHlwZW9mIG5vZGUubGVmdCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICBpZiAodHlwZW9mIG5vZGUubGVmdCA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgICAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2gobm9kZS5sZWZ0KSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgaGFzaExlZnQgPSBub2RlLmxlZnQ7XG4gICAgICAgICAgICAgICAgfSBlbHNlIGlmIChpc09iamVjdChub2RlLmxlZnQpKSB7XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2Ygbm9kZS5sZWZ0LnZhbCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGhhc2hMZWZ0ID0gZ2V0SGFzaChub2RlLmxlZnQudmFsLCBkZXB0aCwgaW5kZXggKiAyKTtcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGhhc2hMZWZ0ID0gcmVjdXJzaXZlKG5vZGUubGVmdCwgZGVwdGggKyAxLCBpbmRleCAqIDIpO1xuICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgaGFzaExlZnQgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdHlwZSBvZiBsZWZ0IG5vZGUuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignTGVmdCBub2RlIGlzIG1pc3NlZC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgaWYgKGRlcHRoID09PSAwKSB7XG4gICAgICAgICAgICAgICAgcm9vdEJyYW5jaCA9ICdyaWdodCc7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmICh0eXBlb2Ygbm9kZS5yaWdodCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICBpZiAodHlwZW9mIG5vZGUucmlnaHQgPT09ICdzdHJpbmcnKSB7XG4gICAgICAgICAgICAgICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKG5vZGUucmlnaHQpKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgICAgICBoYXNoUmlnaHQgPSBub2RlLnJpZ2h0O1xuICAgICAgICAgICAgICAgIH0gZWxzZSBpZiAoaXNPYmplY3Qobm9kZS5yaWdodCkpIHtcbiAgICAgICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBub2RlLnJpZ2h0LnZhbCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGhhc2hSaWdodCA9IGdldEhhc2gobm9kZS5yaWdodC52YWwsIGRlcHRoLCBpbmRleCAqIDIgKyAxKTtcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGhhc2hSaWdodCA9IHJlY3Vyc2l2ZShub2RlLnJpZ2h0LCBkZXB0aCArIDEsIGluZGV4ICogMiArIDEpO1xuICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgaGFzaFJpZ2h0ID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2YgcmlnaHQgbm9kZS4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgc3VtbWluZ0J1ZmZlciA9IG5ldyBVaW50OEFycmF5KDY0KTtcbiAgICAgICAgICAgICAgICBzdW1taW5nQnVmZmVyLnNldChoZXhhZGVjaW1hbFRvVWludDhBcnJheShoYXNoTGVmdCkpO1xuICAgICAgICAgICAgICAgIHN1bW1pbmdCdWZmZXIuc2V0KGhleGFkZWNpbWFsVG9VaW50OEFycmF5KGhhc2hSaWdodCksIDMyKTtcbiAgICAgICAgICAgIH0gZWxzZSBpZiAoZGVwdGggPT09IDAgfHwgcm9vdEJyYW5jaCA9PT0gJ2xlZnQnKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignUmlnaHQgbGVhZiBpcyBtaXNzZWQgaW4gbGVmdCBicmFuY2ggb2YgdHJlZS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgc3VtbWluZ0J1ZmZlciA9IGhleGFkZWNpbWFsVG9VaW50OEFycmF5KGhhc2hMZWZ0KTtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgcmV0dXJuIGhhc2goc3VtbWluZ0J1ZmZlcik7XG4gICAgICAgIH1cblxuICAgICAgICB2YXIgZWxlbWVudHMgPSBbXTtcbiAgICAgICAgdmFyIHJvb3RCcmFuY2ggPSAnbGVmdCc7XG5cbiAgICAgICAgLy8gdmFsaWRhdGUgcm9vdEhhc2hcbiAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2gocm9vdEhhc2gpKSB7XG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9XG5cbiAgICAgICAgLy8gdmFsaWRhdGUgY291bnRcbiAgICAgICAgaWYgKCEodHlwZW9mIGNvdW50ID09PSAnbnVtYmVyJyB8fCB0eXBlb2YgY291bnQgPT09ICdzdHJpbmcnKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB2YWx1ZSBpcyBwYXNzZWQgYXMgY291bnQgcGFyYW1ldGVyLiBOdW1iZXIgb3Igc3RyaW5nIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICB0cnkge1xuICAgICAgICAgICAgY291bnQgPSBiaWdJbnQoY291bnQpO1xuICAgICAgICB9IGNhdGNoIChlKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIGlzIHBhc3NlZCBhcyBjb3VudCBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9XG4gICAgICAgIGlmIChjb3VudC5sdCgwKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBjb3VudCBwYXJhbWV0ZXIuIENvdW50IGNhblxcJ3QgYmUgYmVsb3cgemVyby4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH1cblxuICAgICAgICAvLyB2YWxpZGF0ZSByYW5nZVxuICAgICAgICBpZiAoIUFycmF5LmlzQXJyYXkocmFuZ2UpIHx8IHJhbmdlLmxlbmd0aCAhPT0gMikge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB0eXBlIG9mIHJhbmdlIHBhcmFtZXRlci4gQXJyYXkgb2YgdHdvIGVsZW1lbnRzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfSBlbHNlIGlmICghKHR5cGVvZiByYW5nZVswXSA9PT0gJ251bWJlcicgfHwgdHlwZW9mIHJhbmdlWzBdID09PSAnc3RyaW5nJykpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdmFsdWUgaXMgcGFzc2VkIGFzIHN0YXJ0IG9mIHJhbmdlIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH0gZWxzZSBpZiAoISh0eXBlb2YgcmFuZ2VbMV0gPT09ICdudW1iZXInIHx8IHR5cGVvZiByYW5nZVsxXSA9PT0gJ3N0cmluZycpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIGlzIHBhc3NlZCBhcyBlbmQgb2YgcmFuZ2UgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICB0cnkge1xuICAgICAgICAgICAgdmFyIHJhbmdlU3RhcnQgPSBiaWdJbnQocmFuZ2VbMF0pO1xuICAgICAgICB9IGNhdGNoIChlKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIGlzIHBhc3NlZCBhcyBzdGFydCBvZiByYW5nZSBwYXJhbWV0ZXIuIE51bWJlciBvciBzdHJpbmcgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9XG4gICAgICAgIHRyeSB7XG4gICAgICAgICAgICB2YXIgcmFuZ2VFbmQgPSBiaWdJbnQocmFuZ2VbMV0pO1xuICAgICAgICB9IGNhdGNoIChlKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIGlzIHBhc3NlZCBhcyBlbmQgb2YgcmFuZ2UgcGFyYW1ldGVyLiBOdW1iZXIgb3Igc3RyaW5nIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICBpZiAocmFuZ2VTdGFydC5ndChyYW5nZUVuZCkpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgcmFuZ2UgcGFyYW1ldGVyLiBTdGFydCBpbmRleCBjYW5cXCd0IGJlIG91dCBvZiByYW5nZS4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH0gZWxzZSBpZiAocmFuZ2VTdGFydC5sdCgwKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCByYW5nZSBwYXJhbWV0ZXIuIFN0YXJ0IGluZGV4IGNhblxcJ3QgYmUgYmVsb3cgemVyby4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH0gZWxzZSBpZiAocmFuZ2VFbmQubHQoMCkpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgcmFuZ2UgcGFyYW1ldGVyLiBFbmQgaW5kZXggY2FuXFwndCBiZSBiZWxvdyB6ZXJvLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfSBlbHNlIGlmIChyYW5nZVN0YXJ0Lmd0KGNvdW50Lm1pbnVzKDEpKSkge1xuICAgICAgICAgICAgcmV0dXJuIFtdO1xuICAgICAgICB9XG5cbiAgICAgICAgLy8gdmFsaWRhdGUgcHJvb2ZOb2RlXG4gICAgICAgIGlmICghaXNPYmplY3QocHJvb2ZOb2RlKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB0eXBlIG9mIHByb29mTm9kZSBwYXJhbWV0ZXIuIE9iamVjdCBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH1cblxuICAgICAgICB2YXIgaGVpZ2h0ID0gY2FsY0hlaWdodChjb3VudCk7XG4gICAgICAgIHZhciBzdGFydCA9IHJhbmdlU3RhcnQ7XG4gICAgICAgIHZhciBlbmQgPSByYW5nZUVuZC5sdChjb3VudCkgPyByYW5nZUVuZCA6IGNvdW50Lm1pbnVzKDEpO1xuICAgICAgICB2YXIgYWN0dWFsSGFzaCA9IHJlY3Vyc2l2ZShwcm9vZk5vZGUsIDAsIDApO1xuXG4gICAgICAgIGlmICh0eXBlb2YgYWN0dWFsSGFzaCA9PT0gJ3VuZGVmaW5lZCcpIHsgLy8gdHJlZSBpcyBpbnZhbGlkXG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9IGVsc2UgaWYgKHJvb3RIYXNoLnRvTG93ZXJDYXNlKCkgIT09IGFjdHVhbEhhc2gpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ3Jvb3RIYXNoIHBhcmFtZXRlciBpcyBub3QgZXF1YWwgdG8gYWN0dWFsIGhhc2guJyk7XG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9IGVsc2UgaWYgKGJpZ0ludChlbGVtZW50cy5sZW5ndGgpLm5lcShlbmQuZXEoc3RhcnQpID8gMSA6IGVuZC5taW51cyhzdGFydCkucGx1cygxKSkpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0FjdHVhbCBlbGVtZW50cyBpbiB0cmVlIGFtb3VudCBpcyBub3QgZXF1YWwgdG8gcmVxdWVzdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBlbGVtZW50cztcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDaGVjayBNZXJrbGUgUGF0cmljaWEgdHJlZSBwcm9vZiBhbmQgcmV0dXJuIGVsZW1lbnRcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gcm9vdEhhc2hcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gcHJvb2ZOb2RlXG4gICAgICogQHBhcmFtIGtleVxuICAgICAqIEBwYXJhbSB7TmV3VHlwZX0gdHlwZSAob3B0aW9uYWwpXG4gICAgICogQHJldHVybiB7T2JqZWN0fVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIG1lcmtsZVBhdHJpY2lhUHJvb2Yocm9vdEhhc2gsIHByb29mTm9kZSwga2V5LCB0eXBlKSB7XG4gICAgICAgIC8qKlxuICAgICAgICAgKiBHZXQgdmFsdWUgZnJvbSBub2RlXG4gICAgICAgICAqIEBwYXJhbSBkYXRhXG4gICAgICAgICAqIEByZXR1cm5zIHtTdHJpbmcgfHwgQXJyYXkgfHwgT2JqZWN0fVxuICAgICAgICAgKi9cbiAgICAgICAgZnVuY3Rpb24gZ2V0SGFzaChkYXRhKSB7XG4gICAgICAgICAgICB2YXIgZWxlbWVudHNIYXNoO1xuXG4gICAgICAgICAgICBpZiAodHlwZW9mIGRhdGEgPT09ICdzdHJpbmcnKSB7XG4gICAgICAgICAgICAgICAgaWYgKHZhbGlkYXRlSGV4SGFzaChkYXRhKSkge1xuICAgICAgICAgICAgICAgICAgICBlbGVtZW50ID0gZGF0YTtcbiAgICAgICAgICAgICAgICAgICAgZWxlbWVudHNIYXNoID0gaGFzaChoZXhhZGVjaW1hbFRvVWludDhBcnJheShlbGVtZW50KSk7XG4gICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBoZXhhZGVjaW1hbCBzdHJpbmcgaXMgcGFzc2VkIGFzIHZhbHVlIGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2UgaWYgKEFycmF5LmlzQXJyYXkoZGF0YSkpIHtcbiAgICAgICAgICAgICAgICBpZiAodmFsaWRhdGVCeXRlc0FycmF5KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgICAgIGVsZW1lbnQgPSBkYXRhLnNsaWNlKDApOyAvLyBjbG9uZSBhcnJheSBvZiA4LWJpdCBpbnRlZ2Vyc1xuICAgICAgICAgICAgICAgICAgICBlbGVtZW50c0hhc2ggPSBoYXNoKGVsZW1lbnQpO1xuICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2UgaWYgKGlzT2JqZWN0KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgZWxlbWVudCA9IG9iamVjdEFzc2lnbihkYXRhKTsgLy8gZGVlcCBjb3B5XG4gICAgICAgICAgICAgICAgZWxlbWVudHNIYXNoID0gaGFzaChlbGVtZW50LCB0eXBlKTtcbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB2YWx1ZSBub2RlIGluIHRyZWUuIE9iamVjdCBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmICh0eXBlb2YgZWxlbWVudHNIYXNoID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgcmV0dXJuIGVsZW1lbnRzSGFzaDtcbiAgICAgICAgfVxuXG4gICAgICAgIC8qKlxuICAgICAgICAgKiBDaGVjayBlaXRoZXIgc3VmZml4IGlzIGEgcGFydCBvZiBzZWFyY2gga2V5XG4gICAgICAgICAqIEBwYXJhbSB7U3RyaW5nfSBwcmVmaXhcbiAgICAgICAgICogQHBhcmFtIHtTdHJpbmd9IHN1ZmZpeFxuICAgICAgICAgKiBAcmV0dXJucyB7Qm9vbGVhbn1cbiAgICAgICAgICovXG4gICAgICAgIGZ1bmN0aW9uIGlzUGFydE9mU2VhcmNoS2V5KHByZWZpeCwgc3VmZml4KSB7XG4gICAgICAgICAgICB2YXIgZGlmZiA9IGtleUJpbmFyeS5zdWJzdHIocHJlZml4Lmxlbmd0aCk7XG4gICAgICAgICAgICByZXR1cm4gZGlmZlswXSA9PT0gc3VmZml4WzBdO1xuICAgICAgICB9XG5cbiAgICAgICAgLyoqXG4gICAgICAgICAqIFJlY3Vyc2l2ZSB0cmVlIHRyYXZlcnNhbCBmdW5jdGlvblxuICAgICAgICAgKiBAcGFyYW0ge09iamVjdH0gbm9kZVxuICAgICAgICAgKiBAcGFyYW0ge1N0cmluZ30ga2V5UHJlZml4XG4gICAgICAgICAqIEByZXR1cm5zIHtTdHJpbmd9XG4gICAgICAgICAqL1xuICAgICAgICBmdW5jdGlvbiByZWN1cnNpdmUobm9kZSwga2V5UHJlZml4KSB7XG4gICAgICAgICAgICBpZiAoZ2V0T2JqZWN0TGVuZ3RoKG5vZGUpICE9PSAyKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBudW1iZXIgb2YgY2hpbGRyZW4gaW4gdGhlIHRyZWUgbm9kZS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgdmFyIGxldmVsRGF0YSA9IHt9O1xuXG4gICAgICAgICAgICBmb3IgKHZhciBrZXlTdWZmaXggaW4gbm9kZSkge1xuICAgICAgICAgICAgICAgIGlmICghbm9kZS5oYXNPd25Qcm9wZXJ0eShrZXlTdWZmaXgpKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnRpbnVlO1xuICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgIC8vIHZhbGlkYXRlIGtleVxuICAgICAgICAgICAgICAgIGlmICghdmFsaWRhdGVCaW5hcnlTdHJpbmcoa2V5U3VmZml4KSkge1xuICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICB2YXIgYnJhbmNoVmFsdWVIYXNoO1xuICAgICAgICAgICAgICAgIHZhciBmdWxsS2V5ID0ga2V5UHJlZml4ICsga2V5U3VmZml4O1xuICAgICAgICAgICAgICAgIHZhciBub2RlVmFsdWUgPSBub2RlW2tleVN1ZmZpeF07XG4gICAgICAgICAgICAgICAgdmFyIGJyYW5jaFR5cGU7XG4gICAgICAgICAgICAgICAgdmFyIGJyYW5jaEtleTtcbiAgICAgICAgICAgICAgICB2YXIgYnJhbmNoS2V5SGFzaDtcblxuICAgICAgICAgICAgICAgIGlmIChmdWxsS2V5Lmxlbmd0aCA9PT0gQ09OU1QuTUVSS0xFX1BBVFJJQ0lBX0tFWV9MRU5HVEggKiA4KSB7XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2Ygbm9kZVZhbHVlID09PSAnc3RyaW5nJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2gobm9kZVZhbHVlKSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgICAgICAgICAgYnJhbmNoVmFsdWVIYXNoID0gbm9kZVZhbHVlO1xuICAgICAgICAgICAgICAgICAgICAgICAgYnJhbmNoVHlwZSA9ICdoYXNoJztcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIGlmIChpc09iamVjdChub2RlVmFsdWUpKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBpZiAodHlwZW9mIG5vZGVWYWx1ZS52YWwgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignTGVhZiB0cmVlIGNvbnRhaW5zIGludmFsaWQgZGF0YS4nKTtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgICAgIH0gZWxzZSBpZiAodHlwZW9mIGVsZW1lbnQgIT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignVHJlZSBjYW4gbm90IGNvbnRhaW5zIG1vcmUgdGhhbiBvbmUgbm9kZSB3aXRoIHZhbHVlLicpO1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgICAgICAgICBicmFuY2hWYWx1ZUhhc2ggPSBnZXRIYXNoKG5vZGVWYWx1ZS52YWwpO1xuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBicmFuY2hWYWx1ZUhhc2ggPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgICAgICAgICBicmFuY2hUeXBlID0gJ3ZhbHVlJztcbiAgICAgICAgICAgICAgICAgICAgfSAgZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2Ygbm9kZSBpbiB0cmVlIGxlYWYuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgICAgIGJyYW5jaEtleUhhc2ggPSBiaW5hcnlTdHJpbmdUb0hleGFkZWNpbWFsKGZ1bGxLZXkpO1xuICAgICAgICAgICAgICAgICAgICBicmFuY2hLZXkgPSB7XG4gICAgICAgICAgICAgICAgICAgICAgICB2YXJpYW50OiAxLFxuICAgICAgICAgICAgICAgICAgICAgICAga2V5OiBicmFuY2hLZXlIYXNoLFxuICAgICAgICAgICAgICAgICAgICAgICAgbGVuZ3RoOiAwXG4gICAgICAgICAgICAgICAgICAgIH07XG4gICAgICAgICAgICAgICAgfSBlbHNlIGlmIChmdWxsS2V5Lmxlbmd0aCA8IENPTlNULk1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIICogOCkgeyAvLyBub2RlIGlzIGJyYW5jaFxuICAgICAgICAgICAgICAgICAgICBpZiAodHlwZW9mIG5vZGVWYWx1ZSA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKG5vZGVWYWx1ZSkpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgICAgIGJyYW5jaFZhbHVlSGFzaCA9IG5vZGVWYWx1ZTtcbiAgICAgICAgICAgICAgICAgICAgICAgIGJyYW5jaFR5cGUgPSAnaGFzaCc7XG4gICAgICAgICAgICAgICAgICAgIH0gZWxzZSBpZiAoaXNPYmplY3Qobm9kZVZhbHVlKSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBub2RlVmFsdWUudmFsICE9PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ05vZGUgd2l0aCB2YWx1ZSBpcyBhdCBub24tbGVhZiBwb3NpdGlvbiBpbiB0cmVlLicpO1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgICAgICAgICBicmFuY2hWYWx1ZUhhc2ggPSByZWN1cnNpdmUobm9kZVZhbHVlLCBmdWxsS2V5KTtcbiAgICAgICAgICAgICAgICAgICAgICAgIGlmIChicmFuY2hWYWx1ZUhhc2ggPT09IG51bGwpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgICAgIGJyYW5jaFR5cGUgPSAnYnJhbmNoJztcbiAgICAgICAgICAgICAgICAgICAgfSAgZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2Ygbm9kZSBpbiB0cmVlLicpO1xuICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgICAgICB2YXIgYmluYXJ5S2V5TGVuZ3RoID0gZnVsbEtleS5sZW5ndGg7XG4gICAgICAgICAgICAgICAgICAgIHZhciBiaW5hcnlLZXkgPSBmdWxsS2V5O1xuXG4gICAgICAgICAgICAgICAgICAgIGZvciAodmFyIGogPSAwOyBqIDwgKENPTlNULk1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIICogOCAtIGZ1bGxLZXkubGVuZ3RoKTsgaisrKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBiaW5hcnlLZXkgKz0gJzAnO1xuICAgICAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICAgICAgYnJhbmNoS2V5SGFzaCA9IGJpbmFyeVN0cmluZ1RvSGV4YWRlY2ltYWwoYmluYXJ5S2V5KTtcbiAgICAgICAgICAgICAgICAgICAgYnJhbmNoS2V5ID0ge1xuICAgICAgICAgICAgICAgICAgICAgICAgdmFyaWFudDogMCxcbiAgICAgICAgICAgICAgICAgICAgICAgIGtleTogYnJhbmNoS2V5SGFzaCxcbiAgICAgICAgICAgICAgICAgICAgICAgIGxlbmd0aDogYmluYXJ5S2V5TGVuZ3RoXG4gICAgICAgICAgICAgICAgICAgIH07XG4gICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBsZW5ndGggb2Yga2V5IGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgIGlmIChrZXlTdWZmaXhbMF0gPT09ICcwJykgeyAvLyAnMCcgYXQgdGhlIGJlZ2lubmluZyBtZWFucyBsZWZ0IGJyYW5jaC9sZWFmXG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgbGV2ZWxEYXRhLmxlZnQgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBsZXZlbERhdGEubGVmdCA9IHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBoYXNoOiBicmFuY2hWYWx1ZUhhc2gsXG4gICAgICAgICAgICAgICAgICAgICAgICAgICAga2V5OiBicmFuY2hLZXksXG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgdHlwZTogYnJhbmNoVHlwZSxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBzdWZmaXg6IGtleVN1ZmZpeCxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBzaXplOiBmdWxsS2V5Lmxlbmd0aFxuICAgICAgICAgICAgICAgICAgICAgICAgfTtcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0xlZnQgbm9kZSBpcyBkdXBsaWNhdGVkIGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgIH0gZWxzZSB7IC8vICcxJyBhdCB0aGUgYmVnaW5uaW5nIG1lYW5zIHJpZ2h0IGJyYW5jaC9sZWFmXG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgbGV2ZWxEYXRhLnJpZ2h0ID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgbGV2ZWxEYXRhLnJpZ2h0ID0ge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIGhhc2g6IGJyYW5jaFZhbHVlSGFzaCxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBrZXk6IGJyYW5jaEtleSxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICB0eXBlOiBicmFuY2hUeXBlLFxuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHN1ZmZpeDoga2V5U3VmZml4LFxuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHNpemU6IGZ1bGxLZXkubGVuZ3RoXG4gICAgICAgICAgICAgICAgICAgICAgICB9O1xuICAgICAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignUmlnaHQgbm9kZSBpcyBkdXBsaWNhdGVkIGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgaWYgKChsZXZlbERhdGEubGVmdC50eXBlID09PSAnaGFzaCcpICYmIChsZXZlbERhdGEucmlnaHQudHlwZSA9PT0gJ2hhc2gnKSAmJiAoZnVsbEtleS5sZW5ndGggPCBDT05TVC5NRVJLTEVfUEFUUklDSUFfS0VZX0xFTkdUSCAqIDgpKSB7XG4gICAgICAgICAgICAgICAgaWYgKGlzUGFydE9mU2VhcmNoS2V5KGtleVByZWZpeCwgbGV2ZWxEYXRhLmxlZnQuc3VmZml4KSkge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdUcmVlIGlzIGludmFsaWQuIExlZnQga2V5IGlzIGEgcGFydCBvZiBzZWFyY2gga2V5IGJ1dCBpdHMgYnJhbmNoIGlzIG5vdCBleHBhbmRlZC4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgfSBlbHNlIGlmIChpc1BhcnRPZlNlYXJjaEtleShrZXlQcmVmaXgsIGxldmVsRGF0YS5yaWdodC5zdWZmaXgpKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1RyZWUgaXMgaW52YWxpZC4gUmlnaHQga2V5IGlzIGEgcGFydCBvZiBzZWFyY2gga2V5IGJ1dCBpdHMgYnJhbmNoIGlzIG5vdCBleHBhbmRlZC4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICByZXR1cm4gaGFzaCh7XG4gICAgICAgICAgICAgICAgbGVmdF9oYXNoOiBsZXZlbERhdGEubGVmdC5oYXNoLFxuICAgICAgICAgICAgICAgIHJpZ2h0X2hhc2g6IGxldmVsRGF0YS5yaWdodC5oYXNoLFxuICAgICAgICAgICAgICAgIGxlZnRfa2V5OiBsZXZlbERhdGEubGVmdC5rZXksXG4gICAgICAgICAgICAgICAgcmlnaHRfa2V5OiBsZXZlbERhdGEucmlnaHQua2V5XG4gICAgICAgICAgICB9LCBCcmFuY2gpO1xuICAgICAgICB9XG5cbiAgICAgICAgdmFyIGVsZW1lbnQ7XG5cbiAgICAgICAgLy8gdmFsaWRhdGUgcm9vdEhhc2ggcGFyYW1ldGVyXG4gICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKHJvb3RIYXNoKSkge1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICB2YXIgcm9vdEhhc2ggPSByb290SGFzaC50b0xvd2VyQ2FzZSgpO1xuXG4gICAgICAgIC8vIHZhbGlkYXRlIHByb29mTm9kZSBwYXJhbWV0ZXJcbiAgICAgICAgaWYgKCFpc09iamVjdChwcm9vZk5vZGUpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2YgcHJvb2ZOb2RlIHBhcmFtZXRlci4gT2JqZWN0IGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuXG4gICAgICAgIC8vIHZhbGlkYXRlIGtleSBwYXJhbWV0ZXJcbiAgICAgICAgdmFyIGtleSA9IGtleTtcbiAgICAgICAgaWYgKEFycmF5LmlzQXJyYXkoa2V5KSkge1xuICAgICAgICAgICAgaWYgKHZhbGlkYXRlQnl0ZXNBcnJheShrZXksIENPTlNULk1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIKSkge1xuICAgICAgICAgICAgICAgIGtleSA9IHVpbnQ4QXJyYXlUb0hleGFkZWNpbWFsKGtleSk7XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgICAgICB9XG4gICAgICAgIH0gZWxzZSBpZiAodHlwZW9mIGtleSA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKGtleSwgQ09OU1QuTUVSS0xFX1BBVFJJQ0lBX0tFWV9MRU5HVEgpKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdHlwZSBvZiBrZXkgcGFyYW1ldGVyLiBBYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMgb3IgaGV4YWRlY2ltYWwgc3RyaW5nIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICB2YXIga2V5QmluYXJ5ID0gaGV4YWRlY2ltYWxUb0JpbmFyeVN0cmluZyhrZXkpO1xuXG4gICAgICAgIHZhciBwcm9vZk5vZGVSb290TnVtYmVyT2ZOb2RlcyA9IGdldE9iamVjdExlbmd0aChwcm9vZk5vZGUpO1xuICAgICAgICBpZiAocHJvb2ZOb2RlUm9vdE51bWJlck9mTm9kZXMgPT09IDApIHtcbiAgICAgICAgICAgIGlmIChyb290SGFzaCA9PT0gKG5ldyBVaW50OEFycmF5KENPTlNULk1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIICogMikpLmpvaW4oJycpKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgcm9vdEhhc2ggcGFyYW1ldGVyIG9mIGVtcHR5IHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIGlmIChwcm9vZk5vZGVSb290TnVtYmVyT2ZOb2RlcyA9PT0gMSkge1xuICAgICAgICAgICAgZm9yICh2YXIgaSBpbiBwcm9vZk5vZGUpIHtcbiAgICAgICAgICAgICAgICBpZiAoIXByb29mTm9kZS5oYXNPd25Qcm9wZXJ0eShpKSkge1xuICAgICAgICAgICAgICAgICAgICBjb250aW51ZTtcbiAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICBpZiAoIXZhbGlkYXRlQmluYXJ5U3RyaW5nKGksIDI1NikpIHtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICB2YXIgZGF0YSA9IHByb29mTm9kZVtpXTtcbiAgICAgICAgICAgICAgICB2YXIgbm9kZUtleUJ1ZmZlciA9IGJpbmFyeVN0cmluZ1RvVWludDhBcnJheShpKTtcbiAgICAgICAgICAgICAgICB2YXIgbm9kZUtleSA9IHVpbnQ4QXJyYXlUb0hleGFkZWNpbWFsKG5vZGVLZXlCdWZmZXIpO1xuICAgICAgICAgICAgICAgIHZhciBub2RlSGFzaDtcblxuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgZGF0YSA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgICAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2goZGF0YSkpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgICAgICBub2RlSGFzaCA9IGhhc2goe1xuICAgICAgICAgICAgICAgICAgICAgICAga2V5OiB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgdmFyaWFudDogMSxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBrZXk6IG5vZGVLZXksXG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgbGVuZ3RoOiAwXG4gICAgICAgICAgICAgICAgICAgICAgICB9LFxuICAgICAgICAgICAgICAgICAgICAgICAgaGFzaDogZGF0YVxuICAgICAgICAgICAgICAgICAgICB9LCBSb290QnJhbmNoKTtcblxuICAgICAgICAgICAgICAgICAgICBpZiAocm9vdEhhc2ggPT09IG5vZGVIYXNoKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBpZiAoa2V5ICE9PSBub2RlS2V5KSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7IC8vIG5vIGVsZW1lbnQgd2l0aCBkYXRhIGluIHRyZWVcbiAgICAgICAgICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBrZXkgd2l0aCBoYXNoIGlzIGluIHRoZSByb290IG9mIHByb29mTm9kZSBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ3Jvb3RIYXNoIHBhcmFtZXRlciBpcyBub3QgZXF1YWwgdG8gYWN0dWFsIGhhc2guJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgfSBlbHNlIGlmIChpc09iamVjdChkYXRhKSkge1xuICAgICAgICAgICAgICAgICAgICB2YXIgZWxlbWVudHNIYXNoID0gZ2V0SGFzaChkYXRhLnZhbCk7XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgZWxlbWVudHNIYXNoID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgICAgIG5vZGVIYXNoID0gaGFzaCh7XG4gICAgICAgICAgICAgICAgICAgICAgICBrZXk6IHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICB2YXJpYW50OiAxLFxuICAgICAgICAgICAgICAgICAgICAgICAgICAgIGtleTogbm9kZUtleSxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBsZW5ndGg6IDBcbiAgICAgICAgICAgICAgICAgICAgICAgIH0sXG4gICAgICAgICAgICAgICAgICAgICAgICBoYXNoOiBlbGVtZW50c0hhc2hcbiAgICAgICAgICAgICAgICAgICAgfSwgUm9vdEJyYW5jaCk7XG5cbiAgICAgICAgICAgICAgICAgICAgaWYgKHJvb3RIYXNoID09PSBub2RlSGFzaCkge1xuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKGtleSA9PT0gbm9kZUtleSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiBlbGVtZW50O1xuICAgICAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGtleSB3aXRoIHZhbHVlIGlzIGluIHRoZSByb290IG9mIHByb29mTm9kZSBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZFxuICAgICAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcigncm9vdEhhc2ggcGFyYW1ldGVyIGlzIG5vdCBlcXVhbCB0byBhY3R1YWwgaGFzaC4nKTtcbiAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2YgdmFsdWUgaW4gdGhlIHJvb3Qgb2YgcHJvb2ZOb2RlIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgIHZhciBhY3R1YWxIYXNoID0gcmVjdXJzaXZlKHByb29mTm9kZSwgJycpO1xuXG4gICAgICAgICAgICBpZiAoYWN0dWFsSGFzaCA9PT0gbnVsbCkgeyAvLyB0cmVlIGlzIGludmFsaWRcbiAgICAgICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICAgICAgfSBlbHNlIGlmIChyb290SGFzaCAhPT0gYWN0dWFsSGFzaCkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ3Jvb3RIYXNoIHBhcmFtZXRlciBpcyBub3QgZXF1YWwgdG8gYWN0dWFsIGhhc2guJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgIH0gZWxzZSBpZiAodHlwZW9mIGVsZW1lbnQgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7IC8vIG5vIGVsZW1lbnQgd2l0aCBkYXRhIGluIHRyZWVcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgcmV0dXJuIGVsZW1lbnQ7XG4gICAgICAgIH1cbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBWZXJpZmllcyBibG9ja1xuICAgICAqIEBwYXJhbSB7T2JqZWN0fSBkYXRhXG4gICAgICogQHBhcmFtIHtBcnJheX0gdmFsaWRhdG9yc1xuICAgICAqIEByZXR1cm4ge0Jvb2xlYW59XG4gICAgICovXG4gICAgZnVuY3Rpb24gdmVyaWZ5QmxvY2soZGF0YSwgdmFsaWRhdG9ycykge1xuICAgICAgICBpZiAoIWlzT2JqZWN0KGRhdGEpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyB0eXBlIG9mIGRhdGEgcGFyYW1ldGVyLiBPYmplY3QgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH0gZWxzZSBpZiAoIWlzT2JqZWN0KGRhdGEuYmxvY2spKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyB0eXBlIG9mIGJsb2NrIGZpZWxkIGluIGRhdGEgcGFyYW1ldGVyLiBPYmplY3QgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH0gZWxzZSBpZiAoIUFycmF5LmlzQXJyYXkoZGF0YS5wcmVjb21taXRzKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgdHlwZSBvZiBwcmVjb21taXRzIGZpZWxkIGluIGRhdGEgcGFyYW1ldGVyLiBBcnJheSBpcyBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfSBlbHNlIGlmICghQXJyYXkuaXNBcnJheSh2YWxpZGF0b3JzKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgdHlwZSBvZiB2YWxpZGF0b3JzcGFyYW1ldGVyLiBBcnJheSBpcyBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfVxuXG4gICAgICAgIGZvciAodmFyIGkgaW4gdmFsaWRhdG9ycykge1xuICAgICAgICAgICAgaWYgKCF2YWxpZGF0b3JzLmhhc093blByb3BlcnR5KGkpKSB7XG4gICAgICAgICAgICAgICAgY29udGludWU7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKHZhbGlkYXRvcnNbaV0pKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfVxuICAgICAgICB9XG5cbiAgICAgICAgdmFyIHZhbGlkYXRvcnNUb3RhbE51bWJlciA9IHZhbGlkYXRvcnMubGVuZ3RoO1xuICAgICAgICB2YXIgdW5pcXVlVmFsaWRhdG9ycyA9IFtdO1xuXG4gICAgICAgIHZhciByb3VuZDtcbiAgICAgICAgdmFyIGJsb2NrSGFzaCA9IGhhc2goZGF0YS5ibG9jaywgQmxvY2spO1xuXG4gICAgICAgIGZvciAodmFyIGkgaW4gZGF0YS5wcmVjb21taXRzKSB7XG4gICAgICAgICAgICBpZiAoIWRhdGEucHJlY29tbWl0cy5oYXNPd25Qcm9wZXJ0eShpKSkge1xuICAgICAgICAgICAgICAgIGNvbnRpbnVlO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICB2YXIgcHJlY29tbWl0ID0gZGF0YS5wcmVjb21taXRzW2ldO1xuXG4gICAgICAgICAgICBpZiAoIWlzT2JqZWN0KHByZWNvbW1pdC5ib2R5KSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIHR5cGUgb2YgcHJlY29tbWl0cyBib2R5LiBPYmplY3QgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfSBlbHNlIGlmICghdmFsaWRhdGVIZXhIYXNoKHByZWNvbW1pdC5zaWduYXR1cmUsIDY0KSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIHR5cGUgb2YgcHJlY29tbWl0cyBzaWduYXR1cmUuIEhleGFkZWNpbWFsIG9mIDY0IGxlbmd0aCBpcyBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmIChwcmVjb21taXQuYm9keS52YWxpZGF0b3IgPj0gdmFsaWRhdG9yc1RvdGFsTnVtYmVyKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgaW5kZXggcGFzc2VkLiBWYWxpZGF0b3IgZG9lcyBub3QgZXhpc3QuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICBpZiAodW5pcXVlVmFsaWRhdG9ycy5pbmRleE9mKHByZWNvbW1pdC5ib2R5LnZhbGlkYXRvcikgPT09IC0xKSB7XG4gICAgICAgICAgICAgICAgdW5pcXVlVmFsaWRhdG9ycy5wdXNoKHByZWNvbW1pdC5ib2R5LnZhbGlkYXRvcik7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmIChwcmVjb21taXQuYm9keS5oZWlnaHQgIT09IGRhdGEuYmxvY2suaGVpZ2h0KSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgaGVpZ2h0IG9mIGJsb2NrIGluIHByZWNvbW1pdC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9IGVsc2UgaWYgKHByZWNvbW1pdC5ib2R5LmJsb2NrX2hhc2ggIT09IGJsb2NrSGFzaCkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIGhhc2ggb2YgYmxvY2sgaW4gcHJlY29tbWl0LicpO1xuICAgICAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgaWYgKHR5cGVvZiByb3VuZCA9PT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICByb3VuZCA9IHByZWNvbW1pdC5ib2R5LnJvdW5kO1xuICAgICAgICAgICAgfSBlbHNlIGlmIChwcmVjb21taXQuYm9keS5yb3VuZCAhPT0gcm91bmQpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyByb3VuZCBpbiBwcmVjb21taXQuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICB2YXIgcHVibGljS2V5ID0gdmFsaWRhdG9yc1twcmVjb21taXQuYm9keS52YWxpZGF0b3JdO1xuXG4gICAgICAgICAgICBpZiAoIXZlcmlmeVNpZ25hdHVyZShwcmVjb21taXQuYm9keSwgUHJlY29tbWl0LCBwcmVjb21taXQuc2lnbmF0dXJlLCBwdWJsaWNLZXkpKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3Jvbmcgc2lnbmF0dXJlIG9mIHByZWNvbW1pdC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9XG4gICAgICAgIH1cblxuICAgICAgICBpZiAodW5pcXVlVmFsaWRhdG9ycy5sZW5ndGggPD0gdmFsaWRhdG9yc1RvdGFsTnVtYmVyICogMiAvIDMpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ05vdCBlbm91Z2ggcHJlY29tbWl0cyBmcm9tIHVuaXF1ZSB2YWxpZGF0b3JzLicpO1xuICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIHRydWU7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogR2VuZXJhdGUgcmFuZG9tIHBhaXIgb2YgcHVibGljS2V5IGFuZCBzZWNyZXRLZXlcbiAgICAgKiBAcmV0dXJuIHtPYmplY3R9XG4gICAgICogIHB1YmxpY0tleSB7U3RyaW5nfVxuICAgICAqICBzZWNyZXRLZXkge1N0cmluZ31cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBrZXlQYWlyKCkge1xuICAgICAgICB2YXIgcGFpciA9IG5hY2wuc2lnbi5rZXlQYWlyKCk7XG4gICAgICAgIHZhciBwdWJsaWNLZXkgPSB1aW50OEFycmF5VG9IZXhhZGVjaW1hbChwYWlyLnB1YmxpY0tleSk7XG4gICAgICAgIHZhciBzZWNyZXRLZXkgPSB1aW50OEFycmF5VG9IZXhhZGVjaW1hbChwYWlyLnNlY3JldEtleSk7XG5cbiAgICAgICAgcmV0dXJuIHtcbiAgICAgICAgICAgIHB1YmxpY0tleTogcHVibGljS2V5LFxuICAgICAgICAgICAgc2VjcmV0S2V5OiBzZWNyZXRLZXlcbiAgICAgICAgfVxuICAgIH1cblxuICAgIC8qKiBHZW5lcmF0ZSByYW5kb20gbnVtYmVyIG9mIHR5cGUgb2YgVWludDY0XG4gICAgICogQHJldHVybiB7U3RyaW5nfVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIHJhbmRvbVVpbnQ2NCgpIHtcbiAgICAgICAgcmV0dXJuIGJpZ0ludC5yYW5kQmV0d2VlbigwLCBDT05TVC5NQVhfVUlOVDY0KTtcbiAgICB9XG5cbiAgICByZXR1cm4ge1xuXG4gICAgICAgIC8vIEFQSSBtZXRob2RzXG4gICAgICAgIG5ld1R5cGU6IGNyZWF0ZU5ld1R5cGUsXG4gICAgICAgIG5ld01lc3NhZ2U6IGNyZWF0ZU5ld01lc3NhZ2UsXG4gICAgICAgIGhhc2g6IGhhc2gsXG4gICAgICAgIHNpZ246IHNpZ24sXG4gICAgICAgIHZlcmlmeVNpZ25hdHVyZTogdmVyaWZ5U2lnbmF0dXJlLFxuICAgICAgICBtZXJrbGVQcm9vZjogbWVya2xlUHJvb2YsXG4gICAgICAgIG1lcmtsZVBhdHJpY2lhUHJvb2Y6IG1lcmtsZVBhdHJpY2lhUHJvb2YsXG4gICAgICAgIHZlcmlmeUJsb2NrOiB2ZXJpZnlCbG9jayxcbiAgICAgICAga2V5UGFpcjoga2V5UGFpcixcbiAgICAgICAgcmFuZG9tVWludDY0OiByYW5kb21VaW50NjQsXG5cbiAgICAgICAgLy8gYnVpbHQtaW4gdHlwZXNcbiAgICAgICAgSW50ODogSW50OCxcbiAgICAgICAgSW50MTY6IEludDE2LFxuICAgICAgICBJbnQzMjogSW50MzIsXG4gICAgICAgIEludDY0OiBJbnQ2NCxcbiAgICAgICAgVWludDg6IFVpbnQ4LFxuICAgICAgICBVaW50MTY6IFVpbnQxNixcbiAgICAgICAgVWludDMyOiBVaW50MzIsXG4gICAgICAgIFVpbnQ2NDogVWludDY0LFxuICAgICAgICBTdHJpbmc6IFN0cmluZyxcbiAgICAgICAgSGFzaDogSGFzaCxcbiAgICAgICAgRGlnZXN0OiBEaWdlc3QsXG4gICAgICAgIFB1YmxpY0tleTogUHVibGljS2V5LFxuICAgICAgICBUaW1lc3BlYzogVGltZXNwZWMsXG4gICAgICAgIEJvb2w6IEJvb2xcblxuICAgIH07XG5cbn0pKCk7XG5cbm1vZHVsZS5leHBvcnRzID0gRXhvbnVtQ2xpZW50O1xuIl19
