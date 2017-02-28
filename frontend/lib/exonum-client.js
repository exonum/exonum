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
            message_type: {type: Uint16, size: 2, from: 2, to: 4},
            service_id: {type: Uint16, size: 2, from: 4, to: 6},
            payload: {type: Uint32, size: 4, from: 6, to: 10}
        }
    });
    var Precommit = createNewMessage({
        size: 84,
        service_id: 0,
        message_type: 4,
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
        this.message_type = type.message_type;
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
            message_type: this.message_type,
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

            if ((depth + 1) !== height) {
                console.error('Value node is on wrong height in tree.');
                return;
            } else if (index < start || end.lt(index)) {
                console.error('Wrong index of value node.');
                return;
            } else if ((start + elements.length) !== index) {
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
//# sourceMappingURL=data:application/json;charset=utf-8;base64,eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbIm5vZGVfbW9kdWxlcy9icm93c2VyLXBhY2svX3ByZWx1ZGUuanMiLCJub2RlX21vZHVsZXMvYmFzZTY0LWpzL2luZGV4LmpzIiwibm9kZV9tb2R1bGVzL2JpZy1pbnRlZ2VyL0JpZ0ludGVnZXIuanMiLCJub2RlX21vZHVsZXMvYnJvd3Nlci1yZXNvbHZlL2VtcHR5LmpzIiwibm9kZV9tb2R1bGVzL2J1ZmZlci9pbmRleC5qcyIsIm5vZGVfbW9kdWxlcy9pZWVlNzU0L2luZGV4LmpzIiwibm9kZV9tb2R1bGVzL2luaGVyaXRzL2luaGVyaXRzX2Jyb3dzZXIuanMiLCJub2RlX21vZHVsZXMvaXNhcnJheS9pbmRleC5qcyIsIm5vZGVfbW9kdWxlcy9vYmplY3QtYXNzaWduL2luZGV4LmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9oYXNoLmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9pbmRleC5qcyIsIm5vZGVfbW9kdWxlcy9zaGEuanMvc2hhLmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9zaGExLmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9zaGEyMjQuanMiLCJub2RlX21vZHVsZXMvc2hhLmpzL3NoYTI1Ni5qcyIsIm5vZGVfbW9kdWxlcy9zaGEuanMvc2hhMzg0LmpzIiwibm9kZV9tb2R1bGVzL3NoYS5qcy9zaGE1MTIuanMiLCJub2RlX21vZHVsZXMvdHdlZXRuYWNsL25hY2wtZmFzdC5qcyIsInNyYy9icm93c2VyLmpzIiwic3JjL2luZGV4LmpzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBO0FDQUE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDbEhBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUM5ckNBOzs7OztBQ0FBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7Ozs7QUM3dkRBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOztBQ3BGQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7O0FDdkJBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNMQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7O0FDMUZBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7O0FDckVBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7QUNmQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7Ozs7QUM3RkE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7OztBQ2xHQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBOzs7OztBQ3BEQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7Ozs7O0FDdElBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7Ozs7QUN4REE7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7OztBQ25RQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTs7QUNwMUVBO0FBQ0E7QUFDQTtBQUNBOztBQ0hBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBO0FBQ0E7QUFDQTtBQUNBIiwiZmlsZSI6ImdlbmVyYXRlZC5qcyIsInNvdXJjZVJvb3QiOiIiLCJzb3VyY2VzQ29udGVudCI6WyIoZnVuY3Rpb24gZSh0LG4scil7ZnVuY3Rpb24gcyhvLHUpe2lmKCFuW29dKXtpZighdFtvXSl7dmFyIGE9dHlwZW9mIHJlcXVpcmU9PVwiZnVuY3Rpb25cIiYmcmVxdWlyZTtpZighdSYmYSlyZXR1cm4gYShvLCEwKTtpZihpKXJldHVybiBpKG8sITApO3ZhciBmPW5ldyBFcnJvcihcIkNhbm5vdCBmaW5kIG1vZHVsZSAnXCIrbytcIidcIik7dGhyb3cgZi5jb2RlPVwiTU9EVUxFX05PVF9GT1VORFwiLGZ9dmFyIGw9bltvXT17ZXhwb3J0czp7fX07dFtvXVswXS5jYWxsKGwuZXhwb3J0cyxmdW5jdGlvbihlKXt2YXIgbj10W29dWzFdW2VdO3JldHVybiBzKG4/bjplKX0sbCxsLmV4cG9ydHMsZSx0LG4scil9cmV0dXJuIG5bb10uZXhwb3J0c312YXIgaT10eXBlb2YgcmVxdWlyZT09XCJmdW5jdGlvblwiJiZyZXF1aXJlO2Zvcih2YXIgbz0wO288ci5sZW5ndGg7bysrKXMocltvXSk7cmV0dXJuIHN9KSIsIid1c2Ugc3RyaWN0J1xuXG5leHBvcnRzLmJ5dGVMZW5ndGggPSBieXRlTGVuZ3RoXG5leHBvcnRzLnRvQnl0ZUFycmF5ID0gdG9CeXRlQXJyYXlcbmV4cG9ydHMuZnJvbUJ5dGVBcnJheSA9IGZyb21CeXRlQXJyYXlcblxudmFyIGxvb2t1cCA9IFtdXG52YXIgcmV2TG9va3VwID0gW11cbnZhciBBcnIgPSB0eXBlb2YgVWludDhBcnJheSAhPT0gJ3VuZGVmaW5lZCcgPyBVaW50OEFycmF5IDogQXJyYXlcblxudmFyIGNvZGUgPSAnQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVphYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ejAxMjM0NTY3ODkrLydcbmZvciAodmFyIGkgPSAwLCBsZW4gPSBjb2RlLmxlbmd0aDsgaSA8IGxlbjsgKytpKSB7XG4gIGxvb2t1cFtpXSA9IGNvZGVbaV1cbiAgcmV2TG9va3VwW2NvZGUuY2hhckNvZGVBdChpKV0gPSBpXG59XG5cbnJldkxvb2t1cFsnLScuY2hhckNvZGVBdCgwKV0gPSA2MlxucmV2TG9va3VwWydfJy5jaGFyQ29kZUF0KDApXSA9IDYzXG5cbmZ1bmN0aW9uIHBsYWNlSG9sZGVyc0NvdW50IChiNjQpIHtcbiAgdmFyIGxlbiA9IGI2NC5sZW5ndGhcbiAgaWYgKGxlbiAlIDQgPiAwKSB7XG4gICAgdGhyb3cgbmV3IEVycm9yKCdJbnZhbGlkIHN0cmluZy4gTGVuZ3RoIG11c3QgYmUgYSBtdWx0aXBsZSBvZiA0JylcbiAgfVxuXG4gIC8vIHRoZSBudW1iZXIgb2YgZXF1YWwgc2lnbnMgKHBsYWNlIGhvbGRlcnMpXG4gIC8vIGlmIHRoZXJlIGFyZSB0d28gcGxhY2Vob2xkZXJzLCB0aGFuIHRoZSB0d28gY2hhcmFjdGVycyBiZWZvcmUgaXRcbiAgLy8gcmVwcmVzZW50IG9uZSBieXRlXG4gIC8vIGlmIHRoZXJlIGlzIG9ubHkgb25lLCB0aGVuIHRoZSB0aHJlZSBjaGFyYWN0ZXJzIGJlZm9yZSBpdCByZXByZXNlbnQgMiBieXRlc1xuICAvLyB0aGlzIGlzIGp1c3QgYSBjaGVhcCBoYWNrIHRvIG5vdCBkbyBpbmRleE9mIHR3aWNlXG4gIHJldHVybiBiNjRbbGVuIC0gMl0gPT09ICc9JyA/IDIgOiBiNjRbbGVuIC0gMV0gPT09ICc9JyA/IDEgOiAwXG59XG5cbmZ1bmN0aW9uIGJ5dGVMZW5ndGggKGI2NCkge1xuICAvLyBiYXNlNjQgaXMgNC8zICsgdXAgdG8gdHdvIGNoYXJhY3RlcnMgb2YgdGhlIG9yaWdpbmFsIGRhdGFcbiAgcmV0dXJuIGI2NC5sZW5ndGggKiAzIC8gNCAtIHBsYWNlSG9sZGVyc0NvdW50KGI2NClcbn1cblxuZnVuY3Rpb24gdG9CeXRlQXJyYXkgKGI2NCkge1xuICB2YXIgaSwgaiwgbCwgdG1wLCBwbGFjZUhvbGRlcnMsIGFyclxuICB2YXIgbGVuID0gYjY0Lmxlbmd0aFxuICBwbGFjZUhvbGRlcnMgPSBwbGFjZUhvbGRlcnNDb3VudChiNjQpXG5cbiAgYXJyID0gbmV3IEFycihsZW4gKiAzIC8gNCAtIHBsYWNlSG9sZGVycylcblxuICAvLyBpZiB0aGVyZSBhcmUgcGxhY2Vob2xkZXJzLCBvbmx5IGdldCB1cCB0byB0aGUgbGFzdCBjb21wbGV0ZSA0IGNoYXJzXG4gIGwgPSBwbGFjZUhvbGRlcnMgPiAwID8gbGVuIC0gNCA6IGxlblxuXG4gIHZhciBMID0gMFxuXG4gIGZvciAoaSA9IDAsIGogPSAwOyBpIDwgbDsgaSArPSA0LCBqICs9IDMpIHtcbiAgICB0bXAgPSAocmV2TG9va3VwW2I2NC5jaGFyQ29kZUF0KGkpXSA8PCAxOCkgfCAocmV2TG9va3VwW2I2NC5jaGFyQ29kZUF0KGkgKyAxKV0gPDwgMTIpIHwgKHJldkxvb2t1cFtiNjQuY2hhckNvZGVBdChpICsgMildIDw8IDYpIHwgcmV2TG9va3VwW2I2NC5jaGFyQ29kZUF0KGkgKyAzKV1cbiAgICBhcnJbTCsrXSA9ICh0bXAgPj4gMTYpICYgMHhGRlxuICAgIGFycltMKytdID0gKHRtcCA+PiA4KSAmIDB4RkZcbiAgICBhcnJbTCsrXSA9IHRtcCAmIDB4RkZcbiAgfVxuXG4gIGlmIChwbGFjZUhvbGRlcnMgPT09IDIpIHtcbiAgICB0bXAgPSAocmV2TG9va3VwW2I2NC5jaGFyQ29kZUF0KGkpXSA8PCAyKSB8IChyZXZMb29rdXBbYjY0LmNoYXJDb2RlQXQoaSArIDEpXSA+PiA0KVxuICAgIGFycltMKytdID0gdG1wICYgMHhGRlxuICB9IGVsc2UgaWYgKHBsYWNlSG9sZGVycyA9PT0gMSkge1xuICAgIHRtcCA9IChyZXZMb29rdXBbYjY0LmNoYXJDb2RlQXQoaSldIDw8IDEwKSB8IChyZXZMb29rdXBbYjY0LmNoYXJDb2RlQXQoaSArIDEpXSA8PCA0KSB8IChyZXZMb29rdXBbYjY0LmNoYXJDb2RlQXQoaSArIDIpXSA+PiAyKVxuICAgIGFycltMKytdID0gKHRtcCA+PiA4KSAmIDB4RkZcbiAgICBhcnJbTCsrXSA9IHRtcCAmIDB4RkZcbiAgfVxuXG4gIHJldHVybiBhcnJcbn1cblxuZnVuY3Rpb24gdHJpcGxldFRvQmFzZTY0IChudW0pIHtcbiAgcmV0dXJuIGxvb2t1cFtudW0gPj4gMTggJiAweDNGXSArIGxvb2t1cFtudW0gPj4gMTIgJiAweDNGXSArIGxvb2t1cFtudW0gPj4gNiAmIDB4M0ZdICsgbG9va3VwW251bSAmIDB4M0ZdXG59XG5cbmZ1bmN0aW9uIGVuY29kZUNodW5rICh1aW50OCwgc3RhcnQsIGVuZCkge1xuICB2YXIgdG1wXG4gIHZhciBvdXRwdXQgPSBbXVxuICBmb3IgKHZhciBpID0gc3RhcnQ7IGkgPCBlbmQ7IGkgKz0gMykge1xuICAgIHRtcCA9ICh1aW50OFtpXSA8PCAxNikgKyAodWludDhbaSArIDFdIDw8IDgpICsgKHVpbnQ4W2kgKyAyXSlcbiAgICBvdXRwdXQucHVzaCh0cmlwbGV0VG9CYXNlNjQodG1wKSlcbiAgfVxuICByZXR1cm4gb3V0cHV0LmpvaW4oJycpXG59XG5cbmZ1bmN0aW9uIGZyb21CeXRlQXJyYXkgKHVpbnQ4KSB7XG4gIHZhciB0bXBcbiAgdmFyIGxlbiA9IHVpbnQ4Lmxlbmd0aFxuICB2YXIgZXh0cmFCeXRlcyA9IGxlbiAlIDMgLy8gaWYgd2UgaGF2ZSAxIGJ5dGUgbGVmdCwgcGFkIDIgYnl0ZXNcbiAgdmFyIG91dHB1dCA9ICcnXG4gIHZhciBwYXJ0cyA9IFtdXG4gIHZhciBtYXhDaHVua0xlbmd0aCA9IDE2MzgzIC8vIG11c3QgYmUgbXVsdGlwbGUgb2YgM1xuXG4gIC8vIGdvIHRocm91Z2ggdGhlIGFycmF5IGV2ZXJ5IHRocmVlIGJ5dGVzLCB3ZSdsbCBkZWFsIHdpdGggdHJhaWxpbmcgc3R1ZmYgbGF0ZXJcbiAgZm9yICh2YXIgaSA9IDAsIGxlbjIgPSBsZW4gLSBleHRyYUJ5dGVzOyBpIDwgbGVuMjsgaSArPSBtYXhDaHVua0xlbmd0aCkge1xuICAgIHBhcnRzLnB1c2goZW5jb2RlQ2h1bmsodWludDgsIGksIChpICsgbWF4Q2h1bmtMZW5ndGgpID4gbGVuMiA/IGxlbjIgOiAoaSArIG1heENodW5rTGVuZ3RoKSkpXG4gIH1cblxuICAvLyBwYWQgdGhlIGVuZCB3aXRoIHplcm9zLCBidXQgbWFrZSBzdXJlIHRvIG5vdCBmb3JnZXQgdGhlIGV4dHJhIGJ5dGVzXG4gIGlmIChleHRyYUJ5dGVzID09PSAxKSB7XG4gICAgdG1wID0gdWludDhbbGVuIC0gMV1cbiAgICBvdXRwdXQgKz0gbG9va3VwW3RtcCA+PiAyXVxuICAgIG91dHB1dCArPSBsb29rdXBbKHRtcCA8PCA0KSAmIDB4M0ZdXG4gICAgb3V0cHV0ICs9ICc9PSdcbiAgfSBlbHNlIGlmIChleHRyYUJ5dGVzID09PSAyKSB7XG4gICAgdG1wID0gKHVpbnQ4W2xlbiAtIDJdIDw8IDgpICsgKHVpbnQ4W2xlbiAtIDFdKVxuICAgIG91dHB1dCArPSBsb29rdXBbdG1wID4+IDEwXVxuICAgIG91dHB1dCArPSBsb29rdXBbKHRtcCA+PiA0KSAmIDB4M0ZdXG4gICAgb3V0cHV0ICs9IGxvb2t1cFsodG1wIDw8IDIpICYgMHgzRl1cbiAgICBvdXRwdXQgKz0gJz0nXG4gIH1cblxuICBwYXJ0cy5wdXNoKG91dHB1dClcblxuICByZXR1cm4gcGFydHMuam9pbignJylcbn1cbiIsInZhciBiaWdJbnQgPSAoZnVuY3Rpb24gKHVuZGVmaW5lZCkge1xyXG4gICAgXCJ1c2Ugc3RyaWN0XCI7XHJcblxyXG4gICAgdmFyIEJBU0UgPSAxZTcsXHJcbiAgICAgICAgTE9HX0JBU0UgPSA3LFxyXG4gICAgICAgIE1BWF9JTlQgPSA5MDA3MTk5MjU0NzQwOTkyLFxyXG4gICAgICAgIE1BWF9JTlRfQVJSID0gc21hbGxUb0FycmF5KE1BWF9JTlQpLFxyXG4gICAgICAgIExPR19NQVhfSU5UID0gTWF0aC5sb2coTUFYX0lOVCk7XHJcblxyXG4gICAgZnVuY3Rpb24gSW50ZWdlcih2LCByYWRpeCkge1xyXG4gICAgICAgIGlmICh0eXBlb2YgdiA9PT0gXCJ1bmRlZmluZWRcIikgcmV0dXJuIEludGVnZXJbMF07XHJcbiAgICAgICAgaWYgKHR5cGVvZiByYWRpeCAhPT0gXCJ1bmRlZmluZWRcIikgcmV0dXJuICtyYWRpeCA9PT0gMTAgPyBwYXJzZVZhbHVlKHYpIDogcGFyc2VCYXNlKHYsIHJhZGl4KTtcclxuICAgICAgICByZXR1cm4gcGFyc2VWYWx1ZSh2KTtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBCaWdJbnRlZ2VyKHZhbHVlLCBzaWduKSB7XHJcbiAgICAgICAgdGhpcy52YWx1ZSA9IHZhbHVlO1xyXG4gICAgICAgIHRoaXMuc2lnbiA9IHNpZ247XHJcbiAgICAgICAgdGhpcy5pc1NtYWxsID0gZmFsc2U7XHJcbiAgICB9XHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZSA9IE9iamVjdC5jcmVhdGUoSW50ZWdlci5wcm90b3R5cGUpO1xyXG5cclxuICAgIGZ1bmN0aW9uIFNtYWxsSW50ZWdlcih2YWx1ZSkge1xyXG4gICAgICAgIHRoaXMudmFsdWUgPSB2YWx1ZTtcclxuICAgICAgICB0aGlzLnNpZ24gPSB2YWx1ZSA8IDA7XHJcbiAgICAgICAgdGhpcy5pc1NtYWxsID0gdHJ1ZTtcclxuICAgIH1cclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUgPSBPYmplY3QuY3JlYXRlKEludGVnZXIucHJvdG90eXBlKTtcclxuXHJcbiAgICBmdW5jdGlvbiBpc1ByZWNpc2Uobikge1xyXG4gICAgICAgIHJldHVybiAtTUFYX0lOVCA8IG4gJiYgbiA8IE1BWF9JTlQ7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gc21hbGxUb0FycmF5KG4pIHsgLy8gRm9yIHBlcmZvcm1hbmNlIHJlYXNvbnMgZG9lc24ndCByZWZlcmVuY2UgQkFTRSwgbmVlZCB0byBjaGFuZ2UgdGhpcyBmdW5jdGlvbiBpZiBCQVNFIGNoYW5nZXNcclxuICAgICAgICBpZiAobiA8IDFlNylcclxuICAgICAgICAgICAgcmV0dXJuIFtuXTtcclxuICAgICAgICBpZiAobiA8IDFlMTQpXHJcbiAgICAgICAgICAgIHJldHVybiBbbiAlIDFlNywgTWF0aC5mbG9vcihuIC8gMWU3KV07XHJcbiAgICAgICAgcmV0dXJuIFtuICUgMWU3LCBNYXRoLmZsb29yKG4gLyAxZTcpICUgMWU3LCBNYXRoLmZsb29yKG4gLyAxZTE0KV07XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gYXJyYXlUb1NtYWxsKGFycikgeyAvLyBJZiBCQVNFIGNoYW5nZXMgdGhpcyBmdW5jdGlvbiBtYXkgbmVlZCB0byBjaGFuZ2VcclxuICAgICAgICB0cmltKGFycik7XHJcbiAgICAgICAgdmFyIGxlbmd0aCA9IGFyci5sZW5ndGg7XHJcbiAgICAgICAgaWYgKGxlbmd0aCA8IDQgJiYgY29tcGFyZUFicyhhcnIsIE1BWF9JTlRfQVJSKSA8IDApIHtcclxuICAgICAgICAgICAgc3dpdGNoIChsZW5ndGgpIHtcclxuICAgICAgICAgICAgICAgIGNhc2UgMDogcmV0dXJuIDA7XHJcbiAgICAgICAgICAgICAgICBjYXNlIDE6IHJldHVybiBhcnJbMF07XHJcbiAgICAgICAgICAgICAgICBjYXNlIDI6IHJldHVybiBhcnJbMF0gKyBhcnJbMV0gKiBCQVNFO1xyXG4gICAgICAgICAgICAgICAgZGVmYXVsdDogcmV0dXJuIGFyclswXSArIChhcnJbMV0gKyBhcnJbMl0gKiBCQVNFKSAqIEJBU0U7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIGFycjtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiB0cmltKHYpIHtcclxuICAgICAgICB2YXIgaSA9IHYubGVuZ3RoO1xyXG4gICAgICAgIHdoaWxlICh2Wy0taV0gPT09IDApO1xyXG4gICAgICAgIHYubGVuZ3RoID0gaSArIDE7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gY3JlYXRlQXJyYXkobGVuZ3RoKSB7IC8vIGZ1bmN0aW9uIHNoYW1lbGVzc2x5IHN0b2xlbiBmcm9tIFlhZmZsZSdzIGxpYnJhcnkgaHR0cHM6Ly9naXRodWIuY29tL1lhZmZsZS9CaWdJbnRlZ2VyXHJcbiAgICAgICAgdmFyIHggPSBuZXcgQXJyYXkobGVuZ3RoKTtcclxuICAgICAgICB2YXIgaSA9IC0xO1xyXG4gICAgICAgIHdoaWxlICgrK2kgPCBsZW5ndGgpIHtcclxuICAgICAgICAgICAgeFtpXSA9IDA7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiB4O1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIHRydW5jYXRlKG4pIHtcclxuICAgICAgICBpZiAobiA+IDApIHJldHVybiBNYXRoLmZsb29yKG4pO1xyXG4gICAgICAgIHJldHVybiBNYXRoLmNlaWwobik7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gYWRkKGEsIGIpIHsgLy8gYXNzdW1lcyBhIGFuZCBiIGFyZSBhcnJheXMgd2l0aCBhLmxlbmd0aCA+PSBiLmxlbmd0aFxyXG4gICAgICAgIHZhciBsX2EgPSBhLmxlbmd0aCxcclxuICAgICAgICAgICAgbF9iID0gYi5sZW5ndGgsXHJcbiAgICAgICAgICAgIHIgPSBuZXcgQXJyYXkobF9hKSxcclxuICAgICAgICAgICAgY2FycnkgPSAwLFxyXG4gICAgICAgICAgICBiYXNlID0gQkFTRSxcclxuICAgICAgICAgICAgc3VtLCBpO1xyXG4gICAgICAgIGZvciAoaSA9IDA7IGkgPCBsX2I7IGkrKykge1xyXG4gICAgICAgICAgICBzdW0gPSBhW2ldICsgYltpXSArIGNhcnJ5O1xyXG4gICAgICAgICAgICBjYXJyeSA9IHN1bSA+PSBiYXNlID8gMSA6IDA7XHJcbiAgICAgICAgICAgIHJbaV0gPSBzdW0gLSBjYXJyeSAqIGJhc2U7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHdoaWxlIChpIDwgbF9hKSB7XHJcbiAgICAgICAgICAgIHN1bSA9IGFbaV0gKyBjYXJyeTtcclxuICAgICAgICAgICAgY2FycnkgPSBzdW0gPT09IGJhc2UgPyAxIDogMDtcclxuICAgICAgICAgICAgcltpKytdID0gc3VtIC0gY2FycnkgKiBiYXNlO1xyXG4gICAgICAgIH1cclxuICAgICAgICBpZiAoY2FycnkgPiAwKSByLnB1c2goY2FycnkpO1xyXG4gICAgICAgIHJldHVybiByO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIGFkZEFueShhLCBiKSB7XHJcbiAgICAgICAgaWYgKGEubGVuZ3RoID49IGIubGVuZ3RoKSByZXR1cm4gYWRkKGEsIGIpO1xyXG4gICAgICAgIHJldHVybiBhZGQoYiwgYSk7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gYWRkU21hbGwoYSwgY2FycnkpIHsgLy8gYXNzdW1lcyBhIGlzIGFycmF5LCBjYXJyeSBpcyBudW1iZXIgd2l0aCAwIDw9IGNhcnJ5IDwgTUFYX0lOVFxyXG4gICAgICAgIHZhciBsID0gYS5sZW5ndGgsXHJcbiAgICAgICAgICAgIHIgPSBuZXcgQXJyYXkobCksXHJcbiAgICAgICAgICAgIGJhc2UgPSBCQVNFLFxyXG4gICAgICAgICAgICBzdW0sIGk7XHJcbiAgICAgICAgZm9yIChpID0gMDsgaSA8IGw7IGkrKykge1xyXG4gICAgICAgICAgICBzdW0gPSBhW2ldIC0gYmFzZSArIGNhcnJ5O1xyXG4gICAgICAgICAgICBjYXJyeSA9IE1hdGguZmxvb3Ioc3VtIC8gYmFzZSk7XHJcbiAgICAgICAgICAgIHJbaV0gPSBzdW0gLSBjYXJyeSAqIGJhc2U7XHJcbiAgICAgICAgICAgIGNhcnJ5ICs9IDE7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHdoaWxlIChjYXJyeSA+IDApIHtcclxuICAgICAgICAgICAgcltpKytdID0gY2FycnkgJSBiYXNlO1xyXG4gICAgICAgICAgICBjYXJyeSA9IE1hdGguZmxvb3IoY2FycnkgLyBiYXNlKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHI7XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuYWRkID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICB2YXIgdmFsdWUsIG4gPSBwYXJzZVZhbHVlKHYpO1xyXG4gICAgICAgIGlmICh0aGlzLnNpZ24gIT09IG4uc2lnbikge1xyXG4gICAgICAgICAgICByZXR1cm4gdGhpcy5zdWJ0cmFjdChuLm5lZ2F0ZSgpKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgdmFyIGEgPSB0aGlzLnZhbHVlLCBiID0gbi52YWx1ZTtcclxuICAgICAgICBpZiAobi5pc1NtYWxsKSB7XHJcbiAgICAgICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihhZGRTbWFsbChhLCBNYXRoLmFicyhiKSksIHRoaXMuc2lnbik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihhZGRBbnkoYSwgYiksIHRoaXMuc2lnbik7XHJcbiAgICB9O1xyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUucGx1cyA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmFkZDtcclxuXHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmFkZCA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgdmFyIG4gPSBwYXJzZVZhbHVlKHYpO1xyXG4gICAgICAgIHZhciBhID0gdGhpcy52YWx1ZTtcclxuICAgICAgICBpZiAoYSA8IDAgIT09IG4uc2lnbikge1xyXG4gICAgICAgICAgICByZXR1cm4gdGhpcy5zdWJ0cmFjdChuLm5lZ2F0ZSgpKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgdmFyIGIgPSBuLnZhbHVlO1xyXG4gICAgICAgIGlmIChuLmlzU21hbGwpIHtcclxuICAgICAgICAgICAgaWYgKGlzUHJlY2lzZShhICsgYikpIHJldHVybiBuZXcgU21hbGxJbnRlZ2VyKGEgKyBiKTtcclxuICAgICAgICAgICAgYiA9IHNtYWxsVG9BcnJheShNYXRoLmFicyhiKSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihhZGRTbWFsbChiLCBNYXRoLmFicyhhKSksIGEgPCAwKTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnBsdXMgPSBTbWFsbEludGVnZXIucHJvdG90eXBlLmFkZDtcclxuXHJcbiAgICBmdW5jdGlvbiBzdWJ0cmFjdChhLCBiKSB7IC8vIGFzc3VtZXMgYSBhbmQgYiBhcmUgYXJyYXlzIHdpdGggYSA+PSBiXHJcbiAgICAgICAgdmFyIGFfbCA9IGEubGVuZ3RoLFxyXG4gICAgICAgICAgICBiX2wgPSBiLmxlbmd0aCxcclxuICAgICAgICAgICAgciA9IG5ldyBBcnJheShhX2wpLFxyXG4gICAgICAgICAgICBib3Jyb3cgPSAwLFxyXG4gICAgICAgICAgICBiYXNlID0gQkFTRSxcclxuICAgICAgICAgICAgaSwgZGlmZmVyZW5jZTtcclxuICAgICAgICBmb3IgKGkgPSAwOyBpIDwgYl9sOyBpKyspIHtcclxuICAgICAgICAgICAgZGlmZmVyZW5jZSA9IGFbaV0gLSBib3Jyb3cgLSBiW2ldO1xyXG4gICAgICAgICAgICBpZiAoZGlmZmVyZW5jZSA8IDApIHtcclxuICAgICAgICAgICAgICAgIGRpZmZlcmVuY2UgKz0gYmFzZTtcclxuICAgICAgICAgICAgICAgIGJvcnJvdyA9IDE7XHJcbiAgICAgICAgICAgIH0gZWxzZSBib3Jyb3cgPSAwO1xyXG4gICAgICAgICAgICByW2ldID0gZGlmZmVyZW5jZTtcclxuICAgICAgICB9XHJcbiAgICAgICAgZm9yIChpID0gYl9sOyBpIDwgYV9sOyBpKyspIHtcclxuICAgICAgICAgICAgZGlmZmVyZW5jZSA9IGFbaV0gLSBib3Jyb3c7XHJcbiAgICAgICAgICAgIGlmIChkaWZmZXJlbmNlIDwgMCkgZGlmZmVyZW5jZSArPSBiYXNlO1xyXG4gICAgICAgICAgICBlbHNlIHtcclxuICAgICAgICAgICAgICAgIHJbaSsrXSA9IGRpZmZlcmVuY2U7XHJcbiAgICAgICAgICAgICAgICBicmVhaztcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICByW2ldID0gZGlmZmVyZW5jZTtcclxuICAgICAgICB9XHJcbiAgICAgICAgZm9yICg7IGkgPCBhX2w7IGkrKykge1xyXG4gICAgICAgICAgICByW2ldID0gYVtpXTtcclxuICAgICAgICB9XHJcbiAgICAgICAgdHJpbShyKTtcclxuICAgICAgICByZXR1cm4gcjtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBzdWJ0cmFjdEFueShhLCBiLCBzaWduKSB7XHJcbiAgICAgICAgdmFyIHZhbHVlLCBpc1NtYWxsO1xyXG4gICAgICAgIGlmIChjb21wYXJlQWJzKGEsIGIpID49IDApIHtcclxuICAgICAgICAgICAgdmFsdWUgPSBzdWJ0cmFjdChhLGIpO1xyXG4gICAgICAgIH0gZWxzZSB7XHJcbiAgICAgICAgICAgIHZhbHVlID0gc3VidHJhY3QoYiwgYSk7XHJcbiAgICAgICAgICAgIHNpZ24gPSAhc2lnbjtcclxuICAgICAgICB9XHJcbiAgICAgICAgdmFsdWUgPSBhcnJheVRvU21hbGwodmFsdWUpO1xyXG4gICAgICAgIGlmICh0eXBlb2YgdmFsdWUgPT09IFwibnVtYmVyXCIpIHtcclxuICAgICAgICAgICAgaWYgKHNpZ24pIHZhbHVlID0gLXZhbHVlO1xyXG4gICAgICAgICAgICByZXR1cm4gbmV3IFNtYWxsSW50ZWdlcih2YWx1ZSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcih2YWx1ZSwgc2lnbik7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gc3VidHJhY3RTbWFsbChhLCBiLCBzaWduKSB7IC8vIGFzc3VtZXMgYSBpcyBhcnJheSwgYiBpcyBudW1iZXIgd2l0aCAwIDw9IGIgPCBNQVhfSU5UXHJcbiAgICAgICAgdmFyIGwgPSBhLmxlbmd0aCxcclxuICAgICAgICAgICAgciA9IG5ldyBBcnJheShsKSxcclxuICAgICAgICAgICAgY2FycnkgPSAtYixcclxuICAgICAgICAgICAgYmFzZSA9IEJBU0UsXHJcbiAgICAgICAgICAgIGksIGRpZmZlcmVuY2U7XHJcbiAgICAgICAgZm9yIChpID0gMDsgaSA8IGw7IGkrKykge1xyXG4gICAgICAgICAgICBkaWZmZXJlbmNlID0gYVtpXSArIGNhcnJ5O1xyXG4gICAgICAgICAgICBjYXJyeSA9IE1hdGguZmxvb3IoZGlmZmVyZW5jZSAvIGJhc2UpO1xyXG4gICAgICAgICAgICBkaWZmZXJlbmNlICU9IGJhc2U7XHJcbiAgICAgICAgICAgIHJbaV0gPSBkaWZmZXJlbmNlIDwgMCA/IGRpZmZlcmVuY2UgKyBiYXNlIDogZGlmZmVyZW5jZTtcclxuICAgICAgICB9XHJcbiAgICAgICAgciA9IGFycmF5VG9TbWFsbChyKTtcclxuICAgICAgICBpZiAodHlwZW9mIHIgPT09IFwibnVtYmVyXCIpIHtcclxuICAgICAgICAgICAgaWYgKHNpZ24pIHIgPSAtcjtcclxuICAgICAgICAgICAgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIocik7XHJcbiAgICAgICAgfSByZXR1cm4gbmV3IEJpZ0ludGVnZXIociwgc2lnbik7XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuc3VidHJhY3QgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHZhciBuID0gcGFyc2VWYWx1ZSh2KTtcclxuICAgICAgICBpZiAodGhpcy5zaWduICE9PSBuLnNpZ24pIHtcclxuICAgICAgICAgICAgcmV0dXJuIHRoaXMuYWRkKG4ubmVnYXRlKCkpO1xyXG4gICAgICAgIH1cclxuICAgICAgICB2YXIgYSA9IHRoaXMudmFsdWUsIGIgPSBuLnZhbHVlO1xyXG4gICAgICAgIGlmIChuLmlzU21hbGwpXHJcbiAgICAgICAgICAgIHJldHVybiBzdWJ0cmFjdFNtYWxsKGEsIE1hdGguYWJzKGIpLCB0aGlzLnNpZ24pO1xyXG4gICAgICAgIHJldHVybiBzdWJ0cmFjdEFueShhLCBiLCB0aGlzLnNpZ24pO1xyXG4gICAgfTtcclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLm1pbnVzID0gQmlnSW50ZWdlci5wcm90b3R5cGUuc3VidHJhY3Q7XHJcblxyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5zdWJ0cmFjdCA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgdmFyIG4gPSBwYXJzZVZhbHVlKHYpO1xyXG4gICAgICAgIHZhciBhID0gdGhpcy52YWx1ZTtcclxuICAgICAgICBpZiAoYSA8IDAgIT09IG4uc2lnbikge1xyXG4gICAgICAgICAgICByZXR1cm4gdGhpcy5hZGQobi5uZWdhdGUoKSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHZhciBiID0gbi52YWx1ZTtcclxuICAgICAgICBpZiAobi5pc1NtYWxsKSB7XHJcbiAgICAgICAgICAgIHJldHVybiBuZXcgU21hbGxJbnRlZ2VyKGEgLSBiKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHN1YnRyYWN0U21hbGwoYiwgTWF0aC5hYnMoYSksIGEgPj0gMCk7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5taW51cyA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUuc3VidHJhY3Q7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUubmVnYXRlID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcih0aGlzLnZhbHVlLCAhdGhpcy5zaWduKTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLm5lZ2F0ZSA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICB2YXIgc2lnbiA9IHRoaXMuc2lnbjtcclxuICAgICAgICB2YXIgc21hbGwgPSBuZXcgU21hbGxJbnRlZ2VyKC10aGlzLnZhbHVlKTtcclxuICAgICAgICBzbWFsbC5zaWduID0gIXNpZ247XHJcbiAgICAgICAgcmV0dXJuIHNtYWxsO1xyXG4gICAgfTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5hYnMgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKHRoaXMudmFsdWUsIGZhbHNlKTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmFicyA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gbmV3IFNtYWxsSW50ZWdlcihNYXRoLmFicyh0aGlzLnZhbHVlKSk7XHJcbiAgICB9O1xyXG5cclxuICAgIGZ1bmN0aW9uIG11bHRpcGx5TG9uZyhhLCBiKSB7XHJcbiAgICAgICAgdmFyIGFfbCA9IGEubGVuZ3RoLFxyXG4gICAgICAgICAgICBiX2wgPSBiLmxlbmd0aCxcclxuICAgICAgICAgICAgbCA9IGFfbCArIGJfbCxcclxuICAgICAgICAgICAgciA9IGNyZWF0ZUFycmF5KGwpLFxyXG4gICAgICAgICAgICBiYXNlID0gQkFTRSxcclxuICAgICAgICAgICAgcHJvZHVjdCwgY2FycnksIGksIGFfaSwgYl9qO1xyXG4gICAgICAgIGZvciAoaSA9IDA7IGkgPCBhX2w7ICsraSkge1xyXG4gICAgICAgICAgICBhX2kgPSBhW2ldO1xyXG4gICAgICAgICAgICBmb3IgKHZhciBqID0gMDsgaiA8IGJfbDsgKytqKSB7XHJcbiAgICAgICAgICAgICAgICBiX2ogPSBiW2pdO1xyXG4gICAgICAgICAgICAgICAgcHJvZHVjdCA9IGFfaSAqIGJfaiArIHJbaSArIGpdO1xyXG4gICAgICAgICAgICAgICAgY2FycnkgPSBNYXRoLmZsb29yKHByb2R1Y3QgLyBiYXNlKTtcclxuICAgICAgICAgICAgICAgIHJbaSArIGpdID0gcHJvZHVjdCAtIGNhcnJ5ICogYmFzZTtcclxuICAgICAgICAgICAgICAgIHJbaSArIGogKyAxXSArPSBjYXJyeTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgIH1cclxuICAgICAgICB0cmltKHIpO1xyXG4gICAgICAgIHJldHVybiByO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIG11bHRpcGx5U21hbGwoYSwgYikgeyAvLyBhc3N1bWVzIGEgaXMgYXJyYXksIGIgaXMgbnVtYmVyIHdpdGggfGJ8IDwgQkFTRVxyXG4gICAgICAgIHZhciBsID0gYS5sZW5ndGgsXHJcbiAgICAgICAgICAgIHIgPSBuZXcgQXJyYXkobCksXHJcbiAgICAgICAgICAgIGJhc2UgPSBCQVNFLFxyXG4gICAgICAgICAgICBjYXJyeSA9IDAsXHJcbiAgICAgICAgICAgIHByb2R1Y3QsIGk7XHJcbiAgICAgICAgZm9yIChpID0gMDsgaSA8IGw7IGkrKykge1xyXG4gICAgICAgICAgICBwcm9kdWN0ID0gYVtpXSAqIGIgKyBjYXJyeTtcclxuICAgICAgICAgICAgY2FycnkgPSBNYXRoLmZsb29yKHByb2R1Y3QgLyBiYXNlKTtcclxuICAgICAgICAgICAgcltpXSA9IHByb2R1Y3QgLSBjYXJyeSAqIGJhc2U7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHdoaWxlIChjYXJyeSA+IDApIHtcclxuICAgICAgICAgICAgcltpKytdID0gY2FycnkgJSBiYXNlO1xyXG4gICAgICAgICAgICBjYXJyeSA9IE1hdGguZmxvb3IoY2FycnkgLyBiYXNlKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHI7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gc2hpZnRMZWZ0KHgsIG4pIHtcclxuICAgICAgICB2YXIgciA9IFtdO1xyXG4gICAgICAgIHdoaWxlIChuLS0gPiAwKSByLnB1c2goMCk7XHJcbiAgICAgICAgcmV0dXJuIHIuY29uY2F0KHgpO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIG11bHRpcGx5S2FyYXRzdWJhKHgsIHkpIHtcclxuICAgICAgICB2YXIgbiA9IE1hdGgubWF4KHgubGVuZ3RoLCB5Lmxlbmd0aCk7XHJcblxyXG4gICAgICAgIGlmIChuIDw9IDMwKSByZXR1cm4gbXVsdGlwbHlMb25nKHgsIHkpO1xyXG4gICAgICAgIG4gPSBNYXRoLmNlaWwobiAvIDIpO1xyXG5cclxuICAgICAgICB2YXIgYiA9IHguc2xpY2UobiksXHJcbiAgICAgICAgICAgIGEgPSB4LnNsaWNlKDAsIG4pLFxyXG4gICAgICAgICAgICBkID0geS5zbGljZShuKSxcclxuICAgICAgICAgICAgYyA9IHkuc2xpY2UoMCwgbik7XHJcblxyXG4gICAgICAgIHZhciBhYyA9IG11bHRpcGx5S2FyYXRzdWJhKGEsIGMpLFxyXG4gICAgICAgICAgICBiZCA9IG11bHRpcGx5S2FyYXRzdWJhKGIsIGQpLFxyXG4gICAgICAgICAgICBhYmNkID0gbXVsdGlwbHlLYXJhdHN1YmEoYWRkQW55KGEsIGIpLCBhZGRBbnkoYywgZCkpO1xyXG5cclxuICAgICAgICB2YXIgcHJvZHVjdCA9IGFkZEFueShhZGRBbnkoYWMsIHNoaWZ0TGVmdChzdWJ0cmFjdChzdWJ0cmFjdChhYmNkLCBhYyksIGJkKSwgbikpLCBzaGlmdExlZnQoYmQsIDIgKiBuKSk7XHJcbiAgICAgICAgdHJpbShwcm9kdWN0KTtcclxuICAgICAgICByZXR1cm4gcHJvZHVjdDtcclxuICAgIH1cclxuXHJcbiAgICAvLyBUaGUgZm9sbG93aW5nIGZ1bmN0aW9uIGlzIGRlcml2ZWQgZnJvbSBhIHN1cmZhY2UgZml0IG9mIGEgZ3JhcGggcGxvdHRpbmcgdGhlIHBlcmZvcm1hbmNlIGRpZmZlcmVuY2VcclxuICAgIC8vIGJldHdlZW4gbG9uZyBtdWx0aXBsaWNhdGlvbiBhbmQga2FyYXRzdWJhIG11bHRpcGxpY2F0aW9uIHZlcnN1cyB0aGUgbGVuZ3RocyBvZiB0aGUgdHdvIGFycmF5cy5cclxuICAgIGZ1bmN0aW9uIHVzZUthcmF0c3ViYShsMSwgbDIpIHtcclxuICAgICAgICByZXR1cm4gLTAuMDEyICogbDEgLSAwLjAxMiAqIGwyICsgMC4wMDAwMTUgKiBsMSAqIGwyID4gMDtcclxuICAgIH1cclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5tdWx0aXBseSA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgdmFyIHZhbHVlLCBuID0gcGFyc2VWYWx1ZSh2KSxcclxuICAgICAgICAgICAgYSA9IHRoaXMudmFsdWUsIGIgPSBuLnZhbHVlLFxyXG4gICAgICAgICAgICBzaWduID0gdGhpcy5zaWduICE9PSBuLnNpZ24sXHJcbiAgICAgICAgICAgIGFicztcclxuICAgICAgICBpZiAobi5pc1NtYWxsKSB7XHJcbiAgICAgICAgICAgIGlmIChiID09PSAwKSByZXR1cm4gSW50ZWdlclswXTtcclxuICAgICAgICAgICAgaWYgKGIgPT09IDEpIHJldHVybiB0aGlzO1xyXG4gICAgICAgICAgICBpZiAoYiA9PT0gLTEpIHJldHVybiB0aGlzLm5lZ2F0ZSgpO1xyXG4gICAgICAgICAgICBhYnMgPSBNYXRoLmFicyhiKTtcclxuICAgICAgICAgICAgaWYgKGFicyA8IEJBU0UpIHtcclxuICAgICAgICAgICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihtdWx0aXBseVNtYWxsKGEsIGFicyksIHNpZ24pO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIGIgPSBzbWFsbFRvQXJyYXkoYWJzKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKHVzZUthcmF0c3ViYShhLmxlbmd0aCwgYi5sZW5ndGgpKSAvLyBLYXJhdHN1YmEgaXMgb25seSBmYXN0ZXIgZm9yIGNlcnRhaW4gYXJyYXkgc2l6ZXNcclxuICAgICAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKG11bHRpcGx5S2FyYXRzdWJhKGEsIGIpLCBzaWduKTtcclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIobXVsdGlwbHlMb25nKGEsIGIpLCBzaWduKTtcclxuICAgIH07XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUudGltZXMgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5tdWx0aXBseTtcclxuXHJcbiAgICBmdW5jdGlvbiBtdWx0aXBseVNtYWxsQW5kQXJyYXkoYSwgYiwgc2lnbikgeyAvLyBhID49IDBcclxuICAgICAgICBpZiAoYSA8IEJBU0UpIHtcclxuICAgICAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKG11bHRpcGx5U21hbGwoYiwgYSksIHNpZ24pO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIobXVsdGlwbHlMb25nKGIsIHNtYWxsVG9BcnJheShhKSksIHNpZ24pO1xyXG4gICAgfVxyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5fbXVsdGlwbHlCeVNtYWxsID0gZnVuY3Rpb24gKGEpIHtcclxuICAgICAgICAgICAgaWYgKGlzUHJlY2lzZShhLnZhbHVlICogdGhpcy52YWx1ZSkpIHtcclxuICAgICAgICAgICAgICAgIHJldHVybiBuZXcgU21hbGxJbnRlZ2VyKGEudmFsdWUgKiB0aGlzLnZhbHVlKTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICByZXR1cm4gbXVsdGlwbHlTbWFsbEFuZEFycmF5KE1hdGguYWJzKGEudmFsdWUpLCBzbWFsbFRvQXJyYXkoTWF0aC5hYnModGhpcy52YWx1ZSkpLCB0aGlzLnNpZ24gIT09IGEuc2lnbik7XHJcbiAgICB9O1xyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuX211bHRpcGx5QnlTbWFsbCA9IGZ1bmN0aW9uIChhKSB7XHJcbiAgICAgICAgICAgIGlmIChhLnZhbHVlID09PSAwKSByZXR1cm4gSW50ZWdlclswXTtcclxuICAgICAgICAgICAgaWYgKGEudmFsdWUgPT09IDEpIHJldHVybiB0aGlzO1xyXG4gICAgICAgICAgICBpZiAoYS52YWx1ZSA9PT0gLTEpIHJldHVybiB0aGlzLm5lZ2F0ZSgpO1xyXG4gICAgICAgICAgICByZXR1cm4gbXVsdGlwbHlTbWFsbEFuZEFycmF5KE1hdGguYWJzKGEudmFsdWUpLCB0aGlzLnZhbHVlLCB0aGlzLnNpZ24gIT09IGEuc2lnbik7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5tdWx0aXBseSA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgcmV0dXJuIHBhcnNlVmFsdWUodikuX211bHRpcGx5QnlTbWFsbCh0aGlzKTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnRpbWVzID0gU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5tdWx0aXBseTtcclxuXHJcbiAgICBmdW5jdGlvbiBzcXVhcmUoYSkge1xyXG4gICAgICAgIHZhciBsID0gYS5sZW5ndGgsXHJcbiAgICAgICAgICAgIHIgPSBjcmVhdGVBcnJheShsICsgbCksXHJcbiAgICAgICAgICAgIGJhc2UgPSBCQVNFLFxyXG4gICAgICAgICAgICBwcm9kdWN0LCBjYXJyeSwgaSwgYV9pLCBhX2o7XHJcbiAgICAgICAgZm9yIChpID0gMDsgaSA8IGw7IGkrKykge1xyXG4gICAgICAgICAgICBhX2kgPSBhW2ldO1xyXG4gICAgICAgICAgICBmb3IgKHZhciBqID0gMDsgaiA8IGw7IGorKykge1xyXG4gICAgICAgICAgICAgICAgYV9qID0gYVtqXTtcclxuICAgICAgICAgICAgICAgIHByb2R1Y3QgPSBhX2kgKiBhX2ogKyByW2kgKyBqXTtcclxuICAgICAgICAgICAgICAgIGNhcnJ5ID0gTWF0aC5mbG9vcihwcm9kdWN0IC8gYmFzZSk7XHJcbiAgICAgICAgICAgICAgICByW2kgKyBqXSA9IHByb2R1Y3QgLSBjYXJyeSAqIGJhc2U7XHJcbiAgICAgICAgICAgICAgICByW2kgKyBqICsgMV0gKz0gY2Fycnk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICB9XHJcbiAgICAgICAgdHJpbShyKTtcclxuICAgICAgICByZXR1cm4gcjtcclxuICAgIH1cclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5zcXVhcmUgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIG5ldyBCaWdJbnRlZ2VyKHNxdWFyZSh0aGlzLnZhbHVlKSwgZmFsc2UpO1xyXG4gICAgfTtcclxuXHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnNxdWFyZSA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICB2YXIgdmFsdWUgPSB0aGlzLnZhbHVlICogdGhpcy52YWx1ZTtcclxuICAgICAgICBpZiAoaXNQcmVjaXNlKHZhbHVlKSkgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIodmFsdWUpO1xyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihzcXVhcmUoc21hbGxUb0FycmF5KE1hdGguYWJzKHRoaXMudmFsdWUpKSksIGZhbHNlKTtcclxuICAgIH07XHJcblxyXG4gICAgZnVuY3Rpb24gZGl2TW9kMShhLCBiKSB7IC8vIExlZnQgb3ZlciBmcm9tIHByZXZpb3VzIHZlcnNpb24uIFBlcmZvcm1zIGZhc3RlciB0aGFuIGRpdk1vZDIgb24gc21hbGxlciBpbnB1dCBzaXplcy5cclxuICAgICAgICB2YXIgYV9sID0gYS5sZW5ndGgsXHJcbiAgICAgICAgICAgIGJfbCA9IGIubGVuZ3RoLFxyXG4gICAgICAgICAgICBiYXNlID0gQkFTRSxcclxuICAgICAgICAgICAgcmVzdWx0ID0gY3JlYXRlQXJyYXkoYi5sZW5ndGgpLFxyXG4gICAgICAgICAgICBkaXZpc29yTW9zdFNpZ25pZmljYW50RGlnaXQgPSBiW2JfbCAtIDFdLFxyXG4gICAgICAgICAgICAvLyBub3JtYWxpemF0aW9uXHJcbiAgICAgICAgICAgIGxhbWJkYSA9IE1hdGguY2VpbChiYXNlIC8gKDIgKiBkaXZpc29yTW9zdFNpZ25pZmljYW50RGlnaXQpKSxcclxuICAgICAgICAgICAgcmVtYWluZGVyID0gbXVsdGlwbHlTbWFsbChhLCBsYW1iZGEpLFxyXG4gICAgICAgICAgICBkaXZpc29yID0gbXVsdGlwbHlTbWFsbChiLCBsYW1iZGEpLFxyXG4gICAgICAgICAgICBxdW90aWVudERpZ2l0LCBzaGlmdCwgY2FycnksIGJvcnJvdywgaSwgbCwgcTtcclxuICAgICAgICBpZiAocmVtYWluZGVyLmxlbmd0aCA8PSBhX2wpIHJlbWFpbmRlci5wdXNoKDApO1xyXG4gICAgICAgIGRpdmlzb3IucHVzaCgwKTtcclxuICAgICAgICBkaXZpc29yTW9zdFNpZ25pZmljYW50RGlnaXQgPSBkaXZpc29yW2JfbCAtIDFdO1xyXG4gICAgICAgIGZvciAoc2hpZnQgPSBhX2wgLSBiX2w7IHNoaWZ0ID49IDA7IHNoaWZ0LS0pIHtcclxuICAgICAgICAgICAgcXVvdGllbnREaWdpdCA9IGJhc2UgLSAxO1xyXG4gICAgICAgICAgICBpZiAocmVtYWluZGVyW3NoaWZ0ICsgYl9sXSAhPT0gZGl2aXNvck1vc3RTaWduaWZpY2FudERpZ2l0KSB7XHJcbiAgICAgICAgICAgICAgcXVvdGllbnREaWdpdCA9IE1hdGguZmxvb3IoKHJlbWFpbmRlcltzaGlmdCArIGJfbF0gKiBiYXNlICsgcmVtYWluZGVyW3NoaWZ0ICsgYl9sIC0gMV0pIC8gZGl2aXNvck1vc3RTaWduaWZpY2FudERpZ2l0KTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICAvLyBxdW90aWVudERpZ2l0IDw9IGJhc2UgLSAxXHJcbiAgICAgICAgICAgIGNhcnJ5ID0gMDtcclxuICAgICAgICAgICAgYm9ycm93ID0gMDtcclxuICAgICAgICAgICAgbCA9IGRpdmlzb3IubGVuZ3RoO1xyXG4gICAgICAgICAgICBmb3IgKGkgPSAwOyBpIDwgbDsgaSsrKSB7XHJcbiAgICAgICAgICAgICAgICBjYXJyeSArPSBxdW90aWVudERpZ2l0ICogZGl2aXNvcltpXTtcclxuICAgICAgICAgICAgICAgIHEgPSBNYXRoLmZsb29yKGNhcnJ5IC8gYmFzZSk7XHJcbiAgICAgICAgICAgICAgICBib3Jyb3cgKz0gcmVtYWluZGVyW3NoaWZ0ICsgaV0gLSAoY2FycnkgLSBxICogYmFzZSk7XHJcbiAgICAgICAgICAgICAgICBjYXJyeSA9IHE7XHJcbiAgICAgICAgICAgICAgICBpZiAoYm9ycm93IDwgMCkge1xyXG4gICAgICAgICAgICAgICAgICAgIHJlbWFpbmRlcltzaGlmdCArIGldID0gYm9ycm93ICsgYmFzZTtcclxuICAgICAgICAgICAgICAgICAgICBib3Jyb3cgPSAtMTtcclxuICAgICAgICAgICAgICAgIH0gZWxzZSB7XHJcbiAgICAgICAgICAgICAgICAgICAgcmVtYWluZGVyW3NoaWZ0ICsgaV0gPSBib3Jyb3c7XHJcbiAgICAgICAgICAgICAgICAgICAgYm9ycm93ID0gMDtcclxuICAgICAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICB3aGlsZSAoYm9ycm93ICE9PSAwKSB7XHJcbiAgICAgICAgICAgICAgICBxdW90aWVudERpZ2l0IC09IDE7XHJcbiAgICAgICAgICAgICAgICBjYXJyeSA9IDA7XHJcbiAgICAgICAgICAgICAgICBmb3IgKGkgPSAwOyBpIDwgbDsgaSsrKSB7XHJcbiAgICAgICAgICAgICAgICAgICAgY2FycnkgKz0gcmVtYWluZGVyW3NoaWZ0ICsgaV0gLSBiYXNlICsgZGl2aXNvcltpXTtcclxuICAgICAgICAgICAgICAgICAgICBpZiAoY2FycnkgPCAwKSB7XHJcbiAgICAgICAgICAgICAgICAgICAgICAgIHJlbWFpbmRlcltzaGlmdCArIGldID0gY2FycnkgKyBiYXNlO1xyXG4gICAgICAgICAgICAgICAgICAgICAgICBjYXJyeSA9IDA7XHJcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcclxuICAgICAgICAgICAgICAgICAgICAgICAgcmVtYWluZGVyW3NoaWZ0ICsgaV0gPSBjYXJyeTtcclxuICAgICAgICAgICAgICAgICAgICAgICAgY2FycnkgPSAxO1xyXG4gICAgICAgICAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgICAgIGJvcnJvdyArPSBjYXJyeTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICByZXN1bHRbc2hpZnRdID0gcXVvdGllbnREaWdpdDtcclxuICAgICAgICB9XHJcbiAgICAgICAgLy8gZGVub3JtYWxpemF0aW9uXHJcbiAgICAgICAgcmVtYWluZGVyID0gZGl2TW9kU21hbGwocmVtYWluZGVyLCBsYW1iZGEpWzBdO1xyXG4gICAgICAgIHJldHVybiBbYXJyYXlUb1NtYWxsKHJlc3VsdCksIGFycmF5VG9TbWFsbChyZW1haW5kZXIpXTtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBkaXZNb2QyKGEsIGIpIHsgLy8gSW1wbGVtZW50YXRpb24gaWRlYSBzaGFtZWxlc3NseSBzdG9sZW4gZnJvbSBTaWxlbnQgTWF0dCdzIGxpYnJhcnkgaHR0cDovL3NpbGVudG1hdHQuY29tL2JpZ2ludGVnZXIvXHJcbiAgICAgICAgLy8gUGVyZm9ybXMgZmFzdGVyIHRoYW4gZGl2TW9kMSBvbiBsYXJnZXIgaW5wdXQgc2l6ZXMuXHJcbiAgICAgICAgdmFyIGFfbCA9IGEubGVuZ3RoLFxyXG4gICAgICAgICAgICBiX2wgPSBiLmxlbmd0aCxcclxuICAgICAgICAgICAgcmVzdWx0ID0gW10sXHJcbiAgICAgICAgICAgIHBhcnQgPSBbXSxcclxuICAgICAgICAgICAgYmFzZSA9IEJBU0UsXHJcbiAgICAgICAgICAgIGd1ZXNzLCB4bGVuLCBoaWdoeCwgaGlnaHksIGNoZWNrO1xyXG4gICAgICAgIHdoaWxlIChhX2wpIHtcclxuICAgICAgICAgICAgcGFydC51bnNoaWZ0KGFbLS1hX2xdKTtcclxuICAgICAgICAgICAgaWYgKGNvbXBhcmVBYnMocGFydCwgYikgPCAwKSB7XHJcbiAgICAgICAgICAgICAgICByZXN1bHQucHVzaCgwKTtcclxuICAgICAgICAgICAgICAgIGNvbnRpbnVlO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIHhsZW4gPSBwYXJ0Lmxlbmd0aDtcclxuICAgICAgICAgICAgaGlnaHggPSBwYXJ0W3hsZW4gLSAxXSAqIGJhc2UgKyBwYXJ0W3hsZW4gLSAyXTtcclxuICAgICAgICAgICAgaGlnaHkgPSBiW2JfbCAtIDFdICogYmFzZSArIGJbYl9sIC0gMl07XHJcbiAgICAgICAgICAgIGlmICh4bGVuID4gYl9sKSB7XHJcbiAgICAgICAgICAgICAgICBoaWdoeCA9IChoaWdoeCArIDEpICogYmFzZTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICBndWVzcyA9IE1hdGguY2VpbChoaWdoeCAvIGhpZ2h5KTtcclxuICAgICAgICAgICAgZG8ge1xyXG4gICAgICAgICAgICAgICAgY2hlY2sgPSBtdWx0aXBseVNtYWxsKGIsIGd1ZXNzKTtcclxuICAgICAgICAgICAgICAgIGlmIChjb21wYXJlQWJzKGNoZWNrLCBwYXJ0KSA8PSAwKSBicmVhaztcclxuICAgICAgICAgICAgICAgIGd1ZXNzLS07XHJcbiAgICAgICAgICAgIH0gd2hpbGUgKGd1ZXNzKTtcclxuICAgICAgICAgICAgcmVzdWx0LnB1c2goZ3Vlc3MpO1xyXG4gICAgICAgICAgICBwYXJ0ID0gc3VidHJhY3QocGFydCwgY2hlY2spO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXN1bHQucmV2ZXJzZSgpO1xyXG4gICAgICAgIHJldHVybiBbYXJyYXlUb1NtYWxsKHJlc3VsdCksIGFycmF5VG9TbWFsbChwYXJ0KV07XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gZGl2TW9kU21hbGwodmFsdWUsIGxhbWJkYSkge1xyXG4gICAgICAgIHZhciBsZW5ndGggPSB2YWx1ZS5sZW5ndGgsXHJcbiAgICAgICAgICAgIHF1b3RpZW50ID0gY3JlYXRlQXJyYXkobGVuZ3RoKSxcclxuICAgICAgICAgICAgYmFzZSA9IEJBU0UsXHJcbiAgICAgICAgICAgIGksIHEsIHJlbWFpbmRlciwgZGl2aXNvcjtcclxuICAgICAgICByZW1haW5kZXIgPSAwO1xyXG4gICAgICAgIGZvciAoaSA9IGxlbmd0aCAtIDE7IGkgPj0gMDsgLS1pKSB7XHJcbiAgICAgICAgICAgIGRpdmlzb3IgPSByZW1haW5kZXIgKiBiYXNlICsgdmFsdWVbaV07XHJcbiAgICAgICAgICAgIHEgPSB0cnVuY2F0ZShkaXZpc29yIC8gbGFtYmRhKTtcclxuICAgICAgICAgICAgcmVtYWluZGVyID0gZGl2aXNvciAtIHEgKiBsYW1iZGE7XHJcbiAgICAgICAgICAgIHF1b3RpZW50W2ldID0gcSB8IDA7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBbcXVvdGllbnQsIHJlbWFpbmRlciB8IDBdO1xyXG4gICAgfVxyXG5cclxuICAgIGZ1bmN0aW9uIGRpdk1vZEFueShzZWxmLCB2KSB7XHJcbiAgICAgICAgdmFyIHZhbHVlLCBuID0gcGFyc2VWYWx1ZSh2KTtcclxuICAgICAgICB2YXIgYSA9IHNlbGYudmFsdWUsIGIgPSBuLnZhbHVlO1xyXG4gICAgICAgIHZhciBxdW90aWVudDtcclxuICAgICAgICBpZiAoYiA9PT0gMCkgdGhyb3cgbmV3IEVycm9yKFwiQ2Fubm90IGRpdmlkZSBieSB6ZXJvXCIpO1xyXG4gICAgICAgIGlmIChzZWxmLmlzU21hbGwpIHtcclxuICAgICAgICAgICAgaWYgKG4uaXNTbWFsbCkge1xyXG4gICAgICAgICAgICAgICAgcmV0dXJuIFtuZXcgU21hbGxJbnRlZ2VyKHRydW5jYXRlKGEgLyBiKSksIG5ldyBTbWFsbEludGVnZXIoYSAlIGIpXTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICByZXR1cm4gW0ludGVnZXJbMF0sIHNlbGZdO1xyXG4gICAgICAgIH1cclxuICAgICAgICBpZiAobi5pc1NtYWxsKSB7XHJcbiAgICAgICAgICAgIGlmIChiID09PSAxKSByZXR1cm4gW3NlbGYsIEludGVnZXJbMF1dO1xyXG4gICAgICAgICAgICBpZiAoYiA9PSAtMSkgcmV0dXJuIFtzZWxmLm5lZ2F0ZSgpLCBJbnRlZ2VyWzBdXTtcclxuICAgICAgICAgICAgdmFyIGFicyA9IE1hdGguYWJzKGIpO1xyXG4gICAgICAgICAgICBpZiAoYWJzIDwgQkFTRSkge1xyXG4gICAgICAgICAgICAgICAgdmFsdWUgPSBkaXZNb2RTbWFsbChhLCBhYnMpO1xyXG4gICAgICAgICAgICAgICAgcXVvdGllbnQgPSBhcnJheVRvU21hbGwodmFsdWVbMF0pO1xyXG4gICAgICAgICAgICAgICAgdmFyIHJlbWFpbmRlciA9IHZhbHVlWzFdO1xyXG4gICAgICAgICAgICAgICAgaWYgKHNlbGYuc2lnbikgcmVtYWluZGVyID0gLXJlbWFpbmRlcjtcclxuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgcXVvdGllbnQgPT09IFwibnVtYmVyXCIpIHtcclxuICAgICAgICAgICAgICAgICAgICBpZiAoc2VsZi5zaWduICE9PSBuLnNpZ24pIHF1b3RpZW50ID0gLXF1b3RpZW50O1xyXG4gICAgICAgICAgICAgICAgICAgIHJldHVybiBbbmV3IFNtYWxsSW50ZWdlcihxdW90aWVudCksIG5ldyBTbWFsbEludGVnZXIocmVtYWluZGVyKV07XHJcbiAgICAgICAgICAgICAgICB9XHJcbiAgICAgICAgICAgICAgICByZXR1cm4gW25ldyBCaWdJbnRlZ2VyKHF1b3RpZW50LCBzZWxmLnNpZ24gIT09IG4uc2lnbiksIG5ldyBTbWFsbEludGVnZXIocmVtYWluZGVyKV07XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgYiA9IHNtYWxsVG9BcnJheShhYnMpO1xyXG4gICAgICAgIH1cclxuICAgICAgICB2YXIgY29tcGFyaXNvbiA9IGNvbXBhcmVBYnMoYSwgYik7XHJcbiAgICAgICAgaWYgKGNvbXBhcmlzb24gPT09IC0xKSByZXR1cm4gW0ludGVnZXJbMF0sIHNlbGZdO1xyXG4gICAgICAgIGlmIChjb21wYXJpc29uID09PSAwKSByZXR1cm4gW0ludGVnZXJbc2VsZi5zaWduID09PSBuLnNpZ24gPyAxIDogLTFdLCBJbnRlZ2VyWzBdXTtcclxuXHJcbiAgICAgICAgLy8gZGl2TW9kMSBpcyBmYXN0ZXIgb24gc21hbGxlciBpbnB1dCBzaXplc1xyXG4gICAgICAgIGlmIChhLmxlbmd0aCArIGIubGVuZ3RoIDw9IDIwMClcclxuICAgICAgICAgICAgdmFsdWUgPSBkaXZNb2QxKGEsIGIpO1xyXG4gICAgICAgIGVsc2UgdmFsdWUgPSBkaXZNb2QyKGEsIGIpO1xyXG5cclxuICAgICAgICBxdW90aWVudCA9IHZhbHVlWzBdO1xyXG4gICAgICAgIHZhciBxU2lnbiA9IHNlbGYuc2lnbiAhPT0gbi5zaWduLFxyXG4gICAgICAgICAgICBtb2QgPSB2YWx1ZVsxXSxcclxuICAgICAgICAgICAgbVNpZ24gPSBzZWxmLnNpZ247XHJcbiAgICAgICAgaWYgKHR5cGVvZiBxdW90aWVudCA9PT0gXCJudW1iZXJcIikge1xyXG4gICAgICAgICAgICBpZiAocVNpZ24pIHF1b3RpZW50ID0gLXF1b3RpZW50O1xyXG4gICAgICAgICAgICBxdW90aWVudCA9IG5ldyBTbWFsbEludGVnZXIocXVvdGllbnQpO1xyXG4gICAgICAgIH0gZWxzZSBxdW90aWVudCA9IG5ldyBCaWdJbnRlZ2VyKHF1b3RpZW50LCBxU2lnbik7XHJcbiAgICAgICAgaWYgKHR5cGVvZiBtb2QgPT09IFwibnVtYmVyXCIpIHtcclxuICAgICAgICAgICAgaWYgKG1TaWduKSBtb2QgPSAtbW9kO1xyXG4gICAgICAgICAgICBtb2QgPSBuZXcgU21hbGxJbnRlZ2VyKG1vZCk7XHJcbiAgICAgICAgfSBlbHNlIG1vZCA9IG5ldyBCaWdJbnRlZ2VyKG1vZCwgbVNpZ24pO1xyXG4gICAgICAgIHJldHVybiBbcXVvdGllbnQsIG1vZF07XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuZGl2bW9kID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICB2YXIgcmVzdWx0ID0gZGl2TW9kQW55KHRoaXMsIHYpO1xyXG4gICAgICAgIHJldHVybiB7XHJcbiAgICAgICAgICAgIHF1b3RpZW50OiByZXN1bHRbMF0sXHJcbiAgICAgICAgICAgIHJlbWFpbmRlcjogcmVzdWx0WzFdXHJcbiAgICAgICAgfTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmRpdm1vZCA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmRpdm1vZDtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5kaXZpZGUgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHJldHVybiBkaXZNb2RBbnkodGhpcywgdilbMF07XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5vdmVyID0gU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5kaXZpZGUgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5vdmVyID0gQmlnSW50ZWdlci5wcm90b3R5cGUuZGl2aWRlO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLm1vZCA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgcmV0dXJuIGRpdk1vZEFueSh0aGlzLCB2KVsxXTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnJlbWFpbmRlciA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUubW9kID0gQmlnSW50ZWdlci5wcm90b3R5cGUucmVtYWluZGVyID0gQmlnSW50ZWdlci5wcm90b3R5cGUubW9kO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLnBvdyA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgdmFyIG4gPSBwYXJzZVZhbHVlKHYpLFxyXG4gICAgICAgICAgICBhID0gdGhpcy52YWx1ZSxcclxuICAgICAgICAgICAgYiA9IG4udmFsdWUsXHJcbiAgICAgICAgICAgIHZhbHVlLCB4LCB5O1xyXG4gICAgICAgIGlmIChiID09PSAwKSByZXR1cm4gSW50ZWdlclsxXTtcclxuICAgICAgICBpZiAoYSA9PT0gMCkgcmV0dXJuIEludGVnZXJbMF07XHJcbiAgICAgICAgaWYgKGEgPT09IDEpIHJldHVybiBJbnRlZ2VyWzFdO1xyXG4gICAgICAgIGlmIChhID09PSAtMSkgcmV0dXJuIG4uaXNFdmVuKCkgPyBJbnRlZ2VyWzFdIDogSW50ZWdlclstMV07XHJcbiAgICAgICAgaWYgKG4uc2lnbikge1xyXG4gICAgICAgICAgICByZXR1cm4gSW50ZWdlclswXTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKCFuLmlzU21hbGwpIHRocm93IG5ldyBFcnJvcihcIlRoZSBleHBvbmVudCBcIiArIG4udG9TdHJpbmcoKSArIFwiIGlzIHRvbyBsYXJnZS5cIik7XHJcbiAgICAgICAgaWYgKHRoaXMuaXNTbWFsbCkge1xyXG4gICAgICAgICAgICBpZiAoaXNQcmVjaXNlKHZhbHVlID0gTWF0aC5wb3coYSwgYikpKVxyXG4gICAgICAgICAgICAgICAgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIodHJ1bmNhdGUodmFsdWUpKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgeCA9IHRoaXM7XHJcbiAgICAgICAgeSA9IEludGVnZXJbMV07XHJcbiAgICAgICAgd2hpbGUgKHRydWUpIHtcclxuICAgICAgICAgICAgaWYgKGIgJiAxID09PSAxKSB7XHJcbiAgICAgICAgICAgICAgICB5ID0geS50aW1lcyh4KTtcclxuICAgICAgICAgICAgICAgIC0tYjtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICBpZiAoYiA9PT0gMCkgYnJlYWs7XHJcbiAgICAgICAgICAgIGIgLz0gMjtcclxuICAgICAgICAgICAgeCA9IHguc3F1YXJlKCk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiB5O1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUucG93ID0gQmlnSW50ZWdlci5wcm90b3R5cGUucG93O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLm1vZFBvdyA9IGZ1bmN0aW9uIChleHAsIG1vZCkge1xyXG4gICAgICAgIGV4cCA9IHBhcnNlVmFsdWUoZXhwKTtcclxuICAgICAgICBtb2QgPSBwYXJzZVZhbHVlKG1vZCk7XHJcbiAgICAgICAgaWYgKG1vZC5pc1plcm8oKSkgdGhyb3cgbmV3IEVycm9yKFwiQ2Fubm90IHRha2UgbW9kUG93IHdpdGggbW9kdWx1cyAwXCIpO1xyXG4gICAgICAgIHZhciByID0gSW50ZWdlclsxXSxcclxuICAgICAgICAgICAgYmFzZSA9IHRoaXMubW9kKG1vZCk7XHJcbiAgICAgICAgd2hpbGUgKGV4cC5pc1Bvc2l0aXZlKCkpIHtcclxuICAgICAgICAgICAgaWYgKGJhc2UuaXNaZXJvKCkpIHJldHVybiBJbnRlZ2VyWzBdO1xyXG4gICAgICAgICAgICBpZiAoZXhwLmlzT2RkKCkpIHIgPSByLm11bHRpcGx5KGJhc2UpLm1vZChtb2QpO1xyXG4gICAgICAgICAgICBleHAgPSBleHAuZGl2aWRlKDIpO1xyXG4gICAgICAgICAgICBiYXNlID0gYmFzZS5zcXVhcmUoKS5tb2QobW9kKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHI7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5tb2RQb3cgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5tb2RQb3c7XHJcblxyXG4gICAgZnVuY3Rpb24gY29tcGFyZUFicyhhLCBiKSB7XHJcbiAgICAgICAgaWYgKGEubGVuZ3RoICE9PSBiLmxlbmd0aCkge1xyXG4gICAgICAgICAgICByZXR1cm4gYS5sZW5ndGggPiBiLmxlbmd0aCA/IDEgOiAtMTtcclxuICAgICAgICB9XHJcbiAgICAgICAgZm9yICh2YXIgaSA9IGEubGVuZ3RoIC0gMTsgaSA+PSAwOyBpLS0pIHtcclxuICAgICAgICAgICAgaWYgKGFbaV0gIT09IGJbaV0pIHJldHVybiBhW2ldID4gYltpXSA/IDEgOiAtMTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIDA7XHJcbiAgICB9XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuY29tcGFyZUFicyA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgdmFyIG4gPSBwYXJzZVZhbHVlKHYpLFxyXG4gICAgICAgICAgICBhID0gdGhpcy52YWx1ZSxcclxuICAgICAgICAgICAgYiA9IG4udmFsdWU7XHJcbiAgICAgICAgaWYgKG4uaXNTbWFsbCkgcmV0dXJuIDE7XHJcbiAgICAgICAgcmV0dXJuIGNvbXBhcmVBYnMoYSwgYik7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5jb21wYXJlQWJzID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICB2YXIgbiA9IHBhcnNlVmFsdWUodiksXHJcbiAgICAgICAgICAgIGEgPSBNYXRoLmFicyh0aGlzLnZhbHVlKSxcclxuICAgICAgICAgICAgYiA9IG4udmFsdWU7XHJcbiAgICAgICAgaWYgKG4uaXNTbWFsbCkge1xyXG4gICAgICAgICAgICBiID0gTWF0aC5hYnMoYik7XHJcbiAgICAgICAgICAgIHJldHVybiBhID09PSBiID8gMCA6IGEgPiBiID8gMSA6IC0xO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gLTE7XHJcbiAgICB9O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmNvbXBhcmUgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIC8vIFNlZSBkaXNjdXNzaW9uIGFib3V0IGNvbXBhcmlzb24gd2l0aCBJbmZpbml0eTpcclxuICAgICAgICAvLyBodHRwczovL2dpdGh1Yi5jb20vcGV0ZXJvbHNvbi9CaWdJbnRlZ2VyLmpzL2lzc3Vlcy82MVxyXG4gICAgICAgIGlmICh2ID09PSBJbmZpbml0eSkge1xyXG4gICAgICAgICAgICByZXR1cm4gLTE7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGlmICh2ID09PSAtSW5maW5pdHkpIHtcclxuICAgICAgICAgICAgcmV0dXJuIDE7XHJcbiAgICAgICAgfVxyXG5cclxuICAgICAgICB2YXIgbiA9IHBhcnNlVmFsdWUodiksXHJcbiAgICAgICAgICAgIGEgPSB0aGlzLnZhbHVlLFxyXG4gICAgICAgICAgICBiID0gbi52YWx1ZTtcclxuICAgICAgICBpZiAodGhpcy5zaWduICE9PSBuLnNpZ24pIHtcclxuICAgICAgICAgICAgcmV0dXJuIG4uc2lnbiA/IDEgOiAtMTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKG4uaXNTbWFsbCkge1xyXG4gICAgICAgICAgICByZXR1cm4gdGhpcy5zaWduID8gLTEgOiAxO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gY29tcGFyZUFicyhhLCBiKSAqICh0aGlzLnNpZ24gPyAtMSA6IDEpO1xyXG4gICAgfTtcclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmNvbXBhcmVUbyA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmNvbXBhcmU7XHJcblxyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5jb21wYXJlID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICBpZiAodiA9PT0gSW5maW5pdHkpIHtcclxuICAgICAgICAgICAgcmV0dXJuIC0xO1xyXG4gICAgICAgIH1cclxuICAgICAgICBpZiAodiA9PT0gLUluZmluaXR5KSB7XHJcbiAgICAgICAgICAgIHJldHVybiAxO1xyXG4gICAgICAgIH1cclxuXHJcbiAgICAgICAgdmFyIG4gPSBwYXJzZVZhbHVlKHYpLFxyXG4gICAgICAgICAgICBhID0gdGhpcy52YWx1ZSxcclxuICAgICAgICAgICAgYiA9IG4udmFsdWU7XHJcbiAgICAgICAgaWYgKG4uaXNTbWFsbCkge1xyXG4gICAgICAgICAgICByZXR1cm4gYSA9PSBiID8gMCA6IGEgPiBiID8gMSA6IC0xO1xyXG4gICAgICAgIH1cclxuICAgICAgICBpZiAoYSA8IDAgIT09IG4uc2lnbikge1xyXG4gICAgICAgICAgICByZXR1cm4gYSA8IDAgPyAtMSA6IDE7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBhIDwgMCA/IDEgOiAtMTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmNvbXBhcmVUbyA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUuY29tcGFyZTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5lcXVhbHMgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHJldHVybiB0aGlzLmNvbXBhcmUodikgPT09IDA7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5lcSA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUuZXF1YWxzID0gQmlnSW50ZWdlci5wcm90b3R5cGUuZXEgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5lcXVhbHM7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUubm90RXF1YWxzID0gZnVuY3Rpb24gKHYpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy5jb21wYXJlKHYpICE9PSAwO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUubmVxID0gU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5ub3RFcXVhbHMgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5uZXEgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5ub3RFcXVhbHM7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuZ3JlYXRlciA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgcmV0dXJuIHRoaXMuY29tcGFyZSh2KSA+IDA7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5ndCA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUuZ3JlYXRlciA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmd0ID0gQmlnSW50ZWdlci5wcm90b3R5cGUuZ3JlYXRlcjtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5sZXNzZXIgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHJldHVybiB0aGlzLmNvbXBhcmUodikgPCAwO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUubHQgPSBTbWFsbEludGVnZXIucHJvdG90eXBlLmxlc3NlciA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmx0ID0gQmlnSW50ZWdlci5wcm90b3R5cGUubGVzc2VyO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmdyZWF0ZXJPckVxdWFscyA9IGZ1bmN0aW9uICh2KSB7XHJcbiAgICAgICAgcmV0dXJuIHRoaXMuY29tcGFyZSh2KSA+PSAwO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuZ2VxID0gU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5ncmVhdGVyT3JFcXVhbHMgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5nZXEgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5ncmVhdGVyT3JFcXVhbHM7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUubGVzc2VyT3JFcXVhbHMgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHJldHVybiB0aGlzLmNvbXBhcmUodikgPD0gMDtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmxlcSA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUubGVzc2VyT3JFcXVhbHMgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5sZXEgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5sZXNzZXJPckVxdWFscztcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc0V2ZW4gPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuICh0aGlzLnZhbHVlWzBdICYgMSkgPT09IDA7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5pc0V2ZW4gPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuICh0aGlzLnZhbHVlICYgMSkgPT09IDA7XHJcbiAgICB9O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmlzT2RkID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiAodGhpcy52YWx1ZVswXSAmIDEpID09PSAxO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuaXNPZGQgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuICh0aGlzLnZhbHVlICYgMSkgPT09IDE7XHJcbiAgICB9O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmlzUG9zaXRpdmUgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuICF0aGlzLnNpZ247XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5pc1Bvc2l0aXZlID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiB0aGlzLnZhbHVlID4gMDtcclxuICAgIH07XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUuaXNOZWdhdGl2ZSA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy5zaWduO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuaXNOZWdhdGl2ZSA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gdGhpcy52YWx1ZSA8IDA7XHJcbiAgICB9O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmlzVW5pdCA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICByZXR1cm4gZmFsc2U7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5pc1VuaXQgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIE1hdGguYWJzKHRoaXMudmFsdWUpID09PSAxO1xyXG4gICAgfTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc1plcm8gPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIGZhbHNlO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuaXNaZXJvID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiB0aGlzLnZhbHVlID09PSAwO1xyXG4gICAgfTtcclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmlzRGl2aXNpYmxlQnkgPSBmdW5jdGlvbiAodikge1xyXG4gICAgICAgIHZhciBuID0gcGFyc2VWYWx1ZSh2KTtcclxuICAgICAgICB2YXIgdmFsdWUgPSBuLnZhbHVlO1xyXG4gICAgICAgIGlmICh2YWx1ZSA9PT0gMCkgcmV0dXJuIGZhbHNlO1xyXG4gICAgICAgIGlmICh2YWx1ZSA9PT0gMSkgcmV0dXJuIHRydWU7XHJcbiAgICAgICAgaWYgKHZhbHVlID09PSAyKSByZXR1cm4gdGhpcy5pc0V2ZW4oKTtcclxuICAgICAgICByZXR1cm4gdGhpcy5tb2QobikuZXF1YWxzKEludGVnZXJbMF0pO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuaXNEaXZpc2libGVCeSA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmlzRGl2aXNpYmxlQnk7XHJcblxyXG4gICAgZnVuY3Rpb24gaXNCYXNpY1ByaW1lKHYpIHtcclxuICAgICAgICB2YXIgbiA9IHYuYWJzKCk7XHJcbiAgICAgICAgaWYgKG4uaXNVbml0KCkpIHJldHVybiBmYWxzZTtcclxuICAgICAgICBpZiAobi5lcXVhbHMoMikgfHwgbi5lcXVhbHMoMykgfHwgbi5lcXVhbHMoNSkpIHJldHVybiB0cnVlO1xyXG4gICAgICAgIGlmIChuLmlzRXZlbigpIHx8IG4uaXNEaXZpc2libGVCeSgzKSB8fCBuLmlzRGl2aXNpYmxlQnkoNSkpIHJldHVybiBmYWxzZTtcclxuICAgICAgICBpZiAobi5sZXNzZXIoMjUpKSByZXR1cm4gdHJ1ZTtcclxuICAgICAgICAvLyB3ZSBkb24ndCBrbm93IGlmIGl0J3MgcHJpbWU6IGxldCB0aGUgb3RoZXIgZnVuY3Rpb25zIGZpZ3VyZSBpdCBvdXRcclxuICAgIH1cclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc1ByaW1lID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHZhciBpc1ByaW1lID0gaXNCYXNpY1ByaW1lKHRoaXMpO1xyXG4gICAgICAgIGlmIChpc1ByaW1lICE9PSB1bmRlZmluZWQpIHJldHVybiBpc1ByaW1lO1xyXG4gICAgICAgIHZhciBuID0gdGhpcy5hYnMoKSxcclxuICAgICAgICAgICAgblByZXYgPSBuLnByZXYoKTtcclxuICAgICAgICB2YXIgYSA9IFsyLCAzLCA1LCA3LCAxMSwgMTMsIDE3LCAxOV0sXHJcbiAgICAgICAgICAgIGIgPSBuUHJldixcclxuICAgICAgICAgICAgZCwgdCwgaSwgeDtcclxuICAgICAgICB3aGlsZSAoYi5pc0V2ZW4oKSkgYiA9IGIuZGl2aWRlKDIpO1xyXG4gICAgICAgIGZvciAoaSA9IDA7IGkgPCBhLmxlbmd0aDsgaSsrKSB7XHJcbiAgICAgICAgICAgIHggPSBiaWdJbnQoYVtpXSkubW9kUG93KGIsIG4pO1xyXG4gICAgICAgICAgICBpZiAoeC5lcXVhbHMoSW50ZWdlclsxXSkgfHwgeC5lcXVhbHMoblByZXYpKSBjb250aW51ZTtcclxuICAgICAgICAgICAgZm9yICh0ID0gdHJ1ZSwgZCA9IGI7IHQgJiYgZC5sZXNzZXIoblByZXYpIDsgZCA9IGQubXVsdGlwbHkoMikpIHtcclxuICAgICAgICAgICAgICAgIHggPSB4LnNxdWFyZSgpLm1vZChuKTtcclxuICAgICAgICAgICAgICAgIGlmICh4LmVxdWFscyhuUHJldikpIHQgPSBmYWxzZTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICBpZiAodCkgcmV0dXJuIGZhbHNlO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gdHJ1ZTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmlzUHJpbWUgPSBCaWdJbnRlZ2VyLnByb3RvdHlwZS5pc1ByaW1lO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmlzUHJvYmFibGVQcmltZSA9IGZ1bmN0aW9uIChpdGVyYXRpb25zKSB7XHJcbiAgICAgICAgdmFyIGlzUHJpbWUgPSBpc0Jhc2ljUHJpbWUodGhpcyk7XHJcbiAgICAgICAgaWYgKGlzUHJpbWUgIT09IHVuZGVmaW5lZCkgcmV0dXJuIGlzUHJpbWU7XHJcbiAgICAgICAgdmFyIG4gPSB0aGlzLmFicygpO1xyXG4gICAgICAgIHZhciB0ID0gaXRlcmF0aW9ucyA9PT0gdW5kZWZpbmVkID8gNSA6IGl0ZXJhdGlvbnM7XHJcbiAgICAgICAgLy8gdXNlIHRoZSBGZXJtYXQgcHJpbWFsaXR5IHRlc3RcclxuICAgICAgICBmb3IgKHZhciBpID0gMDsgaSA8IHQ7IGkrKykge1xyXG4gICAgICAgICAgICB2YXIgYSA9IGJpZ0ludC5yYW5kQmV0d2VlbigyLCBuLm1pbnVzKDIpKTtcclxuICAgICAgICAgICAgaWYgKCFhLm1vZFBvdyhuLnByZXYoKSwgbikuaXNVbml0KCkpIHJldHVybiBmYWxzZTsgLy8gZGVmaW5pdGVseSBjb21wb3NpdGVcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHRydWU7IC8vIGxhcmdlIGNoYW5jZSBvZiBiZWluZyBwcmltZVxyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUuaXNQcm9iYWJsZVByaW1lID0gQmlnSW50ZWdlci5wcm90b3R5cGUuaXNQcm9iYWJsZVByaW1lO1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLm1vZEludiA9IGZ1bmN0aW9uIChuKSB7XHJcbiAgICAgICAgdmFyIHQgPSBiaWdJbnQuemVybywgbmV3VCA9IGJpZ0ludC5vbmUsIHIgPSBwYXJzZVZhbHVlKG4pLCBuZXdSID0gdGhpcy5hYnMoKSwgcSwgbGFzdFQsIGxhc3RSO1xyXG4gICAgICAgIHdoaWxlICghbmV3Ui5lcXVhbHMoYmlnSW50Lnplcm8pKSB7XHJcbiAgICAgICAgXHRxID0gci5kaXZpZGUobmV3Uik7XHJcbiAgICAgICAgICBsYXN0VCA9IHQ7XHJcbiAgICAgICAgICBsYXN0UiA9IHI7XHJcbiAgICAgICAgICB0ID0gbmV3VDtcclxuICAgICAgICAgIHIgPSBuZXdSO1xyXG4gICAgICAgICAgbmV3VCA9IGxhc3RULnN1YnRyYWN0KHEubXVsdGlwbHkobmV3VCkpO1xyXG4gICAgICAgICAgbmV3UiA9IGxhc3RSLnN1YnRyYWN0KHEubXVsdGlwbHkobmV3UikpO1xyXG4gICAgICAgIH1cclxuICAgICAgICBpZiAoIXIuZXF1YWxzKDEpKSB0aHJvdyBuZXcgRXJyb3IodGhpcy50b1N0cmluZygpICsgXCIgYW5kIFwiICsgbi50b1N0cmluZygpICsgXCIgYXJlIG5vdCBjby1wcmltZVwiKTtcclxuICAgICAgICBpZiAodC5jb21wYXJlKDApID09PSAtMSkge1xyXG4gICAgICAgIFx0dCA9IHQuYWRkKG4pO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gdDtcclxuICAgIH1cclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUubW9kSW52ID0gQmlnSW50ZWdlci5wcm90b3R5cGUubW9kSW52O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLm5leHQgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgdmFyIHZhbHVlID0gdGhpcy52YWx1ZTtcclxuICAgICAgICBpZiAodGhpcy5zaWduKSB7XHJcbiAgICAgICAgICAgIHJldHVybiBzdWJ0cmFjdFNtYWxsKHZhbHVlLCAxLCB0aGlzLnNpZ24pO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIoYWRkU21hbGwodmFsdWUsIDEpLCB0aGlzLnNpZ24pO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUubmV4dCA9IGZ1bmN0aW9uICgpIHtcclxuICAgICAgICB2YXIgdmFsdWUgPSB0aGlzLnZhbHVlO1xyXG4gICAgICAgIGlmICh2YWx1ZSArIDEgPCBNQVhfSU5UKSByZXR1cm4gbmV3IFNtYWxsSW50ZWdlcih2YWx1ZSArIDEpO1xyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihNQVhfSU5UX0FSUiwgZmFsc2UpO1xyXG4gICAgfTtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5wcmV2ID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHZhciB2YWx1ZSA9IHRoaXMudmFsdWU7XHJcbiAgICAgICAgaWYgKHRoaXMuc2lnbikge1xyXG4gICAgICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIoYWRkU21hbGwodmFsdWUsIDEpLCB0cnVlKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHN1YnRyYWN0U21hbGwodmFsdWUsIDEsIHRoaXMuc2lnbik7XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5wcmV2ID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHZhciB2YWx1ZSA9IHRoaXMudmFsdWU7XHJcbiAgICAgICAgaWYgKHZhbHVlIC0gMSA+IC1NQVhfSU5UKSByZXR1cm4gbmV3IFNtYWxsSW50ZWdlcih2YWx1ZSAtIDEpO1xyXG4gICAgICAgIHJldHVybiBuZXcgQmlnSW50ZWdlcihNQVhfSU5UX0FSUiwgdHJ1ZSk7XHJcbiAgICB9O1xyXG5cclxuICAgIHZhciBwb3dlcnNPZlR3byA9IFsxXTtcclxuICAgIHdoaWxlIChwb3dlcnNPZlR3b1twb3dlcnNPZlR3by5sZW5ndGggLSAxXSA8PSBCQVNFKSBwb3dlcnNPZlR3by5wdXNoKDIgKiBwb3dlcnNPZlR3b1twb3dlcnNPZlR3by5sZW5ndGggLSAxXSk7XHJcbiAgICB2YXIgcG93ZXJzMkxlbmd0aCA9IHBvd2Vyc09mVHdvLmxlbmd0aCwgaGlnaGVzdFBvd2VyMiA9IHBvd2Vyc09mVHdvW3Bvd2VyczJMZW5ndGggLSAxXTtcclxuXHJcbiAgICBmdW5jdGlvbiBzaGlmdF9pc1NtYWxsKG4pIHtcclxuICAgICAgICByZXR1cm4gKCh0eXBlb2YgbiA9PT0gXCJudW1iZXJcIiB8fCB0eXBlb2YgbiA9PT0gXCJzdHJpbmdcIikgJiYgK01hdGguYWJzKG4pIDw9IEJBU0UpIHx8XHJcbiAgICAgICAgICAgIChuIGluc3RhbmNlb2YgQmlnSW50ZWdlciAmJiBuLnZhbHVlLmxlbmd0aCA8PSAxKTtcclxuICAgIH1cclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5zaGlmdExlZnQgPSBmdW5jdGlvbiAobikge1xyXG4gICAgICAgIGlmICghc2hpZnRfaXNTbWFsbChuKSkge1xyXG4gICAgICAgICAgICB0aHJvdyBuZXcgRXJyb3IoU3RyaW5nKG4pICsgXCIgaXMgdG9vIGxhcmdlIGZvciBzaGlmdGluZy5cIik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIG4gPSArbjtcclxuICAgICAgICBpZiAobiA8IDApIHJldHVybiB0aGlzLnNoaWZ0UmlnaHQoLW4pO1xyXG4gICAgICAgIHZhciByZXN1bHQgPSB0aGlzO1xyXG4gICAgICAgIHdoaWxlIChuID49IHBvd2VyczJMZW5ndGgpIHtcclxuICAgICAgICAgICAgcmVzdWx0ID0gcmVzdWx0Lm11bHRpcGx5KGhpZ2hlc3RQb3dlcjIpO1xyXG4gICAgICAgICAgICBuIC09IHBvd2VyczJMZW5ndGggLSAxO1xyXG4gICAgICAgIH1cclxuICAgICAgICByZXR1cm4gcmVzdWx0Lm11bHRpcGx5KHBvd2Vyc09mVHdvW25dKTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLnNoaWZ0TGVmdCA9IEJpZ0ludGVnZXIucHJvdG90eXBlLnNoaWZ0TGVmdDtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5zaGlmdFJpZ2h0ID0gZnVuY3Rpb24gKG4pIHtcclxuICAgICAgICB2YXIgcmVtUXVvO1xyXG4gICAgICAgIGlmICghc2hpZnRfaXNTbWFsbChuKSkge1xyXG4gICAgICAgICAgICB0aHJvdyBuZXcgRXJyb3IoU3RyaW5nKG4pICsgXCIgaXMgdG9vIGxhcmdlIGZvciBzaGlmdGluZy5cIik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIG4gPSArbjtcclxuICAgICAgICBpZiAobiA8IDApIHJldHVybiB0aGlzLnNoaWZ0TGVmdCgtbik7XHJcbiAgICAgICAgdmFyIHJlc3VsdCA9IHRoaXM7XHJcbiAgICAgICAgd2hpbGUgKG4gPj0gcG93ZXJzMkxlbmd0aCkge1xyXG4gICAgICAgICAgICBpZiAocmVzdWx0LmlzWmVybygpKSByZXR1cm4gcmVzdWx0O1xyXG4gICAgICAgICAgICByZW1RdW8gPSBkaXZNb2RBbnkocmVzdWx0LCBoaWdoZXN0UG93ZXIyKTtcclxuICAgICAgICAgICAgcmVzdWx0ID0gcmVtUXVvWzFdLmlzTmVnYXRpdmUoKSA/IHJlbVF1b1swXS5wcmV2KCkgOiByZW1RdW9bMF07XHJcbiAgICAgICAgICAgIG4gLT0gcG93ZXJzMkxlbmd0aCAtIDE7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJlbVF1byA9IGRpdk1vZEFueShyZXN1bHQsIHBvd2Vyc09mVHdvW25dKTtcclxuICAgICAgICByZXR1cm4gcmVtUXVvWzFdLmlzTmVnYXRpdmUoKSA/IHJlbVF1b1swXS5wcmV2KCkgOiByZW1RdW9bMF07XHJcbiAgICB9O1xyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS5zaGlmdFJpZ2h0ID0gQmlnSW50ZWdlci5wcm90b3R5cGUuc2hpZnRSaWdodDtcclxuXHJcbiAgICBmdW5jdGlvbiBiaXR3aXNlKHgsIHksIGZuKSB7XHJcbiAgICAgICAgeSA9IHBhcnNlVmFsdWUoeSk7XHJcbiAgICAgICAgdmFyIHhTaWduID0geC5pc05lZ2F0aXZlKCksIHlTaWduID0geS5pc05lZ2F0aXZlKCk7XHJcbiAgICAgICAgdmFyIHhSZW0gPSB4U2lnbiA/IHgubm90KCkgOiB4LFxyXG4gICAgICAgICAgICB5UmVtID0geVNpZ24gPyB5Lm5vdCgpIDogeTtcclxuICAgICAgICB2YXIgeEJpdHMgPSBbXSwgeUJpdHMgPSBbXTtcclxuICAgICAgICB2YXIgeFN0b3AgPSBmYWxzZSwgeVN0b3AgPSBmYWxzZTtcclxuICAgICAgICB3aGlsZSAoIXhTdG9wIHx8ICF5U3RvcCkge1xyXG4gICAgICAgICAgICBpZiAoeFJlbS5pc1plcm8oKSkgeyAvLyB2aXJ0dWFsIHNpZ24gZXh0ZW5zaW9uIGZvciBzaW11bGF0aW5nIHR3bydzIGNvbXBsZW1lbnRcclxuICAgICAgICAgICAgICAgIHhTdG9wID0gdHJ1ZTtcclxuICAgICAgICAgICAgICAgIHhCaXRzLnB1c2goeFNpZ24gPyAxIDogMCk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgZWxzZSBpZiAoeFNpZ24pIHhCaXRzLnB1c2goeFJlbS5pc0V2ZW4oKSA/IDEgOiAwKTsgLy8gdHdvJ3MgY29tcGxlbWVudCBmb3IgbmVnYXRpdmUgbnVtYmVyc1xyXG4gICAgICAgICAgICBlbHNlIHhCaXRzLnB1c2goeFJlbS5pc0V2ZW4oKSA/IDAgOiAxKTtcclxuXHJcbiAgICAgICAgICAgIGlmICh5UmVtLmlzWmVybygpKSB7XHJcbiAgICAgICAgICAgICAgICB5U3RvcCA9IHRydWU7XHJcbiAgICAgICAgICAgICAgICB5Qml0cy5wdXNoKHlTaWduID8gMSA6IDApO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIGVsc2UgaWYgKHlTaWduKSB5Qml0cy5wdXNoKHlSZW0uaXNFdmVuKCkgPyAxIDogMCk7XHJcbiAgICAgICAgICAgIGVsc2UgeUJpdHMucHVzaCh5UmVtLmlzRXZlbigpID8gMCA6IDEpO1xyXG5cclxuICAgICAgICAgICAgeFJlbSA9IHhSZW0ub3ZlcigyKTtcclxuICAgICAgICAgICAgeVJlbSA9IHlSZW0ub3ZlcigyKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgdmFyIHJlc3VsdCA9IFtdO1xyXG4gICAgICAgIGZvciAodmFyIGkgPSAwOyBpIDwgeEJpdHMubGVuZ3RoOyBpKyspIHJlc3VsdC5wdXNoKGZuKHhCaXRzW2ldLCB5Qml0c1tpXSkpO1xyXG4gICAgICAgIHZhciBzdW0gPSBiaWdJbnQocmVzdWx0LnBvcCgpKS5uZWdhdGUoKS50aW1lcyhiaWdJbnQoMikucG93KHJlc3VsdC5sZW5ndGgpKTtcclxuICAgICAgICB3aGlsZSAocmVzdWx0Lmxlbmd0aCkge1xyXG4gICAgICAgICAgICBzdW0gPSBzdW0uYWRkKGJpZ0ludChyZXN1bHQucG9wKCkpLnRpbWVzKGJpZ0ludCgyKS5wb3cocmVzdWx0Lmxlbmd0aCkpKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHN1bTtcclxuICAgIH1cclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5ub3QgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuIHRoaXMubmVnYXRlKCkucHJldigpO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUubm90ID0gQmlnSW50ZWdlci5wcm90b3R5cGUubm90O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLmFuZCA9IGZ1bmN0aW9uIChuKSB7XHJcbiAgICAgICAgcmV0dXJuIGJpdHdpc2UodGhpcywgbiwgZnVuY3Rpb24gKGEsIGIpIHsgcmV0dXJuIGEgJiBiOyB9KTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLmFuZCA9IEJpZ0ludGVnZXIucHJvdG90eXBlLmFuZDtcclxuXHJcbiAgICBCaWdJbnRlZ2VyLnByb3RvdHlwZS5vciA9IGZ1bmN0aW9uIChuKSB7XHJcbiAgICAgICAgcmV0dXJuIGJpdHdpc2UodGhpcywgbiwgZnVuY3Rpb24gKGEsIGIpIHsgcmV0dXJuIGEgfCBiOyB9KTtcclxuICAgIH07XHJcbiAgICBTbWFsbEludGVnZXIucHJvdG90eXBlLm9yID0gQmlnSW50ZWdlci5wcm90b3R5cGUub3I7XHJcblxyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUueG9yID0gZnVuY3Rpb24gKG4pIHtcclxuICAgICAgICByZXR1cm4gYml0d2lzZSh0aGlzLCBuLCBmdW5jdGlvbiAoYSwgYikgeyByZXR1cm4gYSBeIGI7IH0pO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUueG9yID0gQmlnSW50ZWdlci5wcm90b3R5cGUueG9yO1xyXG5cclxuICAgIHZhciBMT0JNQVNLX0kgPSAxIDw8IDMwLCBMT0JNQVNLX0JJID0gKEJBU0UgJiAtQkFTRSkgKiAoQkFTRSAmIC1CQVNFKSB8IExPQk1BU0tfSTtcclxuICAgIGZ1bmN0aW9uIHJvdWdoTE9CKG4pIHsgLy8gZ2V0IGxvd2VzdE9uZUJpdCAocm91Z2gpXHJcbiAgICAgICAgLy8gU21hbGxJbnRlZ2VyOiByZXR1cm4gTWluKGxvd2VzdE9uZUJpdChuKSwgMSA8PCAzMClcclxuICAgICAgICAvLyBCaWdJbnRlZ2VyOiByZXR1cm4gTWluKGxvd2VzdE9uZUJpdChuKSwgMSA8PCAxNCkgW0JBU0U9MWU3XVxyXG4gICAgICAgIHZhciB2ID0gbi52YWx1ZSwgeCA9IHR5cGVvZiB2ID09PSBcIm51bWJlclwiID8gdiB8IExPQk1BU0tfSSA6IHZbMF0gKyB2WzFdICogQkFTRSB8IExPQk1BU0tfQkk7XHJcbiAgICAgICAgcmV0dXJuIHggJiAteDtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBtYXgoYSwgYikge1xyXG4gICAgICAgIGEgPSBwYXJzZVZhbHVlKGEpO1xyXG4gICAgICAgIGIgPSBwYXJzZVZhbHVlKGIpO1xyXG4gICAgICAgIHJldHVybiBhLmdyZWF0ZXIoYikgPyBhIDogYjtcclxuICAgIH1cclxuICAgIGZ1bmN0aW9uIG1pbihhLGIpIHtcclxuICAgICAgICBhID0gcGFyc2VWYWx1ZShhKTtcclxuICAgICAgICBiID0gcGFyc2VWYWx1ZShiKTtcclxuICAgICAgICByZXR1cm4gYS5sZXNzZXIoYikgPyBhIDogYjtcclxuICAgIH1cclxuICAgIGZ1bmN0aW9uIGdjZChhLCBiKSB7XHJcbiAgICAgICAgYSA9IHBhcnNlVmFsdWUoYSkuYWJzKCk7XHJcbiAgICAgICAgYiA9IHBhcnNlVmFsdWUoYikuYWJzKCk7XHJcbiAgICAgICAgaWYgKGEuZXF1YWxzKGIpKSByZXR1cm4gYTtcclxuICAgICAgICBpZiAoYS5pc1plcm8oKSkgcmV0dXJuIGI7XHJcbiAgICAgICAgaWYgKGIuaXNaZXJvKCkpIHJldHVybiBhO1xyXG4gICAgICAgIHZhciBjID0gSW50ZWdlclsxXSwgZCwgdDtcclxuICAgICAgICB3aGlsZSAoYS5pc0V2ZW4oKSAmJiBiLmlzRXZlbigpKSB7XHJcbiAgICAgICAgICAgIGQgPSBNYXRoLm1pbihyb3VnaExPQihhKSwgcm91Z2hMT0IoYikpO1xyXG4gICAgICAgICAgICBhID0gYS5kaXZpZGUoZCk7XHJcbiAgICAgICAgICAgIGIgPSBiLmRpdmlkZShkKTtcclxuICAgICAgICAgICAgYyA9IGMubXVsdGlwbHkoZCk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHdoaWxlIChhLmlzRXZlbigpKSB7XHJcbiAgICAgICAgICAgIGEgPSBhLmRpdmlkZShyb3VnaExPQihhKSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGRvIHtcclxuICAgICAgICAgICAgd2hpbGUgKGIuaXNFdmVuKCkpIHtcclxuICAgICAgICAgICAgICAgIGIgPSBiLmRpdmlkZShyb3VnaExPQihiKSk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgaWYgKGEuZ3JlYXRlcihiKSkge1xyXG4gICAgICAgICAgICAgICAgdCA9IGI7IGIgPSBhOyBhID0gdDtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICBiID0gYi5zdWJ0cmFjdChhKTtcclxuICAgICAgICB9IHdoaWxlICghYi5pc1plcm8oKSk7XHJcbiAgICAgICAgcmV0dXJuIGMuaXNVbml0KCkgPyBhIDogYS5tdWx0aXBseShjKTtcclxuICAgIH1cclxuICAgIGZ1bmN0aW9uIGxjbShhLCBiKSB7XHJcbiAgICAgICAgYSA9IHBhcnNlVmFsdWUoYSkuYWJzKCk7XHJcbiAgICAgICAgYiA9IHBhcnNlVmFsdWUoYikuYWJzKCk7XHJcbiAgICAgICAgcmV0dXJuIGEuZGl2aWRlKGdjZChhLCBiKSkubXVsdGlwbHkoYik7XHJcbiAgICB9XHJcbiAgICBmdW5jdGlvbiByYW5kQmV0d2VlbihhLCBiKSB7XHJcbiAgICAgICAgYSA9IHBhcnNlVmFsdWUoYSk7XHJcbiAgICAgICAgYiA9IHBhcnNlVmFsdWUoYik7XHJcbiAgICAgICAgdmFyIGxvdyA9IG1pbihhLCBiKSwgaGlnaCA9IG1heChhLCBiKTtcclxuICAgICAgICB2YXIgcmFuZ2UgPSBoaWdoLnN1YnRyYWN0KGxvdyk7XHJcbiAgICAgICAgaWYgKHJhbmdlLmlzU21hbGwpIHJldHVybiBsb3cuYWRkKE1hdGgucm91bmQoTWF0aC5yYW5kb20oKSAqIHJhbmdlKSk7XHJcbiAgICAgICAgdmFyIGxlbmd0aCA9IHJhbmdlLnZhbHVlLmxlbmd0aCAtIDE7XHJcbiAgICAgICAgdmFyIHJlc3VsdCA9IFtdLCByZXN0cmljdGVkID0gdHJ1ZTtcclxuICAgICAgICBmb3IgKHZhciBpID0gbGVuZ3RoOyBpID49IDA7IGktLSkge1xyXG4gICAgICAgICAgICB2YXIgdG9wID0gcmVzdHJpY3RlZCA/IHJhbmdlLnZhbHVlW2ldIDogQkFTRTtcclxuICAgICAgICAgICAgdmFyIGRpZ2l0ID0gdHJ1bmNhdGUoTWF0aC5yYW5kb20oKSAqIHRvcCk7XHJcbiAgICAgICAgICAgIHJlc3VsdC51bnNoaWZ0KGRpZ2l0KTtcclxuICAgICAgICAgICAgaWYgKGRpZ2l0IDwgdG9wKSByZXN0cmljdGVkID0gZmFsc2U7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJlc3VsdCA9IGFycmF5VG9TbWFsbChyZXN1bHQpO1xyXG4gICAgICAgIHJldHVybiBsb3cuYWRkKHR5cGVvZiByZXN1bHQgPT09IFwibnVtYmVyXCIgPyBuZXcgU21hbGxJbnRlZ2VyKHJlc3VsdCkgOiBuZXcgQmlnSW50ZWdlcihyZXN1bHQsIGZhbHNlKSk7XHJcbiAgICB9XHJcbiAgICB2YXIgcGFyc2VCYXNlID0gZnVuY3Rpb24gKHRleHQsIGJhc2UpIHtcclxuICAgICAgICB2YXIgdmFsID0gSW50ZWdlclswXSwgcG93ID0gSW50ZWdlclsxXSxcclxuICAgICAgICAgICAgbGVuZ3RoID0gdGV4dC5sZW5ndGg7XHJcbiAgICAgICAgaWYgKDIgPD0gYmFzZSAmJiBiYXNlIDw9IDM2KSB7XHJcbiAgICAgICAgICAgIGlmIChsZW5ndGggPD0gTE9HX01BWF9JTlQgLyBNYXRoLmxvZyhiYXNlKSkge1xyXG4gICAgICAgICAgICAgICAgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIocGFyc2VJbnQodGV4dCwgYmFzZSkpO1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGJhc2UgPSBwYXJzZVZhbHVlKGJhc2UpO1xyXG4gICAgICAgIHZhciBkaWdpdHMgPSBbXTtcclxuICAgICAgICB2YXIgaTtcclxuICAgICAgICB2YXIgaXNOZWdhdGl2ZSA9IHRleHRbMF0gPT09IFwiLVwiO1xyXG4gICAgICAgIGZvciAoaSA9IGlzTmVnYXRpdmUgPyAxIDogMDsgaSA8IHRleHQubGVuZ3RoOyBpKyspIHtcclxuICAgICAgICAgICAgdmFyIGMgPSB0ZXh0W2ldLnRvTG93ZXJDYXNlKCksXHJcbiAgICAgICAgICAgICAgICBjaGFyQ29kZSA9IGMuY2hhckNvZGVBdCgwKTtcclxuICAgICAgICAgICAgaWYgKDQ4IDw9IGNoYXJDb2RlICYmIGNoYXJDb2RlIDw9IDU3KSBkaWdpdHMucHVzaChwYXJzZVZhbHVlKGMpKTtcclxuICAgICAgICAgICAgZWxzZSBpZiAoOTcgPD0gY2hhckNvZGUgJiYgY2hhckNvZGUgPD0gMTIyKSBkaWdpdHMucHVzaChwYXJzZVZhbHVlKGMuY2hhckNvZGVBdCgwKSAtIDg3KSk7XHJcbiAgICAgICAgICAgIGVsc2UgaWYgKGMgPT09IFwiPFwiKSB7XHJcbiAgICAgICAgICAgICAgICB2YXIgc3RhcnQgPSBpO1xyXG4gICAgICAgICAgICAgICAgZG8geyBpKys7IH0gd2hpbGUgKHRleHRbaV0gIT09IFwiPlwiKTtcclxuICAgICAgICAgICAgICAgIGRpZ2l0cy5wdXNoKHBhcnNlVmFsdWUodGV4dC5zbGljZShzdGFydCArIDEsIGkpKSk7XHJcbiAgICAgICAgICAgIH1cclxuICAgICAgICAgICAgZWxzZSB0aHJvdyBuZXcgRXJyb3IoYyArIFwiIGlzIG5vdCBhIHZhbGlkIGNoYXJhY3RlclwiKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgZGlnaXRzLnJldmVyc2UoKTtcclxuICAgICAgICBmb3IgKGkgPSAwOyBpIDwgZGlnaXRzLmxlbmd0aDsgaSsrKSB7XHJcbiAgICAgICAgICAgIHZhbCA9IHZhbC5hZGQoZGlnaXRzW2ldLnRpbWVzKHBvdykpO1xyXG4gICAgICAgICAgICBwb3cgPSBwb3cudGltZXMoYmFzZSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBpc05lZ2F0aXZlID8gdmFsLm5lZ2F0ZSgpIDogdmFsO1xyXG4gICAgfTtcclxuXHJcbiAgICBmdW5jdGlvbiBzdHJpbmdpZnkoZGlnaXQpIHtcclxuICAgICAgICB2YXIgdiA9IGRpZ2l0LnZhbHVlO1xyXG4gICAgICAgIGlmICh0eXBlb2YgdiA9PT0gXCJudW1iZXJcIikgdiA9IFt2XTtcclxuICAgICAgICBpZiAodi5sZW5ndGggPT09IDEgJiYgdlswXSA8PSAzNSkge1xyXG4gICAgICAgICAgICByZXR1cm4gXCIwMTIzNDU2Nzg5YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXpcIi5jaGFyQXQodlswXSk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHJldHVybiBcIjxcIiArIHYgKyBcIj5cIjtcclxuICAgIH1cclxuICAgIGZ1bmN0aW9uIHRvQmFzZShuLCBiYXNlKSB7XHJcbiAgICAgICAgYmFzZSA9IGJpZ0ludChiYXNlKTtcclxuICAgICAgICBpZiAoYmFzZS5pc1plcm8oKSkge1xyXG4gICAgICAgICAgICBpZiAobi5pc1plcm8oKSkgcmV0dXJuIFwiMFwiO1xyXG4gICAgICAgICAgICB0aHJvdyBuZXcgRXJyb3IoXCJDYW5ub3QgY29udmVydCBub256ZXJvIG51bWJlcnMgdG8gYmFzZSAwLlwiKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgaWYgKGJhc2UuZXF1YWxzKC0xKSkge1xyXG4gICAgICAgICAgICBpZiAobi5pc1plcm8oKSkgcmV0dXJuIFwiMFwiO1xyXG4gICAgICAgICAgICBpZiAobi5pc05lZ2F0aXZlKCkpIHJldHVybiBuZXcgQXJyYXkoMSAtIG4pLmpvaW4oXCIxMFwiKTtcclxuICAgICAgICAgICAgcmV0dXJuIFwiMVwiICsgbmV3IEFycmF5KCtuKS5qb2luKFwiMDFcIik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHZhciBtaW51c1NpZ24gPSBcIlwiO1xyXG4gICAgICAgIGlmIChuLmlzTmVnYXRpdmUoKSAmJiBiYXNlLmlzUG9zaXRpdmUoKSkge1xyXG4gICAgICAgICAgICBtaW51c1NpZ24gPSBcIi1cIjtcclxuICAgICAgICAgICAgbiA9IG4uYWJzKCk7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGlmIChiYXNlLmVxdWFscygxKSkge1xyXG4gICAgICAgICAgICBpZiAobi5pc1plcm8oKSkgcmV0dXJuIFwiMFwiO1xyXG4gICAgICAgICAgICByZXR1cm4gbWludXNTaWduICsgbmV3IEFycmF5KCtuICsgMSkuam9pbigxKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgdmFyIG91dCA9IFtdO1xyXG4gICAgICAgIHZhciBsZWZ0ID0gbiwgZGl2bW9kO1xyXG4gICAgICAgIHdoaWxlIChsZWZ0LmlzTmVnYXRpdmUoKSB8fCBsZWZ0LmNvbXBhcmVBYnMoYmFzZSkgPj0gMCkge1xyXG4gICAgICAgICAgICBkaXZtb2QgPSBsZWZ0LmRpdm1vZChiYXNlKTtcclxuICAgICAgICAgICAgbGVmdCA9IGRpdm1vZC5xdW90aWVudDtcclxuICAgICAgICAgICAgdmFyIGRpZ2l0ID0gZGl2bW9kLnJlbWFpbmRlcjtcclxuICAgICAgICAgICAgaWYgKGRpZ2l0LmlzTmVnYXRpdmUoKSkge1xyXG4gICAgICAgICAgICAgICAgZGlnaXQgPSBiYXNlLm1pbnVzKGRpZ2l0KS5hYnMoKTtcclxuICAgICAgICAgICAgICAgIGxlZnQgPSBsZWZ0Lm5leHQoKTtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICBvdXQucHVzaChzdHJpbmdpZnkoZGlnaXQpKTtcclxuICAgICAgICB9XHJcbiAgICAgICAgb3V0LnB1c2goc3RyaW5naWZ5KGxlZnQpKTtcclxuICAgICAgICByZXR1cm4gbWludXNTaWduICsgb3V0LnJldmVyc2UoKS5qb2luKFwiXCIpO1xyXG4gICAgfVxyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLnRvU3RyaW5nID0gZnVuY3Rpb24gKHJhZGl4KSB7XHJcbiAgICAgICAgaWYgKHJhZGl4ID09PSB1bmRlZmluZWQpIHJhZGl4ID0gMTA7XHJcbiAgICAgICAgaWYgKHJhZGl4ICE9PSAxMCkgcmV0dXJuIHRvQmFzZSh0aGlzLCByYWRpeCk7XHJcbiAgICAgICAgdmFyIHYgPSB0aGlzLnZhbHVlLCBsID0gdi5sZW5ndGgsIHN0ciA9IFN0cmluZyh2Wy0tbF0pLCB6ZXJvcyA9IFwiMDAwMDAwMFwiLCBkaWdpdDtcclxuICAgICAgICB3aGlsZSAoLS1sID49IDApIHtcclxuICAgICAgICAgICAgZGlnaXQgPSBTdHJpbmcodltsXSk7XHJcbiAgICAgICAgICAgIHN0ciArPSB6ZXJvcy5zbGljZShkaWdpdC5sZW5ndGgpICsgZGlnaXQ7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIHZhciBzaWduID0gdGhpcy5zaWduID8gXCItXCIgOiBcIlwiO1xyXG4gICAgICAgIHJldHVybiBzaWduICsgc3RyO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUudG9TdHJpbmcgPSBmdW5jdGlvbiAocmFkaXgpIHtcclxuICAgICAgICBpZiAocmFkaXggPT09IHVuZGVmaW5lZCkgcmFkaXggPSAxMDtcclxuICAgICAgICBpZiAocmFkaXggIT0gMTApIHJldHVybiB0b0Jhc2UodGhpcywgcmFkaXgpO1xyXG4gICAgICAgIHJldHVybiBTdHJpbmcodGhpcy52YWx1ZSk7XHJcbiAgICB9O1xyXG5cclxuICAgIEJpZ0ludGVnZXIucHJvdG90eXBlLnZhbHVlT2YgPSBmdW5jdGlvbiAoKSB7XHJcbiAgICAgICAgcmV0dXJuICt0aGlzLnRvU3RyaW5nKCk7XHJcbiAgICB9O1xyXG4gICAgQmlnSW50ZWdlci5wcm90b3R5cGUudG9KU051bWJlciA9IEJpZ0ludGVnZXIucHJvdG90eXBlLnZhbHVlT2Y7XHJcblxyXG4gICAgU21hbGxJbnRlZ2VyLnByb3RvdHlwZS52YWx1ZU9mID0gZnVuY3Rpb24gKCkge1xyXG4gICAgICAgIHJldHVybiB0aGlzLnZhbHVlO1xyXG4gICAgfTtcclxuICAgIFNtYWxsSW50ZWdlci5wcm90b3R5cGUudG9KU051bWJlciA9IFNtYWxsSW50ZWdlci5wcm90b3R5cGUudmFsdWVPZjtcclxuXHJcbiAgICBmdW5jdGlvbiBwYXJzZVN0cmluZ1ZhbHVlKHYpIHtcclxuICAgICAgICAgICAgaWYgKGlzUHJlY2lzZSgrdikpIHtcclxuICAgICAgICAgICAgICAgIHZhciB4ID0gK3Y7XHJcbiAgICAgICAgICAgICAgICBpZiAoeCA9PT0gdHJ1bmNhdGUoeCkpXHJcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG5ldyBTbWFsbEludGVnZXIoeCk7XHJcbiAgICAgICAgICAgICAgICB0aHJvdyBcIkludmFsaWQgaW50ZWdlcjogXCIgKyB2O1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIHZhciBzaWduID0gdlswXSA9PT0gXCItXCI7XHJcbiAgICAgICAgICAgIGlmIChzaWduKSB2ID0gdi5zbGljZSgxKTtcclxuICAgICAgICAgICAgdmFyIHNwbGl0ID0gdi5zcGxpdCgvZS9pKTtcclxuICAgICAgICAgICAgaWYgKHNwbGl0Lmxlbmd0aCA+IDIpIHRocm93IG5ldyBFcnJvcihcIkludmFsaWQgaW50ZWdlcjogXCIgKyBzcGxpdC5qb2luKFwiZVwiKSk7XHJcbiAgICAgICAgICAgIGlmIChzcGxpdC5sZW5ndGggPT09IDIpIHtcclxuICAgICAgICAgICAgICAgIHZhciBleHAgPSBzcGxpdFsxXTtcclxuICAgICAgICAgICAgICAgIGlmIChleHBbMF0gPT09IFwiK1wiKSBleHAgPSBleHAuc2xpY2UoMSk7XHJcbiAgICAgICAgICAgICAgICBleHAgPSArZXhwO1xyXG4gICAgICAgICAgICAgICAgaWYgKGV4cCAhPT0gdHJ1bmNhdGUoZXhwKSB8fCAhaXNQcmVjaXNlKGV4cCkpIHRocm93IG5ldyBFcnJvcihcIkludmFsaWQgaW50ZWdlcjogXCIgKyBleHAgKyBcIiBpcyBub3QgYSB2YWxpZCBleHBvbmVudC5cIik7XHJcbiAgICAgICAgICAgICAgICB2YXIgdGV4dCA9IHNwbGl0WzBdO1xyXG4gICAgICAgICAgICAgICAgdmFyIGRlY2ltYWxQbGFjZSA9IHRleHQuaW5kZXhPZihcIi5cIik7XHJcbiAgICAgICAgICAgICAgICBpZiAoZGVjaW1hbFBsYWNlID49IDApIHtcclxuICAgICAgICAgICAgICAgICAgICBleHAgLT0gdGV4dC5sZW5ndGggLSBkZWNpbWFsUGxhY2UgLSAxO1xyXG4gICAgICAgICAgICAgICAgICAgIHRleHQgPSB0ZXh0LnNsaWNlKDAsIGRlY2ltYWxQbGFjZSkgKyB0ZXh0LnNsaWNlKGRlY2ltYWxQbGFjZSArIDEpO1xyXG4gICAgICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICAgICAgaWYgKGV4cCA8IDApIHRocm93IG5ldyBFcnJvcihcIkNhbm5vdCBpbmNsdWRlIG5lZ2F0aXZlIGV4cG9uZW50IHBhcnQgZm9yIGludGVnZXJzXCIpO1xyXG4gICAgICAgICAgICAgICAgdGV4dCArPSAobmV3IEFycmF5KGV4cCArIDEpKS5qb2luKFwiMFwiKTtcclxuICAgICAgICAgICAgICAgIHYgPSB0ZXh0O1xyXG4gICAgICAgICAgICB9XHJcbiAgICAgICAgICAgIHZhciBpc1ZhbGlkID0gL14oWzAtOV1bMC05XSopJC8udGVzdCh2KTtcclxuICAgICAgICAgICAgaWYgKCFpc1ZhbGlkKSB0aHJvdyBuZXcgRXJyb3IoXCJJbnZhbGlkIGludGVnZXI6IFwiICsgdik7XHJcbiAgICAgICAgICAgIHZhciByID0gW10sIG1heCA9IHYubGVuZ3RoLCBsID0gTE9HX0JBU0UsIG1pbiA9IG1heCAtIGw7XHJcbiAgICAgICAgICAgIHdoaWxlIChtYXggPiAwKSB7XHJcbiAgICAgICAgICAgICAgICByLnB1c2goK3Yuc2xpY2UobWluLCBtYXgpKTtcclxuICAgICAgICAgICAgICAgIG1pbiAtPSBsO1xyXG4gICAgICAgICAgICAgICAgaWYgKG1pbiA8IDApIG1pbiA9IDA7XHJcbiAgICAgICAgICAgICAgICBtYXggLT0gbDtcclxuICAgICAgICAgICAgfVxyXG4gICAgICAgICAgICB0cmltKHIpO1xyXG4gICAgICAgICAgICByZXR1cm4gbmV3IEJpZ0ludGVnZXIociwgc2lnbik7XHJcbiAgICB9XHJcblxyXG4gICAgZnVuY3Rpb24gcGFyc2VOdW1iZXJWYWx1ZSh2KSB7XHJcbiAgICAgICAgaWYgKGlzUHJlY2lzZSh2KSkge1xyXG4gICAgICAgICAgICBpZiAodiAhPT0gdHJ1bmNhdGUodikpIHRocm93IG5ldyBFcnJvcih2ICsgXCIgaXMgbm90IGFuIGludGVnZXIuXCIpO1xyXG4gICAgICAgICAgICByZXR1cm4gbmV3IFNtYWxsSW50ZWdlcih2KTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHBhcnNlU3RyaW5nVmFsdWUodi50b1N0cmluZygpKTtcclxuICAgIH1cclxuXHJcbiAgICBmdW5jdGlvbiBwYXJzZVZhbHVlKHYpIHtcclxuICAgICAgICBpZiAodHlwZW9mIHYgPT09IFwibnVtYmVyXCIpIHtcclxuICAgICAgICAgICAgcmV0dXJuIHBhcnNlTnVtYmVyVmFsdWUodik7XHJcbiAgICAgICAgfVxyXG4gICAgICAgIGlmICh0eXBlb2YgdiA9PT0gXCJzdHJpbmdcIikge1xyXG4gICAgICAgICAgICByZXR1cm4gcGFyc2VTdHJpbmdWYWx1ZSh2KTtcclxuICAgICAgICB9XHJcbiAgICAgICAgcmV0dXJuIHY7XHJcbiAgICB9XHJcbiAgICAvLyBQcmUtZGVmaW5lIG51bWJlcnMgaW4gcmFuZ2UgWy05OTksOTk5XVxyXG4gICAgZm9yICh2YXIgaSA9IDA7IGkgPCAxMDAwOyBpKyspIHtcclxuICAgICAgICBJbnRlZ2VyW2ldID0gbmV3IFNtYWxsSW50ZWdlcihpKTtcclxuICAgICAgICBpZiAoaSA+IDApIEludGVnZXJbLWldID0gbmV3IFNtYWxsSW50ZWdlcigtaSk7XHJcbiAgICB9XHJcbiAgICAvLyBCYWNrd2FyZHMgY29tcGF0aWJpbGl0eVxyXG4gICAgSW50ZWdlci5vbmUgPSBJbnRlZ2VyWzFdO1xyXG4gICAgSW50ZWdlci56ZXJvID0gSW50ZWdlclswXTtcclxuICAgIEludGVnZXIubWludXNPbmUgPSBJbnRlZ2VyWy0xXTtcclxuICAgIEludGVnZXIubWF4ID0gbWF4O1xyXG4gICAgSW50ZWdlci5taW4gPSBtaW47XHJcbiAgICBJbnRlZ2VyLmdjZCA9IGdjZDtcclxuICAgIEludGVnZXIubGNtID0gbGNtO1xyXG4gICAgSW50ZWdlci5pc0luc3RhbmNlID0gZnVuY3Rpb24gKHgpIHsgcmV0dXJuIHggaW5zdGFuY2VvZiBCaWdJbnRlZ2VyIHx8IHggaW5zdGFuY2VvZiBTbWFsbEludGVnZXI7IH07XHJcbiAgICBJbnRlZ2VyLnJhbmRCZXR3ZWVuID0gcmFuZEJldHdlZW47XHJcbiAgICByZXR1cm4gSW50ZWdlcjtcclxufSkoKTtcclxuXHJcbi8vIE5vZGUuanMgY2hlY2tcclxuaWYgKHR5cGVvZiBtb2R1bGUgIT09IFwidW5kZWZpbmVkXCIgJiYgbW9kdWxlLmhhc093blByb3BlcnR5KFwiZXhwb3J0c1wiKSkge1xyXG4gICAgbW9kdWxlLmV4cG9ydHMgPSBiaWdJbnQ7XHJcbn1cclxuIiwiIiwiLyohXG4gKiBUaGUgYnVmZmVyIG1vZHVsZSBmcm9tIG5vZGUuanMsIGZvciB0aGUgYnJvd3Nlci5cbiAqXG4gKiBAYXV0aG9yICAgRmVyb3NzIEFib3VraGFkaWplaCA8ZmVyb3NzQGZlcm9zcy5vcmc+IDxodHRwOi8vZmVyb3NzLm9yZz5cbiAqIEBsaWNlbnNlICBNSVRcbiAqL1xuLyogZXNsaW50LWRpc2FibGUgbm8tcHJvdG8gKi9cblxuJ3VzZSBzdHJpY3QnXG5cbnZhciBiYXNlNjQgPSByZXF1aXJlKCdiYXNlNjQtanMnKVxudmFyIGllZWU3NTQgPSByZXF1aXJlKCdpZWVlNzU0JylcbnZhciBpc0FycmF5ID0gcmVxdWlyZSgnaXNhcnJheScpXG5cbmV4cG9ydHMuQnVmZmVyID0gQnVmZmVyXG5leHBvcnRzLlNsb3dCdWZmZXIgPSBTbG93QnVmZmVyXG5leHBvcnRzLklOU1BFQ1RfTUFYX0JZVEVTID0gNTBcblxuLyoqXG4gKiBJZiBgQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlRgOlxuICogICA9PT0gdHJ1ZSAgICBVc2UgVWludDhBcnJheSBpbXBsZW1lbnRhdGlvbiAoZmFzdGVzdClcbiAqICAgPT09IGZhbHNlICAgVXNlIE9iamVjdCBpbXBsZW1lbnRhdGlvbiAobW9zdCBjb21wYXRpYmxlLCBldmVuIElFNilcbiAqXG4gKiBCcm93c2VycyB0aGF0IHN1cHBvcnQgdHlwZWQgYXJyYXlzIGFyZSBJRSAxMCssIEZpcmVmb3ggNCssIENocm9tZSA3KywgU2FmYXJpIDUuMSssXG4gKiBPcGVyYSAxMS42KywgaU9TIDQuMisuXG4gKlxuICogRHVlIHRvIHZhcmlvdXMgYnJvd3NlciBidWdzLCBzb21ldGltZXMgdGhlIE9iamVjdCBpbXBsZW1lbnRhdGlvbiB3aWxsIGJlIHVzZWQgZXZlblxuICogd2hlbiB0aGUgYnJvd3NlciBzdXBwb3J0cyB0eXBlZCBhcnJheXMuXG4gKlxuICogTm90ZTpcbiAqXG4gKiAgIC0gRmlyZWZveCA0LTI5IGxhY2tzIHN1cHBvcnQgZm9yIGFkZGluZyBuZXcgcHJvcGVydGllcyB0byBgVWludDhBcnJheWAgaW5zdGFuY2VzLFxuICogICAgIFNlZTogaHR0cHM6Ly9idWd6aWxsYS5tb3ppbGxhLm9yZy9zaG93X2J1Zy5jZ2k/aWQ9Njk1NDM4LlxuICpcbiAqICAgLSBDaHJvbWUgOS0xMCBpcyBtaXNzaW5nIHRoZSBgVHlwZWRBcnJheS5wcm90b3R5cGUuc3ViYXJyYXlgIGZ1bmN0aW9uLlxuICpcbiAqICAgLSBJRTEwIGhhcyBhIGJyb2tlbiBgVHlwZWRBcnJheS5wcm90b3R5cGUuc3ViYXJyYXlgIGZ1bmN0aW9uIHdoaWNoIHJldHVybnMgYXJyYXlzIG9mXG4gKiAgICAgaW5jb3JyZWN0IGxlbmd0aCBpbiBzb21lIHNpdHVhdGlvbnMuXG5cbiAqIFdlIGRldGVjdCB0aGVzZSBidWdneSBicm93c2VycyBhbmQgc2V0IGBCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVGAgdG8gYGZhbHNlYCBzbyB0aGV5XG4gKiBnZXQgdGhlIE9iamVjdCBpbXBsZW1lbnRhdGlvbiwgd2hpY2ggaXMgc2xvd2VyIGJ1dCBiZWhhdmVzIGNvcnJlY3RseS5cbiAqL1xuQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQgPSBnbG9iYWwuVFlQRURfQVJSQVlfU1VQUE9SVCAhPT0gdW5kZWZpbmVkXG4gID8gZ2xvYmFsLlRZUEVEX0FSUkFZX1NVUFBPUlRcbiAgOiB0eXBlZEFycmF5U3VwcG9ydCgpXG5cbi8qXG4gKiBFeHBvcnQga01heExlbmd0aCBhZnRlciB0eXBlZCBhcnJheSBzdXBwb3J0IGlzIGRldGVybWluZWQuXG4gKi9cbmV4cG9ydHMua01heExlbmd0aCA9IGtNYXhMZW5ndGgoKVxuXG5mdW5jdGlvbiB0eXBlZEFycmF5U3VwcG9ydCAoKSB7XG4gIHRyeSB7XG4gICAgdmFyIGFyciA9IG5ldyBVaW50OEFycmF5KDEpXG4gICAgYXJyLl9fcHJvdG9fXyA9IHtfX3Byb3RvX186IFVpbnQ4QXJyYXkucHJvdG90eXBlLCBmb286IGZ1bmN0aW9uICgpIHsgcmV0dXJuIDQyIH19XG4gICAgcmV0dXJuIGFyci5mb28oKSA9PT0gNDIgJiYgLy8gdHlwZWQgYXJyYXkgaW5zdGFuY2VzIGNhbiBiZSBhdWdtZW50ZWRcbiAgICAgICAgdHlwZW9mIGFyci5zdWJhcnJheSA9PT0gJ2Z1bmN0aW9uJyAmJiAvLyBjaHJvbWUgOS0xMCBsYWNrIGBzdWJhcnJheWBcbiAgICAgICAgYXJyLnN1YmFycmF5KDEsIDEpLmJ5dGVMZW5ndGggPT09IDAgLy8gaWUxMCBoYXMgYnJva2VuIGBzdWJhcnJheWBcbiAgfSBjYXRjaCAoZSkge1xuICAgIHJldHVybiBmYWxzZVxuICB9XG59XG5cbmZ1bmN0aW9uIGtNYXhMZW5ndGggKCkge1xuICByZXR1cm4gQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlRcbiAgICA/IDB4N2ZmZmZmZmZcbiAgICA6IDB4M2ZmZmZmZmZcbn1cblxuZnVuY3Rpb24gY3JlYXRlQnVmZmVyICh0aGF0LCBsZW5ndGgpIHtcbiAgaWYgKGtNYXhMZW5ndGgoKSA8IGxlbmd0aCkge1xuICAgIHRocm93IG5ldyBSYW5nZUVycm9yKCdJbnZhbGlkIHR5cGVkIGFycmF5IGxlbmd0aCcpXG4gIH1cbiAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgLy8gUmV0dXJuIGFuIGF1Z21lbnRlZCBgVWludDhBcnJheWAgaW5zdGFuY2UsIGZvciBiZXN0IHBlcmZvcm1hbmNlXG4gICAgdGhhdCA9IG5ldyBVaW50OEFycmF5KGxlbmd0aClcbiAgICB0aGF0Ll9fcHJvdG9fXyA9IEJ1ZmZlci5wcm90b3R5cGVcbiAgfSBlbHNlIHtcbiAgICAvLyBGYWxsYmFjazogUmV0dXJuIGFuIG9iamVjdCBpbnN0YW5jZSBvZiB0aGUgQnVmZmVyIGNsYXNzXG4gICAgaWYgKHRoYXQgPT09IG51bGwpIHtcbiAgICAgIHRoYXQgPSBuZXcgQnVmZmVyKGxlbmd0aClcbiAgICB9XG4gICAgdGhhdC5sZW5ndGggPSBsZW5ndGhcbiAgfVxuXG4gIHJldHVybiB0aGF0XG59XG5cbi8qKlxuICogVGhlIEJ1ZmZlciBjb25zdHJ1Y3RvciByZXR1cm5zIGluc3RhbmNlcyBvZiBgVWludDhBcnJheWAgdGhhdCBoYXZlIHRoZWlyXG4gKiBwcm90b3R5cGUgY2hhbmdlZCB0byBgQnVmZmVyLnByb3RvdHlwZWAuIEZ1cnRoZXJtb3JlLCBgQnVmZmVyYCBpcyBhIHN1YmNsYXNzIG9mXG4gKiBgVWludDhBcnJheWAsIHNvIHRoZSByZXR1cm5lZCBpbnN0YW5jZXMgd2lsbCBoYXZlIGFsbCB0aGUgbm9kZSBgQnVmZmVyYCBtZXRob2RzXG4gKiBhbmQgdGhlIGBVaW50OEFycmF5YCBtZXRob2RzLiBTcXVhcmUgYnJhY2tldCBub3RhdGlvbiB3b3JrcyBhcyBleHBlY3RlZCAtLSBpdFxuICogcmV0dXJucyBhIHNpbmdsZSBvY3RldC5cbiAqXG4gKiBUaGUgYFVpbnQ4QXJyYXlgIHByb3RvdHlwZSByZW1haW5zIHVubW9kaWZpZWQuXG4gKi9cblxuZnVuY3Rpb24gQnVmZmVyIChhcmcsIGVuY29kaW5nT3JPZmZzZXQsIGxlbmd0aCkge1xuICBpZiAoIUJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUICYmICEodGhpcyBpbnN0YW5jZW9mIEJ1ZmZlcikpIHtcbiAgICByZXR1cm4gbmV3IEJ1ZmZlcihhcmcsIGVuY29kaW5nT3JPZmZzZXQsIGxlbmd0aClcbiAgfVxuXG4gIC8vIENvbW1vbiBjYXNlLlxuICBpZiAodHlwZW9mIGFyZyA9PT0gJ251bWJlcicpIHtcbiAgICBpZiAodHlwZW9mIGVuY29kaW5nT3JPZmZzZXQgPT09ICdzdHJpbmcnKSB7XG4gICAgICB0aHJvdyBuZXcgRXJyb3IoXG4gICAgICAgICdJZiBlbmNvZGluZyBpcyBzcGVjaWZpZWQgdGhlbiB0aGUgZmlyc3QgYXJndW1lbnQgbXVzdCBiZSBhIHN0cmluZydcbiAgICAgIClcbiAgICB9XG4gICAgcmV0dXJuIGFsbG9jVW5zYWZlKHRoaXMsIGFyZylcbiAgfVxuICByZXR1cm4gZnJvbSh0aGlzLCBhcmcsIGVuY29kaW5nT3JPZmZzZXQsIGxlbmd0aClcbn1cblxuQnVmZmVyLnBvb2xTaXplID0gODE5MiAvLyBub3QgdXNlZCBieSB0aGlzIGltcGxlbWVudGF0aW9uXG5cbi8vIFRPRE86IExlZ2FjeSwgbm90IG5lZWRlZCBhbnltb3JlLiBSZW1vdmUgaW4gbmV4dCBtYWpvciB2ZXJzaW9uLlxuQnVmZmVyLl9hdWdtZW50ID0gZnVuY3Rpb24gKGFycikge1xuICBhcnIuX19wcm90b19fID0gQnVmZmVyLnByb3RvdHlwZVxuICByZXR1cm4gYXJyXG59XG5cbmZ1bmN0aW9uIGZyb20gKHRoYXQsIHZhbHVlLCBlbmNvZGluZ09yT2Zmc2V0LCBsZW5ndGgpIHtcbiAgaWYgKHR5cGVvZiB2YWx1ZSA9PT0gJ251bWJlcicpIHtcbiAgICB0aHJvdyBuZXcgVHlwZUVycm9yKCdcInZhbHVlXCIgYXJndW1lbnQgbXVzdCBub3QgYmUgYSBudW1iZXInKVxuICB9XG5cbiAgaWYgKHR5cGVvZiBBcnJheUJ1ZmZlciAhPT0gJ3VuZGVmaW5lZCcgJiYgdmFsdWUgaW5zdGFuY2VvZiBBcnJheUJ1ZmZlcikge1xuICAgIHJldHVybiBmcm9tQXJyYXlCdWZmZXIodGhhdCwgdmFsdWUsIGVuY29kaW5nT3JPZmZzZXQsIGxlbmd0aClcbiAgfVxuXG4gIGlmICh0eXBlb2YgdmFsdWUgPT09ICdzdHJpbmcnKSB7XG4gICAgcmV0dXJuIGZyb21TdHJpbmcodGhhdCwgdmFsdWUsIGVuY29kaW5nT3JPZmZzZXQpXG4gIH1cblxuICByZXR1cm4gZnJvbU9iamVjdCh0aGF0LCB2YWx1ZSlcbn1cblxuLyoqXG4gKiBGdW5jdGlvbmFsbHkgZXF1aXZhbGVudCB0byBCdWZmZXIoYXJnLCBlbmNvZGluZykgYnV0IHRocm93cyBhIFR5cGVFcnJvclxuICogaWYgdmFsdWUgaXMgYSBudW1iZXIuXG4gKiBCdWZmZXIuZnJvbShzdHJbLCBlbmNvZGluZ10pXG4gKiBCdWZmZXIuZnJvbShhcnJheSlcbiAqIEJ1ZmZlci5mcm9tKGJ1ZmZlcilcbiAqIEJ1ZmZlci5mcm9tKGFycmF5QnVmZmVyWywgYnl0ZU9mZnNldFssIGxlbmd0aF1dKVxuICoqL1xuQnVmZmVyLmZyb20gPSBmdW5jdGlvbiAodmFsdWUsIGVuY29kaW5nT3JPZmZzZXQsIGxlbmd0aCkge1xuICByZXR1cm4gZnJvbShudWxsLCB2YWx1ZSwgZW5jb2RpbmdPck9mZnNldCwgbGVuZ3RoKVxufVxuXG5pZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgQnVmZmVyLnByb3RvdHlwZS5fX3Byb3RvX18gPSBVaW50OEFycmF5LnByb3RvdHlwZVxuICBCdWZmZXIuX19wcm90b19fID0gVWludDhBcnJheVxuICBpZiAodHlwZW9mIFN5bWJvbCAhPT0gJ3VuZGVmaW5lZCcgJiYgU3ltYm9sLnNwZWNpZXMgJiZcbiAgICAgIEJ1ZmZlcltTeW1ib2wuc3BlY2llc10gPT09IEJ1ZmZlcikge1xuICAgIC8vIEZpeCBzdWJhcnJheSgpIGluIEVTMjAxNi4gU2VlOiBodHRwczovL2dpdGh1Yi5jb20vZmVyb3NzL2J1ZmZlci9wdWxsLzk3XG4gICAgT2JqZWN0LmRlZmluZVByb3BlcnR5KEJ1ZmZlciwgU3ltYm9sLnNwZWNpZXMsIHtcbiAgICAgIHZhbHVlOiBudWxsLFxuICAgICAgY29uZmlndXJhYmxlOiB0cnVlXG4gICAgfSlcbiAgfVxufVxuXG5mdW5jdGlvbiBhc3NlcnRTaXplIChzaXplKSB7XG4gIGlmICh0eXBlb2Ygc2l6ZSAhPT0gJ251bWJlcicpIHtcbiAgICB0aHJvdyBuZXcgVHlwZUVycm9yKCdcInNpemVcIiBhcmd1bWVudCBtdXN0IGJlIGEgbnVtYmVyJylcbiAgfSBlbHNlIGlmIChzaXplIDwgMCkge1xuICAgIHRocm93IG5ldyBSYW5nZUVycm9yKCdcInNpemVcIiBhcmd1bWVudCBtdXN0IG5vdCBiZSBuZWdhdGl2ZScpXG4gIH1cbn1cblxuZnVuY3Rpb24gYWxsb2MgKHRoYXQsIHNpemUsIGZpbGwsIGVuY29kaW5nKSB7XG4gIGFzc2VydFNpemUoc2l6ZSlcbiAgaWYgKHNpemUgPD0gMCkge1xuICAgIHJldHVybiBjcmVhdGVCdWZmZXIodGhhdCwgc2l6ZSlcbiAgfVxuICBpZiAoZmlsbCAhPT0gdW5kZWZpbmVkKSB7XG4gICAgLy8gT25seSBwYXkgYXR0ZW50aW9uIHRvIGVuY29kaW5nIGlmIGl0J3MgYSBzdHJpbmcuIFRoaXNcbiAgICAvLyBwcmV2ZW50cyBhY2NpZGVudGFsbHkgc2VuZGluZyBpbiBhIG51bWJlciB0aGF0IHdvdWxkXG4gICAgLy8gYmUgaW50ZXJwcmV0dGVkIGFzIGEgc3RhcnQgb2Zmc2V0LlxuICAgIHJldHVybiB0eXBlb2YgZW5jb2RpbmcgPT09ICdzdHJpbmcnXG4gICAgICA/IGNyZWF0ZUJ1ZmZlcih0aGF0LCBzaXplKS5maWxsKGZpbGwsIGVuY29kaW5nKVxuICAgICAgOiBjcmVhdGVCdWZmZXIodGhhdCwgc2l6ZSkuZmlsbChmaWxsKVxuICB9XG4gIHJldHVybiBjcmVhdGVCdWZmZXIodGhhdCwgc2l6ZSlcbn1cblxuLyoqXG4gKiBDcmVhdGVzIGEgbmV3IGZpbGxlZCBCdWZmZXIgaW5zdGFuY2UuXG4gKiBhbGxvYyhzaXplWywgZmlsbFssIGVuY29kaW5nXV0pXG4gKiovXG5CdWZmZXIuYWxsb2MgPSBmdW5jdGlvbiAoc2l6ZSwgZmlsbCwgZW5jb2RpbmcpIHtcbiAgcmV0dXJuIGFsbG9jKG51bGwsIHNpemUsIGZpbGwsIGVuY29kaW5nKVxufVxuXG5mdW5jdGlvbiBhbGxvY1Vuc2FmZSAodGhhdCwgc2l6ZSkge1xuICBhc3NlcnRTaXplKHNpemUpXG4gIHRoYXQgPSBjcmVhdGVCdWZmZXIodGhhdCwgc2l6ZSA8IDAgPyAwIDogY2hlY2tlZChzaXplKSB8IDApXG4gIGlmICghQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgICBmb3IgKHZhciBpID0gMDsgaSA8IHNpemU7ICsraSkge1xuICAgICAgdGhhdFtpXSA9IDBcbiAgICB9XG4gIH1cbiAgcmV0dXJuIHRoYXRcbn1cblxuLyoqXG4gKiBFcXVpdmFsZW50IHRvIEJ1ZmZlcihudW0pLCBieSBkZWZhdWx0IGNyZWF0ZXMgYSBub24temVyby1maWxsZWQgQnVmZmVyIGluc3RhbmNlLlxuICogKi9cbkJ1ZmZlci5hbGxvY1Vuc2FmZSA9IGZ1bmN0aW9uIChzaXplKSB7XG4gIHJldHVybiBhbGxvY1Vuc2FmZShudWxsLCBzaXplKVxufVxuLyoqXG4gKiBFcXVpdmFsZW50IHRvIFNsb3dCdWZmZXIobnVtKSwgYnkgZGVmYXVsdCBjcmVhdGVzIGEgbm9uLXplcm8tZmlsbGVkIEJ1ZmZlciBpbnN0YW5jZS5cbiAqL1xuQnVmZmVyLmFsbG9jVW5zYWZlU2xvdyA9IGZ1bmN0aW9uIChzaXplKSB7XG4gIHJldHVybiBhbGxvY1Vuc2FmZShudWxsLCBzaXplKVxufVxuXG5mdW5jdGlvbiBmcm9tU3RyaW5nICh0aGF0LCBzdHJpbmcsIGVuY29kaW5nKSB7XG4gIGlmICh0eXBlb2YgZW5jb2RpbmcgIT09ICdzdHJpbmcnIHx8IGVuY29kaW5nID09PSAnJykge1xuICAgIGVuY29kaW5nID0gJ3V0ZjgnXG4gIH1cblxuICBpZiAoIUJ1ZmZlci5pc0VuY29kaW5nKGVuY29kaW5nKSkge1xuICAgIHRocm93IG5ldyBUeXBlRXJyb3IoJ1wiZW5jb2RpbmdcIiBtdXN0IGJlIGEgdmFsaWQgc3RyaW5nIGVuY29kaW5nJylcbiAgfVxuXG4gIHZhciBsZW5ndGggPSBieXRlTGVuZ3RoKHN0cmluZywgZW5jb2RpbmcpIHwgMFxuICB0aGF0ID0gY3JlYXRlQnVmZmVyKHRoYXQsIGxlbmd0aClcblxuICB2YXIgYWN0dWFsID0gdGhhdC53cml0ZShzdHJpbmcsIGVuY29kaW5nKVxuXG4gIGlmIChhY3R1YWwgIT09IGxlbmd0aCkge1xuICAgIC8vIFdyaXRpbmcgYSBoZXggc3RyaW5nLCBmb3IgZXhhbXBsZSwgdGhhdCBjb250YWlucyBpbnZhbGlkIGNoYXJhY3RlcnMgd2lsbFxuICAgIC8vIGNhdXNlIGV2ZXJ5dGhpbmcgYWZ0ZXIgdGhlIGZpcnN0IGludmFsaWQgY2hhcmFjdGVyIHRvIGJlIGlnbm9yZWQuIChlLmcuXG4gICAgLy8gJ2FieHhjZCcgd2lsbCBiZSB0cmVhdGVkIGFzICdhYicpXG4gICAgdGhhdCA9IHRoYXQuc2xpY2UoMCwgYWN0dWFsKVxuICB9XG5cbiAgcmV0dXJuIHRoYXRcbn1cblxuZnVuY3Rpb24gZnJvbUFycmF5TGlrZSAodGhhdCwgYXJyYXkpIHtcbiAgdmFyIGxlbmd0aCA9IGFycmF5Lmxlbmd0aCA8IDAgPyAwIDogY2hlY2tlZChhcnJheS5sZW5ndGgpIHwgMFxuICB0aGF0ID0gY3JlYXRlQnVmZmVyKHRoYXQsIGxlbmd0aClcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBsZW5ndGg7IGkgKz0gMSkge1xuICAgIHRoYXRbaV0gPSBhcnJheVtpXSAmIDI1NVxuICB9XG4gIHJldHVybiB0aGF0XG59XG5cbmZ1bmN0aW9uIGZyb21BcnJheUJ1ZmZlciAodGhhdCwgYXJyYXksIGJ5dGVPZmZzZXQsIGxlbmd0aCkge1xuICBhcnJheS5ieXRlTGVuZ3RoIC8vIHRoaXMgdGhyb3dzIGlmIGBhcnJheWAgaXMgbm90IGEgdmFsaWQgQXJyYXlCdWZmZXJcblxuICBpZiAoYnl0ZU9mZnNldCA8IDAgfHwgYXJyYXkuYnl0ZUxlbmd0aCA8IGJ5dGVPZmZzZXQpIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignXFwnb2Zmc2V0XFwnIGlzIG91dCBvZiBib3VuZHMnKVxuICB9XG5cbiAgaWYgKGFycmF5LmJ5dGVMZW5ndGggPCBieXRlT2Zmc2V0ICsgKGxlbmd0aCB8fCAwKSkge1xuICAgIHRocm93IG5ldyBSYW5nZUVycm9yKCdcXCdsZW5ndGhcXCcgaXMgb3V0IG9mIGJvdW5kcycpXG4gIH1cblxuICBpZiAoYnl0ZU9mZnNldCA9PT0gdW5kZWZpbmVkICYmIGxlbmd0aCA9PT0gdW5kZWZpbmVkKSB7XG4gICAgYXJyYXkgPSBuZXcgVWludDhBcnJheShhcnJheSlcbiAgfSBlbHNlIGlmIChsZW5ndGggPT09IHVuZGVmaW5lZCkge1xuICAgIGFycmF5ID0gbmV3IFVpbnQ4QXJyYXkoYXJyYXksIGJ5dGVPZmZzZXQpXG4gIH0gZWxzZSB7XG4gICAgYXJyYXkgPSBuZXcgVWludDhBcnJheShhcnJheSwgYnl0ZU9mZnNldCwgbGVuZ3RoKVxuICB9XG5cbiAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgLy8gUmV0dXJuIGFuIGF1Z21lbnRlZCBgVWludDhBcnJheWAgaW5zdGFuY2UsIGZvciBiZXN0IHBlcmZvcm1hbmNlXG4gICAgdGhhdCA9IGFycmF5XG4gICAgdGhhdC5fX3Byb3RvX18gPSBCdWZmZXIucHJvdG90eXBlXG4gIH0gZWxzZSB7XG4gICAgLy8gRmFsbGJhY2s6IFJldHVybiBhbiBvYmplY3QgaW5zdGFuY2Ugb2YgdGhlIEJ1ZmZlciBjbGFzc1xuICAgIHRoYXQgPSBmcm9tQXJyYXlMaWtlKHRoYXQsIGFycmF5KVxuICB9XG4gIHJldHVybiB0aGF0XG59XG5cbmZ1bmN0aW9uIGZyb21PYmplY3QgKHRoYXQsIG9iaikge1xuICBpZiAoQnVmZmVyLmlzQnVmZmVyKG9iaikpIHtcbiAgICB2YXIgbGVuID0gY2hlY2tlZChvYmoubGVuZ3RoKSB8IDBcbiAgICB0aGF0ID0gY3JlYXRlQnVmZmVyKHRoYXQsIGxlbilcblxuICAgIGlmICh0aGF0Lmxlbmd0aCA9PT0gMCkge1xuICAgICAgcmV0dXJuIHRoYXRcbiAgICB9XG5cbiAgICBvYmouY29weSh0aGF0LCAwLCAwLCBsZW4pXG4gICAgcmV0dXJuIHRoYXRcbiAgfVxuXG4gIGlmIChvYmopIHtcbiAgICBpZiAoKHR5cGVvZiBBcnJheUJ1ZmZlciAhPT0gJ3VuZGVmaW5lZCcgJiZcbiAgICAgICAgb2JqLmJ1ZmZlciBpbnN0YW5jZW9mIEFycmF5QnVmZmVyKSB8fCAnbGVuZ3RoJyBpbiBvYmopIHtcbiAgICAgIGlmICh0eXBlb2Ygb2JqLmxlbmd0aCAhPT0gJ251bWJlcicgfHwgaXNuYW4ob2JqLmxlbmd0aCkpIHtcbiAgICAgICAgcmV0dXJuIGNyZWF0ZUJ1ZmZlcih0aGF0LCAwKVxuICAgICAgfVxuICAgICAgcmV0dXJuIGZyb21BcnJheUxpa2UodGhhdCwgb2JqKVxuICAgIH1cblxuICAgIGlmIChvYmoudHlwZSA9PT0gJ0J1ZmZlcicgJiYgaXNBcnJheShvYmouZGF0YSkpIHtcbiAgICAgIHJldHVybiBmcm9tQXJyYXlMaWtlKHRoYXQsIG9iai5kYXRhKVxuICAgIH1cbiAgfVxuXG4gIHRocm93IG5ldyBUeXBlRXJyb3IoJ0ZpcnN0IGFyZ3VtZW50IG11c3QgYmUgYSBzdHJpbmcsIEJ1ZmZlciwgQXJyYXlCdWZmZXIsIEFycmF5LCBvciBhcnJheS1saWtlIG9iamVjdC4nKVxufVxuXG5mdW5jdGlvbiBjaGVja2VkIChsZW5ndGgpIHtcbiAgLy8gTm90ZTogY2Fubm90IHVzZSBgbGVuZ3RoIDwga01heExlbmd0aCgpYCBoZXJlIGJlY2F1c2UgdGhhdCBmYWlscyB3aGVuXG4gIC8vIGxlbmd0aCBpcyBOYU4gKHdoaWNoIGlzIG90aGVyd2lzZSBjb2VyY2VkIHRvIHplcm8uKVxuICBpZiAobGVuZ3RoID49IGtNYXhMZW5ndGgoKSkge1xuICAgIHRocm93IG5ldyBSYW5nZUVycm9yKCdBdHRlbXB0IHRvIGFsbG9jYXRlIEJ1ZmZlciBsYXJnZXIgdGhhbiBtYXhpbXVtICcgK1xuICAgICAgICAgICAgICAgICAgICAgICAgICdzaXplOiAweCcgKyBrTWF4TGVuZ3RoKCkudG9TdHJpbmcoMTYpICsgJyBieXRlcycpXG4gIH1cbiAgcmV0dXJuIGxlbmd0aCB8IDBcbn1cblxuZnVuY3Rpb24gU2xvd0J1ZmZlciAobGVuZ3RoKSB7XG4gIGlmICgrbGVuZ3RoICE9IGxlbmd0aCkgeyAvLyBlc2xpbnQtZGlzYWJsZS1saW5lIGVxZXFlcVxuICAgIGxlbmd0aCA9IDBcbiAgfVxuICByZXR1cm4gQnVmZmVyLmFsbG9jKCtsZW5ndGgpXG59XG5cbkJ1ZmZlci5pc0J1ZmZlciA9IGZ1bmN0aW9uIGlzQnVmZmVyIChiKSB7XG4gIHJldHVybiAhIShiICE9IG51bGwgJiYgYi5faXNCdWZmZXIpXG59XG5cbkJ1ZmZlci5jb21wYXJlID0gZnVuY3Rpb24gY29tcGFyZSAoYSwgYikge1xuICBpZiAoIUJ1ZmZlci5pc0J1ZmZlcihhKSB8fCAhQnVmZmVyLmlzQnVmZmVyKGIpKSB7XG4gICAgdGhyb3cgbmV3IFR5cGVFcnJvcignQXJndW1lbnRzIG11c3QgYmUgQnVmZmVycycpXG4gIH1cblxuICBpZiAoYSA9PT0gYikgcmV0dXJuIDBcblxuICB2YXIgeCA9IGEubGVuZ3RoXG4gIHZhciB5ID0gYi5sZW5ndGhcblxuICBmb3IgKHZhciBpID0gMCwgbGVuID0gTWF0aC5taW4oeCwgeSk7IGkgPCBsZW47ICsraSkge1xuICAgIGlmIChhW2ldICE9PSBiW2ldKSB7XG4gICAgICB4ID0gYVtpXVxuICAgICAgeSA9IGJbaV1cbiAgICAgIGJyZWFrXG4gICAgfVxuICB9XG5cbiAgaWYgKHggPCB5KSByZXR1cm4gLTFcbiAgaWYgKHkgPCB4KSByZXR1cm4gMVxuICByZXR1cm4gMFxufVxuXG5CdWZmZXIuaXNFbmNvZGluZyA9IGZ1bmN0aW9uIGlzRW5jb2RpbmcgKGVuY29kaW5nKSB7XG4gIHN3aXRjaCAoU3RyaW5nKGVuY29kaW5nKS50b0xvd2VyQ2FzZSgpKSB7XG4gICAgY2FzZSAnaGV4JzpcbiAgICBjYXNlICd1dGY4JzpcbiAgICBjYXNlICd1dGYtOCc6XG4gICAgY2FzZSAnYXNjaWknOlxuICAgIGNhc2UgJ2xhdGluMSc6XG4gICAgY2FzZSAnYmluYXJ5JzpcbiAgICBjYXNlICdiYXNlNjQnOlxuICAgIGNhc2UgJ3VjczInOlxuICAgIGNhc2UgJ3Vjcy0yJzpcbiAgICBjYXNlICd1dGYxNmxlJzpcbiAgICBjYXNlICd1dGYtMTZsZSc6XG4gICAgICByZXR1cm4gdHJ1ZVxuICAgIGRlZmF1bHQ6XG4gICAgICByZXR1cm4gZmFsc2VcbiAgfVxufVxuXG5CdWZmZXIuY29uY2F0ID0gZnVuY3Rpb24gY29uY2F0IChsaXN0LCBsZW5ndGgpIHtcbiAgaWYgKCFpc0FycmF5KGxpc3QpKSB7XG4gICAgdGhyb3cgbmV3IFR5cGVFcnJvcignXCJsaXN0XCIgYXJndW1lbnQgbXVzdCBiZSBhbiBBcnJheSBvZiBCdWZmZXJzJylcbiAgfVxuXG4gIGlmIChsaXN0Lmxlbmd0aCA9PT0gMCkge1xuICAgIHJldHVybiBCdWZmZXIuYWxsb2MoMClcbiAgfVxuXG4gIHZhciBpXG4gIGlmIChsZW5ndGggPT09IHVuZGVmaW5lZCkge1xuICAgIGxlbmd0aCA9IDBcbiAgICBmb3IgKGkgPSAwOyBpIDwgbGlzdC5sZW5ndGg7ICsraSkge1xuICAgICAgbGVuZ3RoICs9IGxpc3RbaV0ubGVuZ3RoXG4gICAgfVxuICB9XG5cbiAgdmFyIGJ1ZmZlciA9IEJ1ZmZlci5hbGxvY1Vuc2FmZShsZW5ndGgpXG4gIHZhciBwb3MgPSAwXG4gIGZvciAoaSA9IDA7IGkgPCBsaXN0Lmxlbmd0aDsgKytpKSB7XG4gICAgdmFyIGJ1ZiA9IGxpc3RbaV1cbiAgICBpZiAoIUJ1ZmZlci5pc0J1ZmZlcihidWYpKSB7XG4gICAgICB0aHJvdyBuZXcgVHlwZUVycm9yKCdcImxpc3RcIiBhcmd1bWVudCBtdXN0IGJlIGFuIEFycmF5IG9mIEJ1ZmZlcnMnKVxuICAgIH1cbiAgICBidWYuY29weShidWZmZXIsIHBvcylcbiAgICBwb3MgKz0gYnVmLmxlbmd0aFxuICB9XG4gIHJldHVybiBidWZmZXJcbn1cblxuZnVuY3Rpb24gYnl0ZUxlbmd0aCAoc3RyaW5nLCBlbmNvZGluZykge1xuICBpZiAoQnVmZmVyLmlzQnVmZmVyKHN0cmluZykpIHtcbiAgICByZXR1cm4gc3RyaW5nLmxlbmd0aFxuICB9XG4gIGlmICh0eXBlb2YgQXJyYXlCdWZmZXIgIT09ICd1bmRlZmluZWQnICYmIHR5cGVvZiBBcnJheUJ1ZmZlci5pc1ZpZXcgPT09ICdmdW5jdGlvbicgJiZcbiAgICAgIChBcnJheUJ1ZmZlci5pc1ZpZXcoc3RyaW5nKSB8fCBzdHJpbmcgaW5zdGFuY2VvZiBBcnJheUJ1ZmZlcikpIHtcbiAgICByZXR1cm4gc3RyaW5nLmJ5dGVMZW5ndGhcbiAgfVxuICBpZiAodHlwZW9mIHN0cmluZyAhPT0gJ3N0cmluZycpIHtcbiAgICBzdHJpbmcgPSAnJyArIHN0cmluZ1xuICB9XG5cbiAgdmFyIGxlbiA9IHN0cmluZy5sZW5ndGhcbiAgaWYgKGxlbiA9PT0gMCkgcmV0dXJuIDBcblxuICAvLyBVc2UgYSBmb3IgbG9vcCB0byBhdm9pZCByZWN1cnNpb25cbiAgdmFyIGxvd2VyZWRDYXNlID0gZmFsc2VcbiAgZm9yICg7Oykge1xuICAgIHN3aXRjaCAoZW5jb2RpbmcpIHtcbiAgICAgIGNhc2UgJ2FzY2lpJzpcbiAgICAgIGNhc2UgJ2xhdGluMSc6XG4gICAgICBjYXNlICdiaW5hcnknOlxuICAgICAgICByZXR1cm4gbGVuXG4gICAgICBjYXNlICd1dGY4JzpcbiAgICAgIGNhc2UgJ3V0Zi04JzpcbiAgICAgIGNhc2UgdW5kZWZpbmVkOlxuICAgICAgICByZXR1cm4gdXRmOFRvQnl0ZXMoc3RyaW5nKS5sZW5ndGhcbiAgICAgIGNhc2UgJ3VjczInOlxuICAgICAgY2FzZSAndWNzLTInOlxuICAgICAgY2FzZSAndXRmMTZsZSc6XG4gICAgICBjYXNlICd1dGYtMTZsZSc6XG4gICAgICAgIHJldHVybiBsZW4gKiAyXG4gICAgICBjYXNlICdoZXgnOlxuICAgICAgICByZXR1cm4gbGVuID4+PiAxXG4gICAgICBjYXNlICdiYXNlNjQnOlxuICAgICAgICByZXR1cm4gYmFzZTY0VG9CeXRlcyhzdHJpbmcpLmxlbmd0aFxuICAgICAgZGVmYXVsdDpcbiAgICAgICAgaWYgKGxvd2VyZWRDYXNlKSByZXR1cm4gdXRmOFRvQnl0ZXMoc3RyaW5nKS5sZW5ndGggLy8gYXNzdW1lIHV0ZjhcbiAgICAgICAgZW5jb2RpbmcgPSAoJycgKyBlbmNvZGluZykudG9Mb3dlckNhc2UoKVxuICAgICAgICBsb3dlcmVkQ2FzZSA9IHRydWVcbiAgICB9XG4gIH1cbn1cbkJ1ZmZlci5ieXRlTGVuZ3RoID0gYnl0ZUxlbmd0aFxuXG5mdW5jdGlvbiBzbG93VG9TdHJpbmcgKGVuY29kaW5nLCBzdGFydCwgZW5kKSB7XG4gIHZhciBsb3dlcmVkQ2FzZSA9IGZhbHNlXG5cbiAgLy8gTm8gbmVlZCB0byB2ZXJpZnkgdGhhdCBcInRoaXMubGVuZ3RoIDw9IE1BWF9VSU5UMzJcIiBzaW5jZSBpdCdzIGEgcmVhZC1vbmx5XG4gIC8vIHByb3BlcnR5IG9mIGEgdHlwZWQgYXJyYXkuXG5cbiAgLy8gVGhpcyBiZWhhdmVzIG5laXRoZXIgbGlrZSBTdHJpbmcgbm9yIFVpbnQ4QXJyYXkgaW4gdGhhdCB3ZSBzZXQgc3RhcnQvZW5kXG4gIC8vIHRvIHRoZWlyIHVwcGVyL2xvd2VyIGJvdW5kcyBpZiB0aGUgdmFsdWUgcGFzc2VkIGlzIG91dCBvZiByYW5nZS5cbiAgLy8gdW5kZWZpbmVkIGlzIGhhbmRsZWQgc3BlY2lhbGx5IGFzIHBlciBFQ01BLTI2MiA2dGggRWRpdGlvbixcbiAgLy8gU2VjdGlvbiAxMy4zLjMuNyBSdW50aW1lIFNlbWFudGljczogS2V5ZWRCaW5kaW5nSW5pdGlhbGl6YXRpb24uXG4gIGlmIChzdGFydCA9PT0gdW5kZWZpbmVkIHx8IHN0YXJ0IDwgMCkge1xuICAgIHN0YXJ0ID0gMFxuICB9XG4gIC8vIFJldHVybiBlYXJseSBpZiBzdGFydCA+IHRoaXMubGVuZ3RoLiBEb25lIGhlcmUgdG8gcHJldmVudCBwb3RlbnRpYWwgdWludDMyXG4gIC8vIGNvZXJjaW9uIGZhaWwgYmVsb3cuXG4gIGlmIChzdGFydCA+IHRoaXMubGVuZ3RoKSB7XG4gICAgcmV0dXJuICcnXG4gIH1cblxuICBpZiAoZW5kID09PSB1bmRlZmluZWQgfHwgZW5kID4gdGhpcy5sZW5ndGgpIHtcbiAgICBlbmQgPSB0aGlzLmxlbmd0aFxuICB9XG5cbiAgaWYgKGVuZCA8PSAwKSB7XG4gICAgcmV0dXJuICcnXG4gIH1cblxuICAvLyBGb3JjZSBjb2Vyc2lvbiB0byB1aW50MzIuIFRoaXMgd2lsbCBhbHNvIGNvZXJjZSBmYWxzZXkvTmFOIHZhbHVlcyB0byAwLlxuICBlbmQgPj4+PSAwXG4gIHN0YXJ0ID4+Pj0gMFxuXG4gIGlmIChlbmQgPD0gc3RhcnQpIHtcbiAgICByZXR1cm4gJydcbiAgfVxuXG4gIGlmICghZW5jb2RpbmcpIGVuY29kaW5nID0gJ3V0ZjgnXG5cbiAgd2hpbGUgKHRydWUpIHtcbiAgICBzd2l0Y2ggKGVuY29kaW5nKSB7XG4gICAgICBjYXNlICdoZXgnOlxuICAgICAgICByZXR1cm4gaGV4U2xpY2UodGhpcywgc3RhcnQsIGVuZClcblxuICAgICAgY2FzZSAndXRmOCc6XG4gICAgICBjYXNlICd1dGYtOCc6XG4gICAgICAgIHJldHVybiB1dGY4U2xpY2UodGhpcywgc3RhcnQsIGVuZClcblxuICAgICAgY2FzZSAnYXNjaWknOlxuICAgICAgICByZXR1cm4gYXNjaWlTbGljZSh0aGlzLCBzdGFydCwgZW5kKVxuXG4gICAgICBjYXNlICdsYXRpbjEnOlxuICAgICAgY2FzZSAnYmluYXJ5JzpcbiAgICAgICAgcmV0dXJuIGxhdGluMVNsaWNlKHRoaXMsIHN0YXJ0LCBlbmQpXG5cbiAgICAgIGNhc2UgJ2Jhc2U2NCc6XG4gICAgICAgIHJldHVybiBiYXNlNjRTbGljZSh0aGlzLCBzdGFydCwgZW5kKVxuXG4gICAgICBjYXNlICd1Y3MyJzpcbiAgICAgIGNhc2UgJ3Vjcy0yJzpcbiAgICAgIGNhc2UgJ3V0ZjE2bGUnOlxuICAgICAgY2FzZSAndXRmLTE2bGUnOlxuICAgICAgICByZXR1cm4gdXRmMTZsZVNsaWNlKHRoaXMsIHN0YXJ0LCBlbmQpXG5cbiAgICAgIGRlZmF1bHQ6XG4gICAgICAgIGlmIChsb3dlcmVkQ2FzZSkgdGhyb3cgbmV3IFR5cGVFcnJvcignVW5rbm93biBlbmNvZGluZzogJyArIGVuY29kaW5nKVxuICAgICAgICBlbmNvZGluZyA9IChlbmNvZGluZyArICcnKS50b0xvd2VyQ2FzZSgpXG4gICAgICAgIGxvd2VyZWRDYXNlID0gdHJ1ZVxuICAgIH1cbiAgfVxufVxuXG4vLyBUaGUgcHJvcGVydHkgaXMgdXNlZCBieSBgQnVmZmVyLmlzQnVmZmVyYCBhbmQgYGlzLWJ1ZmZlcmAgKGluIFNhZmFyaSA1LTcpIHRvIGRldGVjdFxuLy8gQnVmZmVyIGluc3RhbmNlcy5cbkJ1ZmZlci5wcm90b3R5cGUuX2lzQnVmZmVyID0gdHJ1ZVxuXG5mdW5jdGlvbiBzd2FwIChiLCBuLCBtKSB7XG4gIHZhciBpID0gYltuXVxuICBiW25dID0gYlttXVxuICBiW21dID0gaVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnN3YXAxNiA9IGZ1bmN0aW9uIHN3YXAxNiAoKSB7XG4gIHZhciBsZW4gPSB0aGlzLmxlbmd0aFxuICBpZiAobGVuICUgMiAhPT0gMCkge1xuICAgIHRocm93IG5ldyBSYW5nZUVycm9yKCdCdWZmZXIgc2l6ZSBtdXN0IGJlIGEgbXVsdGlwbGUgb2YgMTYtYml0cycpXG4gIH1cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBsZW47IGkgKz0gMikge1xuICAgIHN3YXAodGhpcywgaSwgaSArIDEpXG4gIH1cbiAgcmV0dXJuIHRoaXNcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5zd2FwMzIgPSBmdW5jdGlvbiBzd2FwMzIgKCkge1xuICB2YXIgbGVuID0gdGhpcy5sZW5ndGhcbiAgaWYgKGxlbiAlIDQgIT09IDApIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignQnVmZmVyIHNpemUgbXVzdCBiZSBhIG11bHRpcGxlIG9mIDMyLWJpdHMnKVxuICB9XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgbGVuOyBpICs9IDQpIHtcbiAgICBzd2FwKHRoaXMsIGksIGkgKyAzKVxuICAgIHN3YXAodGhpcywgaSArIDEsIGkgKyAyKVxuICB9XG4gIHJldHVybiB0aGlzXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUuc3dhcDY0ID0gZnVuY3Rpb24gc3dhcDY0ICgpIHtcbiAgdmFyIGxlbiA9IHRoaXMubGVuZ3RoXG4gIGlmIChsZW4gJSA4ICE9PSAwKSB7XG4gICAgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ0J1ZmZlciBzaXplIG11c3QgYmUgYSBtdWx0aXBsZSBvZiA2NC1iaXRzJylcbiAgfVxuICBmb3IgKHZhciBpID0gMDsgaSA8IGxlbjsgaSArPSA4KSB7XG4gICAgc3dhcCh0aGlzLCBpLCBpICsgNylcbiAgICBzd2FwKHRoaXMsIGkgKyAxLCBpICsgNilcbiAgICBzd2FwKHRoaXMsIGkgKyAyLCBpICsgNSlcbiAgICBzd2FwKHRoaXMsIGkgKyAzLCBpICsgNClcbiAgfVxuICByZXR1cm4gdGhpc1xufVxuXG5CdWZmZXIucHJvdG90eXBlLnRvU3RyaW5nID0gZnVuY3Rpb24gdG9TdHJpbmcgKCkge1xuICB2YXIgbGVuZ3RoID0gdGhpcy5sZW5ndGggfCAwXG4gIGlmIChsZW5ndGggPT09IDApIHJldHVybiAnJ1xuICBpZiAoYXJndW1lbnRzLmxlbmd0aCA9PT0gMCkgcmV0dXJuIHV0ZjhTbGljZSh0aGlzLCAwLCBsZW5ndGgpXG4gIHJldHVybiBzbG93VG9TdHJpbmcuYXBwbHkodGhpcywgYXJndW1lbnRzKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLmVxdWFscyA9IGZ1bmN0aW9uIGVxdWFscyAoYikge1xuICBpZiAoIUJ1ZmZlci5pc0J1ZmZlcihiKSkgdGhyb3cgbmV3IFR5cGVFcnJvcignQXJndW1lbnQgbXVzdCBiZSBhIEJ1ZmZlcicpXG4gIGlmICh0aGlzID09PSBiKSByZXR1cm4gdHJ1ZVxuICByZXR1cm4gQnVmZmVyLmNvbXBhcmUodGhpcywgYikgPT09IDBcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5pbnNwZWN0ID0gZnVuY3Rpb24gaW5zcGVjdCAoKSB7XG4gIHZhciBzdHIgPSAnJ1xuICB2YXIgbWF4ID0gZXhwb3J0cy5JTlNQRUNUX01BWF9CWVRFU1xuICBpZiAodGhpcy5sZW5ndGggPiAwKSB7XG4gICAgc3RyID0gdGhpcy50b1N0cmluZygnaGV4JywgMCwgbWF4KS5tYXRjaCgvLnsyfS9nKS5qb2luKCcgJylcbiAgICBpZiAodGhpcy5sZW5ndGggPiBtYXgpIHN0ciArPSAnIC4uLiAnXG4gIH1cbiAgcmV0dXJuICc8QnVmZmVyICcgKyBzdHIgKyAnPidcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5jb21wYXJlID0gZnVuY3Rpb24gY29tcGFyZSAodGFyZ2V0LCBzdGFydCwgZW5kLCB0aGlzU3RhcnQsIHRoaXNFbmQpIHtcbiAgaWYgKCFCdWZmZXIuaXNCdWZmZXIodGFyZ2V0KSkge1xuICAgIHRocm93IG5ldyBUeXBlRXJyb3IoJ0FyZ3VtZW50IG11c3QgYmUgYSBCdWZmZXInKVxuICB9XG5cbiAgaWYgKHN0YXJ0ID09PSB1bmRlZmluZWQpIHtcbiAgICBzdGFydCA9IDBcbiAgfVxuICBpZiAoZW5kID09PSB1bmRlZmluZWQpIHtcbiAgICBlbmQgPSB0YXJnZXQgPyB0YXJnZXQubGVuZ3RoIDogMFxuICB9XG4gIGlmICh0aGlzU3RhcnQgPT09IHVuZGVmaW5lZCkge1xuICAgIHRoaXNTdGFydCA9IDBcbiAgfVxuICBpZiAodGhpc0VuZCA9PT0gdW5kZWZpbmVkKSB7XG4gICAgdGhpc0VuZCA9IHRoaXMubGVuZ3RoXG4gIH1cblxuICBpZiAoc3RhcnQgPCAwIHx8IGVuZCA+IHRhcmdldC5sZW5ndGggfHwgdGhpc1N0YXJ0IDwgMCB8fCB0aGlzRW5kID4gdGhpcy5sZW5ndGgpIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignb3V0IG9mIHJhbmdlIGluZGV4JylcbiAgfVxuXG4gIGlmICh0aGlzU3RhcnQgPj0gdGhpc0VuZCAmJiBzdGFydCA+PSBlbmQpIHtcbiAgICByZXR1cm4gMFxuICB9XG4gIGlmICh0aGlzU3RhcnQgPj0gdGhpc0VuZCkge1xuICAgIHJldHVybiAtMVxuICB9XG4gIGlmIChzdGFydCA+PSBlbmQpIHtcbiAgICByZXR1cm4gMVxuICB9XG5cbiAgc3RhcnQgPj4+PSAwXG4gIGVuZCA+Pj49IDBcbiAgdGhpc1N0YXJ0ID4+Pj0gMFxuICB0aGlzRW5kID4+Pj0gMFxuXG4gIGlmICh0aGlzID09PSB0YXJnZXQpIHJldHVybiAwXG5cbiAgdmFyIHggPSB0aGlzRW5kIC0gdGhpc1N0YXJ0XG4gIHZhciB5ID0gZW5kIC0gc3RhcnRcbiAgdmFyIGxlbiA9IE1hdGgubWluKHgsIHkpXG5cbiAgdmFyIHRoaXNDb3B5ID0gdGhpcy5zbGljZSh0aGlzU3RhcnQsIHRoaXNFbmQpXG4gIHZhciB0YXJnZXRDb3B5ID0gdGFyZ2V0LnNsaWNlKHN0YXJ0LCBlbmQpXG5cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBsZW47ICsraSkge1xuICAgIGlmICh0aGlzQ29weVtpXSAhPT0gdGFyZ2V0Q29weVtpXSkge1xuICAgICAgeCA9IHRoaXNDb3B5W2ldXG4gICAgICB5ID0gdGFyZ2V0Q29weVtpXVxuICAgICAgYnJlYWtcbiAgICB9XG4gIH1cblxuICBpZiAoeCA8IHkpIHJldHVybiAtMVxuICBpZiAoeSA8IHgpIHJldHVybiAxXG4gIHJldHVybiAwXG59XG5cbi8vIEZpbmRzIGVpdGhlciB0aGUgZmlyc3QgaW5kZXggb2YgYHZhbGAgaW4gYGJ1ZmZlcmAgYXQgb2Zmc2V0ID49IGBieXRlT2Zmc2V0YCxcbi8vIE9SIHRoZSBsYXN0IGluZGV4IG9mIGB2YWxgIGluIGBidWZmZXJgIGF0IG9mZnNldCA8PSBgYnl0ZU9mZnNldGAuXG4vL1xuLy8gQXJndW1lbnRzOlxuLy8gLSBidWZmZXIgLSBhIEJ1ZmZlciB0byBzZWFyY2hcbi8vIC0gdmFsIC0gYSBzdHJpbmcsIEJ1ZmZlciwgb3IgbnVtYmVyXG4vLyAtIGJ5dGVPZmZzZXQgLSBhbiBpbmRleCBpbnRvIGBidWZmZXJgOyB3aWxsIGJlIGNsYW1wZWQgdG8gYW4gaW50MzJcbi8vIC0gZW5jb2RpbmcgLSBhbiBvcHRpb25hbCBlbmNvZGluZywgcmVsZXZhbnQgaXMgdmFsIGlzIGEgc3RyaW5nXG4vLyAtIGRpciAtIHRydWUgZm9yIGluZGV4T2YsIGZhbHNlIGZvciBsYXN0SW5kZXhPZlxuZnVuY3Rpb24gYmlkaXJlY3Rpb25hbEluZGV4T2YgKGJ1ZmZlciwgdmFsLCBieXRlT2Zmc2V0LCBlbmNvZGluZywgZGlyKSB7XG4gIC8vIEVtcHR5IGJ1ZmZlciBtZWFucyBubyBtYXRjaFxuICBpZiAoYnVmZmVyLmxlbmd0aCA9PT0gMCkgcmV0dXJuIC0xXG5cbiAgLy8gTm9ybWFsaXplIGJ5dGVPZmZzZXRcbiAgaWYgKHR5cGVvZiBieXRlT2Zmc2V0ID09PSAnc3RyaW5nJykge1xuICAgIGVuY29kaW5nID0gYnl0ZU9mZnNldFxuICAgIGJ5dGVPZmZzZXQgPSAwXG4gIH0gZWxzZSBpZiAoYnl0ZU9mZnNldCA+IDB4N2ZmZmZmZmYpIHtcbiAgICBieXRlT2Zmc2V0ID0gMHg3ZmZmZmZmZlxuICB9IGVsc2UgaWYgKGJ5dGVPZmZzZXQgPCAtMHg4MDAwMDAwMCkge1xuICAgIGJ5dGVPZmZzZXQgPSAtMHg4MDAwMDAwMFxuICB9XG4gIGJ5dGVPZmZzZXQgPSArYnl0ZU9mZnNldCAgLy8gQ29lcmNlIHRvIE51bWJlci5cbiAgaWYgKGlzTmFOKGJ5dGVPZmZzZXQpKSB7XG4gICAgLy8gYnl0ZU9mZnNldDogaXQgaXQncyB1bmRlZmluZWQsIG51bGwsIE5hTiwgXCJmb29cIiwgZXRjLCBzZWFyY2ggd2hvbGUgYnVmZmVyXG4gICAgYnl0ZU9mZnNldCA9IGRpciA/IDAgOiAoYnVmZmVyLmxlbmd0aCAtIDEpXG4gIH1cblxuICAvLyBOb3JtYWxpemUgYnl0ZU9mZnNldDogbmVnYXRpdmUgb2Zmc2V0cyBzdGFydCBmcm9tIHRoZSBlbmQgb2YgdGhlIGJ1ZmZlclxuICBpZiAoYnl0ZU9mZnNldCA8IDApIGJ5dGVPZmZzZXQgPSBidWZmZXIubGVuZ3RoICsgYnl0ZU9mZnNldFxuICBpZiAoYnl0ZU9mZnNldCA+PSBidWZmZXIubGVuZ3RoKSB7XG4gICAgaWYgKGRpcikgcmV0dXJuIC0xXG4gICAgZWxzZSBieXRlT2Zmc2V0ID0gYnVmZmVyLmxlbmd0aCAtIDFcbiAgfSBlbHNlIGlmIChieXRlT2Zmc2V0IDwgMCkge1xuICAgIGlmIChkaXIpIGJ5dGVPZmZzZXQgPSAwXG4gICAgZWxzZSByZXR1cm4gLTFcbiAgfVxuXG4gIC8vIE5vcm1hbGl6ZSB2YWxcbiAgaWYgKHR5cGVvZiB2YWwgPT09ICdzdHJpbmcnKSB7XG4gICAgdmFsID0gQnVmZmVyLmZyb20odmFsLCBlbmNvZGluZylcbiAgfVxuXG4gIC8vIEZpbmFsbHksIHNlYXJjaCBlaXRoZXIgaW5kZXhPZiAoaWYgZGlyIGlzIHRydWUpIG9yIGxhc3RJbmRleE9mXG4gIGlmIChCdWZmZXIuaXNCdWZmZXIodmFsKSkge1xuICAgIC8vIFNwZWNpYWwgY2FzZTogbG9va2luZyBmb3IgZW1wdHkgc3RyaW5nL2J1ZmZlciBhbHdheXMgZmFpbHNcbiAgICBpZiAodmFsLmxlbmd0aCA9PT0gMCkge1xuICAgICAgcmV0dXJuIC0xXG4gICAgfVxuICAgIHJldHVybiBhcnJheUluZGV4T2YoYnVmZmVyLCB2YWwsIGJ5dGVPZmZzZXQsIGVuY29kaW5nLCBkaXIpXG4gIH0gZWxzZSBpZiAodHlwZW9mIHZhbCA9PT0gJ251bWJlcicpIHtcbiAgICB2YWwgPSB2YWwgJiAweEZGIC8vIFNlYXJjaCBmb3IgYSBieXRlIHZhbHVlIFswLTI1NV1cbiAgICBpZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQgJiZcbiAgICAgICAgdHlwZW9mIFVpbnQ4QXJyYXkucHJvdG90eXBlLmluZGV4T2YgPT09ICdmdW5jdGlvbicpIHtcbiAgICAgIGlmIChkaXIpIHtcbiAgICAgICAgcmV0dXJuIFVpbnQ4QXJyYXkucHJvdG90eXBlLmluZGV4T2YuY2FsbChidWZmZXIsIHZhbCwgYnl0ZU9mZnNldClcbiAgICAgIH0gZWxzZSB7XG4gICAgICAgIHJldHVybiBVaW50OEFycmF5LnByb3RvdHlwZS5sYXN0SW5kZXhPZi5jYWxsKGJ1ZmZlciwgdmFsLCBieXRlT2Zmc2V0KVxuICAgICAgfVxuICAgIH1cbiAgICByZXR1cm4gYXJyYXlJbmRleE9mKGJ1ZmZlciwgWyB2YWwgXSwgYnl0ZU9mZnNldCwgZW5jb2RpbmcsIGRpcilcbiAgfVxuXG4gIHRocm93IG5ldyBUeXBlRXJyb3IoJ3ZhbCBtdXN0IGJlIHN0cmluZywgbnVtYmVyIG9yIEJ1ZmZlcicpXG59XG5cbmZ1bmN0aW9uIGFycmF5SW5kZXhPZiAoYXJyLCB2YWwsIGJ5dGVPZmZzZXQsIGVuY29kaW5nLCBkaXIpIHtcbiAgdmFyIGluZGV4U2l6ZSA9IDFcbiAgdmFyIGFyckxlbmd0aCA9IGFyci5sZW5ndGhcbiAgdmFyIHZhbExlbmd0aCA9IHZhbC5sZW5ndGhcblxuICBpZiAoZW5jb2RpbmcgIT09IHVuZGVmaW5lZCkge1xuICAgIGVuY29kaW5nID0gU3RyaW5nKGVuY29kaW5nKS50b0xvd2VyQ2FzZSgpXG4gICAgaWYgKGVuY29kaW5nID09PSAndWNzMicgfHwgZW5jb2RpbmcgPT09ICd1Y3MtMicgfHxcbiAgICAgICAgZW5jb2RpbmcgPT09ICd1dGYxNmxlJyB8fCBlbmNvZGluZyA9PT0gJ3V0Zi0xNmxlJykge1xuICAgICAgaWYgKGFyci5sZW5ndGggPCAyIHx8IHZhbC5sZW5ndGggPCAyKSB7XG4gICAgICAgIHJldHVybiAtMVxuICAgICAgfVxuICAgICAgaW5kZXhTaXplID0gMlxuICAgICAgYXJyTGVuZ3RoIC89IDJcbiAgICAgIHZhbExlbmd0aCAvPSAyXG4gICAgICBieXRlT2Zmc2V0IC89IDJcbiAgICB9XG4gIH1cblxuICBmdW5jdGlvbiByZWFkIChidWYsIGkpIHtcbiAgICBpZiAoaW5kZXhTaXplID09PSAxKSB7XG4gICAgICByZXR1cm4gYnVmW2ldXG4gICAgfSBlbHNlIHtcbiAgICAgIHJldHVybiBidWYucmVhZFVJbnQxNkJFKGkgKiBpbmRleFNpemUpXG4gICAgfVxuICB9XG5cbiAgdmFyIGlcbiAgaWYgKGRpcikge1xuICAgIHZhciBmb3VuZEluZGV4ID0gLTFcbiAgICBmb3IgKGkgPSBieXRlT2Zmc2V0OyBpIDwgYXJyTGVuZ3RoOyBpKyspIHtcbiAgICAgIGlmIChyZWFkKGFyciwgaSkgPT09IHJlYWQodmFsLCBmb3VuZEluZGV4ID09PSAtMSA/IDAgOiBpIC0gZm91bmRJbmRleCkpIHtcbiAgICAgICAgaWYgKGZvdW5kSW5kZXggPT09IC0xKSBmb3VuZEluZGV4ID0gaVxuICAgICAgICBpZiAoaSAtIGZvdW5kSW5kZXggKyAxID09PSB2YWxMZW5ndGgpIHJldHVybiBmb3VuZEluZGV4ICogaW5kZXhTaXplXG4gICAgICB9IGVsc2Uge1xuICAgICAgICBpZiAoZm91bmRJbmRleCAhPT0gLTEpIGkgLT0gaSAtIGZvdW5kSW5kZXhcbiAgICAgICAgZm91bmRJbmRleCA9IC0xXG4gICAgICB9XG4gICAgfVxuICB9IGVsc2Uge1xuICAgIGlmIChieXRlT2Zmc2V0ICsgdmFsTGVuZ3RoID4gYXJyTGVuZ3RoKSBieXRlT2Zmc2V0ID0gYXJyTGVuZ3RoIC0gdmFsTGVuZ3RoXG4gICAgZm9yIChpID0gYnl0ZU9mZnNldDsgaSA+PSAwOyBpLS0pIHtcbiAgICAgIHZhciBmb3VuZCA9IHRydWVcbiAgICAgIGZvciAodmFyIGogPSAwOyBqIDwgdmFsTGVuZ3RoOyBqKyspIHtcbiAgICAgICAgaWYgKHJlYWQoYXJyLCBpICsgaikgIT09IHJlYWQodmFsLCBqKSkge1xuICAgICAgICAgIGZvdW5kID0gZmFsc2VcbiAgICAgICAgICBicmVha1xuICAgICAgICB9XG4gICAgICB9XG4gICAgICBpZiAoZm91bmQpIHJldHVybiBpXG4gICAgfVxuICB9XG5cbiAgcmV0dXJuIC0xXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUuaW5jbHVkZXMgPSBmdW5jdGlvbiBpbmNsdWRlcyAodmFsLCBieXRlT2Zmc2V0LCBlbmNvZGluZykge1xuICByZXR1cm4gdGhpcy5pbmRleE9mKHZhbCwgYnl0ZU9mZnNldCwgZW5jb2RpbmcpICE9PSAtMVxufVxuXG5CdWZmZXIucHJvdG90eXBlLmluZGV4T2YgPSBmdW5jdGlvbiBpbmRleE9mICh2YWwsIGJ5dGVPZmZzZXQsIGVuY29kaW5nKSB7XG4gIHJldHVybiBiaWRpcmVjdGlvbmFsSW5kZXhPZih0aGlzLCB2YWwsIGJ5dGVPZmZzZXQsIGVuY29kaW5nLCB0cnVlKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLmxhc3RJbmRleE9mID0gZnVuY3Rpb24gbGFzdEluZGV4T2YgKHZhbCwgYnl0ZU9mZnNldCwgZW5jb2RpbmcpIHtcbiAgcmV0dXJuIGJpZGlyZWN0aW9uYWxJbmRleE9mKHRoaXMsIHZhbCwgYnl0ZU9mZnNldCwgZW5jb2RpbmcsIGZhbHNlKVxufVxuXG5mdW5jdGlvbiBoZXhXcml0ZSAoYnVmLCBzdHJpbmcsIG9mZnNldCwgbGVuZ3RoKSB7XG4gIG9mZnNldCA9IE51bWJlcihvZmZzZXQpIHx8IDBcbiAgdmFyIHJlbWFpbmluZyA9IGJ1Zi5sZW5ndGggLSBvZmZzZXRcbiAgaWYgKCFsZW5ndGgpIHtcbiAgICBsZW5ndGggPSByZW1haW5pbmdcbiAgfSBlbHNlIHtcbiAgICBsZW5ndGggPSBOdW1iZXIobGVuZ3RoKVxuICAgIGlmIChsZW5ndGggPiByZW1haW5pbmcpIHtcbiAgICAgIGxlbmd0aCA9IHJlbWFpbmluZ1xuICAgIH1cbiAgfVxuXG4gIC8vIG11c3QgYmUgYW4gZXZlbiBudW1iZXIgb2YgZGlnaXRzXG4gIHZhciBzdHJMZW4gPSBzdHJpbmcubGVuZ3RoXG4gIGlmIChzdHJMZW4gJSAyICE9PSAwKSB0aHJvdyBuZXcgVHlwZUVycm9yKCdJbnZhbGlkIGhleCBzdHJpbmcnKVxuXG4gIGlmIChsZW5ndGggPiBzdHJMZW4gLyAyKSB7XG4gICAgbGVuZ3RoID0gc3RyTGVuIC8gMlxuICB9XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgbGVuZ3RoOyArK2kpIHtcbiAgICB2YXIgcGFyc2VkID0gcGFyc2VJbnQoc3RyaW5nLnN1YnN0cihpICogMiwgMiksIDE2KVxuICAgIGlmIChpc05hTihwYXJzZWQpKSByZXR1cm4gaVxuICAgIGJ1ZltvZmZzZXQgKyBpXSA9IHBhcnNlZFxuICB9XG4gIHJldHVybiBpXG59XG5cbmZ1bmN0aW9uIHV0ZjhXcml0ZSAoYnVmLCBzdHJpbmcsIG9mZnNldCwgbGVuZ3RoKSB7XG4gIHJldHVybiBibGl0QnVmZmVyKHV0ZjhUb0J5dGVzKHN0cmluZywgYnVmLmxlbmd0aCAtIG9mZnNldCksIGJ1Ziwgb2Zmc2V0LCBsZW5ndGgpXG59XG5cbmZ1bmN0aW9uIGFzY2lpV3JpdGUgKGJ1Ziwgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aCkge1xuICByZXR1cm4gYmxpdEJ1ZmZlcihhc2NpaVRvQnl0ZXMoc3RyaW5nKSwgYnVmLCBvZmZzZXQsIGxlbmd0aClcbn1cblxuZnVuY3Rpb24gbGF0aW4xV3JpdGUgKGJ1Ziwgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aCkge1xuICByZXR1cm4gYXNjaWlXcml0ZShidWYsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpXG59XG5cbmZ1bmN0aW9uIGJhc2U2NFdyaXRlIChidWYsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpIHtcbiAgcmV0dXJuIGJsaXRCdWZmZXIoYmFzZTY0VG9CeXRlcyhzdHJpbmcpLCBidWYsIG9mZnNldCwgbGVuZ3RoKVxufVxuXG5mdW5jdGlvbiB1Y3MyV3JpdGUgKGJ1Ziwgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aCkge1xuICByZXR1cm4gYmxpdEJ1ZmZlcih1dGYxNmxlVG9CeXRlcyhzdHJpbmcsIGJ1Zi5sZW5ndGggLSBvZmZzZXQpLCBidWYsIG9mZnNldCwgbGVuZ3RoKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlID0gZnVuY3Rpb24gd3JpdGUgKHN0cmluZywgb2Zmc2V0LCBsZW5ndGgsIGVuY29kaW5nKSB7XG4gIC8vIEJ1ZmZlciN3cml0ZShzdHJpbmcpXG4gIGlmIChvZmZzZXQgPT09IHVuZGVmaW5lZCkge1xuICAgIGVuY29kaW5nID0gJ3V0ZjgnXG4gICAgbGVuZ3RoID0gdGhpcy5sZW5ndGhcbiAgICBvZmZzZXQgPSAwXG4gIC8vIEJ1ZmZlciN3cml0ZShzdHJpbmcsIGVuY29kaW5nKVxuICB9IGVsc2UgaWYgKGxlbmd0aCA9PT0gdW5kZWZpbmVkICYmIHR5cGVvZiBvZmZzZXQgPT09ICdzdHJpbmcnKSB7XG4gICAgZW5jb2RpbmcgPSBvZmZzZXRcbiAgICBsZW5ndGggPSB0aGlzLmxlbmd0aFxuICAgIG9mZnNldCA9IDBcbiAgLy8gQnVmZmVyI3dyaXRlKHN0cmluZywgb2Zmc2V0WywgbGVuZ3RoXVssIGVuY29kaW5nXSlcbiAgfSBlbHNlIGlmIChpc0Zpbml0ZShvZmZzZXQpKSB7XG4gICAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICAgIGlmIChpc0Zpbml0ZShsZW5ndGgpKSB7XG4gICAgICBsZW5ndGggPSBsZW5ndGggfCAwXG4gICAgICBpZiAoZW5jb2RpbmcgPT09IHVuZGVmaW5lZCkgZW5jb2RpbmcgPSAndXRmOCdcbiAgICB9IGVsc2Uge1xuICAgICAgZW5jb2RpbmcgPSBsZW5ndGhcbiAgICAgIGxlbmd0aCA9IHVuZGVmaW5lZFxuICAgIH1cbiAgLy8gbGVnYWN5IHdyaXRlKHN0cmluZywgZW5jb2RpbmcsIG9mZnNldCwgbGVuZ3RoKSAtIHJlbW92ZSBpbiB2MC4xM1xuICB9IGVsc2Uge1xuICAgIHRocm93IG5ldyBFcnJvcihcbiAgICAgICdCdWZmZXIud3JpdGUoc3RyaW5nLCBlbmNvZGluZywgb2Zmc2V0WywgbGVuZ3RoXSkgaXMgbm8gbG9uZ2VyIHN1cHBvcnRlZCdcbiAgICApXG4gIH1cblxuICB2YXIgcmVtYWluaW5nID0gdGhpcy5sZW5ndGggLSBvZmZzZXRcbiAgaWYgKGxlbmd0aCA9PT0gdW5kZWZpbmVkIHx8IGxlbmd0aCA+IHJlbWFpbmluZykgbGVuZ3RoID0gcmVtYWluaW5nXG5cbiAgaWYgKChzdHJpbmcubGVuZ3RoID4gMCAmJiAobGVuZ3RoIDwgMCB8fCBvZmZzZXQgPCAwKSkgfHwgb2Zmc2V0ID4gdGhpcy5sZW5ndGgpIHtcbiAgICB0aHJvdyBuZXcgUmFuZ2VFcnJvcignQXR0ZW1wdCB0byB3cml0ZSBvdXRzaWRlIGJ1ZmZlciBib3VuZHMnKVxuICB9XG5cbiAgaWYgKCFlbmNvZGluZykgZW5jb2RpbmcgPSAndXRmOCdcblxuICB2YXIgbG93ZXJlZENhc2UgPSBmYWxzZVxuICBmb3IgKDs7KSB7XG4gICAgc3dpdGNoIChlbmNvZGluZykge1xuICAgICAgY2FzZSAnaGV4JzpcbiAgICAgICAgcmV0dXJuIGhleFdyaXRlKHRoaXMsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpXG5cbiAgICAgIGNhc2UgJ3V0ZjgnOlxuICAgICAgY2FzZSAndXRmLTgnOlxuICAgICAgICByZXR1cm4gdXRmOFdyaXRlKHRoaXMsIHN0cmluZywgb2Zmc2V0LCBsZW5ndGgpXG5cbiAgICAgIGNhc2UgJ2FzY2lpJzpcbiAgICAgICAgcmV0dXJuIGFzY2lpV3JpdGUodGhpcywgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aClcblxuICAgICAgY2FzZSAnbGF0aW4xJzpcbiAgICAgIGNhc2UgJ2JpbmFyeSc6XG4gICAgICAgIHJldHVybiBsYXRpbjFXcml0ZSh0aGlzLCBzdHJpbmcsIG9mZnNldCwgbGVuZ3RoKVxuXG4gICAgICBjYXNlICdiYXNlNjQnOlxuICAgICAgICAvLyBXYXJuaW5nOiBtYXhMZW5ndGggbm90IHRha2VuIGludG8gYWNjb3VudCBpbiBiYXNlNjRXcml0ZVxuICAgICAgICByZXR1cm4gYmFzZTY0V3JpdGUodGhpcywgc3RyaW5nLCBvZmZzZXQsIGxlbmd0aClcblxuICAgICAgY2FzZSAndWNzMic6XG4gICAgICBjYXNlICd1Y3MtMic6XG4gICAgICBjYXNlICd1dGYxNmxlJzpcbiAgICAgIGNhc2UgJ3V0Zi0xNmxlJzpcbiAgICAgICAgcmV0dXJuIHVjczJXcml0ZSh0aGlzLCBzdHJpbmcsIG9mZnNldCwgbGVuZ3RoKVxuXG4gICAgICBkZWZhdWx0OlxuICAgICAgICBpZiAobG93ZXJlZENhc2UpIHRocm93IG5ldyBUeXBlRXJyb3IoJ1Vua25vd24gZW5jb2Rpbmc6ICcgKyBlbmNvZGluZylcbiAgICAgICAgZW5jb2RpbmcgPSAoJycgKyBlbmNvZGluZykudG9Mb3dlckNhc2UoKVxuICAgICAgICBsb3dlcmVkQ2FzZSA9IHRydWVcbiAgICB9XG4gIH1cbn1cblxuQnVmZmVyLnByb3RvdHlwZS50b0pTT04gPSBmdW5jdGlvbiB0b0pTT04gKCkge1xuICByZXR1cm4ge1xuICAgIHR5cGU6ICdCdWZmZXInLFxuICAgIGRhdGE6IEFycmF5LnByb3RvdHlwZS5zbGljZS5jYWxsKHRoaXMuX2FyciB8fCB0aGlzLCAwKVxuICB9XG59XG5cbmZ1bmN0aW9uIGJhc2U2NFNsaWNlIChidWYsIHN0YXJ0LCBlbmQpIHtcbiAgaWYgKHN0YXJ0ID09PSAwICYmIGVuZCA9PT0gYnVmLmxlbmd0aCkge1xuICAgIHJldHVybiBiYXNlNjQuZnJvbUJ5dGVBcnJheShidWYpXG4gIH0gZWxzZSB7XG4gICAgcmV0dXJuIGJhc2U2NC5mcm9tQnl0ZUFycmF5KGJ1Zi5zbGljZShzdGFydCwgZW5kKSlcbiAgfVxufVxuXG5mdW5jdGlvbiB1dGY4U2xpY2UgKGJ1Ziwgc3RhcnQsIGVuZCkge1xuICBlbmQgPSBNYXRoLm1pbihidWYubGVuZ3RoLCBlbmQpXG4gIHZhciByZXMgPSBbXVxuXG4gIHZhciBpID0gc3RhcnRcbiAgd2hpbGUgKGkgPCBlbmQpIHtcbiAgICB2YXIgZmlyc3RCeXRlID0gYnVmW2ldXG4gICAgdmFyIGNvZGVQb2ludCA9IG51bGxcbiAgICB2YXIgYnl0ZXNQZXJTZXF1ZW5jZSA9IChmaXJzdEJ5dGUgPiAweEVGKSA/IDRcbiAgICAgIDogKGZpcnN0Qnl0ZSA+IDB4REYpID8gM1xuICAgICAgOiAoZmlyc3RCeXRlID4gMHhCRikgPyAyXG4gICAgICA6IDFcblxuICAgIGlmIChpICsgYnl0ZXNQZXJTZXF1ZW5jZSA8PSBlbmQpIHtcbiAgICAgIHZhciBzZWNvbmRCeXRlLCB0aGlyZEJ5dGUsIGZvdXJ0aEJ5dGUsIHRlbXBDb2RlUG9pbnRcblxuICAgICAgc3dpdGNoIChieXRlc1BlclNlcXVlbmNlKSB7XG4gICAgICAgIGNhc2UgMTpcbiAgICAgICAgICBpZiAoZmlyc3RCeXRlIDwgMHg4MCkge1xuICAgICAgICAgICAgY29kZVBvaW50ID0gZmlyc3RCeXRlXG4gICAgICAgICAgfVxuICAgICAgICAgIGJyZWFrXG4gICAgICAgIGNhc2UgMjpcbiAgICAgICAgICBzZWNvbmRCeXRlID0gYnVmW2kgKyAxXVxuICAgICAgICAgIGlmICgoc2Vjb25kQnl0ZSAmIDB4QzApID09PSAweDgwKSB7XG4gICAgICAgICAgICB0ZW1wQ29kZVBvaW50ID0gKGZpcnN0Qnl0ZSAmIDB4MUYpIDw8IDB4NiB8IChzZWNvbmRCeXRlICYgMHgzRilcbiAgICAgICAgICAgIGlmICh0ZW1wQ29kZVBvaW50ID4gMHg3Rikge1xuICAgICAgICAgICAgICBjb2RlUG9pbnQgPSB0ZW1wQ29kZVBvaW50XG4gICAgICAgICAgICB9XG4gICAgICAgICAgfVxuICAgICAgICAgIGJyZWFrXG4gICAgICAgIGNhc2UgMzpcbiAgICAgICAgICBzZWNvbmRCeXRlID0gYnVmW2kgKyAxXVxuICAgICAgICAgIHRoaXJkQnl0ZSA9IGJ1ZltpICsgMl1cbiAgICAgICAgICBpZiAoKHNlY29uZEJ5dGUgJiAweEMwKSA9PT0gMHg4MCAmJiAodGhpcmRCeXRlICYgMHhDMCkgPT09IDB4ODApIHtcbiAgICAgICAgICAgIHRlbXBDb2RlUG9pbnQgPSAoZmlyc3RCeXRlICYgMHhGKSA8PCAweEMgfCAoc2Vjb25kQnl0ZSAmIDB4M0YpIDw8IDB4NiB8ICh0aGlyZEJ5dGUgJiAweDNGKVxuICAgICAgICAgICAgaWYgKHRlbXBDb2RlUG9pbnQgPiAweDdGRiAmJiAodGVtcENvZGVQb2ludCA8IDB4RDgwMCB8fCB0ZW1wQ29kZVBvaW50ID4gMHhERkZGKSkge1xuICAgICAgICAgICAgICBjb2RlUG9pbnQgPSB0ZW1wQ29kZVBvaW50XG4gICAgICAgICAgICB9XG4gICAgICAgICAgfVxuICAgICAgICAgIGJyZWFrXG4gICAgICAgIGNhc2UgNDpcbiAgICAgICAgICBzZWNvbmRCeXRlID0gYnVmW2kgKyAxXVxuICAgICAgICAgIHRoaXJkQnl0ZSA9IGJ1ZltpICsgMl1cbiAgICAgICAgICBmb3VydGhCeXRlID0gYnVmW2kgKyAzXVxuICAgICAgICAgIGlmICgoc2Vjb25kQnl0ZSAmIDB4QzApID09PSAweDgwICYmICh0aGlyZEJ5dGUgJiAweEMwKSA9PT0gMHg4MCAmJiAoZm91cnRoQnl0ZSAmIDB4QzApID09PSAweDgwKSB7XG4gICAgICAgICAgICB0ZW1wQ29kZVBvaW50ID0gKGZpcnN0Qnl0ZSAmIDB4RikgPDwgMHgxMiB8IChzZWNvbmRCeXRlICYgMHgzRikgPDwgMHhDIHwgKHRoaXJkQnl0ZSAmIDB4M0YpIDw8IDB4NiB8IChmb3VydGhCeXRlICYgMHgzRilcbiAgICAgICAgICAgIGlmICh0ZW1wQ29kZVBvaW50ID4gMHhGRkZGICYmIHRlbXBDb2RlUG9pbnQgPCAweDExMDAwMCkge1xuICAgICAgICAgICAgICBjb2RlUG9pbnQgPSB0ZW1wQ29kZVBvaW50XG4gICAgICAgICAgICB9XG4gICAgICAgICAgfVxuICAgICAgfVxuICAgIH1cblxuICAgIGlmIChjb2RlUG9pbnQgPT09IG51bGwpIHtcbiAgICAgIC8vIHdlIGRpZCBub3QgZ2VuZXJhdGUgYSB2YWxpZCBjb2RlUG9pbnQgc28gaW5zZXJ0IGFcbiAgICAgIC8vIHJlcGxhY2VtZW50IGNoYXIgKFUrRkZGRCkgYW5kIGFkdmFuY2Ugb25seSAxIGJ5dGVcbiAgICAgIGNvZGVQb2ludCA9IDB4RkZGRFxuICAgICAgYnl0ZXNQZXJTZXF1ZW5jZSA9IDFcbiAgICB9IGVsc2UgaWYgKGNvZGVQb2ludCA+IDB4RkZGRikge1xuICAgICAgLy8gZW5jb2RlIHRvIHV0ZjE2IChzdXJyb2dhdGUgcGFpciBkYW5jZSlcbiAgICAgIGNvZGVQb2ludCAtPSAweDEwMDAwXG4gICAgICByZXMucHVzaChjb2RlUG9pbnQgPj4+IDEwICYgMHgzRkYgfCAweEQ4MDApXG4gICAgICBjb2RlUG9pbnQgPSAweERDMDAgfCBjb2RlUG9pbnQgJiAweDNGRlxuICAgIH1cblxuICAgIHJlcy5wdXNoKGNvZGVQb2ludClcbiAgICBpICs9IGJ5dGVzUGVyU2VxdWVuY2VcbiAgfVxuXG4gIHJldHVybiBkZWNvZGVDb2RlUG9pbnRzQXJyYXkocmVzKVxufVxuXG4vLyBCYXNlZCBvbiBodHRwOi8vc3RhY2tvdmVyZmxvdy5jb20vYS8yMjc0NzI3Mi82ODA3NDIsIHRoZSBicm93c2VyIHdpdGhcbi8vIHRoZSBsb3dlc3QgbGltaXQgaXMgQ2hyb21lLCB3aXRoIDB4MTAwMDAgYXJncy5cbi8vIFdlIGdvIDEgbWFnbml0dWRlIGxlc3MsIGZvciBzYWZldHlcbnZhciBNQVhfQVJHVU1FTlRTX0xFTkdUSCA9IDB4MTAwMFxuXG5mdW5jdGlvbiBkZWNvZGVDb2RlUG9pbnRzQXJyYXkgKGNvZGVQb2ludHMpIHtcbiAgdmFyIGxlbiA9IGNvZGVQb2ludHMubGVuZ3RoXG4gIGlmIChsZW4gPD0gTUFYX0FSR1VNRU5UU19MRU5HVEgpIHtcbiAgICByZXR1cm4gU3RyaW5nLmZyb21DaGFyQ29kZS5hcHBseShTdHJpbmcsIGNvZGVQb2ludHMpIC8vIGF2b2lkIGV4dHJhIHNsaWNlKClcbiAgfVxuXG4gIC8vIERlY29kZSBpbiBjaHVua3MgdG8gYXZvaWQgXCJjYWxsIHN0YWNrIHNpemUgZXhjZWVkZWRcIi5cbiAgdmFyIHJlcyA9ICcnXG4gIHZhciBpID0gMFxuICB3aGlsZSAoaSA8IGxlbikge1xuICAgIHJlcyArPSBTdHJpbmcuZnJvbUNoYXJDb2RlLmFwcGx5KFxuICAgICAgU3RyaW5nLFxuICAgICAgY29kZVBvaW50cy5zbGljZShpLCBpICs9IE1BWF9BUkdVTUVOVFNfTEVOR1RIKVxuICAgIClcbiAgfVxuICByZXR1cm4gcmVzXG59XG5cbmZ1bmN0aW9uIGFzY2lpU2xpY2UgKGJ1Ziwgc3RhcnQsIGVuZCkge1xuICB2YXIgcmV0ID0gJydcbiAgZW5kID0gTWF0aC5taW4oYnVmLmxlbmd0aCwgZW5kKVxuXG4gIGZvciAodmFyIGkgPSBzdGFydDsgaSA8IGVuZDsgKytpKSB7XG4gICAgcmV0ICs9IFN0cmluZy5mcm9tQ2hhckNvZGUoYnVmW2ldICYgMHg3RilcbiAgfVxuICByZXR1cm4gcmV0XG59XG5cbmZ1bmN0aW9uIGxhdGluMVNsaWNlIChidWYsIHN0YXJ0LCBlbmQpIHtcbiAgdmFyIHJldCA9ICcnXG4gIGVuZCA9IE1hdGgubWluKGJ1Zi5sZW5ndGgsIGVuZClcblxuICBmb3IgKHZhciBpID0gc3RhcnQ7IGkgPCBlbmQ7ICsraSkge1xuICAgIHJldCArPSBTdHJpbmcuZnJvbUNoYXJDb2RlKGJ1ZltpXSlcbiAgfVxuICByZXR1cm4gcmV0XG59XG5cbmZ1bmN0aW9uIGhleFNsaWNlIChidWYsIHN0YXJ0LCBlbmQpIHtcbiAgdmFyIGxlbiA9IGJ1Zi5sZW5ndGhcblxuICBpZiAoIXN0YXJ0IHx8IHN0YXJ0IDwgMCkgc3RhcnQgPSAwXG4gIGlmICghZW5kIHx8IGVuZCA8IDAgfHwgZW5kID4gbGVuKSBlbmQgPSBsZW5cblxuICB2YXIgb3V0ID0gJydcbiAgZm9yICh2YXIgaSA9IHN0YXJ0OyBpIDwgZW5kOyArK2kpIHtcbiAgICBvdXQgKz0gdG9IZXgoYnVmW2ldKVxuICB9XG4gIHJldHVybiBvdXRcbn1cblxuZnVuY3Rpb24gdXRmMTZsZVNsaWNlIChidWYsIHN0YXJ0LCBlbmQpIHtcbiAgdmFyIGJ5dGVzID0gYnVmLnNsaWNlKHN0YXJ0LCBlbmQpXG4gIHZhciByZXMgPSAnJ1xuICBmb3IgKHZhciBpID0gMDsgaSA8IGJ5dGVzLmxlbmd0aDsgaSArPSAyKSB7XG4gICAgcmVzICs9IFN0cmluZy5mcm9tQ2hhckNvZGUoYnl0ZXNbaV0gKyBieXRlc1tpICsgMV0gKiAyNTYpXG4gIH1cbiAgcmV0dXJuIHJlc1xufVxuXG5CdWZmZXIucHJvdG90eXBlLnNsaWNlID0gZnVuY3Rpb24gc2xpY2UgKHN0YXJ0LCBlbmQpIHtcbiAgdmFyIGxlbiA9IHRoaXMubGVuZ3RoXG4gIHN0YXJ0ID0gfn5zdGFydFxuICBlbmQgPSBlbmQgPT09IHVuZGVmaW5lZCA/IGxlbiA6IH5+ZW5kXG5cbiAgaWYgKHN0YXJ0IDwgMCkge1xuICAgIHN0YXJ0ICs9IGxlblxuICAgIGlmIChzdGFydCA8IDApIHN0YXJ0ID0gMFxuICB9IGVsc2UgaWYgKHN0YXJ0ID4gbGVuKSB7XG4gICAgc3RhcnQgPSBsZW5cbiAgfVxuXG4gIGlmIChlbmQgPCAwKSB7XG4gICAgZW5kICs9IGxlblxuICAgIGlmIChlbmQgPCAwKSBlbmQgPSAwXG4gIH0gZWxzZSBpZiAoZW5kID4gbGVuKSB7XG4gICAgZW5kID0gbGVuXG4gIH1cblxuICBpZiAoZW5kIDwgc3RhcnQpIGVuZCA9IHN0YXJ0XG5cbiAgdmFyIG5ld0J1ZlxuICBpZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgICBuZXdCdWYgPSB0aGlzLnN1YmFycmF5KHN0YXJ0LCBlbmQpXG4gICAgbmV3QnVmLl9fcHJvdG9fXyA9IEJ1ZmZlci5wcm90b3R5cGVcbiAgfSBlbHNlIHtcbiAgICB2YXIgc2xpY2VMZW4gPSBlbmQgLSBzdGFydFxuICAgIG5ld0J1ZiA9IG5ldyBCdWZmZXIoc2xpY2VMZW4sIHVuZGVmaW5lZClcbiAgICBmb3IgKHZhciBpID0gMDsgaSA8IHNsaWNlTGVuOyArK2kpIHtcbiAgICAgIG5ld0J1ZltpXSA9IHRoaXNbaSArIHN0YXJ0XVxuICAgIH1cbiAgfVxuXG4gIHJldHVybiBuZXdCdWZcbn1cblxuLypcbiAqIE5lZWQgdG8gbWFrZSBzdXJlIHRoYXQgYnVmZmVyIGlzbid0IHRyeWluZyB0byB3cml0ZSBvdXQgb2YgYm91bmRzLlxuICovXG5mdW5jdGlvbiBjaGVja09mZnNldCAob2Zmc2V0LCBleHQsIGxlbmd0aCkge1xuICBpZiAoKG9mZnNldCAlIDEpICE9PSAwIHx8IG9mZnNldCA8IDApIHRocm93IG5ldyBSYW5nZUVycm9yKCdvZmZzZXQgaXMgbm90IHVpbnQnKVxuICBpZiAob2Zmc2V0ICsgZXh0ID4gbGVuZ3RoKSB0aHJvdyBuZXcgUmFuZ2VFcnJvcignVHJ5aW5nIHRvIGFjY2VzcyBiZXlvbmQgYnVmZmVyIGxlbmd0aCcpXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZFVJbnRMRSA9IGZ1bmN0aW9uIHJlYWRVSW50TEUgKG9mZnNldCwgYnl0ZUxlbmd0aCwgbm9Bc3NlcnQpIHtcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBieXRlTGVuZ3RoID0gYnl0ZUxlbmd0aCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCBieXRlTGVuZ3RoLCB0aGlzLmxlbmd0aClcblxuICB2YXIgdmFsID0gdGhpc1tvZmZzZXRdXG4gIHZhciBtdWwgPSAxXG4gIHZhciBpID0gMFxuICB3aGlsZSAoKytpIDwgYnl0ZUxlbmd0aCAmJiAobXVsICo9IDB4MTAwKSkge1xuICAgIHZhbCArPSB0aGlzW29mZnNldCArIGldICogbXVsXG4gIH1cblxuICByZXR1cm4gdmFsXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZFVJbnRCRSA9IGZ1bmN0aW9uIHJlYWRVSW50QkUgKG9mZnNldCwgYnl0ZUxlbmd0aCwgbm9Bc3NlcnQpIHtcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBieXRlTGVuZ3RoID0gYnl0ZUxlbmd0aCB8IDBcbiAgaWYgKCFub0Fzc2VydCkge1xuICAgIGNoZWNrT2Zmc2V0KG9mZnNldCwgYnl0ZUxlbmd0aCwgdGhpcy5sZW5ndGgpXG4gIH1cblxuICB2YXIgdmFsID0gdGhpc1tvZmZzZXQgKyAtLWJ5dGVMZW5ndGhdXG4gIHZhciBtdWwgPSAxXG4gIHdoaWxlIChieXRlTGVuZ3RoID4gMCAmJiAobXVsICo9IDB4MTAwKSkge1xuICAgIHZhbCArPSB0aGlzW29mZnNldCArIC0tYnl0ZUxlbmd0aF0gKiBtdWxcbiAgfVxuXG4gIHJldHVybiB2YWxcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5yZWFkVUludDggPSBmdW5jdGlvbiByZWFkVUludDggKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCAxLCB0aGlzLmxlbmd0aClcbiAgcmV0dXJuIHRoaXNbb2Zmc2V0XVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRVSW50MTZMRSA9IGZ1bmN0aW9uIHJlYWRVSW50MTZMRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDIsIHRoaXMubGVuZ3RoKVxuICByZXR1cm4gdGhpc1tvZmZzZXRdIHwgKHRoaXNbb2Zmc2V0ICsgMV0gPDwgOClcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5yZWFkVUludDE2QkUgPSBmdW5jdGlvbiByZWFkVUludDE2QkUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCAyLCB0aGlzLmxlbmd0aClcbiAgcmV0dXJuICh0aGlzW29mZnNldF0gPDwgOCkgfCB0aGlzW29mZnNldCArIDFdXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZFVJbnQzMkxFID0gZnVuY3Rpb24gcmVhZFVJbnQzMkxFIChvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgNCwgdGhpcy5sZW5ndGgpXG5cbiAgcmV0dXJuICgodGhpc1tvZmZzZXRdKSB8XG4gICAgICAodGhpc1tvZmZzZXQgKyAxXSA8PCA4KSB8XG4gICAgICAodGhpc1tvZmZzZXQgKyAyXSA8PCAxNikpICtcbiAgICAgICh0aGlzW29mZnNldCArIDNdICogMHgxMDAwMDAwKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRVSW50MzJCRSA9IGZ1bmN0aW9uIHJlYWRVSW50MzJCRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDQsIHRoaXMubGVuZ3RoKVxuXG4gIHJldHVybiAodGhpc1tvZmZzZXRdICogMHgxMDAwMDAwKSArXG4gICAgKCh0aGlzW29mZnNldCArIDFdIDw8IDE2KSB8XG4gICAgKHRoaXNbb2Zmc2V0ICsgMl0gPDwgOCkgfFxuICAgIHRoaXNbb2Zmc2V0ICsgM10pXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZEludExFID0gZnVuY3Rpb24gcmVhZEludExFIChvZmZzZXQsIGJ5dGVMZW5ndGgsIG5vQXNzZXJ0KSB7XG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgYnl0ZUxlbmd0aCA9IGJ5dGVMZW5ndGggfCAwXG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgYnl0ZUxlbmd0aCwgdGhpcy5sZW5ndGgpXG5cbiAgdmFyIHZhbCA9IHRoaXNbb2Zmc2V0XVxuICB2YXIgbXVsID0gMVxuICB2YXIgaSA9IDBcbiAgd2hpbGUgKCsraSA8IGJ5dGVMZW5ndGggJiYgKG11bCAqPSAweDEwMCkpIHtcbiAgICB2YWwgKz0gdGhpc1tvZmZzZXQgKyBpXSAqIG11bFxuICB9XG4gIG11bCAqPSAweDgwXG5cbiAgaWYgKHZhbCA+PSBtdWwpIHZhbCAtPSBNYXRoLnBvdygyLCA4ICogYnl0ZUxlbmd0aClcblxuICByZXR1cm4gdmFsXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZEludEJFID0gZnVuY3Rpb24gcmVhZEludEJFIChvZmZzZXQsIGJ5dGVMZW5ndGgsIG5vQXNzZXJ0KSB7XG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgYnl0ZUxlbmd0aCA9IGJ5dGVMZW5ndGggfCAwXG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgYnl0ZUxlbmd0aCwgdGhpcy5sZW5ndGgpXG5cbiAgdmFyIGkgPSBieXRlTGVuZ3RoXG4gIHZhciBtdWwgPSAxXG4gIHZhciB2YWwgPSB0aGlzW29mZnNldCArIC0taV1cbiAgd2hpbGUgKGkgPiAwICYmIChtdWwgKj0gMHgxMDApKSB7XG4gICAgdmFsICs9IHRoaXNbb2Zmc2V0ICsgLS1pXSAqIG11bFxuICB9XG4gIG11bCAqPSAweDgwXG5cbiAgaWYgKHZhbCA+PSBtdWwpIHZhbCAtPSBNYXRoLnBvdygyLCA4ICogYnl0ZUxlbmd0aClcblxuICByZXR1cm4gdmFsXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZEludDggPSBmdW5jdGlvbiByZWFkSW50OCAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDEsIHRoaXMubGVuZ3RoKVxuICBpZiAoISh0aGlzW29mZnNldF0gJiAweDgwKSkgcmV0dXJuICh0aGlzW29mZnNldF0pXG4gIHJldHVybiAoKDB4ZmYgLSB0aGlzW29mZnNldF0gKyAxKSAqIC0xKVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWRJbnQxNkxFID0gZnVuY3Rpb24gcmVhZEludDE2TEUgKG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tPZmZzZXQob2Zmc2V0LCAyLCB0aGlzLmxlbmd0aClcbiAgdmFyIHZhbCA9IHRoaXNbb2Zmc2V0XSB8ICh0aGlzW29mZnNldCArIDFdIDw8IDgpXG4gIHJldHVybiAodmFsICYgMHg4MDAwKSA/IHZhbCB8IDB4RkZGRjAwMDAgOiB2YWxcbn1cblxuQnVmZmVyLnByb3RvdHlwZS5yZWFkSW50MTZCRSA9IGZ1bmN0aW9uIHJlYWRJbnQxNkJFIChvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgMiwgdGhpcy5sZW5ndGgpXG4gIHZhciB2YWwgPSB0aGlzW29mZnNldCArIDFdIHwgKHRoaXNbb2Zmc2V0XSA8PCA4KVxuICByZXR1cm4gKHZhbCAmIDB4ODAwMCkgPyB2YWwgfCAweEZGRkYwMDAwIDogdmFsXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZEludDMyTEUgPSBmdW5jdGlvbiByZWFkSW50MzJMRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDQsIHRoaXMubGVuZ3RoKVxuXG4gIHJldHVybiAodGhpc1tvZmZzZXRdKSB8XG4gICAgKHRoaXNbb2Zmc2V0ICsgMV0gPDwgOCkgfFxuICAgICh0aGlzW29mZnNldCArIDJdIDw8IDE2KSB8XG4gICAgKHRoaXNbb2Zmc2V0ICsgM10gPDwgMjQpXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZEludDMyQkUgPSBmdW5jdGlvbiByZWFkSW50MzJCRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDQsIHRoaXMubGVuZ3RoKVxuXG4gIHJldHVybiAodGhpc1tvZmZzZXRdIDw8IDI0KSB8XG4gICAgKHRoaXNbb2Zmc2V0ICsgMV0gPDwgMTYpIHxcbiAgICAodGhpc1tvZmZzZXQgKyAyXSA8PCA4KSB8XG4gICAgKHRoaXNbb2Zmc2V0ICsgM10pXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZEZsb2F0TEUgPSBmdW5jdGlvbiByZWFkRmxvYXRMRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDQsIHRoaXMubGVuZ3RoKVxuICByZXR1cm4gaWVlZTc1NC5yZWFkKHRoaXMsIG9mZnNldCwgdHJ1ZSwgMjMsIDQpXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZEZsb2F0QkUgPSBmdW5jdGlvbiByZWFkRmxvYXRCRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDQsIHRoaXMubGVuZ3RoKVxuICByZXR1cm4gaWVlZTc1NC5yZWFkKHRoaXMsIG9mZnNldCwgZmFsc2UsIDIzLCA0KVxufVxuXG5CdWZmZXIucHJvdG90eXBlLnJlYWREb3VibGVMRSA9IGZ1bmN0aW9uIHJlYWREb3VibGVMRSAob2Zmc2V0LCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSBjaGVja09mZnNldChvZmZzZXQsIDgsIHRoaXMubGVuZ3RoKVxuICByZXR1cm4gaWVlZTc1NC5yZWFkKHRoaXMsIG9mZnNldCwgdHJ1ZSwgNTIsIDgpXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUucmVhZERvdWJsZUJFID0gZnVuY3Rpb24gcmVhZERvdWJsZUJFIChvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrT2Zmc2V0KG9mZnNldCwgOCwgdGhpcy5sZW5ndGgpXG4gIHJldHVybiBpZWVlNzU0LnJlYWQodGhpcywgb2Zmc2V0LCBmYWxzZSwgNTIsIDgpXG59XG5cbmZ1bmN0aW9uIGNoZWNrSW50IChidWYsIHZhbHVlLCBvZmZzZXQsIGV4dCwgbWF4LCBtaW4pIHtcbiAgaWYgKCFCdWZmZXIuaXNCdWZmZXIoYnVmKSkgdGhyb3cgbmV3IFR5cGVFcnJvcignXCJidWZmZXJcIiBhcmd1bWVudCBtdXN0IGJlIGEgQnVmZmVyIGluc3RhbmNlJylcbiAgaWYgKHZhbHVlID4gbWF4IHx8IHZhbHVlIDwgbWluKSB0aHJvdyBuZXcgUmFuZ2VFcnJvcignXCJ2YWx1ZVwiIGFyZ3VtZW50IGlzIG91dCBvZiBib3VuZHMnKVxuICBpZiAob2Zmc2V0ICsgZXh0ID4gYnVmLmxlbmd0aCkgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ0luZGV4IG91dCBvZiByYW5nZScpXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVVSW50TEUgPSBmdW5jdGlvbiB3cml0ZVVJbnRMRSAodmFsdWUsIG9mZnNldCwgYnl0ZUxlbmd0aCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBieXRlTGVuZ3RoID0gYnl0ZUxlbmd0aCB8IDBcbiAgaWYgKCFub0Fzc2VydCkge1xuICAgIHZhciBtYXhCeXRlcyA9IE1hdGgucG93KDIsIDggKiBieXRlTGVuZ3RoKSAtIDFcbiAgICBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCBieXRlTGVuZ3RoLCBtYXhCeXRlcywgMClcbiAgfVxuXG4gIHZhciBtdWwgPSAxXG4gIHZhciBpID0gMFxuICB0aGlzW29mZnNldF0gPSB2YWx1ZSAmIDB4RkZcbiAgd2hpbGUgKCsraSA8IGJ5dGVMZW5ndGggJiYgKG11bCAqPSAweDEwMCkpIHtcbiAgICB0aGlzW29mZnNldCArIGldID0gKHZhbHVlIC8gbXVsKSAmIDB4RkZcbiAgfVxuXG4gIHJldHVybiBvZmZzZXQgKyBieXRlTGVuZ3RoXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVVSW50QkUgPSBmdW5jdGlvbiB3cml0ZVVJbnRCRSAodmFsdWUsIG9mZnNldCwgYnl0ZUxlbmd0aCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBieXRlTGVuZ3RoID0gYnl0ZUxlbmd0aCB8IDBcbiAgaWYgKCFub0Fzc2VydCkge1xuICAgIHZhciBtYXhCeXRlcyA9IE1hdGgucG93KDIsIDggKiBieXRlTGVuZ3RoKSAtIDFcbiAgICBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCBieXRlTGVuZ3RoLCBtYXhCeXRlcywgMClcbiAgfVxuXG4gIHZhciBpID0gYnl0ZUxlbmd0aCAtIDFcbiAgdmFyIG11bCA9IDFcbiAgdGhpc1tvZmZzZXQgKyBpXSA9IHZhbHVlICYgMHhGRlxuICB3aGlsZSAoLS1pID49IDAgJiYgKG11bCAqPSAweDEwMCkpIHtcbiAgICB0aGlzW29mZnNldCArIGldID0gKHZhbHVlIC8gbXVsKSAmIDB4RkZcbiAgfVxuXG4gIHJldHVybiBvZmZzZXQgKyBieXRlTGVuZ3RoXG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVVSW50OCA9IGZ1bmN0aW9uIHdyaXRlVUludDggKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgMSwgMHhmZiwgMClcbiAgaWYgKCFCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkgdmFsdWUgPSBNYXRoLmZsb29yKHZhbHVlKVxuICB0aGlzW29mZnNldF0gPSAodmFsdWUgJiAweGZmKVxuICByZXR1cm4gb2Zmc2V0ICsgMVxufVxuXG5mdW5jdGlvbiBvYmplY3RXcml0ZVVJbnQxNiAoYnVmLCB2YWx1ZSwgb2Zmc2V0LCBsaXR0bGVFbmRpYW4pIHtcbiAgaWYgKHZhbHVlIDwgMCkgdmFsdWUgPSAweGZmZmYgKyB2YWx1ZSArIDFcbiAgZm9yICh2YXIgaSA9IDAsIGogPSBNYXRoLm1pbihidWYubGVuZ3RoIC0gb2Zmc2V0LCAyKTsgaSA8IGo7ICsraSkge1xuICAgIGJ1ZltvZmZzZXQgKyBpXSA9ICh2YWx1ZSAmICgweGZmIDw8ICg4ICogKGxpdHRsZUVuZGlhbiA/IGkgOiAxIC0gaSkpKSkgPj4+XG4gICAgICAobGl0dGxlRW5kaWFuID8gaSA6IDEgLSBpKSAqIDhcbiAgfVxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlVUludDE2TEUgPSBmdW5jdGlvbiB3cml0ZVVJbnQxNkxFICh2YWx1ZSwgb2Zmc2V0LCBub0Fzc2VydCkge1xuICB2YWx1ZSA9ICt2YWx1ZVxuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrSW50KHRoaXMsIHZhbHVlLCBvZmZzZXQsIDIsIDB4ZmZmZiwgMClcbiAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgdGhpc1tvZmZzZXRdID0gKHZhbHVlICYgMHhmZilcbiAgICB0aGlzW29mZnNldCArIDFdID0gKHZhbHVlID4+PiA4KVxuICB9IGVsc2Uge1xuICAgIG9iamVjdFdyaXRlVUludDE2KHRoaXMsIHZhbHVlLCBvZmZzZXQsIHRydWUpXG4gIH1cbiAgcmV0dXJuIG9mZnNldCArIDJcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZVVJbnQxNkJFID0gZnVuY3Rpb24gd3JpdGVVSW50MTZCRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCAyLCAweGZmZmYsIDApXG4gIGlmIChCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkge1xuICAgIHRoaXNbb2Zmc2V0XSA9ICh2YWx1ZSA+Pj4gOClcbiAgICB0aGlzW29mZnNldCArIDFdID0gKHZhbHVlICYgMHhmZilcbiAgfSBlbHNlIHtcbiAgICBvYmplY3RXcml0ZVVJbnQxNih0aGlzLCB2YWx1ZSwgb2Zmc2V0LCBmYWxzZSlcbiAgfVxuICByZXR1cm4gb2Zmc2V0ICsgMlxufVxuXG5mdW5jdGlvbiBvYmplY3RXcml0ZVVJbnQzMiAoYnVmLCB2YWx1ZSwgb2Zmc2V0LCBsaXR0bGVFbmRpYW4pIHtcbiAgaWYgKHZhbHVlIDwgMCkgdmFsdWUgPSAweGZmZmZmZmZmICsgdmFsdWUgKyAxXG4gIGZvciAodmFyIGkgPSAwLCBqID0gTWF0aC5taW4oYnVmLmxlbmd0aCAtIG9mZnNldCwgNCk7IGkgPCBqOyArK2kpIHtcbiAgICBidWZbb2Zmc2V0ICsgaV0gPSAodmFsdWUgPj4+IChsaXR0bGVFbmRpYW4gPyBpIDogMyAtIGkpICogOCkgJiAweGZmXG4gIH1cbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZVVJbnQzMkxFID0gZnVuY3Rpb24gd3JpdGVVSW50MzJMRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCA0LCAweGZmZmZmZmZmLCAwKVxuICBpZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgICB0aGlzW29mZnNldCArIDNdID0gKHZhbHVlID4+PiAyNClcbiAgICB0aGlzW29mZnNldCArIDJdID0gKHZhbHVlID4+PiAxNilcbiAgICB0aGlzW29mZnNldCArIDFdID0gKHZhbHVlID4+PiA4KVxuICAgIHRoaXNbb2Zmc2V0XSA9ICh2YWx1ZSAmIDB4ZmYpXG4gIH0gZWxzZSB7XG4gICAgb2JqZWN0V3JpdGVVSW50MzIodGhpcywgdmFsdWUsIG9mZnNldCwgdHJ1ZSlcbiAgfVxuICByZXR1cm4gb2Zmc2V0ICsgNFxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlVUludDMyQkUgPSBmdW5jdGlvbiB3cml0ZVVJbnQzMkJFICh2YWx1ZSwgb2Zmc2V0LCBub0Fzc2VydCkge1xuICB2YWx1ZSA9ICt2YWx1ZVxuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGlmICghbm9Bc3NlcnQpIGNoZWNrSW50KHRoaXMsIHZhbHVlLCBvZmZzZXQsIDQsIDB4ZmZmZmZmZmYsIDApXG4gIGlmIChCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkge1xuICAgIHRoaXNbb2Zmc2V0XSA9ICh2YWx1ZSA+Pj4gMjQpXG4gICAgdGhpc1tvZmZzZXQgKyAxXSA9ICh2YWx1ZSA+Pj4gMTYpXG4gICAgdGhpc1tvZmZzZXQgKyAyXSA9ICh2YWx1ZSA+Pj4gOClcbiAgICB0aGlzW29mZnNldCArIDNdID0gKHZhbHVlICYgMHhmZilcbiAgfSBlbHNlIHtcbiAgICBvYmplY3RXcml0ZVVJbnQzMih0aGlzLCB2YWx1ZSwgb2Zmc2V0LCBmYWxzZSlcbiAgfVxuICByZXR1cm4gb2Zmc2V0ICsgNFxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlSW50TEUgPSBmdW5jdGlvbiB3cml0ZUludExFICh2YWx1ZSwgb2Zmc2V0LCBieXRlTGVuZ3RoLCBub0Fzc2VydCkge1xuICB2YWx1ZSA9ICt2YWx1ZVxuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGlmICghbm9Bc3NlcnQpIHtcbiAgICB2YXIgbGltaXQgPSBNYXRoLnBvdygyLCA4ICogYnl0ZUxlbmd0aCAtIDEpXG5cbiAgICBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCBieXRlTGVuZ3RoLCBsaW1pdCAtIDEsIC1saW1pdClcbiAgfVxuXG4gIHZhciBpID0gMFxuICB2YXIgbXVsID0gMVxuICB2YXIgc3ViID0gMFxuICB0aGlzW29mZnNldF0gPSB2YWx1ZSAmIDB4RkZcbiAgd2hpbGUgKCsraSA8IGJ5dGVMZW5ndGggJiYgKG11bCAqPSAweDEwMCkpIHtcbiAgICBpZiAodmFsdWUgPCAwICYmIHN1YiA9PT0gMCAmJiB0aGlzW29mZnNldCArIGkgLSAxXSAhPT0gMCkge1xuICAgICAgc3ViID0gMVxuICAgIH1cbiAgICB0aGlzW29mZnNldCArIGldID0gKCh2YWx1ZSAvIG11bCkgPj4gMCkgLSBzdWIgJiAweEZGXG4gIH1cblxuICByZXR1cm4gb2Zmc2V0ICsgYnl0ZUxlbmd0aFxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlSW50QkUgPSBmdW5jdGlvbiB3cml0ZUludEJFICh2YWx1ZSwgb2Zmc2V0LCBieXRlTGVuZ3RoLCBub0Fzc2VydCkge1xuICB2YWx1ZSA9ICt2YWx1ZVxuICBvZmZzZXQgPSBvZmZzZXQgfCAwXG4gIGlmICghbm9Bc3NlcnQpIHtcbiAgICB2YXIgbGltaXQgPSBNYXRoLnBvdygyLCA4ICogYnl0ZUxlbmd0aCAtIDEpXG5cbiAgICBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCBieXRlTGVuZ3RoLCBsaW1pdCAtIDEsIC1saW1pdClcbiAgfVxuXG4gIHZhciBpID0gYnl0ZUxlbmd0aCAtIDFcbiAgdmFyIG11bCA9IDFcbiAgdmFyIHN1YiA9IDBcbiAgdGhpc1tvZmZzZXQgKyBpXSA9IHZhbHVlICYgMHhGRlxuICB3aGlsZSAoLS1pID49IDAgJiYgKG11bCAqPSAweDEwMCkpIHtcbiAgICBpZiAodmFsdWUgPCAwICYmIHN1YiA9PT0gMCAmJiB0aGlzW29mZnNldCArIGkgKyAxXSAhPT0gMCkge1xuICAgICAgc3ViID0gMVxuICAgIH1cbiAgICB0aGlzW29mZnNldCArIGldID0gKCh2YWx1ZSAvIG11bCkgPj4gMCkgLSBzdWIgJiAweEZGXG4gIH1cblxuICByZXR1cm4gb2Zmc2V0ICsgYnl0ZUxlbmd0aFxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlSW50OCA9IGZ1bmN0aW9uIHdyaXRlSW50OCAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCAxLCAweDdmLCAtMHg4MClcbiAgaWYgKCFCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkgdmFsdWUgPSBNYXRoLmZsb29yKHZhbHVlKVxuICBpZiAodmFsdWUgPCAwKSB2YWx1ZSA9IDB4ZmYgKyB2YWx1ZSArIDFcbiAgdGhpc1tvZmZzZXRdID0gKHZhbHVlICYgMHhmZilcbiAgcmV0dXJuIG9mZnNldCArIDFcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZUludDE2TEUgPSBmdW5jdGlvbiB3cml0ZUludDE2TEUgKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgMiwgMHg3ZmZmLCAtMHg4MDAwKVxuICBpZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgICB0aGlzW29mZnNldF0gPSAodmFsdWUgJiAweGZmKVxuICAgIHRoaXNbb2Zmc2V0ICsgMV0gPSAodmFsdWUgPj4+IDgpXG4gIH0gZWxzZSB7XG4gICAgb2JqZWN0V3JpdGVVSW50MTYodGhpcywgdmFsdWUsIG9mZnNldCwgdHJ1ZSlcbiAgfVxuICByZXR1cm4gb2Zmc2V0ICsgMlxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlSW50MTZCRSA9IGZ1bmN0aW9uIHdyaXRlSW50MTZCRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCAyLCAweDdmZmYsIC0weDgwMDApXG4gIGlmIChCdWZmZXIuVFlQRURfQVJSQVlfU1VQUE9SVCkge1xuICAgIHRoaXNbb2Zmc2V0XSA9ICh2YWx1ZSA+Pj4gOClcbiAgICB0aGlzW29mZnNldCArIDFdID0gKHZhbHVlICYgMHhmZilcbiAgfSBlbHNlIHtcbiAgICBvYmplY3RXcml0ZVVJbnQxNih0aGlzLCB2YWx1ZSwgb2Zmc2V0LCBmYWxzZSlcbiAgfVxuICByZXR1cm4gb2Zmc2V0ICsgMlxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlSW50MzJMRSA9IGZ1bmN0aW9uIHdyaXRlSW50MzJMRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgdmFsdWUgPSArdmFsdWVcbiAgb2Zmc2V0ID0gb2Zmc2V0IHwgMFxuICBpZiAoIW5vQXNzZXJ0KSBjaGVja0ludCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCA0LCAweDdmZmZmZmZmLCAtMHg4MDAwMDAwMClcbiAgaWYgKEJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgdGhpc1tvZmZzZXRdID0gKHZhbHVlICYgMHhmZilcbiAgICB0aGlzW29mZnNldCArIDFdID0gKHZhbHVlID4+PiA4KVxuICAgIHRoaXNbb2Zmc2V0ICsgMl0gPSAodmFsdWUgPj4+IDE2KVxuICAgIHRoaXNbb2Zmc2V0ICsgM10gPSAodmFsdWUgPj4+IDI0KVxuICB9IGVsc2Uge1xuICAgIG9iamVjdFdyaXRlVUludDMyKHRoaXMsIHZhbHVlLCBvZmZzZXQsIHRydWUpXG4gIH1cbiAgcmV0dXJuIG9mZnNldCArIDRcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZUludDMyQkUgPSBmdW5jdGlvbiB3cml0ZUludDMyQkUgKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHZhbHVlID0gK3ZhbHVlXG4gIG9mZnNldCA9IG9mZnNldCB8IDBcbiAgaWYgKCFub0Fzc2VydCkgY2hlY2tJbnQodGhpcywgdmFsdWUsIG9mZnNldCwgNCwgMHg3ZmZmZmZmZiwgLTB4ODAwMDAwMDApXG4gIGlmICh2YWx1ZSA8IDApIHZhbHVlID0gMHhmZmZmZmZmZiArIHZhbHVlICsgMVxuICBpZiAoQnVmZmVyLlRZUEVEX0FSUkFZX1NVUFBPUlQpIHtcbiAgICB0aGlzW29mZnNldF0gPSAodmFsdWUgPj4+IDI0KVxuICAgIHRoaXNbb2Zmc2V0ICsgMV0gPSAodmFsdWUgPj4+IDE2KVxuICAgIHRoaXNbb2Zmc2V0ICsgMl0gPSAodmFsdWUgPj4+IDgpXG4gICAgdGhpc1tvZmZzZXQgKyAzXSA9ICh2YWx1ZSAmIDB4ZmYpXG4gIH0gZWxzZSB7XG4gICAgb2JqZWN0V3JpdGVVSW50MzIodGhpcywgdmFsdWUsIG9mZnNldCwgZmFsc2UpXG4gIH1cbiAgcmV0dXJuIG9mZnNldCArIDRcbn1cblxuZnVuY3Rpb24gY2hlY2tJRUVFNzU0IChidWYsIHZhbHVlLCBvZmZzZXQsIGV4dCwgbWF4LCBtaW4pIHtcbiAgaWYgKG9mZnNldCArIGV4dCA+IGJ1Zi5sZW5ndGgpIHRocm93IG5ldyBSYW5nZUVycm9yKCdJbmRleCBvdXQgb2YgcmFuZ2UnKVxuICBpZiAob2Zmc2V0IDwgMCkgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ0luZGV4IG91dCBvZiByYW5nZScpXG59XG5cbmZ1bmN0aW9uIHdyaXRlRmxvYXQgKGJ1ZiwgdmFsdWUsIG9mZnNldCwgbGl0dGxlRW5kaWFuLCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSB7XG4gICAgY2hlY2tJRUVFNzU0KGJ1ZiwgdmFsdWUsIG9mZnNldCwgNCwgMy40MDI4MjM0NjYzODUyODg2ZSszOCwgLTMuNDAyODIzNDY2Mzg1Mjg4NmUrMzgpXG4gIH1cbiAgaWVlZTc1NC53cml0ZShidWYsIHZhbHVlLCBvZmZzZXQsIGxpdHRsZUVuZGlhbiwgMjMsIDQpXG4gIHJldHVybiBvZmZzZXQgKyA0XG59XG5cbkJ1ZmZlci5wcm90b3R5cGUud3JpdGVGbG9hdExFID0gZnVuY3Rpb24gd3JpdGVGbG9hdExFICh2YWx1ZSwgb2Zmc2V0LCBub0Fzc2VydCkge1xuICByZXR1cm4gd3JpdGVGbG9hdCh0aGlzLCB2YWx1ZSwgb2Zmc2V0LCB0cnVlLCBub0Fzc2VydClcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZUZsb2F0QkUgPSBmdW5jdGlvbiB3cml0ZUZsb2F0QkUgKHZhbHVlLCBvZmZzZXQsIG5vQXNzZXJ0KSB7XG4gIHJldHVybiB3cml0ZUZsb2F0KHRoaXMsIHZhbHVlLCBvZmZzZXQsIGZhbHNlLCBub0Fzc2VydClcbn1cblxuZnVuY3Rpb24gd3JpdGVEb3VibGUgKGJ1ZiwgdmFsdWUsIG9mZnNldCwgbGl0dGxlRW5kaWFuLCBub0Fzc2VydCkge1xuICBpZiAoIW5vQXNzZXJ0KSB7XG4gICAgY2hlY2tJRUVFNzU0KGJ1ZiwgdmFsdWUsIG9mZnNldCwgOCwgMS43OTc2OTMxMzQ4NjIzMTU3RSszMDgsIC0xLjc5NzY5MzEzNDg2MjMxNTdFKzMwOClcbiAgfVxuICBpZWVlNzU0LndyaXRlKGJ1ZiwgdmFsdWUsIG9mZnNldCwgbGl0dGxlRW5kaWFuLCA1MiwgOClcbiAgcmV0dXJuIG9mZnNldCArIDhcbn1cblxuQnVmZmVyLnByb3RvdHlwZS53cml0ZURvdWJsZUxFID0gZnVuY3Rpb24gd3JpdGVEb3VibGVMRSAodmFsdWUsIG9mZnNldCwgbm9Bc3NlcnQpIHtcbiAgcmV0dXJuIHdyaXRlRG91YmxlKHRoaXMsIHZhbHVlLCBvZmZzZXQsIHRydWUsIG5vQXNzZXJ0KVxufVxuXG5CdWZmZXIucHJvdG90eXBlLndyaXRlRG91YmxlQkUgPSBmdW5jdGlvbiB3cml0ZURvdWJsZUJFICh2YWx1ZSwgb2Zmc2V0LCBub0Fzc2VydCkge1xuICByZXR1cm4gd3JpdGVEb3VibGUodGhpcywgdmFsdWUsIG9mZnNldCwgZmFsc2UsIG5vQXNzZXJ0KVxufVxuXG4vLyBjb3B5KHRhcmdldEJ1ZmZlciwgdGFyZ2V0U3RhcnQ9MCwgc291cmNlU3RhcnQ9MCwgc291cmNlRW5kPWJ1ZmZlci5sZW5ndGgpXG5CdWZmZXIucHJvdG90eXBlLmNvcHkgPSBmdW5jdGlvbiBjb3B5ICh0YXJnZXQsIHRhcmdldFN0YXJ0LCBzdGFydCwgZW5kKSB7XG4gIGlmICghc3RhcnQpIHN0YXJ0ID0gMFxuICBpZiAoIWVuZCAmJiBlbmQgIT09IDApIGVuZCA9IHRoaXMubGVuZ3RoXG4gIGlmICh0YXJnZXRTdGFydCA+PSB0YXJnZXQubGVuZ3RoKSB0YXJnZXRTdGFydCA9IHRhcmdldC5sZW5ndGhcbiAgaWYgKCF0YXJnZXRTdGFydCkgdGFyZ2V0U3RhcnQgPSAwXG4gIGlmIChlbmQgPiAwICYmIGVuZCA8IHN0YXJ0KSBlbmQgPSBzdGFydFxuXG4gIC8vIENvcHkgMCBieXRlczsgd2UncmUgZG9uZVxuICBpZiAoZW5kID09PSBzdGFydCkgcmV0dXJuIDBcbiAgaWYgKHRhcmdldC5sZW5ndGggPT09IDAgfHwgdGhpcy5sZW5ndGggPT09IDApIHJldHVybiAwXG5cbiAgLy8gRmF0YWwgZXJyb3IgY29uZGl0aW9uc1xuICBpZiAodGFyZ2V0U3RhcnQgPCAwKSB7XG4gICAgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ3RhcmdldFN0YXJ0IG91dCBvZiBib3VuZHMnKVxuICB9XG4gIGlmIChzdGFydCA8IDAgfHwgc3RhcnQgPj0gdGhpcy5sZW5ndGgpIHRocm93IG5ldyBSYW5nZUVycm9yKCdzb3VyY2VTdGFydCBvdXQgb2YgYm91bmRzJylcbiAgaWYgKGVuZCA8IDApIHRocm93IG5ldyBSYW5nZUVycm9yKCdzb3VyY2VFbmQgb3V0IG9mIGJvdW5kcycpXG5cbiAgLy8gQXJlIHdlIG9vYj9cbiAgaWYgKGVuZCA+IHRoaXMubGVuZ3RoKSBlbmQgPSB0aGlzLmxlbmd0aFxuICBpZiAodGFyZ2V0Lmxlbmd0aCAtIHRhcmdldFN0YXJ0IDwgZW5kIC0gc3RhcnQpIHtcbiAgICBlbmQgPSB0YXJnZXQubGVuZ3RoIC0gdGFyZ2V0U3RhcnQgKyBzdGFydFxuICB9XG5cbiAgdmFyIGxlbiA9IGVuZCAtIHN0YXJ0XG4gIHZhciBpXG5cbiAgaWYgKHRoaXMgPT09IHRhcmdldCAmJiBzdGFydCA8IHRhcmdldFN0YXJ0ICYmIHRhcmdldFN0YXJ0IDwgZW5kKSB7XG4gICAgLy8gZGVzY2VuZGluZyBjb3B5IGZyb20gZW5kXG4gICAgZm9yIChpID0gbGVuIC0gMTsgaSA+PSAwOyAtLWkpIHtcbiAgICAgIHRhcmdldFtpICsgdGFyZ2V0U3RhcnRdID0gdGhpc1tpICsgc3RhcnRdXG4gICAgfVxuICB9IGVsc2UgaWYgKGxlbiA8IDEwMDAgfHwgIUJ1ZmZlci5UWVBFRF9BUlJBWV9TVVBQT1JUKSB7XG4gICAgLy8gYXNjZW5kaW5nIGNvcHkgZnJvbSBzdGFydFxuICAgIGZvciAoaSA9IDA7IGkgPCBsZW47ICsraSkge1xuICAgICAgdGFyZ2V0W2kgKyB0YXJnZXRTdGFydF0gPSB0aGlzW2kgKyBzdGFydF1cbiAgICB9XG4gIH0gZWxzZSB7XG4gICAgVWludDhBcnJheS5wcm90b3R5cGUuc2V0LmNhbGwoXG4gICAgICB0YXJnZXQsXG4gICAgICB0aGlzLnN1YmFycmF5KHN0YXJ0LCBzdGFydCArIGxlbiksXG4gICAgICB0YXJnZXRTdGFydFxuICAgIClcbiAgfVxuXG4gIHJldHVybiBsZW5cbn1cblxuLy8gVXNhZ2U6XG4vLyAgICBidWZmZXIuZmlsbChudW1iZXJbLCBvZmZzZXRbLCBlbmRdXSlcbi8vICAgIGJ1ZmZlci5maWxsKGJ1ZmZlclssIG9mZnNldFssIGVuZF1dKVxuLy8gICAgYnVmZmVyLmZpbGwoc3RyaW5nWywgb2Zmc2V0WywgZW5kXV1bLCBlbmNvZGluZ10pXG5CdWZmZXIucHJvdG90eXBlLmZpbGwgPSBmdW5jdGlvbiBmaWxsICh2YWwsIHN0YXJ0LCBlbmQsIGVuY29kaW5nKSB7XG4gIC8vIEhhbmRsZSBzdHJpbmcgY2FzZXM6XG4gIGlmICh0eXBlb2YgdmFsID09PSAnc3RyaW5nJykge1xuICAgIGlmICh0eXBlb2Ygc3RhcnQgPT09ICdzdHJpbmcnKSB7XG4gICAgICBlbmNvZGluZyA9IHN0YXJ0XG4gICAgICBzdGFydCA9IDBcbiAgICAgIGVuZCA9IHRoaXMubGVuZ3RoXG4gICAgfSBlbHNlIGlmICh0eXBlb2YgZW5kID09PSAnc3RyaW5nJykge1xuICAgICAgZW5jb2RpbmcgPSBlbmRcbiAgICAgIGVuZCA9IHRoaXMubGVuZ3RoXG4gICAgfVxuICAgIGlmICh2YWwubGVuZ3RoID09PSAxKSB7XG4gICAgICB2YXIgY29kZSA9IHZhbC5jaGFyQ29kZUF0KDApXG4gICAgICBpZiAoY29kZSA8IDI1Nikge1xuICAgICAgICB2YWwgPSBjb2RlXG4gICAgICB9XG4gICAgfVxuICAgIGlmIChlbmNvZGluZyAhPT0gdW5kZWZpbmVkICYmIHR5cGVvZiBlbmNvZGluZyAhPT0gJ3N0cmluZycpIHtcbiAgICAgIHRocm93IG5ldyBUeXBlRXJyb3IoJ2VuY29kaW5nIG11c3QgYmUgYSBzdHJpbmcnKVxuICAgIH1cbiAgICBpZiAodHlwZW9mIGVuY29kaW5nID09PSAnc3RyaW5nJyAmJiAhQnVmZmVyLmlzRW5jb2RpbmcoZW5jb2RpbmcpKSB7XG4gICAgICB0aHJvdyBuZXcgVHlwZUVycm9yKCdVbmtub3duIGVuY29kaW5nOiAnICsgZW5jb2RpbmcpXG4gICAgfVxuICB9IGVsc2UgaWYgKHR5cGVvZiB2YWwgPT09ICdudW1iZXInKSB7XG4gICAgdmFsID0gdmFsICYgMjU1XG4gIH1cblxuICAvLyBJbnZhbGlkIHJhbmdlcyBhcmUgbm90IHNldCB0byBhIGRlZmF1bHQsIHNvIGNhbiByYW5nZSBjaGVjayBlYXJseS5cbiAgaWYgKHN0YXJ0IDwgMCB8fCB0aGlzLmxlbmd0aCA8IHN0YXJ0IHx8IHRoaXMubGVuZ3RoIDwgZW5kKSB7XG4gICAgdGhyb3cgbmV3IFJhbmdlRXJyb3IoJ091dCBvZiByYW5nZSBpbmRleCcpXG4gIH1cblxuICBpZiAoZW5kIDw9IHN0YXJ0KSB7XG4gICAgcmV0dXJuIHRoaXNcbiAgfVxuXG4gIHN0YXJ0ID0gc3RhcnQgPj4+IDBcbiAgZW5kID0gZW5kID09PSB1bmRlZmluZWQgPyB0aGlzLmxlbmd0aCA6IGVuZCA+Pj4gMFxuXG4gIGlmICghdmFsKSB2YWwgPSAwXG5cbiAgdmFyIGlcbiAgaWYgKHR5cGVvZiB2YWwgPT09ICdudW1iZXInKSB7XG4gICAgZm9yIChpID0gc3RhcnQ7IGkgPCBlbmQ7ICsraSkge1xuICAgICAgdGhpc1tpXSA9IHZhbFxuICAgIH1cbiAgfSBlbHNlIHtcbiAgICB2YXIgYnl0ZXMgPSBCdWZmZXIuaXNCdWZmZXIodmFsKVxuICAgICAgPyB2YWxcbiAgICAgIDogdXRmOFRvQnl0ZXMobmV3IEJ1ZmZlcih2YWwsIGVuY29kaW5nKS50b1N0cmluZygpKVxuICAgIHZhciBsZW4gPSBieXRlcy5sZW5ndGhcbiAgICBmb3IgKGkgPSAwOyBpIDwgZW5kIC0gc3RhcnQ7ICsraSkge1xuICAgICAgdGhpc1tpICsgc3RhcnRdID0gYnl0ZXNbaSAlIGxlbl1cbiAgICB9XG4gIH1cblxuICByZXR1cm4gdGhpc1xufVxuXG4vLyBIRUxQRVIgRlVOQ1RJT05TXG4vLyA9PT09PT09PT09PT09PT09XG5cbnZhciBJTlZBTElEX0JBU0U2NF9SRSA9IC9bXitcXC8wLTlBLVphLXotX10vZ1xuXG5mdW5jdGlvbiBiYXNlNjRjbGVhbiAoc3RyKSB7XG4gIC8vIE5vZGUgc3RyaXBzIG91dCBpbnZhbGlkIGNoYXJhY3RlcnMgbGlrZSBcXG4gYW5kIFxcdCBmcm9tIHRoZSBzdHJpbmcsIGJhc2U2NC1qcyBkb2VzIG5vdFxuICBzdHIgPSBzdHJpbmd0cmltKHN0cikucmVwbGFjZShJTlZBTElEX0JBU0U2NF9SRSwgJycpXG4gIC8vIE5vZGUgY29udmVydHMgc3RyaW5ncyB3aXRoIGxlbmd0aCA8IDIgdG8gJydcbiAgaWYgKHN0ci5sZW5ndGggPCAyKSByZXR1cm4gJydcbiAgLy8gTm9kZSBhbGxvd3MgZm9yIG5vbi1wYWRkZWQgYmFzZTY0IHN0cmluZ3MgKG1pc3NpbmcgdHJhaWxpbmcgPT09KSwgYmFzZTY0LWpzIGRvZXMgbm90XG4gIHdoaWxlIChzdHIubGVuZ3RoICUgNCAhPT0gMCkge1xuICAgIHN0ciA9IHN0ciArICc9J1xuICB9XG4gIHJldHVybiBzdHJcbn1cblxuZnVuY3Rpb24gc3RyaW5ndHJpbSAoc3RyKSB7XG4gIGlmIChzdHIudHJpbSkgcmV0dXJuIHN0ci50cmltKClcbiAgcmV0dXJuIHN0ci5yZXBsYWNlKC9eXFxzK3xcXHMrJC9nLCAnJylcbn1cblxuZnVuY3Rpb24gdG9IZXggKG4pIHtcbiAgaWYgKG4gPCAxNikgcmV0dXJuICcwJyArIG4udG9TdHJpbmcoMTYpXG4gIHJldHVybiBuLnRvU3RyaW5nKDE2KVxufVxuXG5mdW5jdGlvbiB1dGY4VG9CeXRlcyAoc3RyaW5nLCB1bml0cykge1xuICB1bml0cyA9IHVuaXRzIHx8IEluZmluaXR5XG4gIHZhciBjb2RlUG9pbnRcbiAgdmFyIGxlbmd0aCA9IHN0cmluZy5sZW5ndGhcbiAgdmFyIGxlYWRTdXJyb2dhdGUgPSBudWxsXG4gIHZhciBieXRlcyA9IFtdXG5cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBsZW5ndGg7ICsraSkge1xuICAgIGNvZGVQb2ludCA9IHN0cmluZy5jaGFyQ29kZUF0KGkpXG5cbiAgICAvLyBpcyBzdXJyb2dhdGUgY29tcG9uZW50XG4gICAgaWYgKGNvZGVQb2ludCA+IDB4RDdGRiAmJiBjb2RlUG9pbnQgPCAweEUwMDApIHtcbiAgICAgIC8vIGxhc3QgY2hhciB3YXMgYSBsZWFkXG4gICAgICBpZiAoIWxlYWRTdXJyb2dhdGUpIHtcbiAgICAgICAgLy8gbm8gbGVhZCB5ZXRcbiAgICAgICAgaWYgKGNvZGVQb2ludCA+IDB4REJGRikge1xuICAgICAgICAgIC8vIHVuZXhwZWN0ZWQgdHJhaWxcbiAgICAgICAgICBpZiAoKHVuaXRzIC09IDMpID4gLTEpIGJ5dGVzLnB1c2goMHhFRiwgMHhCRiwgMHhCRClcbiAgICAgICAgICBjb250aW51ZVxuICAgICAgICB9IGVsc2UgaWYgKGkgKyAxID09PSBsZW5ndGgpIHtcbiAgICAgICAgICAvLyB1bnBhaXJlZCBsZWFkXG4gICAgICAgICAgaWYgKCh1bml0cyAtPSAzKSA+IC0xKSBieXRlcy5wdXNoKDB4RUYsIDB4QkYsIDB4QkQpXG4gICAgICAgICAgY29udGludWVcbiAgICAgICAgfVxuXG4gICAgICAgIC8vIHZhbGlkIGxlYWRcbiAgICAgICAgbGVhZFN1cnJvZ2F0ZSA9IGNvZGVQb2ludFxuXG4gICAgICAgIGNvbnRpbnVlXG4gICAgICB9XG5cbiAgICAgIC8vIDIgbGVhZHMgaW4gYSByb3dcbiAgICAgIGlmIChjb2RlUG9pbnQgPCAweERDMDApIHtcbiAgICAgICAgaWYgKCh1bml0cyAtPSAzKSA+IC0xKSBieXRlcy5wdXNoKDB4RUYsIDB4QkYsIDB4QkQpXG4gICAgICAgIGxlYWRTdXJyb2dhdGUgPSBjb2RlUG9pbnRcbiAgICAgICAgY29udGludWVcbiAgICAgIH1cblxuICAgICAgLy8gdmFsaWQgc3Vycm9nYXRlIHBhaXJcbiAgICAgIGNvZGVQb2ludCA9IChsZWFkU3Vycm9nYXRlIC0gMHhEODAwIDw8IDEwIHwgY29kZVBvaW50IC0gMHhEQzAwKSArIDB4MTAwMDBcbiAgICB9IGVsc2UgaWYgKGxlYWRTdXJyb2dhdGUpIHtcbiAgICAgIC8vIHZhbGlkIGJtcCBjaGFyLCBidXQgbGFzdCBjaGFyIHdhcyBhIGxlYWRcbiAgICAgIGlmICgodW5pdHMgLT0gMykgPiAtMSkgYnl0ZXMucHVzaCgweEVGLCAweEJGLCAweEJEKVxuICAgIH1cblxuICAgIGxlYWRTdXJyb2dhdGUgPSBudWxsXG5cbiAgICAvLyBlbmNvZGUgdXRmOFxuICAgIGlmIChjb2RlUG9pbnQgPCAweDgwKSB7XG4gICAgICBpZiAoKHVuaXRzIC09IDEpIDwgMCkgYnJlYWtcbiAgICAgIGJ5dGVzLnB1c2goY29kZVBvaW50KVxuICAgIH0gZWxzZSBpZiAoY29kZVBvaW50IDwgMHg4MDApIHtcbiAgICAgIGlmICgodW5pdHMgLT0gMikgPCAwKSBicmVha1xuICAgICAgYnl0ZXMucHVzaChcbiAgICAgICAgY29kZVBvaW50ID4+IDB4NiB8IDB4QzAsXG4gICAgICAgIGNvZGVQb2ludCAmIDB4M0YgfCAweDgwXG4gICAgICApXG4gICAgfSBlbHNlIGlmIChjb2RlUG9pbnQgPCAweDEwMDAwKSB7XG4gICAgICBpZiAoKHVuaXRzIC09IDMpIDwgMCkgYnJlYWtcbiAgICAgIGJ5dGVzLnB1c2goXG4gICAgICAgIGNvZGVQb2ludCA+PiAweEMgfCAweEUwLFxuICAgICAgICBjb2RlUG9pbnQgPj4gMHg2ICYgMHgzRiB8IDB4ODAsXG4gICAgICAgIGNvZGVQb2ludCAmIDB4M0YgfCAweDgwXG4gICAgICApXG4gICAgfSBlbHNlIGlmIChjb2RlUG9pbnQgPCAweDExMDAwMCkge1xuICAgICAgaWYgKCh1bml0cyAtPSA0KSA8IDApIGJyZWFrXG4gICAgICBieXRlcy5wdXNoKFxuICAgICAgICBjb2RlUG9pbnQgPj4gMHgxMiB8IDB4RjAsXG4gICAgICAgIGNvZGVQb2ludCA+PiAweEMgJiAweDNGIHwgMHg4MCxcbiAgICAgICAgY29kZVBvaW50ID4+IDB4NiAmIDB4M0YgfCAweDgwLFxuICAgICAgICBjb2RlUG9pbnQgJiAweDNGIHwgMHg4MFxuICAgICAgKVxuICAgIH0gZWxzZSB7XG4gICAgICB0aHJvdyBuZXcgRXJyb3IoJ0ludmFsaWQgY29kZSBwb2ludCcpXG4gICAgfVxuICB9XG5cbiAgcmV0dXJuIGJ5dGVzXG59XG5cbmZ1bmN0aW9uIGFzY2lpVG9CeXRlcyAoc3RyKSB7XG4gIHZhciBieXRlQXJyYXkgPSBbXVxuICBmb3IgKHZhciBpID0gMDsgaSA8IHN0ci5sZW5ndGg7ICsraSkge1xuICAgIC8vIE5vZGUncyBjb2RlIHNlZW1zIHRvIGJlIGRvaW5nIHRoaXMgYW5kIG5vdCAmIDB4N0YuLlxuICAgIGJ5dGVBcnJheS5wdXNoKHN0ci5jaGFyQ29kZUF0KGkpICYgMHhGRilcbiAgfVxuICByZXR1cm4gYnl0ZUFycmF5XG59XG5cbmZ1bmN0aW9uIHV0ZjE2bGVUb0J5dGVzIChzdHIsIHVuaXRzKSB7XG4gIHZhciBjLCBoaSwgbG9cbiAgdmFyIGJ5dGVBcnJheSA9IFtdXG4gIGZvciAodmFyIGkgPSAwOyBpIDwgc3RyLmxlbmd0aDsgKytpKSB7XG4gICAgaWYgKCh1bml0cyAtPSAyKSA8IDApIGJyZWFrXG5cbiAgICBjID0gc3RyLmNoYXJDb2RlQXQoaSlcbiAgICBoaSA9IGMgPj4gOFxuICAgIGxvID0gYyAlIDI1NlxuICAgIGJ5dGVBcnJheS5wdXNoKGxvKVxuICAgIGJ5dGVBcnJheS5wdXNoKGhpKVxuICB9XG5cbiAgcmV0dXJuIGJ5dGVBcnJheVxufVxuXG5mdW5jdGlvbiBiYXNlNjRUb0J5dGVzIChzdHIpIHtcbiAgcmV0dXJuIGJhc2U2NC50b0J5dGVBcnJheShiYXNlNjRjbGVhbihzdHIpKVxufVxuXG5mdW5jdGlvbiBibGl0QnVmZmVyIChzcmMsIGRzdCwgb2Zmc2V0LCBsZW5ndGgpIHtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBsZW5ndGg7ICsraSkge1xuICAgIGlmICgoaSArIG9mZnNldCA+PSBkc3QubGVuZ3RoKSB8fCAoaSA+PSBzcmMubGVuZ3RoKSkgYnJlYWtcbiAgICBkc3RbaSArIG9mZnNldF0gPSBzcmNbaV1cbiAgfVxuICByZXR1cm4gaVxufVxuXG5mdW5jdGlvbiBpc25hbiAodmFsKSB7XG4gIHJldHVybiB2YWwgIT09IHZhbCAvLyBlc2xpbnQtZGlzYWJsZS1saW5lIG5vLXNlbGYtY29tcGFyZVxufVxuIiwiZXhwb3J0cy5yZWFkID0gZnVuY3Rpb24gKGJ1ZmZlciwgb2Zmc2V0LCBpc0xFLCBtTGVuLCBuQnl0ZXMpIHtcbiAgdmFyIGUsIG1cbiAgdmFyIGVMZW4gPSBuQnl0ZXMgKiA4IC0gbUxlbiAtIDFcbiAgdmFyIGVNYXggPSAoMSA8PCBlTGVuKSAtIDFcbiAgdmFyIGVCaWFzID0gZU1heCA+PiAxXG4gIHZhciBuQml0cyA9IC03XG4gIHZhciBpID0gaXNMRSA/IChuQnl0ZXMgLSAxKSA6IDBcbiAgdmFyIGQgPSBpc0xFID8gLTEgOiAxXG4gIHZhciBzID0gYnVmZmVyW29mZnNldCArIGldXG5cbiAgaSArPSBkXG5cbiAgZSA9IHMgJiAoKDEgPDwgKC1uQml0cykpIC0gMSlcbiAgcyA+Pj0gKC1uQml0cylcbiAgbkJpdHMgKz0gZUxlblxuICBmb3IgKDsgbkJpdHMgPiAwOyBlID0gZSAqIDI1NiArIGJ1ZmZlcltvZmZzZXQgKyBpXSwgaSArPSBkLCBuQml0cyAtPSA4KSB7fVxuXG4gIG0gPSBlICYgKCgxIDw8ICgtbkJpdHMpKSAtIDEpXG4gIGUgPj49ICgtbkJpdHMpXG4gIG5CaXRzICs9IG1MZW5cbiAgZm9yICg7IG5CaXRzID4gMDsgbSA9IG0gKiAyNTYgKyBidWZmZXJbb2Zmc2V0ICsgaV0sIGkgKz0gZCwgbkJpdHMgLT0gOCkge31cblxuICBpZiAoZSA9PT0gMCkge1xuICAgIGUgPSAxIC0gZUJpYXNcbiAgfSBlbHNlIGlmIChlID09PSBlTWF4KSB7XG4gICAgcmV0dXJuIG0gPyBOYU4gOiAoKHMgPyAtMSA6IDEpICogSW5maW5pdHkpXG4gIH0gZWxzZSB7XG4gICAgbSA9IG0gKyBNYXRoLnBvdygyLCBtTGVuKVxuICAgIGUgPSBlIC0gZUJpYXNcbiAgfVxuICByZXR1cm4gKHMgPyAtMSA6IDEpICogbSAqIE1hdGgucG93KDIsIGUgLSBtTGVuKVxufVxuXG5leHBvcnRzLndyaXRlID0gZnVuY3Rpb24gKGJ1ZmZlciwgdmFsdWUsIG9mZnNldCwgaXNMRSwgbUxlbiwgbkJ5dGVzKSB7XG4gIHZhciBlLCBtLCBjXG4gIHZhciBlTGVuID0gbkJ5dGVzICogOCAtIG1MZW4gLSAxXG4gIHZhciBlTWF4ID0gKDEgPDwgZUxlbikgLSAxXG4gIHZhciBlQmlhcyA9IGVNYXggPj4gMVxuICB2YXIgcnQgPSAobUxlbiA9PT0gMjMgPyBNYXRoLnBvdygyLCAtMjQpIC0gTWF0aC5wb3coMiwgLTc3KSA6IDApXG4gIHZhciBpID0gaXNMRSA/IDAgOiAobkJ5dGVzIC0gMSlcbiAgdmFyIGQgPSBpc0xFID8gMSA6IC0xXG4gIHZhciBzID0gdmFsdWUgPCAwIHx8ICh2YWx1ZSA9PT0gMCAmJiAxIC8gdmFsdWUgPCAwKSA/IDEgOiAwXG5cbiAgdmFsdWUgPSBNYXRoLmFicyh2YWx1ZSlcblxuICBpZiAoaXNOYU4odmFsdWUpIHx8IHZhbHVlID09PSBJbmZpbml0eSkge1xuICAgIG0gPSBpc05hTih2YWx1ZSkgPyAxIDogMFxuICAgIGUgPSBlTWF4XG4gIH0gZWxzZSB7XG4gICAgZSA9IE1hdGguZmxvb3IoTWF0aC5sb2codmFsdWUpIC8gTWF0aC5MTjIpXG4gICAgaWYgKHZhbHVlICogKGMgPSBNYXRoLnBvdygyLCAtZSkpIDwgMSkge1xuICAgICAgZS0tXG4gICAgICBjICo9IDJcbiAgICB9XG4gICAgaWYgKGUgKyBlQmlhcyA+PSAxKSB7XG4gICAgICB2YWx1ZSArPSBydCAvIGNcbiAgICB9IGVsc2Uge1xuICAgICAgdmFsdWUgKz0gcnQgKiBNYXRoLnBvdygyLCAxIC0gZUJpYXMpXG4gICAgfVxuICAgIGlmICh2YWx1ZSAqIGMgPj0gMikge1xuICAgICAgZSsrXG4gICAgICBjIC89IDJcbiAgICB9XG5cbiAgICBpZiAoZSArIGVCaWFzID49IGVNYXgpIHtcbiAgICAgIG0gPSAwXG4gICAgICBlID0gZU1heFxuICAgIH0gZWxzZSBpZiAoZSArIGVCaWFzID49IDEpIHtcbiAgICAgIG0gPSAodmFsdWUgKiBjIC0gMSkgKiBNYXRoLnBvdygyLCBtTGVuKVxuICAgICAgZSA9IGUgKyBlQmlhc1xuICAgIH0gZWxzZSB7XG4gICAgICBtID0gdmFsdWUgKiBNYXRoLnBvdygyLCBlQmlhcyAtIDEpICogTWF0aC5wb3coMiwgbUxlbilcbiAgICAgIGUgPSAwXG4gICAgfVxuICB9XG5cbiAgZm9yICg7IG1MZW4gPj0gODsgYnVmZmVyW29mZnNldCArIGldID0gbSAmIDB4ZmYsIGkgKz0gZCwgbSAvPSAyNTYsIG1MZW4gLT0gOCkge31cblxuICBlID0gKGUgPDwgbUxlbikgfCBtXG4gIGVMZW4gKz0gbUxlblxuICBmb3IgKDsgZUxlbiA+IDA7IGJ1ZmZlcltvZmZzZXQgKyBpXSA9IGUgJiAweGZmLCBpICs9IGQsIGUgLz0gMjU2LCBlTGVuIC09IDgpIHt9XG5cbiAgYnVmZmVyW29mZnNldCArIGkgLSBkXSB8PSBzICogMTI4XG59XG4iLCJpZiAodHlwZW9mIE9iamVjdC5jcmVhdGUgPT09ICdmdW5jdGlvbicpIHtcbiAgLy8gaW1wbGVtZW50YXRpb24gZnJvbSBzdGFuZGFyZCBub2RlLmpzICd1dGlsJyBtb2R1bGVcbiAgbW9kdWxlLmV4cG9ydHMgPSBmdW5jdGlvbiBpbmhlcml0cyhjdG9yLCBzdXBlckN0b3IpIHtcbiAgICBjdG9yLnN1cGVyXyA9IHN1cGVyQ3RvclxuICAgIGN0b3IucHJvdG90eXBlID0gT2JqZWN0LmNyZWF0ZShzdXBlckN0b3IucHJvdG90eXBlLCB7XG4gICAgICBjb25zdHJ1Y3Rvcjoge1xuICAgICAgICB2YWx1ZTogY3RvcixcbiAgICAgICAgZW51bWVyYWJsZTogZmFsc2UsXG4gICAgICAgIHdyaXRhYmxlOiB0cnVlLFxuICAgICAgICBjb25maWd1cmFibGU6IHRydWVcbiAgICAgIH1cbiAgICB9KTtcbiAgfTtcbn0gZWxzZSB7XG4gIC8vIG9sZCBzY2hvb2wgc2hpbSBmb3Igb2xkIGJyb3dzZXJzXG4gIG1vZHVsZS5leHBvcnRzID0gZnVuY3Rpb24gaW5oZXJpdHMoY3Rvciwgc3VwZXJDdG9yKSB7XG4gICAgY3Rvci5zdXBlcl8gPSBzdXBlckN0b3JcbiAgICB2YXIgVGVtcEN0b3IgPSBmdW5jdGlvbiAoKSB7fVxuICAgIFRlbXBDdG9yLnByb3RvdHlwZSA9IHN1cGVyQ3Rvci5wcm90b3R5cGVcbiAgICBjdG9yLnByb3RvdHlwZSA9IG5ldyBUZW1wQ3RvcigpXG4gICAgY3Rvci5wcm90b3R5cGUuY29uc3RydWN0b3IgPSBjdG9yXG4gIH1cbn1cbiIsInZhciB0b1N0cmluZyA9IHt9LnRvU3RyaW5nO1xuXG5tb2R1bGUuZXhwb3J0cyA9IEFycmF5LmlzQXJyYXkgfHwgZnVuY3Rpb24gKGFycikge1xuICByZXR1cm4gdG9TdHJpbmcuY2FsbChhcnIpID09ICdbb2JqZWN0IEFycmF5XSc7XG59O1xuIiwiLypcbm9iamVjdC1hc3NpZ25cbihjKSBTaW5kcmUgU29yaHVzXG5AbGljZW5zZSBNSVRcbiovXG5cbid1c2Ugc3RyaWN0Jztcbi8qIGVzbGludC1kaXNhYmxlIG5vLXVudXNlZC12YXJzICovXG52YXIgZ2V0T3duUHJvcGVydHlTeW1ib2xzID0gT2JqZWN0LmdldE93blByb3BlcnR5U3ltYm9scztcbnZhciBoYXNPd25Qcm9wZXJ0eSA9IE9iamVjdC5wcm90b3R5cGUuaGFzT3duUHJvcGVydHk7XG52YXIgcHJvcElzRW51bWVyYWJsZSA9IE9iamVjdC5wcm90b3R5cGUucHJvcGVydHlJc0VudW1lcmFibGU7XG5cbmZ1bmN0aW9uIHRvT2JqZWN0KHZhbCkge1xuXHRpZiAodmFsID09PSBudWxsIHx8IHZhbCA9PT0gdW5kZWZpbmVkKSB7XG5cdFx0dGhyb3cgbmV3IFR5cGVFcnJvcignT2JqZWN0LmFzc2lnbiBjYW5ub3QgYmUgY2FsbGVkIHdpdGggbnVsbCBvciB1bmRlZmluZWQnKTtcblx0fVxuXG5cdHJldHVybiBPYmplY3QodmFsKTtcbn1cblxuZnVuY3Rpb24gc2hvdWxkVXNlTmF0aXZlKCkge1xuXHR0cnkge1xuXHRcdGlmICghT2JqZWN0LmFzc2lnbikge1xuXHRcdFx0cmV0dXJuIGZhbHNlO1xuXHRcdH1cblxuXHRcdC8vIERldGVjdCBidWdneSBwcm9wZXJ0eSBlbnVtZXJhdGlvbiBvcmRlciBpbiBvbGRlciBWOCB2ZXJzaW9ucy5cblxuXHRcdC8vIGh0dHBzOi8vYnVncy5jaHJvbWl1bS5vcmcvcC92OC9pc3N1ZXMvZGV0YWlsP2lkPTQxMThcblx0XHR2YXIgdGVzdDEgPSBuZXcgU3RyaW5nKCdhYmMnKTsgIC8vIGVzbGludC1kaXNhYmxlLWxpbmUgbm8tbmV3LXdyYXBwZXJzXG5cdFx0dGVzdDFbNV0gPSAnZGUnO1xuXHRcdGlmIChPYmplY3QuZ2V0T3duUHJvcGVydHlOYW1lcyh0ZXN0MSlbMF0gPT09ICc1Jykge1xuXHRcdFx0cmV0dXJuIGZhbHNlO1xuXHRcdH1cblxuXHRcdC8vIGh0dHBzOi8vYnVncy5jaHJvbWl1bS5vcmcvcC92OC9pc3N1ZXMvZGV0YWlsP2lkPTMwNTZcblx0XHR2YXIgdGVzdDIgPSB7fTtcblx0XHRmb3IgKHZhciBpID0gMDsgaSA8IDEwOyBpKyspIHtcblx0XHRcdHRlc3QyWydfJyArIFN0cmluZy5mcm9tQ2hhckNvZGUoaSldID0gaTtcblx0XHR9XG5cdFx0dmFyIG9yZGVyMiA9IE9iamVjdC5nZXRPd25Qcm9wZXJ0eU5hbWVzKHRlc3QyKS5tYXAoZnVuY3Rpb24gKG4pIHtcblx0XHRcdHJldHVybiB0ZXN0MltuXTtcblx0XHR9KTtcblx0XHRpZiAob3JkZXIyLmpvaW4oJycpICE9PSAnMDEyMzQ1Njc4OScpIHtcblx0XHRcdHJldHVybiBmYWxzZTtcblx0XHR9XG5cblx0XHQvLyBodHRwczovL2J1Z3MuY2hyb21pdW0ub3JnL3AvdjgvaXNzdWVzL2RldGFpbD9pZD0zMDU2XG5cdFx0dmFyIHRlc3QzID0ge307XG5cdFx0J2FiY2RlZmdoaWprbG1ub3BxcnN0Jy5zcGxpdCgnJykuZm9yRWFjaChmdW5jdGlvbiAobGV0dGVyKSB7XG5cdFx0XHR0ZXN0M1tsZXR0ZXJdID0gbGV0dGVyO1xuXHRcdH0pO1xuXHRcdGlmIChPYmplY3Qua2V5cyhPYmplY3QuYXNzaWduKHt9LCB0ZXN0MykpLmpvaW4oJycpICE9PVxuXHRcdFx0XHQnYWJjZGVmZ2hpamtsbW5vcHFyc3QnKSB7XG5cdFx0XHRyZXR1cm4gZmFsc2U7XG5cdFx0fVxuXG5cdFx0cmV0dXJuIHRydWU7XG5cdH0gY2F0Y2ggKGVycikge1xuXHRcdC8vIFdlIGRvbid0IGV4cGVjdCBhbnkgb2YgdGhlIGFib3ZlIHRvIHRocm93LCBidXQgYmV0dGVyIHRvIGJlIHNhZmUuXG5cdFx0cmV0dXJuIGZhbHNlO1xuXHR9XG59XG5cbm1vZHVsZS5leHBvcnRzID0gc2hvdWxkVXNlTmF0aXZlKCkgPyBPYmplY3QuYXNzaWduIDogZnVuY3Rpb24gKHRhcmdldCwgc291cmNlKSB7XG5cdHZhciBmcm9tO1xuXHR2YXIgdG8gPSB0b09iamVjdCh0YXJnZXQpO1xuXHR2YXIgc3ltYm9scztcblxuXHRmb3IgKHZhciBzID0gMTsgcyA8IGFyZ3VtZW50cy5sZW5ndGg7IHMrKykge1xuXHRcdGZyb20gPSBPYmplY3QoYXJndW1lbnRzW3NdKTtcblxuXHRcdGZvciAodmFyIGtleSBpbiBmcm9tKSB7XG5cdFx0XHRpZiAoaGFzT3duUHJvcGVydHkuY2FsbChmcm9tLCBrZXkpKSB7XG5cdFx0XHRcdHRvW2tleV0gPSBmcm9tW2tleV07XG5cdFx0XHR9XG5cdFx0fVxuXG5cdFx0aWYgKGdldE93blByb3BlcnR5U3ltYm9scykge1xuXHRcdFx0c3ltYm9scyA9IGdldE93blByb3BlcnR5U3ltYm9scyhmcm9tKTtcblx0XHRcdGZvciAodmFyIGkgPSAwOyBpIDwgc3ltYm9scy5sZW5ndGg7IGkrKykge1xuXHRcdFx0XHRpZiAocHJvcElzRW51bWVyYWJsZS5jYWxsKGZyb20sIHN5bWJvbHNbaV0pKSB7XG5cdFx0XHRcdFx0dG9bc3ltYm9sc1tpXV0gPSBmcm9tW3N5bWJvbHNbaV1dO1xuXHRcdFx0XHR9XG5cdFx0XHR9XG5cdFx0fVxuXHR9XG5cblx0cmV0dXJuIHRvO1xufTtcbiIsIi8vIHByb3RvdHlwZSBjbGFzcyBmb3IgaGFzaCBmdW5jdGlvbnNcbmZ1bmN0aW9uIEhhc2ggKGJsb2NrU2l6ZSwgZmluYWxTaXplKSB7XG4gIHRoaXMuX2Jsb2NrID0gbmV3IEJ1ZmZlcihibG9ja1NpemUpXG4gIHRoaXMuX2ZpbmFsU2l6ZSA9IGZpbmFsU2l6ZVxuICB0aGlzLl9ibG9ja1NpemUgPSBibG9ja1NpemVcbiAgdGhpcy5fbGVuID0gMFxuICB0aGlzLl9zID0gMFxufVxuXG5IYXNoLnByb3RvdHlwZS51cGRhdGUgPSBmdW5jdGlvbiAoZGF0YSwgZW5jKSB7XG4gIGlmICh0eXBlb2YgZGF0YSA9PT0gJ3N0cmluZycpIHtcbiAgICBlbmMgPSBlbmMgfHwgJ3V0ZjgnXG4gICAgZGF0YSA9IG5ldyBCdWZmZXIoZGF0YSwgZW5jKVxuICB9XG5cbiAgdmFyIGwgPSB0aGlzLl9sZW4gKz0gZGF0YS5sZW5ndGhcbiAgdmFyIHMgPSB0aGlzLl9zIHx8IDBcbiAgdmFyIGYgPSAwXG4gIHZhciBidWZmZXIgPSB0aGlzLl9ibG9ja1xuXG4gIHdoaWxlIChzIDwgbCkge1xuICAgIHZhciB0ID0gTWF0aC5taW4oZGF0YS5sZW5ndGgsIGYgKyB0aGlzLl9ibG9ja1NpemUgLSAocyAlIHRoaXMuX2Jsb2NrU2l6ZSkpXG4gICAgdmFyIGNoID0gKHQgLSBmKVxuXG4gICAgZm9yICh2YXIgaSA9IDA7IGkgPCBjaDsgaSsrKSB7XG4gICAgICBidWZmZXJbKHMgJSB0aGlzLl9ibG9ja1NpemUpICsgaV0gPSBkYXRhW2kgKyBmXVxuICAgIH1cblxuICAgIHMgKz0gY2hcbiAgICBmICs9IGNoXG5cbiAgICBpZiAoKHMgJSB0aGlzLl9ibG9ja1NpemUpID09PSAwKSB7XG4gICAgICB0aGlzLl91cGRhdGUoYnVmZmVyKVxuICAgIH1cbiAgfVxuICB0aGlzLl9zID0gc1xuXG4gIHJldHVybiB0aGlzXG59XG5cbkhhc2gucHJvdG90eXBlLmRpZ2VzdCA9IGZ1bmN0aW9uIChlbmMpIHtcbiAgLy8gU3VwcG9zZSB0aGUgbGVuZ3RoIG9mIHRoZSBtZXNzYWdlIE0sIGluIGJpdHMsIGlzIGxcbiAgdmFyIGwgPSB0aGlzLl9sZW4gKiA4XG5cbiAgLy8gQXBwZW5kIHRoZSBiaXQgMSB0byB0aGUgZW5kIG9mIHRoZSBtZXNzYWdlXG4gIHRoaXMuX2Jsb2NrW3RoaXMuX2xlbiAlIHRoaXMuX2Jsb2NrU2l6ZV0gPSAweDgwXG5cbiAgLy8gYW5kIHRoZW4gayB6ZXJvIGJpdHMsIHdoZXJlIGsgaXMgdGhlIHNtYWxsZXN0IG5vbi1uZWdhdGl2ZSBzb2x1dGlvbiB0byB0aGUgZXF1YXRpb24gKGwgKyAxICsgaykgPT09IGZpbmFsU2l6ZSBtb2QgYmxvY2tTaXplXG4gIHRoaXMuX2Jsb2NrLmZpbGwoMCwgdGhpcy5fbGVuICUgdGhpcy5fYmxvY2tTaXplICsgMSlcblxuICBpZiAobCAlICh0aGlzLl9ibG9ja1NpemUgKiA4KSA+PSB0aGlzLl9maW5hbFNpemUgKiA4KSB7XG4gICAgdGhpcy5fdXBkYXRlKHRoaXMuX2Jsb2NrKVxuICAgIHRoaXMuX2Jsb2NrLmZpbGwoMClcbiAgfVxuXG4gIC8vIHRvIHRoaXMgYXBwZW5kIHRoZSBibG9jayB3aGljaCBpcyBlcXVhbCB0byB0aGUgbnVtYmVyIGwgd3JpdHRlbiBpbiBiaW5hcnlcbiAgLy8gVE9ETzogaGFuZGxlIGNhc2Ugd2hlcmUgbCBpcyA+IE1hdGgucG93KDIsIDI5KVxuICB0aGlzLl9ibG9jay53cml0ZUludDMyQkUobCwgdGhpcy5fYmxvY2tTaXplIC0gNClcblxuICB2YXIgaGFzaCA9IHRoaXMuX3VwZGF0ZSh0aGlzLl9ibG9jaykgfHwgdGhpcy5faGFzaCgpXG5cbiAgcmV0dXJuIGVuYyA/IGhhc2gudG9TdHJpbmcoZW5jKSA6IGhhc2hcbn1cblxuSGFzaC5wcm90b3R5cGUuX3VwZGF0ZSA9IGZ1bmN0aW9uICgpIHtcbiAgdGhyb3cgbmV3IEVycm9yKCdfdXBkYXRlIG11c3QgYmUgaW1wbGVtZW50ZWQgYnkgc3ViY2xhc3MnKVxufVxuXG5tb2R1bGUuZXhwb3J0cyA9IEhhc2hcbiIsInZhciBleHBvcnRzID0gbW9kdWxlLmV4cG9ydHMgPSBmdW5jdGlvbiBTSEEgKGFsZ29yaXRobSkge1xuICBhbGdvcml0aG0gPSBhbGdvcml0aG0udG9Mb3dlckNhc2UoKVxuXG4gIHZhciBBbGdvcml0aG0gPSBleHBvcnRzW2FsZ29yaXRobV1cbiAgaWYgKCFBbGdvcml0aG0pIHRocm93IG5ldyBFcnJvcihhbGdvcml0aG0gKyAnIGlzIG5vdCBzdXBwb3J0ZWQgKHdlIGFjY2VwdCBwdWxsIHJlcXVlc3RzKScpXG5cbiAgcmV0dXJuIG5ldyBBbGdvcml0aG0oKVxufVxuXG5leHBvcnRzLnNoYSA9IHJlcXVpcmUoJy4vc2hhJylcbmV4cG9ydHMuc2hhMSA9IHJlcXVpcmUoJy4vc2hhMScpXG5leHBvcnRzLnNoYTIyNCA9IHJlcXVpcmUoJy4vc2hhMjI0JylcbmV4cG9ydHMuc2hhMjU2ID0gcmVxdWlyZSgnLi9zaGEyNTYnKVxuZXhwb3J0cy5zaGEzODQgPSByZXF1aXJlKCcuL3NoYTM4NCcpXG5leHBvcnRzLnNoYTUxMiA9IHJlcXVpcmUoJy4vc2hhNTEyJylcbiIsIi8qXG4gKiBBIEphdmFTY3JpcHQgaW1wbGVtZW50YXRpb24gb2YgdGhlIFNlY3VyZSBIYXNoIEFsZ29yaXRobSwgU0hBLTAsIGFzIGRlZmluZWRcbiAqIGluIEZJUFMgUFVCIDE4MC0xXG4gKiBUaGlzIHNvdXJjZSBjb2RlIGlzIGRlcml2ZWQgZnJvbSBzaGExLmpzIG9mIHRoZSBzYW1lIHJlcG9zaXRvcnkuXG4gKiBUaGUgZGlmZmVyZW5jZSBiZXR3ZWVuIFNIQS0wIGFuZCBTSEEtMSBpcyBqdXN0IGEgYml0d2lzZSByb3RhdGUgbGVmdFxuICogb3BlcmF0aW9uIHdhcyBhZGRlZC5cbiAqL1xuXG52YXIgaW5oZXJpdHMgPSByZXF1aXJlKCdpbmhlcml0cycpXG52YXIgSGFzaCA9IHJlcXVpcmUoJy4vaGFzaCcpXG5cbnZhciBLID0gW1xuICAweDVhODI3OTk5LCAweDZlZDllYmExLCAweDhmMWJiY2RjIHwgMCwgMHhjYTYyYzFkNiB8IDBcbl1cblxudmFyIFcgPSBuZXcgQXJyYXkoODApXG5cbmZ1bmN0aW9uIFNoYSAoKSB7XG4gIHRoaXMuaW5pdCgpXG4gIHRoaXMuX3cgPSBXXG5cbiAgSGFzaC5jYWxsKHRoaXMsIDY0LCA1Nilcbn1cblxuaW5oZXJpdHMoU2hhLCBIYXNoKVxuXG5TaGEucHJvdG90eXBlLmluaXQgPSBmdW5jdGlvbiAoKSB7XG4gIHRoaXMuX2EgPSAweDY3NDUyMzAxXG4gIHRoaXMuX2IgPSAweGVmY2RhYjg5XG4gIHRoaXMuX2MgPSAweDk4YmFkY2ZlXG4gIHRoaXMuX2QgPSAweDEwMzI1NDc2XG4gIHRoaXMuX2UgPSAweGMzZDJlMWYwXG5cbiAgcmV0dXJuIHRoaXNcbn1cblxuZnVuY3Rpb24gcm90bDUgKG51bSkge1xuICByZXR1cm4gKG51bSA8PCA1KSB8IChudW0gPj4+IDI3KVxufVxuXG5mdW5jdGlvbiByb3RsMzAgKG51bSkge1xuICByZXR1cm4gKG51bSA8PCAzMCkgfCAobnVtID4+PiAyKVxufVxuXG5mdW5jdGlvbiBmdCAocywgYiwgYywgZCkge1xuICBpZiAocyA9PT0gMCkgcmV0dXJuIChiICYgYykgfCAoKH5iKSAmIGQpXG4gIGlmIChzID09PSAyKSByZXR1cm4gKGIgJiBjKSB8IChiICYgZCkgfCAoYyAmIGQpXG4gIHJldHVybiBiIF4gYyBeIGRcbn1cblxuU2hhLnByb3RvdHlwZS5fdXBkYXRlID0gZnVuY3Rpb24gKE0pIHtcbiAgdmFyIFcgPSB0aGlzLl93XG5cbiAgdmFyIGEgPSB0aGlzLl9hIHwgMFxuICB2YXIgYiA9IHRoaXMuX2IgfCAwXG4gIHZhciBjID0gdGhpcy5fYyB8IDBcbiAgdmFyIGQgPSB0aGlzLl9kIHwgMFxuICB2YXIgZSA9IHRoaXMuX2UgfCAwXG5cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCAxNjsgKytpKSBXW2ldID0gTS5yZWFkSW50MzJCRShpICogNClcbiAgZm9yICg7IGkgPCA4MDsgKytpKSBXW2ldID0gV1tpIC0gM10gXiBXW2kgLSA4XSBeIFdbaSAtIDE0XSBeIFdbaSAtIDE2XVxuXG4gIGZvciAodmFyIGogPSAwOyBqIDwgODA7ICsraikge1xuICAgIHZhciBzID0gfn4oaiAvIDIwKVxuICAgIHZhciB0ID0gKHJvdGw1KGEpICsgZnQocywgYiwgYywgZCkgKyBlICsgV1tqXSArIEtbc10pIHwgMFxuXG4gICAgZSA9IGRcbiAgICBkID0gY1xuICAgIGMgPSByb3RsMzAoYilcbiAgICBiID0gYVxuICAgIGEgPSB0XG4gIH1cblxuICB0aGlzLl9hID0gKGEgKyB0aGlzLl9hKSB8IDBcbiAgdGhpcy5fYiA9IChiICsgdGhpcy5fYikgfCAwXG4gIHRoaXMuX2MgPSAoYyArIHRoaXMuX2MpIHwgMFxuICB0aGlzLl9kID0gKGQgKyB0aGlzLl9kKSB8IDBcbiAgdGhpcy5fZSA9IChlICsgdGhpcy5fZSkgfCAwXG59XG5cblNoYS5wcm90b3R5cGUuX2hhc2ggPSBmdW5jdGlvbiAoKSB7XG4gIHZhciBIID0gbmV3IEJ1ZmZlcigyMClcblxuICBILndyaXRlSW50MzJCRSh0aGlzLl9hIHwgMCwgMClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fYiB8IDAsIDQpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2MgfCAwLCA4KVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9kIHwgMCwgMTIpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2UgfCAwLCAxNilcblxuICByZXR1cm4gSFxufVxuXG5tb2R1bGUuZXhwb3J0cyA9IFNoYVxuIiwiLypcbiAqIEEgSmF2YVNjcmlwdCBpbXBsZW1lbnRhdGlvbiBvZiB0aGUgU2VjdXJlIEhhc2ggQWxnb3JpdGhtLCBTSEEtMSwgYXMgZGVmaW5lZFxuICogaW4gRklQUyBQVUIgMTgwLTFcbiAqIFZlcnNpb24gMi4xYSBDb3B5cmlnaHQgUGF1bCBKb2huc3RvbiAyMDAwIC0gMjAwMi5cbiAqIE90aGVyIGNvbnRyaWJ1dG9yczogR3JlZyBIb2x0LCBBbmRyZXcgS2VwZXJ0LCBZZG5hciwgTG9zdGluZXRcbiAqIERpc3RyaWJ1dGVkIHVuZGVyIHRoZSBCU0QgTGljZW5zZVxuICogU2VlIGh0dHA6Ly9wYWpob21lLm9yZy51ay9jcnlwdC9tZDUgZm9yIGRldGFpbHMuXG4gKi9cblxudmFyIGluaGVyaXRzID0gcmVxdWlyZSgnaW5oZXJpdHMnKVxudmFyIEhhc2ggPSByZXF1aXJlKCcuL2hhc2gnKVxuXG52YXIgSyA9IFtcbiAgMHg1YTgyNzk5OSwgMHg2ZWQ5ZWJhMSwgMHg4ZjFiYmNkYyB8IDAsIDB4Y2E2MmMxZDYgfCAwXG5dXG5cbnZhciBXID0gbmV3IEFycmF5KDgwKVxuXG5mdW5jdGlvbiBTaGExICgpIHtcbiAgdGhpcy5pbml0KClcbiAgdGhpcy5fdyA9IFdcblxuICBIYXNoLmNhbGwodGhpcywgNjQsIDU2KVxufVxuXG5pbmhlcml0cyhTaGExLCBIYXNoKVxuXG5TaGExLnByb3RvdHlwZS5pbml0ID0gZnVuY3Rpb24gKCkge1xuICB0aGlzLl9hID0gMHg2NzQ1MjMwMVxuICB0aGlzLl9iID0gMHhlZmNkYWI4OVxuICB0aGlzLl9jID0gMHg5OGJhZGNmZVxuICB0aGlzLl9kID0gMHgxMDMyNTQ3NlxuICB0aGlzLl9lID0gMHhjM2QyZTFmMFxuXG4gIHJldHVybiB0aGlzXG59XG5cbmZ1bmN0aW9uIHJvdGwxIChudW0pIHtcbiAgcmV0dXJuIChudW0gPDwgMSkgfCAobnVtID4+PiAzMSlcbn1cblxuZnVuY3Rpb24gcm90bDUgKG51bSkge1xuICByZXR1cm4gKG51bSA8PCA1KSB8IChudW0gPj4+IDI3KVxufVxuXG5mdW5jdGlvbiByb3RsMzAgKG51bSkge1xuICByZXR1cm4gKG51bSA8PCAzMCkgfCAobnVtID4+PiAyKVxufVxuXG5mdW5jdGlvbiBmdCAocywgYiwgYywgZCkge1xuICBpZiAocyA9PT0gMCkgcmV0dXJuIChiICYgYykgfCAoKH5iKSAmIGQpXG4gIGlmIChzID09PSAyKSByZXR1cm4gKGIgJiBjKSB8IChiICYgZCkgfCAoYyAmIGQpXG4gIHJldHVybiBiIF4gYyBeIGRcbn1cblxuU2hhMS5wcm90b3R5cGUuX3VwZGF0ZSA9IGZ1bmN0aW9uIChNKSB7XG4gIHZhciBXID0gdGhpcy5fd1xuXG4gIHZhciBhID0gdGhpcy5fYSB8IDBcbiAgdmFyIGIgPSB0aGlzLl9iIHwgMFxuICB2YXIgYyA9IHRoaXMuX2MgfCAwXG4gIHZhciBkID0gdGhpcy5fZCB8IDBcbiAgdmFyIGUgPSB0aGlzLl9lIHwgMFxuXG4gIGZvciAodmFyIGkgPSAwOyBpIDwgMTY7ICsraSkgV1tpXSA9IE0ucmVhZEludDMyQkUoaSAqIDQpXG4gIGZvciAoOyBpIDwgODA7ICsraSkgV1tpXSA9IHJvdGwxKFdbaSAtIDNdIF4gV1tpIC0gOF0gXiBXW2kgLSAxNF0gXiBXW2kgLSAxNl0pXG5cbiAgZm9yICh2YXIgaiA9IDA7IGogPCA4MDsgKytqKSB7XG4gICAgdmFyIHMgPSB+fihqIC8gMjApXG4gICAgdmFyIHQgPSAocm90bDUoYSkgKyBmdChzLCBiLCBjLCBkKSArIGUgKyBXW2pdICsgS1tzXSkgfCAwXG5cbiAgICBlID0gZFxuICAgIGQgPSBjXG4gICAgYyA9IHJvdGwzMChiKVxuICAgIGIgPSBhXG4gICAgYSA9IHRcbiAgfVxuXG4gIHRoaXMuX2EgPSAoYSArIHRoaXMuX2EpIHwgMFxuICB0aGlzLl9iID0gKGIgKyB0aGlzLl9iKSB8IDBcbiAgdGhpcy5fYyA9IChjICsgdGhpcy5fYykgfCAwXG4gIHRoaXMuX2QgPSAoZCArIHRoaXMuX2QpIHwgMFxuICB0aGlzLl9lID0gKGUgKyB0aGlzLl9lKSB8IDBcbn1cblxuU2hhMS5wcm90b3R5cGUuX2hhc2ggPSBmdW5jdGlvbiAoKSB7XG4gIHZhciBIID0gbmV3IEJ1ZmZlcigyMClcblxuICBILndyaXRlSW50MzJCRSh0aGlzLl9hIHwgMCwgMClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fYiB8IDAsIDQpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2MgfCAwLCA4KVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9kIHwgMCwgMTIpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2UgfCAwLCAxNilcblxuICByZXR1cm4gSFxufVxuXG5tb2R1bGUuZXhwb3J0cyA9IFNoYTFcbiIsIi8qKlxuICogQSBKYXZhU2NyaXB0IGltcGxlbWVudGF0aW9uIG9mIHRoZSBTZWN1cmUgSGFzaCBBbGdvcml0aG0sIFNIQS0yNTYsIGFzIGRlZmluZWRcbiAqIGluIEZJUFMgMTgwLTJcbiAqIFZlcnNpb24gMi4yLWJldGEgQ29weXJpZ2h0IEFuZ2VsIE1hcmluLCBQYXVsIEpvaG5zdG9uIDIwMDAgLSAyMDA5LlxuICogT3RoZXIgY29udHJpYnV0b3JzOiBHcmVnIEhvbHQsIEFuZHJldyBLZXBlcnQsIFlkbmFyLCBMb3N0aW5ldFxuICpcbiAqL1xuXG52YXIgaW5oZXJpdHMgPSByZXF1aXJlKCdpbmhlcml0cycpXG52YXIgU2hhMjU2ID0gcmVxdWlyZSgnLi9zaGEyNTYnKVxudmFyIEhhc2ggPSByZXF1aXJlKCcuL2hhc2gnKVxuXG52YXIgVyA9IG5ldyBBcnJheSg2NClcblxuZnVuY3Rpb24gU2hhMjI0ICgpIHtcbiAgdGhpcy5pbml0KClcblxuICB0aGlzLl93ID0gVyAvLyBuZXcgQXJyYXkoNjQpXG5cbiAgSGFzaC5jYWxsKHRoaXMsIDY0LCA1Nilcbn1cblxuaW5oZXJpdHMoU2hhMjI0LCBTaGEyNTYpXG5cblNoYTIyNC5wcm90b3R5cGUuaW5pdCA9IGZ1bmN0aW9uICgpIHtcbiAgdGhpcy5fYSA9IDB4YzEwNTllZDhcbiAgdGhpcy5fYiA9IDB4MzY3Y2Q1MDdcbiAgdGhpcy5fYyA9IDB4MzA3MGRkMTdcbiAgdGhpcy5fZCA9IDB4ZjcwZTU5MzlcbiAgdGhpcy5fZSA9IDB4ZmZjMDBiMzFcbiAgdGhpcy5fZiA9IDB4Njg1ODE1MTFcbiAgdGhpcy5fZyA9IDB4NjRmOThmYTdcbiAgdGhpcy5faCA9IDB4YmVmYTRmYTRcblxuICByZXR1cm4gdGhpc1xufVxuXG5TaGEyMjQucHJvdG90eXBlLl9oYXNoID0gZnVuY3Rpb24gKCkge1xuICB2YXIgSCA9IG5ldyBCdWZmZXIoMjgpXG5cbiAgSC53cml0ZUludDMyQkUodGhpcy5fYSwgMClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fYiwgNClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fYywgOClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fZCwgMTIpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2UsIDE2KVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9mLCAyMClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fZywgMjQpXG5cbiAgcmV0dXJuIEhcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBTaGEyMjRcbiIsIi8qKlxuICogQSBKYXZhU2NyaXB0IGltcGxlbWVudGF0aW9uIG9mIHRoZSBTZWN1cmUgSGFzaCBBbGdvcml0aG0sIFNIQS0yNTYsIGFzIGRlZmluZWRcbiAqIGluIEZJUFMgMTgwLTJcbiAqIFZlcnNpb24gMi4yLWJldGEgQ29weXJpZ2h0IEFuZ2VsIE1hcmluLCBQYXVsIEpvaG5zdG9uIDIwMDAgLSAyMDA5LlxuICogT3RoZXIgY29udHJpYnV0b3JzOiBHcmVnIEhvbHQsIEFuZHJldyBLZXBlcnQsIFlkbmFyLCBMb3N0aW5ldFxuICpcbiAqL1xuXG52YXIgaW5oZXJpdHMgPSByZXF1aXJlKCdpbmhlcml0cycpXG52YXIgSGFzaCA9IHJlcXVpcmUoJy4vaGFzaCcpXG5cbnZhciBLID0gW1xuICAweDQyOEEyRjk4LCAweDcxMzc0NDkxLCAweEI1QzBGQkNGLCAweEU5QjVEQkE1LFxuICAweDM5NTZDMjVCLCAweDU5RjExMUYxLCAweDkyM0Y4MkE0LCAweEFCMUM1RUQ1LFxuICAweEQ4MDdBQTk4LCAweDEyODM1QjAxLCAweDI0MzE4NUJFLCAweDU1MEM3REMzLFxuICAweDcyQkU1RDc0LCAweDgwREVCMUZFLCAweDlCREMwNkE3LCAweEMxOUJGMTc0LFxuICAweEU0OUI2OUMxLCAweEVGQkU0Nzg2LCAweDBGQzE5REM2LCAweDI0MENBMUNDLFxuICAweDJERTkyQzZGLCAweDRBNzQ4NEFBLCAweDVDQjBBOURDLCAweDc2Rjk4OERBLFxuICAweDk4M0U1MTUyLCAweEE4MzFDNjZELCAweEIwMDMyN0M4LCAweEJGNTk3RkM3LFxuICAweEM2RTAwQkYzLCAweEQ1QTc5MTQ3LCAweDA2Q0E2MzUxLCAweDE0MjkyOTY3LFxuICAweDI3QjcwQTg1LCAweDJFMUIyMTM4LCAweDREMkM2REZDLCAweDUzMzgwRDEzLFxuICAweDY1MEE3MzU0LCAweDc2NkEwQUJCLCAweDgxQzJDOTJFLCAweDkyNzIyQzg1LFxuICAweEEyQkZFOEExLCAweEE4MUE2NjRCLCAweEMyNEI4QjcwLCAweEM3NkM1MUEzLFxuICAweEQxOTJFODE5LCAweEQ2OTkwNjI0LCAweEY0MEUzNTg1LCAweDEwNkFBMDcwLFxuICAweDE5QTRDMTE2LCAweDFFMzc2QzA4LCAweDI3NDg3NzRDLCAweDM0QjBCQ0I1LFxuICAweDM5MUMwQ0IzLCAweDRFRDhBQTRBLCAweDVCOUNDQTRGLCAweDY4MkU2RkYzLFxuICAweDc0OEY4MkVFLCAweDc4QTU2MzZGLCAweDg0Qzg3ODE0LCAweDhDQzcwMjA4LFxuICAweDkwQkVGRkZBLCAweEE0NTA2Q0VCLCAweEJFRjlBM0Y3LCAweEM2NzE3OEYyXG5dXG5cbnZhciBXID0gbmV3IEFycmF5KDY0KVxuXG5mdW5jdGlvbiBTaGEyNTYgKCkge1xuICB0aGlzLmluaXQoKVxuXG4gIHRoaXMuX3cgPSBXIC8vIG5ldyBBcnJheSg2NClcblxuICBIYXNoLmNhbGwodGhpcywgNjQsIDU2KVxufVxuXG5pbmhlcml0cyhTaGEyNTYsIEhhc2gpXG5cblNoYTI1Ni5wcm90b3R5cGUuaW5pdCA9IGZ1bmN0aW9uICgpIHtcbiAgdGhpcy5fYSA9IDB4NmEwOWU2NjdcbiAgdGhpcy5fYiA9IDB4YmI2N2FlODVcbiAgdGhpcy5fYyA9IDB4M2M2ZWYzNzJcbiAgdGhpcy5fZCA9IDB4YTU0ZmY1M2FcbiAgdGhpcy5fZSA9IDB4NTEwZTUyN2ZcbiAgdGhpcy5fZiA9IDB4OWIwNTY4OGNcbiAgdGhpcy5fZyA9IDB4MWY4M2Q5YWJcbiAgdGhpcy5faCA9IDB4NWJlMGNkMTlcblxuICByZXR1cm4gdGhpc1xufVxuXG5mdW5jdGlvbiBjaCAoeCwgeSwgeikge1xuICByZXR1cm4geiBeICh4ICYgKHkgXiB6KSlcbn1cblxuZnVuY3Rpb24gbWFqICh4LCB5LCB6KSB7XG4gIHJldHVybiAoeCAmIHkpIHwgKHogJiAoeCB8IHkpKVxufVxuXG5mdW5jdGlvbiBzaWdtYTAgKHgpIHtcbiAgcmV0dXJuICh4ID4+PiAyIHwgeCA8PCAzMCkgXiAoeCA+Pj4gMTMgfCB4IDw8IDE5KSBeICh4ID4+PiAyMiB8IHggPDwgMTApXG59XG5cbmZ1bmN0aW9uIHNpZ21hMSAoeCkge1xuICByZXR1cm4gKHggPj4+IDYgfCB4IDw8IDI2KSBeICh4ID4+PiAxMSB8IHggPDwgMjEpIF4gKHggPj4+IDI1IHwgeCA8PCA3KVxufVxuXG5mdW5jdGlvbiBnYW1tYTAgKHgpIHtcbiAgcmV0dXJuICh4ID4+PiA3IHwgeCA8PCAyNSkgXiAoeCA+Pj4gMTggfCB4IDw8IDE0KSBeICh4ID4+PiAzKVxufVxuXG5mdW5jdGlvbiBnYW1tYTEgKHgpIHtcbiAgcmV0dXJuICh4ID4+PiAxNyB8IHggPDwgMTUpIF4gKHggPj4+IDE5IHwgeCA8PCAxMykgXiAoeCA+Pj4gMTApXG59XG5cblNoYTI1Ni5wcm90b3R5cGUuX3VwZGF0ZSA9IGZ1bmN0aW9uIChNKSB7XG4gIHZhciBXID0gdGhpcy5fd1xuXG4gIHZhciBhID0gdGhpcy5fYSB8IDBcbiAgdmFyIGIgPSB0aGlzLl9iIHwgMFxuICB2YXIgYyA9IHRoaXMuX2MgfCAwXG4gIHZhciBkID0gdGhpcy5fZCB8IDBcbiAgdmFyIGUgPSB0aGlzLl9lIHwgMFxuICB2YXIgZiA9IHRoaXMuX2YgfCAwXG4gIHZhciBnID0gdGhpcy5fZyB8IDBcbiAgdmFyIGggPSB0aGlzLl9oIHwgMFxuXG4gIGZvciAodmFyIGkgPSAwOyBpIDwgMTY7ICsraSkgV1tpXSA9IE0ucmVhZEludDMyQkUoaSAqIDQpXG4gIGZvciAoOyBpIDwgNjQ7ICsraSkgV1tpXSA9IChnYW1tYTEoV1tpIC0gMl0pICsgV1tpIC0gN10gKyBnYW1tYTAoV1tpIC0gMTVdKSArIFdbaSAtIDE2XSkgfCAwXG5cbiAgZm9yICh2YXIgaiA9IDA7IGogPCA2NDsgKytqKSB7XG4gICAgdmFyIFQxID0gKGggKyBzaWdtYTEoZSkgKyBjaChlLCBmLCBnKSArIEtbal0gKyBXW2pdKSB8IDBcbiAgICB2YXIgVDIgPSAoc2lnbWEwKGEpICsgbWFqKGEsIGIsIGMpKSB8IDBcblxuICAgIGggPSBnXG4gICAgZyA9IGZcbiAgICBmID0gZVxuICAgIGUgPSAoZCArIFQxKSB8IDBcbiAgICBkID0gY1xuICAgIGMgPSBiXG4gICAgYiA9IGFcbiAgICBhID0gKFQxICsgVDIpIHwgMFxuICB9XG5cbiAgdGhpcy5fYSA9IChhICsgdGhpcy5fYSkgfCAwXG4gIHRoaXMuX2IgPSAoYiArIHRoaXMuX2IpIHwgMFxuICB0aGlzLl9jID0gKGMgKyB0aGlzLl9jKSB8IDBcbiAgdGhpcy5fZCA9IChkICsgdGhpcy5fZCkgfCAwXG4gIHRoaXMuX2UgPSAoZSArIHRoaXMuX2UpIHwgMFxuICB0aGlzLl9mID0gKGYgKyB0aGlzLl9mKSB8IDBcbiAgdGhpcy5fZyA9IChnICsgdGhpcy5fZykgfCAwXG4gIHRoaXMuX2ggPSAoaCArIHRoaXMuX2gpIHwgMFxufVxuXG5TaGEyNTYucHJvdG90eXBlLl9oYXNoID0gZnVuY3Rpb24gKCkge1xuICB2YXIgSCA9IG5ldyBCdWZmZXIoMzIpXG5cbiAgSC53cml0ZUludDMyQkUodGhpcy5fYSwgMClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fYiwgNClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fYywgOClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fZCwgMTIpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2UsIDE2KVxuICBILndyaXRlSW50MzJCRSh0aGlzLl9mLCAyMClcbiAgSC53cml0ZUludDMyQkUodGhpcy5fZywgMjQpXG4gIEgud3JpdGVJbnQzMkJFKHRoaXMuX2gsIDI4KVxuXG4gIHJldHVybiBIXG59XG5cbm1vZHVsZS5leHBvcnRzID0gU2hhMjU2XG4iLCJ2YXIgaW5oZXJpdHMgPSByZXF1aXJlKCdpbmhlcml0cycpXG52YXIgU0hBNTEyID0gcmVxdWlyZSgnLi9zaGE1MTInKVxudmFyIEhhc2ggPSByZXF1aXJlKCcuL2hhc2gnKVxuXG52YXIgVyA9IG5ldyBBcnJheSgxNjApXG5cbmZ1bmN0aW9uIFNoYTM4NCAoKSB7XG4gIHRoaXMuaW5pdCgpXG4gIHRoaXMuX3cgPSBXXG5cbiAgSGFzaC5jYWxsKHRoaXMsIDEyOCwgMTEyKVxufVxuXG5pbmhlcml0cyhTaGEzODQsIFNIQTUxMilcblxuU2hhMzg0LnByb3RvdHlwZS5pbml0ID0gZnVuY3Rpb24gKCkge1xuICB0aGlzLl9haCA9IDB4Y2JiYjlkNWRcbiAgdGhpcy5fYmggPSAweDYyOWEyOTJhXG4gIHRoaXMuX2NoID0gMHg5MTU5MDE1YVxuICB0aGlzLl9kaCA9IDB4MTUyZmVjZDhcbiAgdGhpcy5fZWggPSAweDY3MzMyNjY3XG4gIHRoaXMuX2ZoID0gMHg4ZWI0NGE4N1xuICB0aGlzLl9naCA9IDB4ZGIwYzJlMGRcbiAgdGhpcy5faGggPSAweDQ3YjU0ODFkXG5cbiAgdGhpcy5fYWwgPSAweGMxMDU5ZWQ4XG4gIHRoaXMuX2JsID0gMHgzNjdjZDUwN1xuICB0aGlzLl9jbCA9IDB4MzA3MGRkMTdcbiAgdGhpcy5fZGwgPSAweGY3MGU1OTM5XG4gIHRoaXMuX2VsID0gMHhmZmMwMGIzMVxuICB0aGlzLl9mbCA9IDB4Njg1ODE1MTFcbiAgdGhpcy5fZ2wgPSAweDY0Zjk4ZmE3XG4gIHRoaXMuX2hsID0gMHhiZWZhNGZhNFxuXG4gIHJldHVybiB0aGlzXG59XG5cblNoYTM4NC5wcm90b3R5cGUuX2hhc2ggPSBmdW5jdGlvbiAoKSB7XG4gIHZhciBIID0gbmV3IEJ1ZmZlcig0OClcblxuICBmdW5jdGlvbiB3cml0ZUludDY0QkUgKGgsIGwsIG9mZnNldCkge1xuICAgIEgud3JpdGVJbnQzMkJFKGgsIG9mZnNldClcbiAgICBILndyaXRlSW50MzJCRShsLCBvZmZzZXQgKyA0KVxuICB9XG5cbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2FoLCB0aGlzLl9hbCwgMClcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2JoLCB0aGlzLl9ibCwgOClcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2NoLCB0aGlzLl9jbCwgMTYpXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9kaCwgdGhpcy5fZGwsIDI0KVxuICB3cml0ZUludDY0QkUodGhpcy5fZWgsIHRoaXMuX2VsLCAzMilcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2ZoLCB0aGlzLl9mbCwgNDApXG5cbiAgcmV0dXJuIEhcbn1cblxubW9kdWxlLmV4cG9ydHMgPSBTaGEzODRcbiIsInZhciBpbmhlcml0cyA9IHJlcXVpcmUoJ2luaGVyaXRzJylcbnZhciBIYXNoID0gcmVxdWlyZSgnLi9oYXNoJylcblxudmFyIEsgPSBbXG4gIDB4NDI4YTJmOTgsIDB4ZDcyOGFlMjIsIDB4NzEzNzQ0OTEsIDB4MjNlZjY1Y2QsXG4gIDB4YjVjMGZiY2YsIDB4ZWM0ZDNiMmYsIDB4ZTliNWRiYTUsIDB4ODE4OWRiYmMsXG4gIDB4Mzk1NmMyNWIsIDB4ZjM0OGI1MzgsIDB4NTlmMTExZjEsIDB4YjYwNWQwMTksXG4gIDB4OTIzZjgyYTQsIDB4YWYxOTRmOWIsIDB4YWIxYzVlZDUsIDB4ZGE2ZDgxMTgsXG4gIDB4ZDgwN2FhOTgsIDB4YTMwMzAyNDIsIDB4MTI4MzViMDEsIDB4NDU3MDZmYmUsXG4gIDB4MjQzMTg1YmUsIDB4NGVlNGIyOGMsIDB4NTUwYzdkYzMsIDB4ZDVmZmI0ZTIsXG4gIDB4NzJiZTVkNzQsIDB4ZjI3Yjg5NmYsIDB4ODBkZWIxZmUsIDB4M2IxNjk2YjEsXG4gIDB4OWJkYzA2YTcsIDB4MjVjNzEyMzUsIDB4YzE5YmYxNzQsIDB4Y2Y2OTI2OTQsXG4gIDB4ZTQ5YjY5YzEsIDB4OWVmMTRhZDIsIDB4ZWZiZTQ3ODYsIDB4Mzg0ZjI1ZTMsXG4gIDB4MGZjMTlkYzYsIDB4OGI4Y2Q1YjUsIDB4MjQwY2ExY2MsIDB4NzdhYzljNjUsXG4gIDB4MmRlOTJjNmYsIDB4NTkyYjAyNzUsIDB4NGE3NDg0YWEsIDB4NmVhNmU0ODMsXG4gIDB4NWNiMGE5ZGMsIDB4YmQ0MWZiZDQsIDB4NzZmOTg4ZGEsIDB4ODMxMTUzYjUsXG4gIDB4OTgzZTUxNTIsIDB4ZWU2NmRmYWIsIDB4YTgzMWM2NmQsIDB4MmRiNDMyMTAsXG4gIDB4YjAwMzI3YzgsIDB4OThmYjIxM2YsIDB4YmY1OTdmYzcsIDB4YmVlZjBlZTQsXG4gIDB4YzZlMDBiZjMsIDB4M2RhODhmYzIsIDB4ZDVhNzkxNDcsIDB4OTMwYWE3MjUsXG4gIDB4MDZjYTYzNTEsIDB4ZTAwMzgyNmYsIDB4MTQyOTI5NjcsIDB4MGEwZTZlNzAsXG4gIDB4MjdiNzBhODUsIDB4NDZkMjJmZmMsIDB4MmUxYjIxMzgsIDB4NWMyNmM5MjYsXG4gIDB4NGQyYzZkZmMsIDB4NWFjNDJhZWQsIDB4NTMzODBkMTMsIDB4OWQ5NWIzZGYsXG4gIDB4NjUwYTczNTQsIDB4OGJhZjYzZGUsIDB4NzY2YTBhYmIsIDB4M2M3N2IyYTgsXG4gIDB4ODFjMmM5MmUsIDB4NDdlZGFlZTYsIDB4OTI3MjJjODUsIDB4MTQ4MjM1M2IsXG4gIDB4YTJiZmU4YTEsIDB4NGNmMTAzNjQsIDB4YTgxYTY2NGIsIDB4YmM0MjMwMDEsXG4gIDB4YzI0YjhiNzAsIDB4ZDBmODk3OTEsIDB4Yzc2YzUxYTMsIDB4MDY1NGJlMzAsXG4gIDB4ZDE5MmU4MTksIDB4ZDZlZjUyMTgsIDB4ZDY5OTA2MjQsIDB4NTU2NWE5MTAsXG4gIDB4ZjQwZTM1ODUsIDB4NTc3MTIwMmEsIDB4MTA2YWEwNzAsIDB4MzJiYmQxYjgsXG4gIDB4MTlhNGMxMTYsIDB4YjhkMmQwYzgsIDB4MWUzNzZjMDgsIDB4NTE0MWFiNTMsXG4gIDB4Mjc0ODc3NGMsIDB4ZGY4ZWViOTksIDB4MzRiMGJjYjUsIDB4ZTE5YjQ4YTgsXG4gIDB4MzkxYzBjYjMsIDB4YzVjOTVhNjMsIDB4NGVkOGFhNGEsIDB4ZTM0MThhY2IsXG4gIDB4NWI5Y2NhNGYsIDB4Nzc2M2UzNzMsIDB4NjgyZTZmZjMsIDB4ZDZiMmI4YTMsXG4gIDB4NzQ4ZjgyZWUsIDB4NWRlZmIyZmMsIDB4NzhhNTYzNmYsIDB4NDMxNzJmNjAsXG4gIDB4ODRjODc4MTQsIDB4YTFmMGFiNzIsIDB4OGNjNzAyMDgsIDB4MWE2NDM5ZWMsXG4gIDB4OTBiZWZmZmEsIDB4MjM2MzFlMjgsIDB4YTQ1MDZjZWIsIDB4ZGU4MmJkZTksXG4gIDB4YmVmOWEzZjcsIDB4YjJjNjc5MTUsIDB4YzY3MTc4ZjIsIDB4ZTM3MjUzMmIsXG4gIDB4Y2EyNzNlY2UsIDB4ZWEyNjYxOWMsIDB4ZDE4NmI4YzcsIDB4MjFjMGMyMDcsXG4gIDB4ZWFkYTdkZDYsIDB4Y2RlMGViMWUsIDB4ZjU3ZDRmN2YsIDB4ZWU2ZWQxNzgsXG4gIDB4MDZmMDY3YWEsIDB4NzIxNzZmYmEsIDB4MGE2MzdkYzUsIDB4YTJjODk4YTYsXG4gIDB4MTEzZjk4MDQsIDB4YmVmOTBkYWUsIDB4MWI3MTBiMzUsIDB4MTMxYzQ3MWIsXG4gIDB4MjhkYjc3ZjUsIDB4MjMwNDdkODQsIDB4MzJjYWFiN2IsIDB4NDBjNzI0OTMsXG4gIDB4M2M5ZWJlMGEsIDB4MTVjOWJlYmMsIDB4NDMxZDY3YzQsIDB4OWMxMDBkNGMsXG4gIDB4NGNjNWQ0YmUsIDB4Y2IzZTQyYjYsIDB4NTk3ZjI5OWMsIDB4ZmM2NTdlMmEsXG4gIDB4NWZjYjZmYWIsIDB4M2FkNmZhZWMsIDB4NmM0NDE5OGMsIDB4NGE0NzU4MTdcbl1cblxudmFyIFcgPSBuZXcgQXJyYXkoMTYwKVxuXG5mdW5jdGlvbiBTaGE1MTIgKCkge1xuICB0aGlzLmluaXQoKVxuICB0aGlzLl93ID0gV1xuXG4gIEhhc2guY2FsbCh0aGlzLCAxMjgsIDExMilcbn1cblxuaW5oZXJpdHMoU2hhNTEyLCBIYXNoKVxuXG5TaGE1MTIucHJvdG90eXBlLmluaXQgPSBmdW5jdGlvbiAoKSB7XG4gIHRoaXMuX2FoID0gMHg2YTA5ZTY2N1xuICB0aGlzLl9iaCA9IDB4YmI2N2FlODVcbiAgdGhpcy5fY2ggPSAweDNjNmVmMzcyXG4gIHRoaXMuX2RoID0gMHhhNTRmZjUzYVxuICB0aGlzLl9laCA9IDB4NTEwZTUyN2ZcbiAgdGhpcy5fZmggPSAweDliMDU2ODhjXG4gIHRoaXMuX2doID0gMHgxZjgzZDlhYlxuICB0aGlzLl9oaCA9IDB4NWJlMGNkMTlcblxuICB0aGlzLl9hbCA9IDB4ZjNiY2M5MDhcbiAgdGhpcy5fYmwgPSAweDg0Y2FhNzNiXG4gIHRoaXMuX2NsID0gMHhmZTk0ZjgyYlxuICB0aGlzLl9kbCA9IDB4NWYxZDM2ZjFcbiAgdGhpcy5fZWwgPSAweGFkZTY4MmQxXG4gIHRoaXMuX2ZsID0gMHgyYjNlNmMxZlxuICB0aGlzLl9nbCA9IDB4ZmI0MWJkNmJcbiAgdGhpcy5faGwgPSAweDEzN2UyMTc5XG5cbiAgcmV0dXJuIHRoaXNcbn1cblxuZnVuY3Rpb24gQ2ggKHgsIHksIHopIHtcbiAgcmV0dXJuIHogXiAoeCAmICh5IF4geikpXG59XG5cbmZ1bmN0aW9uIG1haiAoeCwgeSwgeikge1xuICByZXR1cm4gKHggJiB5KSB8ICh6ICYgKHggfCB5KSlcbn1cblxuZnVuY3Rpb24gc2lnbWEwICh4LCB4bCkge1xuICByZXR1cm4gKHggPj4+IDI4IHwgeGwgPDwgNCkgXiAoeGwgPj4+IDIgfCB4IDw8IDMwKSBeICh4bCA+Pj4gNyB8IHggPDwgMjUpXG59XG5cbmZ1bmN0aW9uIHNpZ21hMSAoeCwgeGwpIHtcbiAgcmV0dXJuICh4ID4+PiAxNCB8IHhsIDw8IDE4KSBeICh4ID4+PiAxOCB8IHhsIDw8IDE0KSBeICh4bCA+Pj4gOSB8IHggPDwgMjMpXG59XG5cbmZ1bmN0aW9uIEdhbW1hMCAoeCwgeGwpIHtcbiAgcmV0dXJuICh4ID4+PiAxIHwgeGwgPDwgMzEpIF4gKHggPj4+IDggfCB4bCA8PCAyNCkgXiAoeCA+Pj4gNylcbn1cblxuZnVuY3Rpb24gR2FtbWEwbCAoeCwgeGwpIHtcbiAgcmV0dXJuICh4ID4+PiAxIHwgeGwgPDwgMzEpIF4gKHggPj4+IDggfCB4bCA8PCAyNCkgXiAoeCA+Pj4gNyB8IHhsIDw8IDI1KVxufVxuXG5mdW5jdGlvbiBHYW1tYTEgKHgsIHhsKSB7XG4gIHJldHVybiAoeCA+Pj4gMTkgfCB4bCA8PCAxMykgXiAoeGwgPj4+IDI5IHwgeCA8PCAzKSBeICh4ID4+PiA2KVxufVxuXG5mdW5jdGlvbiBHYW1tYTFsICh4LCB4bCkge1xuICByZXR1cm4gKHggPj4+IDE5IHwgeGwgPDwgMTMpIF4gKHhsID4+PiAyOSB8IHggPDwgMykgXiAoeCA+Pj4gNiB8IHhsIDw8IDI2KVxufVxuXG5mdW5jdGlvbiBnZXRDYXJyeSAoYSwgYikge1xuICByZXR1cm4gKGEgPj4+IDApIDwgKGIgPj4+IDApID8gMSA6IDBcbn1cblxuU2hhNTEyLnByb3RvdHlwZS5fdXBkYXRlID0gZnVuY3Rpb24gKE0pIHtcbiAgdmFyIFcgPSB0aGlzLl93XG5cbiAgdmFyIGFoID0gdGhpcy5fYWggfCAwXG4gIHZhciBiaCA9IHRoaXMuX2JoIHwgMFxuICB2YXIgY2ggPSB0aGlzLl9jaCB8IDBcbiAgdmFyIGRoID0gdGhpcy5fZGggfCAwXG4gIHZhciBlaCA9IHRoaXMuX2VoIHwgMFxuICB2YXIgZmggPSB0aGlzLl9maCB8IDBcbiAgdmFyIGdoID0gdGhpcy5fZ2ggfCAwXG4gIHZhciBoaCA9IHRoaXMuX2hoIHwgMFxuXG4gIHZhciBhbCA9IHRoaXMuX2FsIHwgMFxuICB2YXIgYmwgPSB0aGlzLl9ibCB8IDBcbiAgdmFyIGNsID0gdGhpcy5fY2wgfCAwXG4gIHZhciBkbCA9IHRoaXMuX2RsIHwgMFxuICB2YXIgZWwgPSB0aGlzLl9lbCB8IDBcbiAgdmFyIGZsID0gdGhpcy5fZmwgfCAwXG4gIHZhciBnbCA9IHRoaXMuX2dsIHwgMFxuICB2YXIgaGwgPSB0aGlzLl9obCB8IDBcblxuICBmb3IgKHZhciBpID0gMDsgaSA8IDMyOyBpICs9IDIpIHtcbiAgICBXW2ldID0gTS5yZWFkSW50MzJCRShpICogNClcbiAgICBXW2kgKyAxXSA9IE0ucmVhZEludDMyQkUoaSAqIDQgKyA0KVxuICB9XG4gIGZvciAoOyBpIDwgMTYwOyBpICs9IDIpIHtcbiAgICB2YXIgeGggPSBXW2kgLSAxNSAqIDJdXG4gICAgdmFyIHhsID0gV1tpIC0gMTUgKiAyICsgMV1cbiAgICB2YXIgZ2FtbWEwID0gR2FtbWEwKHhoLCB4bClcbiAgICB2YXIgZ2FtbWEwbCA9IEdhbW1hMGwoeGwsIHhoKVxuXG4gICAgeGggPSBXW2kgLSAyICogMl1cbiAgICB4bCA9IFdbaSAtIDIgKiAyICsgMV1cbiAgICB2YXIgZ2FtbWExID0gR2FtbWExKHhoLCB4bClcbiAgICB2YXIgZ2FtbWExbCA9IEdhbW1hMWwoeGwsIHhoKVxuXG4gICAgLy8gV1tpXSA9IGdhbW1hMCArIFdbaSAtIDddICsgZ2FtbWExICsgV1tpIC0gMTZdXG4gICAgdmFyIFdpN2ggPSBXW2kgLSA3ICogMl1cbiAgICB2YXIgV2k3bCA9IFdbaSAtIDcgKiAyICsgMV1cblxuICAgIHZhciBXaTE2aCA9IFdbaSAtIDE2ICogMl1cbiAgICB2YXIgV2kxNmwgPSBXW2kgLSAxNiAqIDIgKyAxXVxuXG4gICAgdmFyIFdpbCA9IChnYW1tYTBsICsgV2k3bCkgfCAwXG4gICAgdmFyIFdpaCA9IChnYW1tYTAgKyBXaTdoICsgZ2V0Q2FycnkoV2lsLCBnYW1tYTBsKSkgfCAwXG4gICAgV2lsID0gKFdpbCArIGdhbW1hMWwpIHwgMFxuICAgIFdpaCA9IChXaWggKyBnYW1tYTEgKyBnZXRDYXJyeShXaWwsIGdhbW1hMWwpKSB8IDBcbiAgICBXaWwgPSAoV2lsICsgV2kxNmwpIHwgMFxuICAgIFdpaCA9IChXaWggKyBXaTE2aCArIGdldENhcnJ5KFdpbCwgV2kxNmwpKSB8IDBcblxuICAgIFdbaV0gPSBXaWhcbiAgICBXW2kgKyAxXSA9IFdpbFxuICB9XG5cbiAgZm9yICh2YXIgaiA9IDA7IGogPCAxNjA7IGogKz0gMikge1xuICAgIFdpaCA9IFdbal1cbiAgICBXaWwgPSBXW2ogKyAxXVxuXG4gICAgdmFyIG1hamggPSBtYWooYWgsIGJoLCBjaClcbiAgICB2YXIgbWFqbCA9IG1haihhbCwgYmwsIGNsKVxuXG4gICAgdmFyIHNpZ21hMGggPSBzaWdtYTAoYWgsIGFsKVxuICAgIHZhciBzaWdtYTBsID0gc2lnbWEwKGFsLCBhaClcbiAgICB2YXIgc2lnbWExaCA9IHNpZ21hMShlaCwgZWwpXG4gICAgdmFyIHNpZ21hMWwgPSBzaWdtYTEoZWwsIGVoKVxuXG4gICAgLy8gdDEgPSBoICsgc2lnbWExICsgY2ggKyBLW2pdICsgV1tqXVxuICAgIHZhciBLaWggPSBLW2pdXG4gICAgdmFyIEtpbCA9IEtbaiArIDFdXG5cbiAgICB2YXIgY2hoID0gQ2goZWgsIGZoLCBnaClcbiAgICB2YXIgY2hsID0gQ2goZWwsIGZsLCBnbClcblxuICAgIHZhciB0MWwgPSAoaGwgKyBzaWdtYTFsKSB8IDBcbiAgICB2YXIgdDFoID0gKGhoICsgc2lnbWExaCArIGdldENhcnJ5KHQxbCwgaGwpKSB8IDBcbiAgICB0MWwgPSAodDFsICsgY2hsKSB8IDBcbiAgICB0MWggPSAodDFoICsgY2hoICsgZ2V0Q2FycnkodDFsLCBjaGwpKSB8IDBcbiAgICB0MWwgPSAodDFsICsgS2lsKSB8IDBcbiAgICB0MWggPSAodDFoICsgS2loICsgZ2V0Q2FycnkodDFsLCBLaWwpKSB8IDBcbiAgICB0MWwgPSAodDFsICsgV2lsKSB8IDBcbiAgICB0MWggPSAodDFoICsgV2loICsgZ2V0Q2FycnkodDFsLCBXaWwpKSB8IDBcblxuICAgIC8vIHQyID0gc2lnbWEwICsgbWFqXG4gICAgdmFyIHQybCA9IChzaWdtYTBsICsgbWFqbCkgfCAwXG4gICAgdmFyIHQyaCA9IChzaWdtYTBoICsgbWFqaCArIGdldENhcnJ5KHQybCwgc2lnbWEwbCkpIHwgMFxuXG4gICAgaGggPSBnaFxuICAgIGhsID0gZ2xcbiAgICBnaCA9IGZoXG4gICAgZ2wgPSBmbFxuICAgIGZoID0gZWhcbiAgICBmbCA9IGVsXG4gICAgZWwgPSAoZGwgKyB0MWwpIHwgMFxuICAgIGVoID0gKGRoICsgdDFoICsgZ2V0Q2FycnkoZWwsIGRsKSkgfCAwXG4gICAgZGggPSBjaFxuICAgIGRsID0gY2xcbiAgICBjaCA9IGJoXG4gICAgY2wgPSBibFxuICAgIGJoID0gYWhcbiAgICBibCA9IGFsXG4gICAgYWwgPSAodDFsICsgdDJsKSB8IDBcbiAgICBhaCA9ICh0MWggKyB0MmggKyBnZXRDYXJyeShhbCwgdDFsKSkgfCAwXG4gIH1cblxuICB0aGlzLl9hbCA9ICh0aGlzLl9hbCArIGFsKSB8IDBcbiAgdGhpcy5fYmwgPSAodGhpcy5fYmwgKyBibCkgfCAwXG4gIHRoaXMuX2NsID0gKHRoaXMuX2NsICsgY2wpIHwgMFxuICB0aGlzLl9kbCA9ICh0aGlzLl9kbCArIGRsKSB8IDBcbiAgdGhpcy5fZWwgPSAodGhpcy5fZWwgKyBlbCkgfCAwXG4gIHRoaXMuX2ZsID0gKHRoaXMuX2ZsICsgZmwpIHwgMFxuICB0aGlzLl9nbCA9ICh0aGlzLl9nbCArIGdsKSB8IDBcbiAgdGhpcy5faGwgPSAodGhpcy5faGwgKyBobCkgfCAwXG5cbiAgdGhpcy5fYWggPSAodGhpcy5fYWggKyBhaCArIGdldENhcnJ5KHRoaXMuX2FsLCBhbCkpIHwgMFxuICB0aGlzLl9iaCA9ICh0aGlzLl9iaCArIGJoICsgZ2V0Q2FycnkodGhpcy5fYmwsIGJsKSkgfCAwXG4gIHRoaXMuX2NoID0gKHRoaXMuX2NoICsgY2ggKyBnZXRDYXJyeSh0aGlzLl9jbCwgY2wpKSB8IDBcbiAgdGhpcy5fZGggPSAodGhpcy5fZGggKyBkaCArIGdldENhcnJ5KHRoaXMuX2RsLCBkbCkpIHwgMFxuICB0aGlzLl9laCA9ICh0aGlzLl9laCArIGVoICsgZ2V0Q2FycnkodGhpcy5fZWwsIGVsKSkgfCAwXG4gIHRoaXMuX2ZoID0gKHRoaXMuX2ZoICsgZmggKyBnZXRDYXJyeSh0aGlzLl9mbCwgZmwpKSB8IDBcbiAgdGhpcy5fZ2ggPSAodGhpcy5fZ2ggKyBnaCArIGdldENhcnJ5KHRoaXMuX2dsLCBnbCkpIHwgMFxuICB0aGlzLl9oaCA9ICh0aGlzLl9oaCArIGhoICsgZ2V0Q2FycnkodGhpcy5faGwsIGhsKSkgfCAwXG59XG5cblNoYTUxMi5wcm90b3R5cGUuX2hhc2ggPSBmdW5jdGlvbiAoKSB7XG4gIHZhciBIID0gbmV3IEJ1ZmZlcig2NClcblxuICBmdW5jdGlvbiB3cml0ZUludDY0QkUgKGgsIGwsIG9mZnNldCkge1xuICAgIEgud3JpdGVJbnQzMkJFKGgsIG9mZnNldClcbiAgICBILndyaXRlSW50MzJCRShsLCBvZmZzZXQgKyA0KVxuICB9XG5cbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2FoLCB0aGlzLl9hbCwgMClcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2JoLCB0aGlzLl9ibCwgOClcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2NoLCB0aGlzLl9jbCwgMTYpXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9kaCwgdGhpcy5fZGwsIDI0KVxuICB3cml0ZUludDY0QkUodGhpcy5fZWgsIHRoaXMuX2VsLCAzMilcbiAgd3JpdGVJbnQ2NEJFKHRoaXMuX2ZoLCB0aGlzLl9mbCwgNDApXG4gIHdyaXRlSW50NjRCRSh0aGlzLl9naCwgdGhpcy5fZ2wsIDQ4KVxuICB3cml0ZUludDY0QkUodGhpcy5faGgsIHRoaXMuX2hsLCA1NilcblxuICByZXR1cm4gSFxufVxuXG5tb2R1bGUuZXhwb3J0cyA9IFNoYTUxMlxuIiwiKGZ1bmN0aW9uKG5hY2wpIHtcbid1c2Ugc3RyaWN0JztcblxuLy8gUG9ydGVkIGluIDIwMTQgYnkgRG1pdHJ5IENoZXN0bnlraCBhbmQgRGV2aSBNYW5kaXJpLlxuLy8gUHVibGljIGRvbWFpbi5cbi8vXG4vLyBJbXBsZW1lbnRhdGlvbiBkZXJpdmVkIGZyb20gVHdlZXROYUNsIHZlcnNpb24gMjAxNDA0MjcuXG4vLyBTZWUgZm9yIGRldGFpbHM6IGh0dHA6Ly90d2VldG5hY2wuY3IueXAudG8vXG5cbnZhciBnZiA9IGZ1bmN0aW9uKGluaXQpIHtcbiAgdmFyIGksIHIgPSBuZXcgRmxvYXQ2NEFycmF5KDE2KTtcbiAgaWYgKGluaXQpIGZvciAoaSA9IDA7IGkgPCBpbml0Lmxlbmd0aDsgaSsrKSByW2ldID0gaW5pdFtpXTtcbiAgcmV0dXJuIHI7XG59O1xuXG4vLyAgUGx1Z2dhYmxlLCBpbml0aWFsaXplZCBpbiBoaWdoLWxldmVsIEFQSSBiZWxvdy5cbnZhciByYW5kb21ieXRlcyA9IGZ1bmN0aW9uKC8qIHgsIG4gKi8pIHsgdGhyb3cgbmV3IEVycm9yKCdubyBQUk5HJyk7IH07XG5cbnZhciBfMCA9IG5ldyBVaW50OEFycmF5KDE2KTtcbnZhciBfOSA9IG5ldyBVaW50OEFycmF5KDMyKTsgXzlbMF0gPSA5O1xuXG52YXIgZ2YwID0gZ2YoKSxcbiAgICBnZjEgPSBnZihbMV0pLFxuICAgIF8xMjE2NjUgPSBnZihbMHhkYjQxLCAxXSksXG4gICAgRCA9IGdmKFsweDc4YTMsIDB4MTM1OSwgMHg0ZGNhLCAweDc1ZWIsIDB4ZDhhYiwgMHg0MTQxLCAweDBhNGQsIDB4MDA3MCwgMHhlODk4LCAweDc3NzksIDB4NDA3OSwgMHg4Y2M3LCAweGZlNzMsIDB4MmI2ZiwgMHg2Y2VlLCAweDUyMDNdKSxcbiAgICBEMiA9IGdmKFsweGYxNTksIDB4MjZiMiwgMHg5Yjk0LCAweGViZDYsIDB4YjE1NiwgMHg4MjgzLCAweDE0OWEsIDB4MDBlMCwgMHhkMTMwLCAweGVlZjMsIDB4ODBmMiwgMHgxOThlLCAweGZjZTcsIDB4NTZkZiwgMHhkOWRjLCAweDI0MDZdKSxcbiAgICBYID0gZ2YoWzB4ZDUxYSwgMHg4ZjI1LCAweDJkNjAsIDB4Yzk1NiwgMHhhN2IyLCAweDk1MjUsIDB4Yzc2MCwgMHg2OTJjLCAweGRjNWMsIDB4ZmRkNiwgMHhlMjMxLCAweGMwYTQsIDB4NTNmZSwgMHhjZDZlLCAweDM2ZDMsIDB4MjE2OV0pLFxuICAgIFkgPSBnZihbMHg2NjU4LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2LCAweDY2NjYsIDB4NjY2NiwgMHg2NjY2XSksXG4gICAgSSA9IGdmKFsweGEwYjAsIDB4NGEwZSwgMHgxYjI3LCAweGM0ZWUsIDB4ZTQ3OCwgMHhhZDJmLCAweDE4MDYsIDB4MmY0MywgMHhkN2E3LCAweDNkZmIsIDB4MDA5OSwgMHgyYjRkLCAweGRmMGIsIDB4NGZjMSwgMHgyNDgwLCAweDJiODNdKTtcblxuZnVuY3Rpb24gdHM2NCh4LCBpLCBoLCBsKSB7XG4gIHhbaV0gICA9IChoID4+IDI0KSAmIDB4ZmY7XG4gIHhbaSsxXSA9IChoID4+IDE2KSAmIDB4ZmY7XG4gIHhbaSsyXSA9IChoID4+ICA4KSAmIDB4ZmY7XG4gIHhbaSszXSA9IGggJiAweGZmO1xuICB4W2krNF0gPSAobCA+PiAyNCkgICYgMHhmZjtcbiAgeFtpKzVdID0gKGwgPj4gMTYpICAmIDB4ZmY7XG4gIHhbaSs2XSA9IChsID4+ICA4KSAgJiAweGZmO1xuICB4W2krN10gPSBsICYgMHhmZjtcbn1cblxuZnVuY3Rpb24gdm4oeCwgeGksIHksIHlpLCBuKSB7XG4gIHZhciBpLGQgPSAwO1xuICBmb3IgKGkgPSAwOyBpIDwgbjsgaSsrKSBkIHw9IHhbeGkraV1eeVt5aStpXTtcbiAgcmV0dXJuICgxICYgKChkIC0gMSkgPj4+IDgpKSAtIDE7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b192ZXJpZnlfMTYoeCwgeGksIHksIHlpKSB7XG4gIHJldHVybiB2bih4LHhpLHkseWksMTYpO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fdmVyaWZ5XzMyKHgsIHhpLCB5LCB5aSkge1xuICByZXR1cm4gdm4oeCx4aSx5LHlpLDMyKTtcbn1cblxuZnVuY3Rpb24gY29yZV9zYWxzYTIwKG8sIHAsIGssIGMpIHtcbiAgdmFyIGowICA9IGNbIDBdICYgMHhmZiB8IChjWyAxXSAmIDB4ZmYpPDw4IHwgKGNbIDJdICYgMHhmZik8PDE2IHwgKGNbIDNdICYgMHhmZik8PDI0LFxuICAgICAgajEgID0ga1sgMF0gJiAweGZmIHwgKGtbIDFdICYgMHhmZik8PDggfCAoa1sgMl0gJiAweGZmKTw8MTYgfCAoa1sgM10gJiAweGZmKTw8MjQsXG4gICAgICBqMiAgPSBrWyA0XSAmIDB4ZmYgfCAoa1sgNV0gJiAweGZmKTw8OCB8IChrWyA2XSAmIDB4ZmYpPDwxNiB8IChrWyA3XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGozICA9IGtbIDhdICYgMHhmZiB8IChrWyA5XSAmIDB4ZmYpPDw4IHwgKGtbMTBdICYgMHhmZik8PDE2IHwgKGtbMTFdICYgMHhmZik8PDI0LFxuICAgICAgajQgID0ga1sxMl0gJiAweGZmIHwgKGtbMTNdICYgMHhmZik8PDggfCAoa1sxNF0gJiAweGZmKTw8MTYgfCAoa1sxNV0gJiAweGZmKTw8MjQsXG4gICAgICBqNSAgPSBjWyA0XSAmIDB4ZmYgfCAoY1sgNV0gJiAweGZmKTw8OCB8IChjWyA2XSAmIDB4ZmYpPDwxNiB8IChjWyA3XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGo2ICA9IHBbIDBdICYgMHhmZiB8IChwWyAxXSAmIDB4ZmYpPDw4IHwgKHBbIDJdICYgMHhmZik8PDE2IHwgKHBbIDNdICYgMHhmZik8PDI0LFxuICAgICAgajcgID0gcFsgNF0gJiAweGZmIHwgKHBbIDVdICYgMHhmZik8PDggfCAocFsgNl0gJiAweGZmKTw8MTYgfCAocFsgN10gJiAweGZmKTw8MjQsXG4gICAgICBqOCAgPSBwWyA4XSAmIDB4ZmYgfCAocFsgOV0gJiAweGZmKTw8OCB8IChwWzEwXSAmIDB4ZmYpPDwxNiB8IChwWzExXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGo5ICA9IHBbMTJdICYgMHhmZiB8IChwWzEzXSAmIDB4ZmYpPDw4IHwgKHBbMTRdICYgMHhmZik8PDE2IHwgKHBbMTVdICYgMHhmZik8PDI0LFxuICAgICAgajEwID0gY1sgOF0gJiAweGZmIHwgKGNbIDldICYgMHhmZik8PDggfCAoY1sxMF0gJiAweGZmKTw8MTYgfCAoY1sxMV0gJiAweGZmKTw8MjQsXG4gICAgICBqMTEgPSBrWzE2XSAmIDB4ZmYgfCAoa1sxN10gJiAweGZmKTw8OCB8IChrWzE4XSAmIDB4ZmYpPDwxNiB8IChrWzE5XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxMiA9IGtbMjBdICYgMHhmZiB8IChrWzIxXSAmIDB4ZmYpPDw4IHwgKGtbMjJdICYgMHhmZik8PDE2IHwgKGtbMjNdICYgMHhmZik8PDI0LFxuICAgICAgajEzID0ga1syNF0gJiAweGZmIHwgKGtbMjVdICYgMHhmZik8PDggfCAoa1syNl0gJiAweGZmKTw8MTYgfCAoa1syN10gJiAweGZmKTw8MjQsXG4gICAgICBqMTQgPSBrWzI4XSAmIDB4ZmYgfCAoa1syOV0gJiAweGZmKTw8OCB8IChrWzMwXSAmIDB4ZmYpPDwxNiB8IChrWzMxXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxNSA9IGNbMTJdICYgMHhmZiB8IChjWzEzXSAmIDB4ZmYpPDw4IHwgKGNbMTRdICYgMHhmZik8PDE2IHwgKGNbMTVdICYgMHhmZik8PDI0O1xuXG4gIHZhciB4MCA9IGowLCB4MSA9IGoxLCB4MiA9IGoyLCB4MyA9IGozLCB4NCA9IGo0LCB4NSA9IGo1LCB4NiA9IGo2LCB4NyA9IGo3LFxuICAgICAgeDggPSBqOCwgeDkgPSBqOSwgeDEwID0gajEwLCB4MTEgPSBqMTEsIHgxMiA9IGoxMiwgeDEzID0gajEzLCB4MTQgPSBqMTQsXG4gICAgICB4MTUgPSBqMTUsIHU7XG5cbiAgZm9yICh2YXIgaSA9IDA7IGkgPCAyMDsgaSArPSAyKSB7XG4gICAgdSA9IHgwICsgeDEyIHwgMDtcbiAgICB4NCBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDQgKyB4MCB8IDA7XG4gICAgeDggXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHg4ICsgeDQgfCAwO1xuICAgIHgxMiBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4MTIgKyB4OCB8IDA7XG4gICAgeDAgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4NSArIHgxIHwgMDtcbiAgICB4OSBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDkgKyB4NSB8IDA7XG4gICAgeDEzIF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4MTMgKyB4OSB8IDA7XG4gICAgeDEgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDEgKyB4MTMgfCAwO1xuICAgIHg1IF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDEwICsgeDYgfCAwO1xuICAgIHgxNCBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDE0ICsgeDEwIHwgMDtcbiAgICB4MiBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDIgKyB4MTQgfCAwO1xuICAgIHg2IF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHg2ICsgeDIgfCAwO1xuICAgIHgxMCBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHgxNSArIHgxMSB8IDA7XG4gICAgeDMgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHgzICsgeDE1IHwgMDtcbiAgICB4NyBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDcgKyB4MyB8IDA7XG4gICAgeDExIF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHgxMSArIHg3IHwgMDtcbiAgICB4MTUgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4MCArIHgzIHwgMDtcbiAgICB4MSBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDEgKyB4MCB8IDA7XG4gICAgeDIgXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHgyICsgeDEgfCAwO1xuICAgIHgzIF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHgzICsgeDIgfCAwO1xuICAgIHgwIF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDUgKyB4NCB8IDA7XG4gICAgeDYgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHg2ICsgeDUgfCAwO1xuICAgIHg3IF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4NyArIHg2IHwgMDtcbiAgICB4NCBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4NCArIHg3IHwgMDtcbiAgICB4NSBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHgxMCArIHg5IHwgMDtcbiAgICB4MTEgXj0gdTw8NyB8IHU+Pj4oMzItNyk7XG4gICAgdSA9IHgxMSArIHgxMCB8IDA7XG4gICAgeDggXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHg4ICsgeDExIHwgMDtcbiAgICB4OSBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4OSArIHg4IHwgMDtcbiAgICB4MTAgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4MTUgKyB4MTQgfCAwO1xuICAgIHgxMiBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDEyICsgeDE1IHwgMDtcbiAgICB4MTMgXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHgxMyArIHgxMiB8IDA7XG4gICAgeDE0IF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHgxNCArIHgxMyB8IDA7XG4gICAgeDE1IF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG4gIH1cbiAgIHgwID0gIHgwICsgIGowIHwgMDtcbiAgIHgxID0gIHgxICsgIGoxIHwgMDtcbiAgIHgyID0gIHgyICsgIGoyIHwgMDtcbiAgIHgzID0gIHgzICsgIGozIHwgMDtcbiAgIHg0ID0gIHg0ICsgIGo0IHwgMDtcbiAgIHg1ID0gIHg1ICsgIGo1IHwgMDtcbiAgIHg2ID0gIHg2ICsgIGo2IHwgMDtcbiAgIHg3ID0gIHg3ICsgIGo3IHwgMDtcbiAgIHg4ID0gIHg4ICsgIGo4IHwgMDtcbiAgIHg5ID0gIHg5ICsgIGo5IHwgMDtcbiAgeDEwID0geDEwICsgajEwIHwgMDtcbiAgeDExID0geDExICsgajExIHwgMDtcbiAgeDEyID0geDEyICsgajEyIHwgMDtcbiAgeDEzID0geDEzICsgajEzIHwgMDtcbiAgeDE0ID0geDE0ICsgajE0IHwgMDtcbiAgeDE1ID0geDE1ICsgajE1IHwgMDtcblxuICBvWyAwXSA9IHgwID4+PiAgMCAmIDB4ZmY7XG4gIG9bIDFdID0geDAgPj4+ICA4ICYgMHhmZjtcbiAgb1sgMl0gPSB4MCA+Pj4gMTYgJiAweGZmO1xuICBvWyAzXSA9IHgwID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1sgNF0gPSB4MSA+Pj4gIDAgJiAweGZmO1xuICBvWyA1XSA9IHgxID4+PiAgOCAmIDB4ZmY7XG4gIG9bIDZdID0geDEgPj4+IDE2ICYgMHhmZjtcbiAgb1sgN10gPSB4MSA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bIDhdID0geDIgPj4+ICAwICYgMHhmZjtcbiAgb1sgOV0gPSB4MiA+Pj4gIDggJiAweGZmO1xuICBvWzEwXSA9IHgyID4+PiAxNiAmIDB4ZmY7XG4gIG9bMTFdID0geDIgPj4+IDI0ICYgMHhmZjtcblxuICBvWzEyXSA9IHgzID4+PiAgMCAmIDB4ZmY7XG4gIG9bMTNdID0geDMgPj4+ICA4ICYgMHhmZjtcbiAgb1sxNF0gPSB4MyA+Pj4gMTYgJiAweGZmO1xuICBvWzE1XSA9IHgzID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1sxNl0gPSB4NCA+Pj4gIDAgJiAweGZmO1xuICBvWzE3XSA9IHg0ID4+PiAgOCAmIDB4ZmY7XG4gIG9bMThdID0geDQgPj4+IDE2ICYgMHhmZjtcbiAgb1sxOV0gPSB4NCA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bMjBdID0geDUgPj4+ICAwICYgMHhmZjtcbiAgb1syMV0gPSB4NSA+Pj4gIDggJiAweGZmO1xuICBvWzIyXSA9IHg1ID4+PiAxNiAmIDB4ZmY7XG4gIG9bMjNdID0geDUgPj4+IDI0ICYgMHhmZjtcblxuICBvWzI0XSA9IHg2ID4+PiAgMCAmIDB4ZmY7XG4gIG9bMjVdID0geDYgPj4+ICA4ICYgMHhmZjtcbiAgb1syNl0gPSB4NiA+Pj4gMTYgJiAweGZmO1xuICBvWzI3XSA9IHg2ID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1syOF0gPSB4NyA+Pj4gIDAgJiAweGZmO1xuICBvWzI5XSA9IHg3ID4+PiAgOCAmIDB4ZmY7XG4gIG9bMzBdID0geDcgPj4+IDE2ICYgMHhmZjtcbiAgb1szMV0gPSB4NyA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bMzJdID0geDggPj4+ICAwICYgMHhmZjtcbiAgb1szM10gPSB4OCA+Pj4gIDggJiAweGZmO1xuICBvWzM0XSA9IHg4ID4+PiAxNiAmIDB4ZmY7XG4gIG9bMzVdID0geDggPj4+IDI0ICYgMHhmZjtcblxuICBvWzM2XSA9IHg5ID4+PiAgMCAmIDB4ZmY7XG4gIG9bMzddID0geDkgPj4+ICA4ICYgMHhmZjtcbiAgb1szOF0gPSB4OSA+Pj4gMTYgJiAweGZmO1xuICBvWzM5XSA9IHg5ID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1s0MF0gPSB4MTAgPj4+ICAwICYgMHhmZjtcbiAgb1s0MV0gPSB4MTAgPj4+ICA4ICYgMHhmZjtcbiAgb1s0Ml0gPSB4MTAgPj4+IDE2ICYgMHhmZjtcbiAgb1s0M10gPSB4MTAgPj4+IDI0ICYgMHhmZjtcblxuICBvWzQ0XSA9IHgxMSA+Pj4gIDAgJiAweGZmO1xuICBvWzQ1XSA9IHgxMSA+Pj4gIDggJiAweGZmO1xuICBvWzQ2XSA9IHgxMSA+Pj4gMTYgJiAweGZmO1xuICBvWzQ3XSA9IHgxMSA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bNDhdID0geDEyID4+PiAgMCAmIDB4ZmY7XG4gIG9bNDldID0geDEyID4+PiAgOCAmIDB4ZmY7XG4gIG9bNTBdID0geDEyID4+PiAxNiAmIDB4ZmY7XG4gIG9bNTFdID0geDEyID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1s1Ml0gPSB4MTMgPj4+ICAwICYgMHhmZjtcbiAgb1s1M10gPSB4MTMgPj4+ICA4ICYgMHhmZjtcbiAgb1s1NF0gPSB4MTMgPj4+IDE2ICYgMHhmZjtcbiAgb1s1NV0gPSB4MTMgPj4+IDI0ICYgMHhmZjtcblxuICBvWzU2XSA9IHgxNCA+Pj4gIDAgJiAweGZmO1xuICBvWzU3XSA9IHgxNCA+Pj4gIDggJiAweGZmO1xuICBvWzU4XSA9IHgxNCA+Pj4gMTYgJiAweGZmO1xuICBvWzU5XSA9IHgxNCA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bNjBdID0geDE1ID4+PiAgMCAmIDB4ZmY7XG4gIG9bNjFdID0geDE1ID4+PiAgOCAmIDB4ZmY7XG4gIG9bNjJdID0geDE1ID4+PiAxNiAmIDB4ZmY7XG4gIG9bNjNdID0geDE1ID4+PiAyNCAmIDB4ZmY7XG59XG5cbmZ1bmN0aW9uIGNvcmVfaHNhbHNhMjAobyxwLGssYykge1xuICB2YXIgajAgID0gY1sgMF0gJiAweGZmIHwgKGNbIDFdICYgMHhmZik8PDggfCAoY1sgMl0gJiAweGZmKTw8MTYgfCAoY1sgM10gJiAweGZmKTw8MjQsXG4gICAgICBqMSAgPSBrWyAwXSAmIDB4ZmYgfCAoa1sgMV0gJiAweGZmKTw8OCB8IChrWyAyXSAmIDB4ZmYpPDwxNiB8IChrWyAzXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoyICA9IGtbIDRdICYgMHhmZiB8IChrWyA1XSAmIDB4ZmYpPDw4IHwgKGtbIDZdICYgMHhmZik8PDE2IHwgKGtbIDddICYgMHhmZik8PDI0LFxuICAgICAgajMgID0ga1sgOF0gJiAweGZmIHwgKGtbIDldICYgMHhmZik8PDggfCAoa1sxMF0gJiAweGZmKTw8MTYgfCAoa1sxMV0gJiAweGZmKTw8MjQsXG4gICAgICBqNCAgPSBrWzEyXSAmIDB4ZmYgfCAoa1sxM10gJiAweGZmKTw8OCB8IChrWzE0XSAmIDB4ZmYpPDwxNiB8IChrWzE1XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGo1ICA9IGNbIDRdICYgMHhmZiB8IChjWyA1XSAmIDB4ZmYpPDw4IHwgKGNbIDZdICYgMHhmZik8PDE2IHwgKGNbIDddICYgMHhmZik8PDI0LFxuICAgICAgajYgID0gcFsgMF0gJiAweGZmIHwgKHBbIDFdICYgMHhmZik8PDggfCAocFsgMl0gJiAweGZmKTw8MTYgfCAocFsgM10gJiAweGZmKTw8MjQsXG4gICAgICBqNyAgPSBwWyA0XSAmIDB4ZmYgfCAocFsgNV0gJiAweGZmKTw8OCB8IChwWyA2XSAmIDB4ZmYpPDwxNiB8IChwWyA3XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGo4ICA9IHBbIDhdICYgMHhmZiB8IChwWyA5XSAmIDB4ZmYpPDw4IHwgKHBbMTBdICYgMHhmZik8PDE2IHwgKHBbMTFdICYgMHhmZik8PDI0LFxuICAgICAgajkgID0gcFsxMl0gJiAweGZmIHwgKHBbMTNdICYgMHhmZik8PDggfCAocFsxNF0gJiAweGZmKTw8MTYgfCAocFsxNV0gJiAweGZmKTw8MjQsXG4gICAgICBqMTAgPSBjWyA4XSAmIDB4ZmYgfCAoY1sgOV0gJiAweGZmKTw8OCB8IChjWzEwXSAmIDB4ZmYpPDwxNiB8IChjWzExXSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxMSA9IGtbMTZdICYgMHhmZiB8IChrWzE3XSAmIDB4ZmYpPDw4IHwgKGtbMThdICYgMHhmZik8PDE2IHwgKGtbMTldICYgMHhmZik8PDI0LFxuICAgICAgajEyID0ga1syMF0gJiAweGZmIHwgKGtbMjFdICYgMHhmZik8PDggfCAoa1syMl0gJiAweGZmKTw8MTYgfCAoa1syM10gJiAweGZmKTw8MjQsXG4gICAgICBqMTMgPSBrWzI0XSAmIDB4ZmYgfCAoa1syNV0gJiAweGZmKTw8OCB8IChrWzI2XSAmIDB4ZmYpPDwxNiB8IChrWzI3XSAmIDB4ZmYpPDwyNCxcbiAgICAgIGoxNCA9IGtbMjhdICYgMHhmZiB8IChrWzI5XSAmIDB4ZmYpPDw4IHwgKGtbMzBdICYgMHhmZik8PDE2IHwgKGtbMzFdICYgMHhmZik8PDI0LFxuICAgICAgajE1ID0gY1sxMl0gJiAweGZmIHwgKGNbMTNdICYgMHhmZik8PDggfCAoY1sxNF0gJiAweGZmKTw8MTYgfCAoY1sxNV0gJiAweGZmKTw8MjQ7XG5cbiAgdmFyIHgwID0gajAsIHgxID0gajEsIHgyID0gajIsIHgzID0gajMsIHg0ID0gajQsIHg1ID0gajUsIHg2ID0gajYsIHg3ID0gajcsXG4gICAgICB4OCA9IGo4LCB4OSA9IGo5LCB4MTAgPSBqMTAsIHgxMSA9IGoxMSwgeDEyID0gajEyLCB4MTMgPSBqMTMsIHgxNCA9IGoxNCxcbiAgICAgIHgxNSA9IGoxNSwgdTtcblxuICBmb3IgKHZhciBpID0gMDsgaSA8IDIwOyBpICs9IDIpIHtcbiAgICB1ID0geDAgKyB4MTIgfCAwO1xuICAgIHg0IF49IHU8PDcgfCB1Pj4+KDMyLTcpO1xuICAgIHUgPSB4NCArIHgwIHwgMDtcbiAgICB4OCBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDggKyB4NCB8IDA7XG4gICAgeDEyIF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHgxMiArIHg4IHwgMDtcbiAgICB4MCBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHg1ICsgeDEgfCAwO1xuICAgIHg5IF49IHU8PDcgfCB1Pj4+KDMyLTcpO1xuICAgIHUgPSB4OSArIHg1IHwgMDtcbiAgICB4MTMgXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHgxMyArIHg5IHwgMDtcbiAgICB4MSBePSB1PDwxMyB8IHU+Pj4oMzItMTMpO1xuICAgIHUgPSB4MSArIHgxMyB8IDA7XG4gICAgeDUgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4MTAgKyB4NiB8IDA7XG4gICAgeDE0IF49IHU8PDcgfCB1Pj4+KDMyLTcpO1xuICAgIHUgPSB4MTQgKyB4MTAgfCAwO1xuICAgIHgyIF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4MiArIHgxNCB8IDA7XG4gICAgeDYgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDYgKyB4MiB8IDA7XG4gICAgeDEwIF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDE1ICsgeDExIHwgMDtcbiAgICB4MyBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDMgKyB4MTUgfCAwO1xuICAgIHg3IF49IHU8PDkgfCB1Pj4+KDMyLTkpO1xuICAgIHUgPSB4NyArIHgzIHwgMDtcbiAgICB4MTEgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDExICsgeDcgfCAwO1xuICAgIHgxNSBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHgwICsgeDMgfCAwO1xuICAgIHgxIF49IHU8PDcgfCB1Pj4+KDMyLTcpO1xuICAgIHUgPSB4MSArIHgwIHwgMDtcbiAgICB4MiBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDIgKyB4MSB8IDA7XG4gICAgeDMgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDMgKyB4MiB8IDA7XG4gICAgeDAgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcblxuICAgIHUgPSB4NSArIHg0IHwgMDtcbiAgICB4NiBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDYgKyB4NSB8IDA7XG4gICAgeDcgXj0gdTw8OSB8IHU+Pj4oMzItOSk7XG4gICAgdSA9IHg3ICsgeDYgfCAwO1xuICAgIHg0IF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHg0ICsgeDcgfCAwO1xuICAgIHg1IF49IHU8PDE4IHwgdT4+PigzMi0xOCk7XG5cbiAgICB1ID0geDEwICsgeDkgfCAwO1xuICAgIHgxMSBePSB1PDw3IHwgdT4+PigzMi03KTtcbiAgICB1ID0geDExICsgeDEwIHwgMDtcbiAgICB4OCBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDggKyB4MTEgfCAwO1xuICAgIHg5IF49IHU8PDEzIHwgdT4+PigzMi0xMyk7XG4gICAgdSA9IHg5ICsgeDggfCAwO1xuICAgIHgxMCBePSB1PDwxOCB8IHU+Pj4oMzItMTgpO1xuXG4gICAgdSA9IHgxNSArIHgxNCB8IDA7XG4gICAgeDEyIF49IHU8PDcgfCB1Pj4+KDMyLTcpO1xuICAgIHUgPSB4MTIgKyB4MTUgfCAwO1xuICAgIHgxMyBePSB1PDw5IHwgdT4+PigzMi05KTtcbiAgICB1ID0geDEzICsgeDEyIHwgMDtcbiAgICB4MTQgXj0gdTw8MTMgfCB1Pj4+KDMyLTEzKTtcbiAgICB1ID0geDE0ICsgeDEzIHwgMDtcbiAgICB4MTUgXj0gdTw8MTggfCB1Pj4+KDMyLTE4KTtcbiAgfVxuXG4gIG9bIDBdID0geDAgPj4+ICAwICYgMHhmZjtcbiAgb1sgMV0gPSB4MCA+Pj4gIDggJiAweGZmO1xuICBvWyAyXSA9IHgwID4+PiAxNiAmIDB4ZmY7XG4gIG9bIDNdID0geDAgPj4+IDI0ICYgMHhmZjtcblxuICBvWyA0XSA9IHg1ID4+PiAgMCAmIDB4ZmY7XG4gIG9bIDVdID0geDUgPj4+ICA4ICYgMHhmZjtcbiAgb1sgNl0gPSB4NSA+Pj4gMTYgJiAweGZmO1xuICBvWyA3XSA9IHg1ID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1sgOF0gPSB4MTAgPj4+ICAwICYgMHhmZjtcbiAgb1sgOV0gPSB4MTAgPj4+ICA4ICYgMHhmZjtcbiAgb1sxMF0gPSB4MTAgPj4+IDE2ICYgMHhmZjtcbiAgb1sxMV0gPSB4MTAgPj4+IDI0ICYgMHhmZjtcblxuICBvWzEyXSA9IHgxNSA+Pj4gIDAgJiAweGZmO1xuICBvWzEzXSA9IHgxNSA+Pj4gIDggJiAweGZmO1xuICBvWzE0XSA9IHgxNSA+Pj4gMTYgJiAweGZmO1xuICBvWzE1XSA9IHgxNSA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bMTZdID0geDYgPj4+ICAwICYgMHhmZjtcbiAgb1sxN10gPSB4NiA+Pj4gIDggJiAweGZmO1xuICBvWzE4XSA9IHg2ID4+PiAxNiAmIDB4ZmY7XG4gIG9bMTldID0geDYgPj4+IDI0ICYgMHhmZjtcblxuICBvWzIwXSA9IHg3ID4+PiAgMCAmIDB4ZmY7XG4gIG9bMjFdID0geDcgPj4+ICA4ICYgMHhmZjtcbiAgb1syMl0gPSB4NyA+Pj4gMTYgJiAweGZmO1xuICBvWzIzXSA9IHg3ID4+PiAyNCAmIDB4ZmY7XG5cbiAgb1syNF0gPSB4OCA+Pj4gIDAgJiAweGZmO1xuICBvWzI1XSA9IHg4ID4+PiAgOCAmIDB4ZmY7XG4gIG9bMjZdID0geDggPj4+IDE2ICYgMHhmZjtcbiAgb1syN10gPSB4OCA+Pj4gMjQgJiAweGZmO1xuXG4gIG9bMjhdID0geDkgPj4+ICAwICYgMHhmZjtcbiAgb1syOV0gPSB4OSA+Pj4gIDggJiAweGZmO1xuICBvWzMwXSA9IHg5ID4+PiAxNiAmIDB4ZmY7XG4gIG9bMzFdID0geDkgPj4+IDI0ICYgMHhmZjtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX2NvcmVfc2Fsc2EyMChvdXQsaW5wLGssYykge1xuICBjb3JlX3NhbHNhMjAob3V0LGlucCxrLGMpO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fY29yZV9oc2Fsc2EyMChvdXQsaW5wLGssYykge1xuICBjb3JlX2hzYWxzYTIwKG91dCxpbnAsayxjKTtcbn1cblxudmFyIHNpZ21hID0gbmV3IFVpbnQ4QXJyYXkoWzEwMSwgMTIwLCAxMTIsIDk3LCAxMTAsIDEwMCwgMzIsIDUxLCA1MCwgNDUsIDk4LCAxMjEsIDExNiwgMTAxLCAzMiwgMTA3XSk7XG4gICAgICAgICAgICAvLyBcImV4cGFuZCAzMi1ieXRlIGtcIlxuXG5mdW5jdGlvbiBjcnlwdG9fc3RyZWFtX3NhbHNhMjBfeG9yKGMsY3BvcyxtLG1wb3MsYixuLGspIHtcbiAgdmFyIHogPSBuZXcgVWludDhBcnJheSgxNiksIHggPSBuZXcgVWludDhBcnJheSg2NCk7XG4gIHZhciB1LCBpO1xuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykgeltpXSA9IDA7XG4gIGZvciAoaSA9IDA7IGkgPCA4OyBpKyspIHpbaV0gPSBuW2ldO1xuICB3aGlsZSAoYiA+PSA2NCkge1xuICAgIGNyeXB0b19jb3JlX3NhbHNhMjAoeCx6LGssc2lnbWEpO1xuICAgIGZvciAoaSA9IDA7IGkgPCA2NDsgaSsrKSBjW2Nwb3MraV0gPSBtW21wb3MraV0gXiB4W2ldO1xuICAgIHUgPSAxO1xuICAgIGZvciAoaSA9IDg7IGkgPCAxNjsgaSsrKSB7XG4gICAgICB1ID0gdSArICh6W2ldICYgMHhmZikgfCAwO1xuICAgICAgeltpXSA9IHUgJiAweGZmO1xuICAgICAgdSA+Pj49IDg7XG4gICAgfVxuICAgIGIgLT0gNjQ7XG4gICAgY3BvcyArPSA2NDtcbiAgICBtcG9zICs9IDY0O1xuICB9XG4gIGlmIChiID4gMCkge1xuICAgIGNyeXB0b19jb3JlX3NhbHNhMjAoeCx6LGssc2lnbWEpO1xuICAgIGZvciAoaSA9IDA7IGkgPCBiOyBpKyspIGNbY3BvcytpXSA9IG1bbXBvcytpXSBeIHhbaV07XG4gIH1cbiAgcmV0dXJuIDA7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19zdHJlYW1fc2Fsc2EyMChjLGNwb3MsYixuLGspIHtcbiAgdmFyIHogPSBuZXcgVWludDhBcnJheSgxNiksIHggPSBuZXcgVWludDhBcnJheSg2NCk7XG4gIHZhciB1LCBpO1xuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykgeltpXSA9IDA7XG4gIGZvciAoaSA9IDA7IGkgPCA4OyBpKyspIHpbaV0gPSBuW2ldO1xuICB3aGlsZSAoYiA+PSA2NCkge1xuICAgIGNyeXB0b19jb3JlX3NhbHNhMjAoeCx6LGssc2lnbWEpO1xuICAgIGZvciAoaSA9IDA7IGkgPCA2NDsgaSsrKSBjW2Nwb3MraV0gPSB4W2ldO1xuICAgIHUgPSAxO1xuICAgIGZvciAoaSA9IDg7IGkgPCAxNjsgaSsrKSB7XG4gICAgICB1ID0gdSArICh6W2ldICYgMHhmZikgfCAwO1xuICAgICAgeltpXSA9IHUgJiAweGZmO1xuICAgICAgdSA+Pj49IDg7XG4gICAgfVxuICAgIGIgLT0gNjQ7XG4gICAgY3BvcyArPSA2NDtcbiAgfVxuICBpZiAoYiA+IDApIHtcbiAgICBjcnlwdG9fY29yZV9zYWxzYTIwKHgseixrLHNpZ21hKTtcbiAgICBmb3IgKGkgPSAwOyBpIDwgYjsgaSsrKSBjW2Nwb3MraV0gPSB4W2ldO1xuICB9XG4gIHJldHVybiAwO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fc3RyZWFtKGMsY3BvcyxkLG4saykge1xuICB2YXIgcyA9IG5ldyBVaW50OEFycmF5KDMyKTtcbiAgY3J5cHRvX2NvcmVfaHNhbHNhMjAocyxuLGssc2lnbWEpO1xuICB2YXIgc24gPSBuZXcgVWludDhBcnJheSg4KTtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCA4OyBpKyspIHNuW2ldID0gbltpKzE2XTtcbiAgcmV0dXJuIGNyeXB0b19zdHJlYW1fc2Fsc2EyMChjLGNwb3MsZCxzbixzKTtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX3N0cmVhbV94b3IoYyxjcG9zLG0sbXBvcyxkLG4saykge1xuICB2YXIgcyA9IG5ldyBVaW50OEFycmF5KDMyKTtcbiAgY3J5cHRvX2NvcmVfaHNhbHNhMjAocyxuLGssc2lnbWEpO1xuICB2YXIgc24gPSBuZXcgVWludDhBcnJheSg4KTtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCA4OyBpKyspIHNuW2ldID0gbltpKzE2XTtcbiAgcmV0dXJuIGNyeXB0b19zdHJlYW1fc2Fsc2EyMF94b3IoYyxjcG9zLG0sbXBvcyxkLHNuLHMpO1xufVxuXG4vKlxuKiBQb3J0IG9mIEFuZHJldyBNb29uJ3MgUG9seTEzMDUtZG9ubmEtMTYuIFB1YmxpYyBkb21haW4uXG4qIGh0dHBzOi8vZ2l0aHViLmNvbS9mbG9vZHliZXJyeS9wb2x5MTMwNS1kb25uYVxuKi9cblxudmFyIHBvbHkxMzA1ID0gZnVuY3Rpb24oa2V5KSB7XG4gIHRoaXMuYnVmZmVyID0gbmV3IFVpbnQ4QXJyYXkoMTYpO1xuICB0aGlzLnIgPSBuZXcgVWludDE2QXJyYXkoMTApO1xuICB0aGlzLmggPSBuZXcgVWludDE2QXJyYXkoMTApO1xuICB0aGlzLnBhZCA9IG5ldyBVaW50MTZBcnJheSg4KTtcbiAgdGhpcy5sZWZ0b3ZlciA9IDA7XG4gIHRoaXMuZmluID0gMDtcblxuICB2YXIgdDAsIHQxLCB0MiwgdDMsIHQ0LCB0NSwgdDYsIHQ3O1xuXG4gIHQwID0ga2V5WyAwXSAmIDB4ZmYgfCAoa2V5WyAxXSAmIDB4ZmYpIDw8IDg7IHRoaXMuclswXSA9ICggdDAgICAgICAgICAgICAgICAgICAgICApICYgMHgxZmZmO1xuICB0MSA9IGtleVsgMl0gJiAweGZmIHwgKGtleVsgM10gJiAweGZmKSA8PCA4OyB0aGlzLnJbMV0gPSAoKHQwID4+PiAxMykgfCAodDEgPDwgIDMpKSAmIDB4MWZmZjtcbiAgdDIgPSBrZXlbIDRdICYgMHhmZiB8IChrZXlbIDVdICYgMHhmZikgPDwgODsgdGhpcy5yWzJdID0gKCh0MSA+Pj4gMTApIHwgKHQyIDw8ICA2KSkgJiAweDFmMDM7XG4gIHQzID0ga2V5WyA2XSAmIDB4ZmYgfCAoa2V5WyA3XSAmIDB4ZmYpIDw8IDg7IHRoaXMuclszXSA9ICgodDIgPj4+ICA3KSB8ICh0MyA8PCAgOSkpICYgMHgxZmZmO1xuICB0NCA9IGtleVsgOF0gJiAweGZmIHwgKGtleVsgOV0gJiAweGZmKSA8PCA4OyB0aGlzLnJbNF0gPSAoKHQzID4+PiAgNCkgfCAodDQgPDwgMTIpKSAmIDB4MDBmZjtcbiAgdGhpcy5yWzVdID0gKCh0NCA+Pj4gIDEpKSAmIDB4MWZmZTtcbiAgdDUgPSBrZXlbMTBdICYgMHhmZiB8IChrZXlbMTFdICYgMHhmZikgPDwgODsgdGhpcy5yWzZdID0gKCh0NCA+Pj4gMTQpIHwgKHQ1IDw8ICAyKSkgJiAweDFmZmY7XG4gIHQ2ID0ga2V5WzEyXSAmIDB4ZmYgfCAoa2V5WzEzXSAmIDB4ZmYpIDw8IDg7IHRoaXMucls3XSA9ICgodDUgPj4+IDExKSB8ICh0NiA8PCAgNSkpICYgMHgxZjgxO1xuICB0NyA9IGtleVsxNF0gJiAweGZmIHwgKGtleVsxNV0gJiAweGZmKSA8PCA4OyB0aGlzLnJbOF0gPSAoKHQ2ID4+PiAgOCkgfCAodDcgPDwgIDgpKSAmIDB4MWZmZjtcbiAgdGhpcy5yWzldID0gKCh0NyA+Pj4gIDUpKSAmIDB4MDA3ZjtcblxuICB0aGlzLnBhZFswXSA9IGtleVsxNl0gJiAweGZmIHwgKGtleVsxN10gJiAweGZmKSA8PCA4O1xuICB0aGlzLnBhZFsxXSA9IGtleVsxOF0gJiAweGZmIHwgKGtleVsxOV0gJiAweGZmKSA8PCA4O1xuICB0aGlzLnBhZFsyXSA9IGtleVsyMF0gJiAweGZmIHwgKGtleVsyMV0gJiAweGZmKSA8PCA4O1xuICB0aGlzLnBhZFszXSA9IGtleVsyMl0gJiAweGZmIHwgKGtleVsyM10gJiAweGZmKSA8PCA4O1xuICB0aGlzLnBhZFs0XSA9IGtleVsyNF0gJiAweGZmIHwgKGtleVsyNV0gJiAweGZmKSA8PCA4O1xuICB0aGlzLnBhZFs1XSA9IGtleVsyNl0gJiAweGZmIHwgKGtleVsyN10gJiAweGZmKSA8PCA4O1xuICB0aGlzLnBhZFs2XSA9IGtleVsyOF0gJiAweGZmIHwgKGtleVsyOV0gJiAweGZmKSA8PCA4O1xuICB0aGlzLnBhZFs3XSA9IGtleVszMF0gJiAweGZmIHwgKGtleVszMV0gJiAweGZmKSA8PCA4O1xufTtcblxucG9seTEzMDUucHJvdG90eXBlLmJsb2NrcyA9IGZ1bmN0aW9uKG0sIG1wb3MsIGJ5dGVzKSB7XG4gIHZhciBoaWJpdCA9IHRoaXMuZmluID8gMCA6ICgxIDw8IDExKTtcbiAgdmFyIHQwLCB0MSwgdDIsIHQzLCB0NCwgdDUsIHQ2LCB0NywgYztcbiAgdmFyIGQwLCBkMSwgZDIsIGQzLCBkNCwgZDUsIGQ2LCBkNywgZDgsIGQ5O1xuXG4gIHZhciBoMCA9IHRoaXMuaFswXSxcbiAgICAgIGgxID0gdGhpcy5oWzFdLFxuICAgICAgaDIgPSB0aGlzLmhbMl0sXG4gICAgICBoMyA9IHRoaXMuaFszXSxcbiAgICAgIGg0ID0gdGhpcy5oWzRdLFxuICAgICAgaDUgPSB0aGlzLmhbNV0sXG4gICAgICBoNiA9IHRoaXMuaFs2XSxcbiAgICAgIGg3ID0gdGhpcy5oWzddLFxuICAgICAgaDggPSB0aGlzLmhbOF0sXG4gICAgICBoOSA9IHRoaXMuaFs5XTtcblxuICB2YXIgcjAgPSB0aGlzLnJbMF0sXG4gICAgICByMSA9IHRoaXMuclsxXSxcbiAgICAgIHIyID0gdGhpcy5yWzJdLFxuICAgICAgcjMgPSB0aGlzLnJbM10sXG4gICAgICByNCA9IHRoaXMucls0XSxcbiAgICAgIHI1ID0gdGhpcy5yWzVdLFxuICAgICAgcjYgPSB0aGlzLnJbNl0sXG4gICAgICByNyA9IHRoaXMucls3XSxcbiAgICAgIHI4ID0gdGhpcy5yWzhdLFxuICAgICAgcjkgPSB0aGlzLnJbOV07XG5cbiAgd2hpbGUgKGJ5dGVzID49IDE2KSB7XG4gICAgdDAgPSBtW21wb3MrIDBdICYgMHhmZiB8IChtW21wb3MrIDFdICYgMHhmZikgPDwgODsgaDAgKz0gKCB0MCAgICAgICAgICAgICAgICAgICAgICkgJiAweDFmZmY7XG4gICAgdDEgPSBtW21wb3MrIDJdICYgMHhmZiB8IChtW21wb3MrIDNdICYgMHhmZikgPDwgODsgaDEgKz0gKCh0MCA+Pj4gMTMpIHwgKHQxIDw8ICAzKSkgJiAweDFmZmY7XG4gICAgdDIgPSBtW21wb3MrIDRdICYgMHhmZiB8IChtW21wb3MrIDVdICYgMHhmZikgPDwgODsgaDIgKz0gKCh0MSA+Pj4gMTApIHwgKHQyIDw8ICA2KSkgJiAweDFmZmY7XG4gICAgdDMgPSBtW21wb3MrIDZdICYgMHhmZiB8IChtW21wb3MrIDddICYgMHhmZikgPDwgODsgaDMgKz0gKCh0MiA+Pj4gIDcpIHwgKHQzIDw8ICA5KSkgJiAweDFmZmY7XG4gICAgdDQgPSBtW21wb3MrIDhdICYgMHhmZiB8IChtW21wb3MrIDldICYgMHhmZikgPDwgODsgaDQgKz0gKCh0MyA+Pj4gIDQpIHwgKHQ0IDw8IDEyKSkgJiAweDFmZmY7XG4gICAgaDUgKz0gKCh0NCA+Pj4gIDEpKSAmIDB4MWZmZjtcbiAgICB0NSA9IG1bbXBvcysxMF0gJiAweGZmIHwgKG1bbXBvcysxMV0gJiAweGZmKSA8PCA4OyBoNiArPSAoKHQ0ID4+PiAxNCkgfCAodDUgPDwgIDIpKSAmIDB4MWZmZjtcbiAgICB0NiA9IG1bbXBvcysxMl0gJiAweGZmIHwgKG1bbXBvcysxM10gJiAweGZmKSA8PCA4OyBoNyArPSAoKHQ1ID4+PiAxMSkgfCAodDYgPDwgIDUpKSAmIDB4MWZmZjtcbiAgICB0NyA9IG1bbXBvcysxNF0gJiAweGZmIHwgKG1bbXBvcysxNV0gJiAweGZmKSA8PCA4OyBoOCArPSAoKHQ2ID4+PiAgOCkgfCAodDcgPDwgIDgpKSAmIDB4MWZmZjtcbiAgICBoOSArPSAoKHQ3ID4+PiA1KSkgfCBoaWJpdDtcblxuICAgIGMgPSAwO1xuXG4gICAgZDAgPSBjO1xuICAgIGQwICs9IGgwICogcjA7XG4gICAgZDAgKz0gaDEgKiAoNSAqIHI5KTtcbiAgICBkMCArPSBoMiAqICg1ICogcjgpO1xuICAgIGQwICs9IGgzICogKDUgKiByNyk7XG4gICAgZDAgKz0gaDQgKiAoNSAqIHI2KTtcbiAgICBjID0gKGQwID4+PiAxMyk7IGQwICY9IDB4MWZmZjtcbiAgICBkMCArPSBoNSAqICg1ICogcjUpO1xuICAgIGQwICs9IGg2ICogKDUgKiByNCk7XG4gICAgZDAgKz0gaDcgKiAoNSAqIHIzKTtcbiAgICBkMCArPSBoOCAqICg1ICogcjIpO1xuICAgIGQwICs9IGg5ICogKDUgKiByMSk7XG4gICAgYyArPSAoZDAgPj4+IDEzKTsgZDAgJj0gMHgxZmZmO1xuXG4gICAgZDEgPSBjO1xuICAgIGQxICs9IGgwICogcjE7XG4gICAgZDEgKz0gaDEgKiByMDtcbiAgICBkMSArPSBoMiAqICg1ICogcjkpO1xuICAgIGQxICs9IGgzICogKDUgKiByOCk7XG4gICAgZDEgKz0gaDQgKiAoNSAqIHI3KTtcbiAgICBjID0gKGQxID4+PiAxMyk7IGQxICY9IDB4MWZmZjtcbiAgICBkMSArPSBoNSAqICg1ICogcjYpO1xuICAgIGQxICs9IGg2ICogKDUgKiByNSk7XG4gICAgZDEgKz0gaDcgKiAoNSAqIHI0KTtcbiAgICBkMSArPSBoOCAqICg1ICogcjMpO1xuICAgIGQxICs9IGg5ICogKDUgKiByMik7XG4gICAgYyArPSAoZDEgPj4+IDEzKTsgZDEgJj0gMHgxZmZmO1xuXG4gICAgZDIgPSBjO1xuICAgIGQyICs9IGgwICogcjI7XG4gICAgZDIgKz0gaDEgKiByMTtcbiAgICBkMiArPSBoMiAqIHIwO1xuICAgIGQyICs9IGgzICogKDUgKiByOSk7XG4gICAgZDIgKz0gaDQgKiAoNSAqIHI4KTtcbiAgICBjID0gKGQyID4+PiAxMyk7IGQyICY9IDB4MWZmZjtcbiAgICBkMiArPSBoNSAqICg1ICogcjcpO1xuICAgIGQyICs9IGg2ICogKDUgKiByNik7XG4gICAgZDIgKz0gaDcgKiAoNSAqIHI1KTtcbiAgICBkMiArPSBoOCAqICg1ICogcjQpO1xuICAgIGQyICs9IGg5ICogKDUgKiByMyk7XG4gICAgYyArPSAoZDIgPj4+IDEzKTsgZDIgJj0gMHgxZmZmO1xuXG4gICAgZDMgPSBjO1xuICAgIGQzICs9IGgwICogcjM7XG4gICAgZDMgKz0gaDEgKiByMjtcbiAgICBkMyArPSBoMiAqIHIxO1xuICAgIGQzICs9IGgzICogcjA7XG4gICAgZDMgKz0gaDQgKiAoNSAqIHI5KTtcbiAgICBjID0gKGQzID4+PiAxMyk7IGQzICY9IDB4MWZmZjtcbiAgICBkMyArPSBoNSAqICg1ICogcjgpO1xuICAgIGQzICs9IGg2ICogKDUgKiByNyk7XG4gICAgZDMgKz0gaDcgKiAoNSAqIHI2KTtcbiAgICBkMyArPSBoOCAqICg1ICogcjUpO1xuICAgIGQzICs9IGg5ICogKDUgKiByNCk7XG4gICAgYyArPSAoZDMgPj4+IDEzKTsgZDMgJj0gMHgxZmZmO1xuXG4gICAgZDQgPSBjO1xuICAgIGQ0ICs9IGgwICogcjQ7XG4gICAgZDQgKz0gaDEgKiByMztcbiAgICBkNCArPSBoMiAqIHIyO1xuICAgIGQ0ICs9IGgzICogcjE7XG4gICAgZDQgKz0gaDQgKiByMDtcbiAgICBjID0gKGQ0ID4+PiAxMyk7IGQ0ICY9IDB4MWZmZjtcbiAgICBkNCArPSBoNSAqICg1ICogcjkpO1xuICAgIGQ0ICs9IGg2ICogKDUgKiByOCk7XG4gICAgZDQgKz0gaDcgKiAoNSAqIHI3KTtcbiAgICBkNCArPSBoOCAqICg1ICogcjYpO1xuICAgIGQ0ICs9IGg5ICogKDUgKiByNSk7XG4gICAgYyArPSAoZDQgPj4+IDEzKTsgZDQgJj0gMHgxZmZmO1xuXG4gICAgZDUgPSBjO1xuICAgIGQ1ICs9IGgwICogcjU7XG4gICAgZDUgKz0gaDEgKiByNDtcbiAgICBkNSArPSBoMiAqIHIzO1xuICAgIGQ1ICs9IGgzICogcjI7XG4gICAgZDUgKz0gaDQgKiByMTtcbiAgICBjID0gKGQ1ID4+PiAxMyk7IGQ1ICY9IDB4MWZmZjtcbiAgICBkNSArPSBoNSAqIHIwO1xuICAgIGQ1ICs9IGg2ICogKDUgKiByOSk7XG4gICAgZDUgKz0gaDcgKiAoNSAqIHI4KTtcbiAgICBkNSArPSBoOCAqICg1ICogcjcpO1xuICAgIGQ1ICs9IGg5ICogKDUgKiByNik7XG4gICAgYyArPSAoZDUgPj4+IDEzKTsgZDUgJj0gMHgxZmZmO1xuXG4gICAgZDYgPSBjO1xuICAgIGQ2ICs9IGgwICogcjY7XG4gICAgZDYgKz0gaDEgKiByNTtcbiAgICBkNiArPSBoMiAqIHI0O1xuICAgIGQ2ICs9IGgzICogcjM7XG4gICAgZDYgKz0gaDQgKiByMjtcbiAgICBjID0gKGQ2ID4+PiAxMyk7IGQ2ICY9IDB4MWZmZjtcbiAgICBkNiArPSBoNSAqIHIxO1xuICAgIGQ2ICs9IGg2ICogcjA7XG4gICAgZDYgKz0gaDcgKiAoNSAqIHI5KTtcbiAgICBkNiArPSBoOCAqICg1ICogcjgpO1xuICAgIGQ2ICs9IGg5ICogKDUgKiByNyk7XG4gICAgYyArPSAoZDYgPj4+IDEzKTsgZDYgJj0gMHgxZmZmO1xuXG4gICAgZDcgPSBjO1xuICAgIGQ3ICs9IGgwICogcjc7XG4gICAgZDcgKz0gaDEgKiByNjtcbiAgICBkNyArPSBoMiAqIHI1O1xuICAgIGQ3ICs9IGgzICogcjQ7XG4gICAgZDcgKz0gaDQgKiByMztcbiAgICBjID0gKGQ3ID4+PiAxMyk7IGQ3ICY9IDB4MWZmZjtcbiAgICBkNyArPSBoNSAqIHIyO1xuICAgIGQ3ICs9IGg2ICogcjE7XG4gICAgZDcgKz0gaDcgKiByMDtcbiAgICBkNyArPSBoOCAqICg1ICogcjkpO1xuICAgIGQ3ICs9IGg5ICogKDUgKiByOCk7XG4gICAgYyArPSAoZDcgPj4+IDEzKTsgZDcgJj0gMHgxZmZmO1xuXG4gICAgZDggPSBjO1xuICAgIGQ4ICs9IGgwICogcjg7XG4gICAgZDggKz0gaDEgKiByNztcbiAgICBkOCArPSBoMiAqIHI2O1xuICAgIGQ4ICs9IGgzICogcjU7XG4gICAgZDggKz0gaDQgKiByNDtcbiAgICBjID0gKGQ4ID4+PiAxMyk7IGQ4ICY9IDB4MWZmZjtcbiAgICBkOCArPSBoNSAqIHIzO1xuICAgIGQ4ICs9IGg2ICogcjI7XG4gICAgZDggKz0gaDcgKiByMTtcbiAgICBkOCArPSBoOCAqIHIwO1xuICAgIGQ4ICs9IGg5ICogKDUgKiByOSk7XG4gICAgYyArPSAoZDggPj4+IDEzKTsgZDggJj0gMHgxZmZmO1xuXG4gICAgZDkgPSBjO1xuICAgIGQ5ICs9IGgwICogcjk7XG4gICAgZDkgKz0gaDEgKiByODtcbiAgICBkOSArPSBoMiAqIHI3O1xuICAgIGQ5ICs9IGgzICogcjY7XG4gICAgZDkgKz0gaDQgKiByNTtcbiAgICBjID0gKGQ5ID4+PiAxMyk7IGQ5ICY9IDB4MWZmZjtcbiAgICBkOSArPSBoNSAqIHI0O1xuICAgIGQ5ICs9IGg2ICogcjM7XG4gICAgZDkgKz0gaDcgKiByMjtcbiAgICBkOSArPSBoOCAqIHIxO1xuICAgIGQ5ICs9IGg5ICogcjA7XG4gICAgYyArPSAoZDkgPj4+IDEzKTsgZDkgJj0gMHgxZmZmO1xuXG4gICAgYyA9ICgoKGMgPDwgMikgKyBjKSkgfCAwO1xuICAgIGMgPSAoYyArIGQwKSB8IDA7XG4gICAgZDAgPSBjICYgMHgxZmZmO1xuICAgIGMgPSAoYyA+Pj4gMTMpO1xuICAgIGQxICs9IGM7XG5cbiAgICBoMCA9IGQwO1xuICAgIGgxID0gZDE7XG4gICAgaDIgPSBkMjtcbiAgICBoMyA9IGQzO1xuICAgIGg0ID0gZDQ7XG4gICAgaDUgPSBkNTtcbiAgICBoNiA9IGQ2O1xuICAgIGg3ID0gZDc7XG4gICAgaDggPSBkODtcbiAgICBoOSA9IGQ5O1xuXG4gICAgbXBvcyArPSAxNjtcbiAgICBieXRlcyAtPSAxNjtcbiAgfVxuICB0aGlzLmhbMF0gPSBoMDtcbiAgdGhpcy5oWzFdID0gaDE7XG4gIHRoaXMuaFsyXSA9IGgyO1xuICB0aGlzLmhbM10gPSBoMztcbiAgdGhpcy5oWzRdID0gaDQ7XG4gIHRoaXMuaFs1XSA9IGg1O1xuICB0aGlzLmhbNl0gPSBoNjtcbiAgdGhpcy5oWzddID0gaDc7XG4gIHRoaXMuaFs4XSA9IGg4O1xuICB0aGlzLmhbOV0gPSBoOTtcbn07XG5cbnBvbHkxMzA1LnByb3RvdHlwZS5maW5pc2ggPSBmdW5jdGlvbihtYWMsIG1hY3Bvcykge1xuICB2YXIgZyA9IG5ldyBVaW50MTZBcnJheSgxMCk7XG4gIHZhciBjLCBtYXNrLCBmLCBpO1xuXG4gIGlmICh0aGlzLmxlZnRvdmVyKSB7XG4gICAgaSA9IHRoaXMubGVmdG92ZXI7XG4gICAgdGhpcy5idWZmZXJbaSsrXSA9IDE7XG4gICAgZm9yICg7IGkgPCAxNjsgaSsrKSB0aGlzLmJ1ZmZlcltpXSA9IDA7XG4gICAgdGhpcy5maW4gPSAxO1xuICAgIHRoaXMuYmxvY2tzKHRoaXMuYnVmZmVyLCAwLCAxNik7XG4gIH1cblxuICBjID0gdGhpcy5oWzFdID4+PiAxMztcbiAgdGhpcy5oWzFdICY9IDB4MWZmZjtcbiAgZm9yIChpID0gMjsgaSA8IDEwOyBpKyspIHtcbiAgICB0aGlzLmhbaV0gKz0gYztcbiAgICBjID0gdGhpcy5oW2ldID4+PiAxMztcbiAgICB0aGlzLmhbaV0gJj0gMHgxZmZmO1xuICB9XG4gIHRoaXMuaFswXSArPSAoYyAqIDUpO1xuICBjID0gdGhpcy5oWzBdID4+PiAxMztcbiAgdGhpcy5oWzBdICY9IDB4MWZmZjtcbiAgdGhpcy5oWzFdICs9IGM7XG4gIGMgPSB0aGlzLmhbMV0gPj4+IDEzO1xuICB0aGlzLmhbMV0gJj0gMHgxZmZmO1xuICB0aGlzLmhbMl0gKz0gYztcblxuICBnWzBdID0gdGhpcy5oWzBdICsgNTtcbiAgYyA9IGdbMF0gPj4+IDEzO1xuICBnWzBdICY9IDB4MWZmZjtcbiAgZm9yIChpID0gMTsgaSA8IDEwOyBpKyspIHtcbiAgICBnW2ldID0gdGhpcy5oW2ldICsgYztcbiAgICBjID0gZ1tpXSA+Pj4gMTM7XG4gICAgZ1tpXSAmPSAweDFmZmY7XG4gIH1cbiAgZ1s5XSAtPSAoMSA8PCAxMyk7XG5cbiAgbWFzayA9IChjIF4gMSkgLSAxO1xuICBmb3IgKGkgPSAwOyBpIDwgMTA7IGkrKykgZ1tpXSAmPSBtYXNrO1xuICBtYXNrID0gfm1hc2s7XG4gIGZvciAoaSA9IDA7IGkgPCAxMDsgaSsrKSB0aGlzLmhbaV0gPSAodGhpcy5oW2ldICYgbWFzaykgfCBnW2ldO1xuXG4gIHRoaXMuaFswXSA9ICgodGhpcy5oWzBdICAgICAgICkgfCAodGhpcy5oWzFdIDw8IDEzKSAgICAgICAgICAgICAgICAgICAgKSAmIDB4ZmZmZjtcbiAgdGhpcy5oWzFdID0gKCh0aGlzLmhbMV0gPj4+ICAzKSB8ICh0aGlzLmhbMl0gPDwgMTApICAgICAgICAgICAgICAgICAgICApICYgMHhmZmZmO1xuICB0aGlzLmhbMl0gPSAoKHRoaXMuaFsyXSA+Pj4gIDYpIHwgKHRoaXMuaFszXSA8PCAgNykgICAgICAgICAgICAgICAgICAgICkgJiAweGZmZmY7XG4gIHRoaXMuaFszXSA9ICgodGhpcy5oWzNdID4+PiAgOSkgfCAodGhpcy5oWzRdIDw8ICA0KSAgICAgICAgICAgICAgICAgICAgKSAmIDB4ZmZmZjtcbiAgdGhpcy5oWzRdID0gKCh0aGlzLmhbNF0gPj4+IDEyKSB8ICh0aGlzLmhbNV0gPDwgIDEpIHwgKHRoaXMuaFs2XSA8PCAxNCkpICYgMHhmZmZmO1xuICB0aGlzLmhbNV0gPSAoKHRoaXMuaFs2XSA+Pj4gIDIpIHwgKHRoaXMuaFs3XSA8PCAxMSkgICAgICAgICAgICAgICAgICAgICkgJiAweGZmZmY7XG4gIHRoaXMuaFs2XSA9ICgodGhpcy5oWzddID4+PiAgNSkgfCAodGhpcy5oWzhdIDw8ICA4KSAgICAgICAgICAgICAgICAgICAgKSAmIDB4ZmZmZjtcbiAgdGhpcy5oWzddID0gKCh0aGlzLmhbOF0gPj4+ICA4KSB8ICh0aGlzLmhbOV0gPDwgIDUpICAgICAgICAgICAgICAgICAgICApICYgMHhmZmZmO1xuXG4gIGYgPSB0aGlzLmhbMF0gKyB0aGlzLnBhZFswXTtcbiAgdGhpcy5oWzBdID0gZiAmIDB4ZmZmZjtcbiAgZm9yIChpID0gMTsgaSA8IDg7IGkrKykge1xuICAgIGYgPSAoKCh0aGlzLmhbaV0gKyB0aGlzLnBhZFtpXSkgfCAwKSArIChmID4+PiAxNikpIHwgMDtcbiAgICB0aGlzLmhbaV0gPSBmICYgMHhmZmZmO1xuICB9XG5cbiAgbWFjW21hY3BvcysgMF0gPSAodGhpcy5oWzBdID4+PiAwKSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrIDFdID0gKHRoaXMuaFswXSA+Pj4gOCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKyAyXSA9ICh0aGlzLmhbMV0gPj4+IDApICYgMHhmZjtcbiAgbWFjW21hY3BvcysgM10gPSAodGhpcy5oWzFdID4+PiA4KSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrIDRdID0gKHRoaXMuaFsyXSA+Pj4gMCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKyA1XSA9ICh0aGlzLmhbMl0gPj4+IDgpICYgMHhmZjtcbiAgbWFjW21hY3BvcysgNl0gPSAodGhpcy5oWzNdID4+PiAwKSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrIDddID0gKHRoaXMuaFszXSA+Pj4gOCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKyA4XSA9ICh0aGlzLmhbNF0gPj4+IDApICYgMHhmZjtcbiAgbWFjW21hY3BvcysgOV0gPSAodGhpcy5oWzRdID4+PiA4KSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrMTBdID0gKHRoaXMuaFs1XSA+Pj4gMCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKzExXSA9ICh0aGlzLmhbNV0gPj4+IDgpICYgMHhmZjtcbiAgbWFjW21hY3BvcysxMl0gPSAodGhpcy5oWzZdID4+PiAwKSAmIDB4ZmY7XG4gIG1hY1ttYWNwb3MrMTNdID0gKHRoaXMuaFs2XSA+Pj4gOCkgJiAweGZmO1xuICBtYWNbbWFjcG9zKzE0XSA9ICh0aGlzLmhbN10gPj4+IDApICYgMHhmZjtcbiAgbWFjW21hY3BvcysxNV0gPSAodGhpcy5oWzddID4+PiA4KSAmIDB4ZmY7XG59O1xuXG5wb2x5MTMwNS5wcm90b3R5cGUudXBkYXRlID0gZnVuY3Rpb24obSwgbXBvcywgYnl0ZXMpIHtcbiAgdmFyIGksIHdhbnQ7XG5cbiAgaWYgKHRoaXMubGVmdG92ZXIpIHtcbiAgICB3YW50ID0gKDE2IC0gdGhpcy5sZWZ0b3Zlcik7XG4gICAgaWYgKHdhbnQgPiBieXRlcylcbiAgICAgIHdhbnQgPSBieXRlcztcbiAgICBmb3IgKGkgPSAwOyBpIDwgd2FudDsgaSsrKVxuICAgICAgdGhpcy5idWZmZXJbdGhpcy5sZWZ0b3ZlciArIGldID0gbVttcG9zK2ldO1xuICAgIGJ5dGVzIC09IHdhbnQ7XG4gICAgbXBvcyArPSB3YW50O1xuICAgIHRoaXMubGVmdG92ZXIgKz0gd2FudDtcbiAgICBpZiAodGhpcy5sZWZ0b3ZlciA8IDE2KVxuICAgICAgcmV0dXJuO1xuICAgIHRoaXMuYmxvY2tzKHRoaXMuYnVmZmVyLCAwLCAxNik7XG4gICAgdGhpcy5sZWZ0b3ZlciA9IDA7XG4gIH1cblxuICBpZiAoYnl0ZXMgPj0gMTYpIHtcbiAgICB3YW50ID0gYnl0ZXMgLSAoYnl0ZXMgJSAxNik7XG4gICAgdGhpcy5ibG9ja3MobSwgbXBvcywgd2FudCk7XG4gICAgbXBvcyArPSB3YW50O1xuICAgIGJ5dGVzIC09IHdhbnQ7XG4gIH1cblxuICBpZiAoYnl0ZXMpIHtcbiAgICBmb3IgKGkgPSAwOyBpIDwgYnl0ZXM7IGkrKylcbiAgICAgIHRoaXMuYnVmZmVyW3RoaXMubGVmdG92ZXIgKyBpXSA9IG1bbXBvcytpXTtcbiAgICB0aGlzLmxlZnRvdmVyICs9IGJ5dGVzO1xuICB9XG59O1xuXG5mdW5jdGlvbiBjcnlwdG9fb25ldGltZWF1dGgob3V0LCBvdXRwb3MsIG0sIG1wb3MsIG4sIGspIHtcbiAgdmFyIHMgPSBuZXcgcG9seTEzMDUoayk7XG4gIHMudXBkYXRlKG0sIG1wb3MsIG4pO1xuICBzLmZpbmlzaChvdXQsIG91dHBvcyk7XG4gIHJldHVybiAwO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fb25ldGltZWF1dGhfdmVyaWZ5KGgsIGhwb3MsIG0sIG1wb3MsIG4sIGspIHtcbiAgdmFyIHggPSBuZXcgVWludDhBcnJheSgxNik7XG4gIGNyeXB0b19vbmV0aW1lYXV0aCh4LDAsbSxtcG9zLG4sayk7XG4gIHJldHVybiBjcnlwdG9fdmVyaWZ5XzE2KGgsaHBvcyx4LDApO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fc2VjcmV0Ym94KGMsbSxkLG4saykge1xuICB2YXIgaTtcbiAgaWYgKGQgPCAzMikgcmV0dXJuIC0xO1xuICBjcnlwdG9fc3RyZWFtX3hvcihjLDAsbSwwLGQsbixrKTtcbiAgY3J5cHRvX29uZXRpbWVhdXRoKGMsIDE2LCBjLCAzMiwgZCAtIDMyLCBjKTtcbiAgZm9yIChpID0gMDsgaSA8IDE2OyBpKyspIGNbaV0gPSAwO1xuICByZXR1cm4gMDtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX3NlY3JldGJveF9vcGVuKG0sYyxkLG4saykge1xuICB2YXIgaTtcbiAgdmFyIHggPSBuZXcgVWludDhBcnJheSgzMik7XG4gIGlmIChkIDwgMzIpIHJldHVybiAtMTtcbiAgY3J5cHRvX3N0cmVhbSh4LDAsMzIsbixrKTtcbiAgaWYgKGNyeXB0b19vbmV0aW1lYXV0aF92ZXJpZnkoYywgMTYsYywgMzIsZCAtIDMyLHgpICE9PSAwKSByZXR1cm4gLTE7XG4gIGNyeXB0b19zdHJlYW1feG9yKG0sMCxjLDAsZCxuLGspO1xuICBmb3IgKGkgPSAwOyBpIDwgMzI7IGkrKykgbVtpXSA9IDA7XG4gIHJldHVybiAwO1xufVxuXG5mdW5jdGlvbiBzZXQyNTUxOShyLCBhKSB7XG4gIHZhciBpO1xuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykgcltpXSA9IGFbaV18MDtcbn1cblxuZnVuY3Rpb24gY2FyMjU1MTkobykge1xuICB2YXIgaSwgdiwgYyA9IDE7XG4gIGZvciAoaSA9IDA7IGkgPCAxNjsgaSsrKSB7XG4gICAgdiA9IG9baV0gKyBjICsgNjU1MzU7XG4gICAgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTtcbiAgICBvW2ldID0gdiAtIGMgKiA2NTUzNjtcbiAgfVxuICBvWzBdICs9IGMtMSArIDM3ICogKGMtMSk7XG59XG5cbmZ1bmN0aW9uIHNlbDI1NTE5KHAsIHEsIGIpIHtcbiAgdmFyIHQsIGMgPSB+KGItMSk7XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgMTY7IGkrKykge1xuICAgIHQgPSBjICYgKHBbaV0gXiBxW2ldKTtcbiAgICBwW2ldIF49IHQ7XG4gICAgcVtpXSBePSB0O1xuICB9XG59XG5cbmZ1bmN0aW9uIHBhY2syNTUxOShvLCBuKSB7XG4gIHZhciBpLCBqLCBiO1xuICB2YXIgbSA9IGdmKCksIHQgPSBnZigpO1xuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykgdFtpXSA9IG5baV07XG4gIGNhcjI1NTE5KHQpO1xuICBjYXIyNTUxOSh0KTtcbiAgY2FyMjU1MTkodCk7XG4gIGZvciAoaiA9IDA7IGogPCAyOyBqKyspIHtcbiAgICBtWzBdID0gdFswXSAtIDB4ZmZlZDtcbiAgICBmb3IgKGkgPSAxOyBpIDwgMTU7IGkrKykge1xuICAgICAgbVtpXSA9IHRbaV0gLSAweGZmZmYgLSAoKG1baS0xXT4+MTYpICYgMSk7XG4gICAgICBtW2ktMV0gJj0gMHhmZmZmO1xuICAgIH1cbiAgICBtWzE1XSA9IHRbMTVdIC0gMHg3ZmZmIC0gKChtWzE0XT4+MTYpICYgMSk7XG4gICAgYiA9IChtWzE1XT4+MTYpICYgMTtcbiAgICBtWzE0XSAmPSAweGZmZmY7XG4gICAgc2VsMjU1MTkodCwgbSwgMS1iKTtcbiAgfVxuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykge1xuICAgIG9bMippXSA9IHRbaV0gJiAweGZmO1xuICAgIG9bMippKzFdID0gdFtpXT4+ODtcbiAgfVxufVxuXG5mdW5jdGlvbiBuZXEyNTUxOShhLCBiKSB7XG4gIHZhciBjID0gbmV3IFVpbnQ4QXJyYXkoMzIpLCBkID0gbmV3IFVpbnQ4QXJyYXkoMzIpO1xuICBwYWNrMjU1MTkoYywgYSk7XG4gIHBhY2syNTUxOShkLCBiKTtcbiAgcmV0dXJuIGNyeXB0b192ZXJpZnlfMzIoYywgMCwgZCwgMCk7XG59XG5cbmZ1bmN0aW9uIHBhcjI1NTE5KGEpIHtcbiAgdmFyIGQgPSBuZXcgVWludDhBcnJheSgzMik7XG4gIHBhY2syNTUxOShkLCBhKTtcbiAgcmV0dXJuIGRbMF0gJiAxO1xufVxuXG5mdW5jdGlvbiB1bnBhY2syNTUxOShvLCBuKSB7XG4gIHZhciBpO1xuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykgb1tpXSA9IG5bMippXSArIChuWzIqaSsxXSA8PCA4KTtcbiAgb1sxNV0gJj0gMHg3ZmZmO1xufVxuXG5mdW5jdGlvbiBBKG8sIGEsIGIpIHtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCAxNjsgaSsrKSBvW2ldID0gYVtpXSArIGJbaV07XG59XG5cbmZ1bmN0aW9uIFoobywgYSwgYikge1xuICBmb3IgKHZhciBpID0gMDsgaSA8IDE2OyBpKyspIG9baV0gPSBhW2ldIC0gYltpXTtcbn1cblxuZnVuY3Rpb24gTShvLCBhLCBiKSB7XG4gIHZhciB2LCBjLFxuICAgICB0MCA9IDAsICB0MSA9IDAsICB0MiA9IDAsICB0MyA9IDAsICB0NCA9IDAsICB0NSA9IDAsICB0NiA9IDAsICB0NyA9IDAsXG4gICAgIHQ4ID0gMCwgIHQ5ID0gMCwgdDEwID0gMCwgdDExID0gMCwgdDEyID0gMCwgdDEzID0gMCwgdDE0ID0gMCwgdDE1ID0gMCxcbiAgICB0MTYgPSAwLCB0MTcgPSAwLCB0MTggPSAwLCB0MTkgPSAwLCB0MjAgPSAwLCB0MjEgPSAwLCB0MjIgPSAwLCB0MjMgPSAwLFxuICAgIHQyNCA9IDAsIHQyNSA9IDAsIHQyNiA9IDAsIHQyNyA9IDAsIHQyOCA9IDAsIHQyOSA9IDAsIHQzMCA9IDAsXG4gICAgYjAgPSBiWzBdLFxuICAgIGIxID0gYlsxXSxcbiAgICBiMiA9IGJbMl0sXG4gICAgYjMgPSBiWzNdLFxuICAgIGI0ID0gYls0XSxcbiAgICBiNSA9IGJbNV0sXG4gICAgYjYgPSBiWzZdLFxuICAgIGI3ID0gYls3XSxcbiAgICBiOCA9IGJbOF0sXG4gICAgYjkgPSBiWzldLFxuICAgIGIxMCA9IGJbMTBdLFxuICAgIGIxMSA9IGJbMTFdLFxuICAgIGIxMiA9IGJbMTJdLFxuICAgIGIxMyA9IGJbMTNdLFxuICAgIGIxNCA9IGJbMTRdLFxuICAgIGIxNSA9IGJbMTVdO1xuXG4gIHYgPSBhWzBdO1xuICB0MCArPSB2ICogYjA7XG4gIHQxICs9IHYgKiBiMTtcbiAgdDIgKz0gdiAqIGIyO1xuICB0MyArPSB2ICogYjM7XG4gIHQ0ICs9IHYgKiBiNDtcbiAgdDUgKz0gdiAqIGI1O1xuICB0NiArPSB2ICogYjY7XG4gIHQ3ICs9IHYgKiBiNztcbiAgdDggKz0gdiAqIGI4O1xuICB0OSArPSB2ICogYjk7XG4gIHQxMCArPSB2ICogYjEwO1xuICB0MTEgKz0gdiAqIGIxMTtcbiAgdDEyICs9IHYgKiBiMTI7XG4gIHQxMyArPSB2ICogYjEzO1xuICB0MTQgKz0gdiAqIGIxNDtcbiAgdDE1ICs9IHYgKiBiMTU7XG4gIHYgPSBhWzFdO1xuICB0MSArPSB2ICogYjA7XG4gIHQyICs9IHYgKiBiMTtcbiAgdDMgKz0gdiAqIGIyO1xuICB0NCArPSB2ICogYjM7XG4gIHQ1ICs9IHYgKiBiNDtcbiAgdDYgKz0gdiAqIGI1O1xuICB0NyArPSB2ICogYjY7XG4gIHQ4ICs9IHYgKiBiNztcbiAgdDkgKz0gdiAqIGI4O1xuICB0MTAgKz0gdiAqIGI5O1xuICB0MTEgKz0gdiAqIGIxMDtcbiAgdDEyICs9IHYgKiBiMTE7XG4gIHQxMyArPSB2ICogYjEyO1xuICB0MTQgKz0gdiAqIGIxMztcbiAgdDE1ICs9IHYgKiBiMTQ7XG4gIHQxNiArPSB2ICogYjE1O1xuICB2ID0gYVsyXTtcbiAgdDIgKz0gdiAqIGIwO1xuICB0MyArPSB2ICogYjE7XG4gIHQ0ICs9IHYgKiBiMjtcbiAgdDUgKz0gdiAqIGIzO1xuICB0NiArPSB2ICogYjQ7XG4gIHQ3ICs9IHYgKiBiNTtcbiAgdDggKz0gdiAqIGI2O1xuICB0OSArPSB2ICogYjc7XG4gIHQxMCArPSB2ICogYjg7XG4gIHQxMSArPSB2ICogYjk7XG4gIHQxMiArPSB2ICogYjEwO1xuICB0MTMgKz0gdiAqIGIxMTtcbiAgdDE0ICs9IHYgKiBiMTI7XG4gIHQxNSArPSB2ICogYjEzO1xuICB0MTYgKz0gdiAqIGIxNDtcbiAgdDE3ICs9IHYgKiBiMTU7XG4gIHYgPSBhWzNdO1xuICB0MyArPSB2ICogYjA7XG4gIHQ0ICs9IHYgKiBiMTtcbiAgdDUgKz0gdiAqIGIyO1xuICB0NiArPSB2ICogYjM7XG4gIHQ3ICs9IHYgKiBiNDtcbiAgdDggKz0gdiAqIGI1O1xuICB0OSArPSB2ICogYjY7XG4gIHQxMCArPSB2ICogYjc7XG4gIHQxMSArPSB2ICogYjg7XG4gIHQxMiArPSB2ICogYjk7XG4gIHQxMyArPSB2ICogYjEwO1xuICB0MTQgKz0gdiAqIGIxMTtcbiAgdDE1ICs9IHYgKiBiMTI7XG4gIHQxNiArPSB2ICogYjEzO1xuICB0MTcgKz0gdiAqIGIxNDtcbiAgdDE4ICs9IHYgKiBiMTU7XG4gIHYgPSBhWzRdO1xuICB0NCArPSB2ICogYjA7XG4gIHQ1ICs9IHYgKiBiMTtcbiAgdDYgKz0gdiAqIGIyO1xuICB0NyArPSB2ICogYjM7XG4gIHQ4ICs9IHYgKiBiNDtcbiAgdDkgKz0gdiAqIGI1O1xuICB0MTAgKz0gdiAqIGI2O1xuICB0MTEgKz0gdiAqIGI3O1xuICB0MTIgKz0gdiAqIGI4O1xuICB0MTMgKz0gdiAqIGI5O1xuICB0MTQgKz0gdiAqIGIxMDtcbiAgdDE1ICs9IHYgKiBiMTE7XG4gIHQxNiArPSB2ICogYjEyO1xuICB0MTcgKz0gdiAqIGIxMztcbiAgdDE4ICs9IHYgKiBiMTQ7XG4gIHQxOSArPSB2ICogYjE1O1xuICB2ID0gYVs1XTtcbiAgdDUgKz0gdiAqIGIwO1xuICB0NiArPSB2ICogYjE7XG4gIHQ3ICs9IHYgKiBiMjtcbiAgdDggKz0gdiAqIGIzO1xuICB0OSArPSB2ICogYjQ7XG4gIHQxMCArPSB2ICogYjU7XG4gIHQxMSArPSB2ICogYjY7XG4gIHQxMiArPSB2ICogYjc7XG4gIHQxMyArPSB2ICogYjg7XG4gIHQxNCArPSB2ICogYjk7XG4gIHQxNSArPSB2ICogYjEwO1xuICB0MTYgKz0gdiAqIGIxMTtcbiAgdDE3ICs9IHYgKiBiMTI7XG4gIHQxOCArPSB2ICogYjEzO1xuICB0MTkgKz0gdiAqIGIxNDtcbiAgdDIwICs9IHYgKiBiMTU7XG4gIHYgPSBhWzZdO1xuICB0NiArPSB2ICogYjA7XG4gIHQ3ICs9IHYgKiBiMTtcbiAgdDggKz0gdiAqIGIyO1xuICB0OSArPSB2ICogYjM7XG4gIHQxMCArPSB2ICogYjQ7XG4gIHQxMSArPSB2ICogYjU7XG4gIHQxMiArPSB2ICogYjY7XG4gIHQxMyArPSB2ICogYjc7XG4gIHQxNCArPSB2ICogYjg7XG4gIHQxNSArPSB2ICogYjk7XG4gIHQxNiArPSB2ICogYjEwO1xuICB0MTcgKz0gdiAqIGIxMTtcbiAgdDE4ICs9IHYgKiBiMTI7XG4gIHQxOSArPSB2ICogYjEzO1xuICB0MjAgKz0gdiAqIGIxNDtcbiAgdDIxICs9IHYgKiBiMTU7XG4gIHYgPSBhWzddO1xuICB0NyArPSB2ICogYjA7XG4gIHQ4ICs9IHYgKiBiMTtcbiAgdDkgKz0gdiAqIGIyO1xuICB0MTAgKz0gdiAqIGIzO1xuICB0MTEgKz0gdiAqIGI0O1xuICB0MTIgKz0gdiAqIGI1O1xuICB0MTMgKz0gdiAqIGI2O1xuICB0MTQgKz0gdiAqIGI3O1xuICB0MTUgKz0gdiAqIGI4O1xuICB0MTYgKz0gdiAqIGI5O1xuICB0MTcgKz0gdiAqIGIxMDtcbiAgdDE4ICs9IHYgKiBiMTE7XG4gIHQxOSArPSB2ICogYjEyO1xuICB0MjAgKz0gdiAqIGIxMztcbiAgdDIxICs9IHYgKiBiMTQ7XG4gIHQyMiArPSB2ICogYjE1O1xuICB2ID0gYVs4XTtcbiAgdDggKz0gdiAqIGIwO1xuICB0OSArPSB2ICogYjE7XG4gIHQxMCArPSB2ICogYjI7XG4gIHQxMSArPSB2ICogYjM7XG4gIHQxMiArPSB2ICogYjQ7XG4gIHQxMyArPSB2ICogYjU7XG4gIHQxNCArPSB2ICogYjY7XG4gIHQxNSArPSB2ICogYjc7XG4gIHQxNiArPSB2ICogYjg7XG4gIHQxNyArPSB2ICogYjk7XG4gIHQxOCArPSB2ICogYjEwO1xuICB0MTkgKz0gdiAqIGIxMTtcbiAgdDIwICs9IHYgKiBiMTI7XG4gIHQyMSArPSB2ICogYjEzO1xuICB0MjIgKz0gdiAqIGIxNDtcbiAgdDIzICs9IHYgKiBiMTU7XG4gIHYgPSBhWzldO1xuICB0OSArPSB2ICogYjA7XG4gIHQxMCArPSB2ICogYjE7XG4gIHQxMSArPSB2ICogYjI7XG4gIHQxMiArPSB2ICogYjM7XG4gIHQxMyArPSB2ICogYjQ7XG4gIHQxNCArPSB2ICogYjU7XG4gIHQxNSArPSB2ICogYjY7XG4gIHQxNiArPSB2ICogYjc7XG4gIHQxNyArPSB2ICogYjg7XG4gIHQxOCArPSB2ICogYjk7XG4gIHQxOSArPSB2ICogYjEwO1xuICB0MjAgKz0gdiAqIGIxMTtcbiAgdDIxICs9IHYgKiBiMTI7XG4gIHQyMiArPSB2ICogYjEzO1xuICB0MjMgKz0gdiAqIGIxNDtcbiAgdDI0ICs9IHYgKiBiMTU7XG4gIHYgPSBhWzEwXTtcbiAgdDEwICs9IHYgKiBiMDtcbiAgdDExICs9IHYgKiBiMTtcbiAgdDEyICs9IHYgKiBiMjtcbiAgdDEzICs9IHYgKiBiMztcbiAgdDE0ICs9IHYgKiBiNDtcbiAgdDE1ICs9IHYgKiBiNTtcbiAgdDE2ICs9IHYgKiBiNjtcbiAgdDE3ICs9IHYgKiBiNztcbiAgdDE4ICs9IHYgKiBiODtcbiAgdDE5ICs9IHYgKiBiOTtcbiAgdDIwICs9IHYgKiBiMTA7XG4gIHQyMSArPSB2ICogYjExO1xuICB0MjIgKz0gdiAqIGIxMjtcbiAgdDIzICs9IHYgKiBiMTM7XG4gIHQyNCArPSB2ICogYjE0O1xuICB0MjUgKz0gdiAqIGIxNTtcbiAgdiA9IGFbMTFdO1xuICB0MTEgKz0gdiAqIGIwO1xuICB0MTIgKz0gdiAqIGIxO1xuICB0MTMgKz0gdiAqIGIyO1xuICB0MTQgKz0gdiAqIGIzO1xuICB0MTUgKz0gdiAqIGI0O1xuICB0MTYgKz0gdiAqIGI1O1xuICB0MTcgKz0gdiAqIGI2O1xuICB0MTggKz0gdiAqIGI3O1xuICB0MTkgKz0gdiAqIGI4O1xuICB0MjAgKz0gdiAqIGI5O1xuICB0MjEgKz0gdiAqIGIxMDtcbiAgdDIyICs9IHYgKiBiMTE7XG4gIHQyMyArPSB2ICogYjEyO1xuICB0MjQgKz0gdiAqIGIxMztcbiAgdDI1ICs9IHYgKiBiMTQ7XG4gIHQyNiArPSB2ICogYjE1O1xuICB2ID0gYVsxMl07XG4gIHQxMiArPSB2ICogYjA7XG4gIHQxMyArPSB2ICogYjE7XG4gIHQxNCArPSB2ICogYjI7XG4gIHQxNSArPSB2ICogYjM7XG4gIHQxNiArPSB2ICogYjQ7XG4gIHQxNyArPSB2ICogYjU7XG4gIHQxOCArPSB2ICogYjY7XG4gIHQxOSArPSB2ICogYjc7XG4gIHQyMCArPSB2ICogYjg7XG4gIHQyMSArPSB2ICogYjk7XG4gIHQyMiArPSB2ICogYjEwO1xuICB0MjMgKz0gdiAqIGIxMTtcbiAgdDI0ICs9IHYgKiBiMTI7XG4gIHQyNSArPSB2ICogYjEzO1xuICB0MjYgKz0gdiAqIGIxNDtcbiAgdDI3ICs9IHYgKiBiMTU7XG4gIHYgPSBhWzEzXTtcbiAgdDEzICs9IHYgKiBiMDtcbiAgdDE0ICs9IHYgKiBiMTtcbiAgdDE1ICs9IHYgKiBiMjtcbiAgdDE2ICs9IHYgKiBiMztcbiAgdDE3ICs9IHYgKiBiNDtcbiAgdDE4ICs9IHYgKiBiNTtcbiAgdDE5ICs9IHYgKiBiNjtcbiAgdDIwICs9IHYgKiBiNztcbiAgdDIxICs9IHYgKiBiODtcbiAgdDIyICs9IHYgKiBiOTtcbiAgdDIzICs9IHYgKiBiMTA7XG4gIHQyNCArPSB2ICogYjExO1xuICB0MjUgKz0gdiAqIGIxMjtcbiAgdDI2ICs9IHYgKiBiMTM7XG4gIHQyNyArPSB2ICogYjE0O1xuICB0MjggKz0gdiAqIGIxNTtcbiAgdiA9IGFbMTRdO1xuICB0MTQgKz0gdiAqIGIwO1xuICB0MTUgKz0gdiAqIGIxO1xuICB0MTYgKz0gdiAqIGIyO1xuICB0MTcgKz0gdiAqIGIzO1xuICB0MTggKz0gdiAqIGI0O1xuICB0MTkgKz0gdiAqIGI1O1xuICB0MjAgKz0gdiAqIGI2O1xuICB0MjEgKz0gdiAqIGI3O1xuICB0MjIgKz0gdiAqIGI4O1xuICB0MjMgKz0gdiAqIGI5O1xuICB0MjQgKz0gdiAqIGIxMDtcbiAgdDI1ICs9IHYgKiBiMTE7XG4gIHQyNiArPSB2ICogYjEyO1xuICB0MjcgKz0gdiAqIGIxMztcbiAgdDI4ICs9IHYgKiBiMTQ7XG4gIHQyOSArPSB2ICogYjE1O1xuICB2ID0gYVsxNV07XG4gIHQxNSArPSB2ICogYjA7XG4gIHQxNiArPSB2ICogYjE7XG4gIHQxNyArPSB2ICogYjI7XG4gIHQxOCArPSB2ICogYjM7XG4gIHQxOSArPSB2ICogYjQ7XG4gIHQyMCArPSB2ICogYjU7XG4gIHQyMSArPSB2ICogYjY7XG4gIHQyMiArPSB2ICogYjc7XG4gIHQyMyArPSB2ICogYjg7XG4gIHQyNCArPSB2ICogYjk7XG4gIHQyNSArPSB2ICogYjEwO1xuICB0MjYgKz0gdiAqIGIxMTtcbiAgdDI3ICs9IHYgKiBiMTI7XG4gIHQyOCArPSB2ICogYjEzO1xuICB0MjkgKz0gdiAqIGIxNDtcbiAgdDMwICs9IHYgKiBiMTU7XG5cbiAgdDAgICs9IDM4ICogdDE2O1xuICB0MSAgKz0gMzggKiB0MTc7XG4gIHQyICArPSAzOCAqIHQxODtcbiAgdDMgICs9IDM4ICogdDE5O1xuICB0NCAgKz0gMzggKiB0MjA7XG4gIHQ1ICArPSAzOCAqIHQyMTtcbiAgdDYgICs9IDM4ICogdDIyO1xuICB0NyAgKz0gMzggKiB0MjM7XG4gIHQ4ICArPSAzOCAqIHQyNDtcbiAgdDkgICs9IDM4ICogdDI1O1xuICB0MTAgKz0gMzggKiB0MjY7XG4gIHQxMSArPSAzOCAqIHQyNztcbiAgdDEyICs9IDM4ICogdDI4O1xuICB0MTMgKz0gMzggKiB0Mjk7XG4gIHQxNCArPSAzOCAqIHQzMDtcbiAgLy8gdDE1IGxlZnQgYXMgaXNcblxuICAvLyBmaXJzdCBjYXJcbiAgYyA9IDE7XG4gIHYgPSAgdDAgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0MCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDEgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0MSA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDIgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0MiA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDMgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0MyA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDQgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0NCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDUgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0NSA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDYgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0NiA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDcgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0NyA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDggKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0OCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSAgdDkgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7ICB0OSA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTAgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxMCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTEgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxMSA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTIgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxMiA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTMgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxMyA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTQgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxNCA9IHYgLSBjICogNjU1MzY7XG4gIHYgPSB0MTUgKyBjICsgNjU1MzU7IGMgPSBNYXRoLmZsb29yKHYgLyA2NTUzNik7IHQxNSA9IHYgLSBjICogNjU1MzY7XG4gIHQwICs9IGMtMSArIDM3ICogKGMtMSk7XG5cbiAgLy8gc2Vjb25kIGNhclxuICBjID0gMTtcbiAgdiA9ICB0MCArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQwID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0MSArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQxID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0MiArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQyID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0MyArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQzID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0NCArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQ0ID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0NSArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQ1ID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0NiArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQ2ID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0NyArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQ3ID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0OCArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQ4ID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9ICB0OSArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgIHQ5ID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9IHQxMCArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgdDEwID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9IHQxMSArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgdDExID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9IHQxMiArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgdDEyID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9IHQxMyArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgdDEzID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9IHQxNCArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgdDE0ID0gdiAtIGMgKiA2NTUzNjtcbiAgdiA9IHQxNSArIGMgKyA2NTUzNTsgYyA9IE1hdGguZmxvb3IodiAvIDY1NTM2KTsgdDE1ID0gdiAtIGMgKiA2NTUzNjtcbiAgdDAgKz0gYy0xICsgMzcgKiAoYy0xKTtcblxuICBvWyAwXSA9IHQwO1xuICBvWyAxXSA9IHQxO1xuICBvWyAyXSA9IHQyO1xuICBvWyAzXSA9IHQzO1xuICBvWyA0XSA9IHQ0O1xuICBvWyA1XSA9IHQ1O1xuICBvWyA2XSA9IHQ2O1xuICBvWyA3XSA9IHQ3O1xuICBvWyA4XSA9IHQ4O1xuICBvWyA5XSA9IHQ5O1xuICBvWzEwXSA9IHQxMDtcbiAgb1sxMV0gPSB0MTE7XG4gIG9bMTJdID0gdDEyO1xuICBvWzEzXSA9IHQxMztcbiAgb1sxNF0gPSB0MTQ7XG4gIG9bMTVdID0gdDE1O1xufVxuXG5mdW5jdGlvbiBTKG8sIGEpIHtcbiAgTShvLCBhLCBhKTtcbn1cblxuZnVuY3Rpb24gaW52MjU1MTkobywgaSkge1xuICB2YXIgYyA9IGdmKCk7XG4gIHZhciBhO1xuICBmb3IgKGEgPSAwOyBhIDwgMTY7IGErKykgY1thXSA9IGlbYV07XG4gIGZvciAoYSA9IDI1MzsgYSA+PSAwOyBhLS0pIHtcbiAgICBTKGMsIGMpO1xuICAgIGlmKGEgIT09IDIgJiYgYSAhPT0gNCkgTShjLCBjLCBpKTtcbiAgfVxuICBmb3IgKGEgPSAwOyBhIDwgMTY7IGErKykgb1thXSA9IGNbYV07XG59XG5cbmZ1bmN0aW9uIHBvdzI1MjMobywgaSkge1xuICB2YXIgYyA9IGdmKCk7XG4gIHZhciBhO1xuICBmb3IgKGEgPSAwOyBhIDwgMTY7IGErKykgY1thXSA9IGlbYV07XG4gIGZvciAoYSA9IDI1MDsgYSA+PSAwOyBhLS0pIHtcbiAgICAgIFMoYywgYyk7XG4gICAgICBpZihhICE9PSAxKSBNKGMsIGMsIGkpO1xuICB9XG4gIGZvciAoYSA9IDA7IGEgPCAxNjsgYSsrKSBvW2FdID0gY1thXTtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX3NjYWxhcm11bHQocSwgbiwgcCkge1xuICB2YXIgeiA9IG5ldyBVaW50OEFycmF5KDMyKTtcbiAgdmFyIHggPSBuZXcgRmxvYXQ2NEFycmF5KDgwKSwgciwgaTtcbiAgdmFyIGEgPSBnZigpLCBiID0gZ2YoKSwgYyA9IGdmKCksXG4gICAgICBkID0gZ2YoKSwgZSA9IGdmKCksIGYgPSBnZigpO1xuICBmb3IgKGkgPSAwOyBpIDwgMzE7IGkrKykgeltpXSA9IG5baV07XG4gIHpbMzFdPShuWzMxXSYxMjcpfDY0O1xuICB6WzBdJj0yNDg7XG4gIHVucGFjazI1NTE5KHgscCk7XG4gIGZvciAoaSA9IDA7IGkgPCAxNjsgaSsrKSB7XG4gICAgYltpXT14W2ldO1xuICAgIGRbaV09YVtpXT1jW2ldPTA7XG4gIH1cbiAgYVswXT1kWzBdPTE7XG4gIGZvciAoaT0yNTQ7IGk+PTA7IC0taSkge1xuICAgIHI9KHpbaT4+PjNdPj4+KGkmNykpJjE7XG4gICAgc2VsMjU1MTkoYSxiLHIpO1xuICAgIHNlbDI1NTE5KGMsZCxyKTtcbiAgICBBKGUsYSxjKTtcbiAgICBaKGEsYSxjKTtcbiAgICBBKGMsYixkKTtcbiAgICBaKGIsYixkKTtcbiAgICBTKGQsZSk7XG4gICAgUyhmLGEpO1xuICAgIE0oYSxjLGEpO1xuICAgIE0oYyxiLGUpO1xuICAgIEEoZSxhLGMpO1xuICAgIFooYSxhLGMpO1xuICAgIFMoYixhKTtcbiAgICBaKGMsZCxmKTtcbiAgICBNKGEsYyxfMTIxNjY1KTtcbiAgICBBKGEsYSxkKTtcbiAgICBNKGMsYyxhKTtcbiAgICBNKGEsZCxmKTtcbiAgICBNKGQsYix4KTtcbiAgICBTKGIsZSk7XG4gICAgc2VsMjU1MTkoYSxiLHIpO1xuICAgIHNlbDI1NTE5KGMsZCxyKTtcbiAgfVxuICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykge1xuICAgIHhbaSsxNl09YVtpXTtcbiAgICB4W2krMzJdPWNbaV07XG4gICAgeFtpKzQ4XT1iW2ldO1xuICAgIHhbaSs2NF09ZFtpXTtcbiAgfVxuICB2YXIgeDMyID0geC5zdWJhcnJheSgzMik7XG4gIHZhciB4MTYgPSB4LnN1YmFycmF5KDE2KTtcbiAgaW52MjU1MTkoeDMyLHgzMik7XG4gIE0oeDE2LHgxNix4MzIpO1xuICBwYWNrMjU1MTkocSx4MTYpO1xuICByZXR1cm4gMDtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX3NjYWxhcm11bHRfYmFzZShxLCBuKSB7XG4gIHJldHVybiBjcnlwdG9fc2NhbGFybXVsdChxLCBuLCBfOSk7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19ib3hfa2V5cGFpcih5LCB4KSB7XG4gIHJhbmRvbWJ5dGVzKHgsIDMyKTtcbiAgcmV0dXJuIGNyeXB0b19zY2FsYXJtdWx0X2Jhc2UoeSwgeCk7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19ib3hfYmVmb3Jlbm0oaywgeSwgeCkge1xuICB2YXIgcyA9IG5ldyBVaW50OEFycmF5KDMyKTtcbiAgY3J5cHRvX3NjYWxhcm11bHQocywgeCwgeSk7XG4gIHJldHVybiBjcnlwdG9fY29yZV9oc2Fsc2EyMChrLCBfMCwgcywgc2lnbWEpO1xufVxuXG52YXIgY3J5cHRvX2JveF9hZnRlcm5tID0gY3J5cHRvX3NlY3JldGJveDtcbnZhciBjcnlwdG9fYm94X29wZW5fYWZ0ZXJubSA9IGNyeXB0b19zZWNyZXRib3hfb3BlbjtcblxuZnVuY3Rpb24gY3J5cHRvX2JveChjLCBtLCBkLCBuLCB5LCB4KSB7XG4gIHZhciBrID0gbmV3IFVpbnQ4QXJyYXkoMzIpO1xuICBjcnlwdG9fYm94X2JlZm9yZW5tKGssIHksIHgpO1xuICByZXR1cm4gY3J5cHRvX2JveF9hZnRlcm5tKGMsIG0sIGQsIG4sIGspO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fYm94X29wZW4obSwgYywgZCwgbiwgeSwgeCkge1xuICB2YXIgayA9IG5ldyBVaW50OEFycmF5KDMyKTtcbiAgY3J5cHRvX2JveF9iZWZvcmVubShrLCB5LCB4KTtcbiAgcmV0dXJuIGNyeXB0b19ib3hfb3Blbl9hZnRlcm5tKG0sIGMsIGQsIG4sIGspO1xufVxuXG52YXIgSyA9IFtcbiAgMHg0MjhhMmY5OCwgMHhkNzI4YWUyMiwgMHg3MTM3NDQ5MSwgMHgyM2VmNjVjZCxcbiAgMHhiNWMwZmJjZiwgMHhlYzRkM2IyZiwgMHhlOWI1ZGJhNSwgMHg4MTg5ZGJiYyxcbiAgMHgzOTU2YzI1YiwgMHhmMzQ4YjUzOCwgMHg1OWYxMTFmMSwgMHhiNjA1ZDAxOSxcbiAgMHg5MjNmODJhNCwgMHhhZjE5NGY5YiwgMHhhYjFjNWVkNSwgMHhkYTZkODExOCxcbiAgMHhkODA3YWE5OCwgMHhhMzAzMDI0MiwgMHgxMjgzNWIwMSwgMHg0NTcwNmZiZSxcbiAgMHgyNDMxODViZSwgMHg0ZWU0YjI4YywgMHg1NTBjN2RjMywgMHhkNWZmYjRlMixcbiAgMHg3MmJlNWQ3NCwgMHhmMjdiODk2ZiwgMHg4MGRlYjFmZSwgMHgzYjE2OTZiMSxcbiAgMHg5YmRjMDZhNywgMHgyNWM3MTIzNSwgMHhjMTliZjE3NCwgMHhjZjY5MjY5NCxcbiAgMHhlNDliNjljMSwgMHg5ZWYxNGFkMiwgMHhlZmJlNDc4NiwgMHgzODRmMjVlMyxcbiAgMHgwZmMxOWRjNiwgMHg4YjhjZDViNSwgMHgyNDBjYTFjYywgMHg3N2FjOWM2NSxcbiAgMHgyZGU5MmM2ZiwgMHg1OTJiMDI3NSwgMHg0YTc0ODRhYSwgMHg2ZWE2ZTQ4MyxcbiAgMHg1Y2IwYTlkYywgMHhiZDQxZmJkNCwgMHg3NmY5ODhkYSwgMHg4MzExNTNiNSxcbiAgMHg5ODNlNTE1MiwgMHhlZTY2ZGZhYiwgMHhhODMxYzY2ZCwgMHgyZGI0MzIxMCxcbiAgMHhiMDAzMjdjOCwgMHg5OGZiMjEzZiwgMHhiZjU5N2ZjNywgMHhiZWVmMGVlNCxcbiAgMHhjNmUwMGJmMywgMHgzZGE4OGZjMiwgMHhkNWE3OTE0NywgMHg5MzBhYTcyNSxcbiAgMHgwNmNhNjM1MSwgMHhlMDAzODI2ZiwgMHgxNDI5Mjk2NywgMHgwYTBlNmU3MCxcbiAgMHgyN2I3MGE4NSwgMHg0NmQyMmZmYywgMHgyZTFiMjEzOCwgMHg1YzI2YzkyNixcbiAgMHg0ZDJjNmRmYywgMHg1YWM0MmFlZCwgMHg1MzM4MGQxMywgMHg5ZDk1YjNkZixcbiAgMHg2NTBhNzM1NCwgMHg4YmFmNjNkZSwgMHg3NjZhMGFiYiwgMHgzYzc3YjJhOCxcbiAgMHg4MWMyYzkyZSwgMHg0N2VkYWVlNiwgMHg5MjcyMmM4NSwgMHgxNDgyMzUzYixcbiAgMHhhMmJmZThhMSwgMHg0Y2YxMDM2NCwgMHhhODFhNjY0YiwgMHhiYzQyMzAwMSxcbiAgMHhjMjRiOGI3MCwgMHhkMGY4OTc5MSwgMHhjNzZjNTFhMywgMHgwNjU0YmUzMCxcbiAgMHhkMTkyZTgxOSwgMHhkNmVmNTIxOCwgMHhkNjk5MDYyNCwgMHg1NTY1YTkxMCxcbiAgMHhmNDBlMzU4NSwgMHg1NzcxMjAyYSwgMHgxMDZhYTA3MCwgMHgzMmJiZDFiOCxcbiAgMHgxOWE0YzExNiwgMHhiOGQyZDBjOCwgMHgxZTM3NmMwOCwgMHg1MTQxYWI1MyxcbiAgMHgyNzQ4Nzc0YywgMHhkZjhlZWI5OSwgMHgzNGIwYmNiNSwgMHhlMTliNDhhOCxcbiAgMHgzOTFjMGNiMywgMHhjNWM5NWE2MywgMHg0ZWQ4YWE0YSwgMHhlMzQxOGFjYixcbiAgMHg1YjljY2E0ZiwgMHg3NzYzZTM3MywgMHg2ODJlNmZmMywgMHhkNmIyYjhhMyxcbiAgMHg3NDhmODJlZSwgMHg1ZGVmYjJmYywgMHg3OGE1NjM2ZiwgMHg0MzE3MmY2MCxcbiAgMHg4NGM4NzgxNCwgMHhhMWYwYWI3MiwgMHg4Y2M3MDIwOCwgMHgxYTY0MzllYyxcbiAgMHg5MGJlZmZmYSwgMHgyMzYzMWUyOCwgMHhhNDUwNmNlYiwgMHhkZTgyYmRlOSxcbiAgMHhiZWY5YTNmNywgMHhiMmM2NzkxNSwgMHhjNjcxNzhmMiwgMHhlMzcyNTMyYixcbiAgMHhjYTI3M2VjZSwgMHhlYTI2NjE5YywgMHhkMTg2YjhjNywgMHgyMWMwYzIwNyxcbiAgMHhlYWRhN2RkNiwgMHhjZGUwZWIxZSwgMHhmNTdkNGY3ZiwgMHhlZTZlZDE3OCxcbiAgMHgwNmYwNjdhYSwgMHg3MjE3NmZiYSwgMHgwYTYzN2RjNSwgMHhhMmM4OThhNixcbiAgMHgxMTNmOTgwNCwgMHhiZWY5MGRhZSwgMHgxYjcxMGIzNSwgMHgxMzFjNDcxYixcbiAgMHgyOGRiNzdmNSwgMHgyMzA0N2Q4NCwgMHgzMmNhYWI3YiwgMHg0MGM3MjQ5MyxcbiAgMHgzYzllYmUwYSwgMHgxNWM5YmViYywgMHg0MzFkNjdjNCwgMHg5YzEwMGQ0YyxcbiAgMHg0Y2M1ZDRiZSwgMHhjYjNlNDJiNiwgMHg1OTdmMjk5YywgMHhmYzY1N2UyYSxcbiAgMHg1ZmNiNmZhYiwgMHgzYWQ2ZmFlYywgMHg2YzQ0MTk4YywgMHg0YTQ3NTgxN1xuXTtcblxuZnVuY3Rpb24gY3J5cHRvX2hhc2hibG9ja3NfaGwoaGgsIGhsLCBtLCBuKSB7XG4gIHZhciB3aCA9IG5ldyBJbnQzMkFycmF5KDE2KSwgd2wgPSBuZXcgSW50MzJBcnJheSgxNiksXG4gICAgICBiaDAsIGJoMSwgYmgyLCBiaDMsIGJoNCwgYmg1LCBiaDYsIGJoNyxcbiAgICAgIGJsMCwgYmwxLCBibDIsIGJsMywgYmw0LCBibDUsIGJsNiwgYmw3LFxuICAgICAgdGgsIHRsLCBpLCBqLCBoLCBsLCBhLCBiLCBjLCBkO1xuXG4gIHZhciBhaDAgPSBoaFswXSxcbiAgICAgIGFoMSA9IGhoWzFdLFxuICAgICAgYWgyID0gaGhbMl0sXG4gICAgICBhaDMgPSBoaFszXSxcbiAgICAgIGFoNCA9IGhoWzRdLFxuICAgICAgYWg1ID0gaGhbNV0sXG4gICAgICBhaDYgPSBoaFs2XSxcbiAgICAgIGFoNyA9IGhoWzddLFxuXG4gICAgICBhbDAgPSBobFswXSxcbiAgICAgIGFsMSA9IGhsWzFdLFxuICAgICAgYWwyID0gaGxbMl0sXG4gICAgICBhbDMgPSBobFszXSxcbiAgICAgIGFsNCA9IGhsWzRdLFxuICAgICAgYWw1ID0gaGxbNV0sXG4gICAgICBhbDYgPSBobFs2XSxcbiAgICAgIGFsNyA9IGhsWzddO1xuXG4gIHZhciBwb3MgPSAwO1xuICB3aGlsZSAobiA+PSAxMjgpIHtcbiAgICBmb3IgKGkgPSAwOyBpIDwgMTY7IGkrKykge1xuICAgICAgaiA9IDggKiBpICsgcG9zO1xuICAgICAgd2hbaV0gPSAobVtqKzBdIDw8IDI0KSB8IChtW2orMV0gPDwgMTYpIHwgKG1baisyXSA8PCA4KSB8IG1baiszXTtcbiAgICAgIHdsW2ldID0gKG1bais0XSA8PCAyNCkgfCAobVtqKzVdIDw8IDE2KSB8IChtW2orNl0gPDwgOCkgfCBtW2orN107XG4gICAgfVxuICAgIGZvciAoaSA9IDA7IGkgPCA4MDsgaSsrKSB7XG4gICAgICBiaDAgPSBhaDA7XG4gICAgICBiaDEgPSBhaDE7XG4gICAgICBiaDIgPSBhaDI7XG4gICAgICBiaDMgPSBhaDM7XG4gICAgICBiaDQgPSBhaDQ7XG4gICAgICBiaDUgPSBhaDU7XG4gICAgICBiaDYgPSBhaDY7XG4gICAgICBiaDcgPSBhaDc7XG5cbiAgICAgIGJsMCA9IGFsMDtcbiAgICAgIGJsMSA9IGFsMTtcbiAgICAgIGJsMiA9IGFsMjtcbiAgICAgIGJsMyA9IGFsMztcbiAgICAgIGJsNCA9IGFsNDtcbiAgICAgIGJsNSA9IGFsNTtcbiAgICAgIGJsNiA9IGFsNjtcbiAgICAgIGJsNyA9IGFsNztcblxuICAgICAgLy8gYWRkXG4gICAgICBoID0gYWg3O1xuICAgICAgbCA9IGFsNztcblxuICAgICAgYSA9IGwgJiAweGZmZmY7IGIgPSBsID4+PiAxNjtcbiAgICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICAgIC8vIFNpZ21hMVxuICAgICAgaCA9ICgoYWg0ID4+PiAxNCkgfCAoYWw0IDw8ICgzMi0xNCkpKSBeICgoYWg0ID4+PiAxOCkgfCAoYWw0IDw8ICgzMi0xOCkpKSBeICgoYWw0ID4+PiAoNDEtMzIpKSB8IChhaDQgPDwgKDMyLSg0MS0zMikpKSk7XG4gICAgICBsID0gKChhbDQgPj4+IDE0KSB8IChhaDQgPDwgKDMyLTE0KSkpIF4gKChhbDQgPj4+IDE4KSB8IChhaDQgPDwgKDMyLTE4KSkpIF4gKChhaDQgPj4+ICg0MS0zMikpIHwgKGFsNCA8PCAoMzItKDQxLTMyKSkpKTtcblxuICAgICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgICAvLyBDaFxuICAgICAgaCA9IChhaDQgJiBhaDUpIF4gKH5haDQgJiBhaDYpO1xuICAgICAgbCA9IChhbDQgJiBhbDUpIF4gKH5hbDQgJiBhbDYpO1xuXG4gICAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgICBjICs9IGggJiAweGZmZmY7IGQgKz0gaCA+Pj4gMTY7XG5cbiAgICAgIC8vIEtcbiAgICAgIGggPSBLW2kqMl07XG4gICAgICBsID0gS1tpKjIrMV07XG5cbiAgICAgIGEgKz0gbCAmIDB4ZmZmZjsgYiArPSBsID4+PiAxNjtcbiAgICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgICAgLy8gd1xuICAgICAgaCA9IHdoW2klMTZdO1xuICAgICAgbCA9IHdsW2klMTZdO1xuXG4gICAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgICBjICs9IGggJiAweGZmZmY7IGQgKz0gaCA+Pj4gMTY7XG5cbiAgICAgIGIgKz0gYSA+Pj4gMTY7XG4gICAgICBjICs9IGIgPj4+IDE2O1xuICAgICAgZCArPSBjID4+PiAxNjtcblxuICAgICAgdGggPSBjICYgMHhmZmZmIHwgZCA8PCAxNjtcbiAgICAgIHRsID0gYSAmIDB4ZmZmZiB8IGIgPDwgMTY7XG5cbiAgICAgIC8vIGFkZFxuICAgICAgaCA9IHRoO1xuICAgICAgbCA9IHRsO1xuXG4gICAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgICAgLy8gU2lnbWEwXG4gICAgICBoID0gKChhaDAgPj4+IDI4KSB8IChhbDAgPDwgKDMyLTI4KSkpIF4gKChhbDAgPj4+ICgzNC0zMikpIHwgKGFoMCA8PCAoMzItKDM0LTMyKSkpKSBeICgoYWwwID4+PiAoMzktMzIpKSB8IChhaDAgPDwgKDMyLSgzOS0zMikpKSk7XG4gICAgICBsID0gKChhbDAgPj4+IDI4KSB8IChhaDAgPDwgKDMyLTI4KSkpIF4gKChhaDAgPj4+ICgzNC0zMikpIHwgKGFsMCA8PCAoMzItKDM0LTMyKSkpKSBeICgoYWgwID4+PiAoMzktMzIpKSB8IChhbDAgPDwgKDMyLSgzOS0zMikpKSk7XG5cbiAgICAgIGEgKz0gbCAmIDB4ZmZmZjsgYiArPSBsID4+PiAxNjtcbiAgICAgIGMgKz0gaCAmIDB4ZmZmZjsgZCArPSBoID4+PiAxNjtcblxuICAgICAgLy8gTWFqXG4gICAgICBoID0gKGFoMCAmIGFoMSkgXiAoYWgwICYgYWgyKSBeIChhaDEgJiBhaDIpO1xuICAgICAgbCA9IChhbDAgJiBhbDEpIF4gKGFsMCAmIGFsMikgXiAoYWwxICYgYWwyKTtcblxuICAgICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgICBiICs9IGEgPj4+IDE2O1xuICAgICAgYyArPSBiID4+PiAxNjtcbiAgICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICAgIGJoNyA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICAgIGJsNyA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgICAgLy8gYWRkXG4gICAgICBoID0gYmgzO1xuICAgICAgbCA9IGJsMztcblxuICAgICAgYSA9IGwgJiAweGZmZmY7IGIgPSBsID4+PiAxNjtcbiAgICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICAgIGggPSB0aDtcbiAgICAgIGwgPSB0bDtcblxuICAgICAgYSArPSBsICYgMHhmZmZmOyBiICs9IGwgPj4+IDE2O1xuICAgICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgICBiICs9IGEgPj4+IDE2O1xuICAgICAgYyArPSBiID4+PiAxNjtcbiAgICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICAgIGJoMyA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICAgIGJsMyA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgICAgYWgxID0gYmgwO1xuICAgICAgYWgyID0gYmgxO1xuICAgICAgYWgzID0gYmgyO1xuICAgICAgYWg0ID0gYmgzO1xuICAgICAgYWg1ID0gYmg0O1xuICAgICAgYWg2ID0gYmg1O1xuICAgICAgYWg3ID0gYmg2O1xuICAgICAgYWgwID0gYmg3O1xuXG4gICAgICBhbDEgPSBibDA7XG4gICAgICBhbDIgPSBibDE7XG4gICAgICBhbDMgPSBibDI7XG4gICAgICBhbDQgPSBibDM7XG4gICAgICBhbDUgPSBibDQ7XG4gICAgICBhbDYgPSBibDU7XG4gICAgICBhbDcgPSBibDY7XG4gICAgICBhbDAgPSBibDc7XG5cbiAgICAgIGlmIChpJTE2ID09PSAxNSkge1xuICAgICAgICBmb3IgKGogPSAwOyBqIDwgMTY7IGorKykge1xuICAgICAgICAgIC8vIGFkZFxuICAgICAgICAgIGggPSB3aFtqXTtcbiAgICAgICAgICBsID0gd2xbal07XG5cbiAgICAgICAgICBhID0gbCAmIDB4ZmZmZjsgYiA9IGwgPj4+IDE2O1xuICAgICAgICAgIGMgPSBoICYgMHhmZmZmOyBkID0gaCA+Pj4gMTY7XG5cbiAgICAgICAgICBoID0gd2hbKGorOSklMTZdO1xuICAgICAgICAgIGwgPSB3bFsoais5KSUxNl07XG5cbiAgICAgICAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgICAgICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgICAgICAgLy8gc2lnbWEwXG4gICAgICAgICAgdGggPSB3aFsoaisxKSUxNl07XG4gICAgICAgICAgdGwgPSB3bFsoaisxKSUxNl07XG4gICAgICAgICAgaCA9ICgodGggPj4+IDEpIHwgKHRsIDw8ICgzMi0xKSkpIF4gKCh0aCA+Pj4gOCkgfCAodGwgPDwgKDMyLTgpKSkgXiAodGggPj4+IDcpO1xuICAgICAgICAgIGwgPSAoKHRsID4+PiAxKSB8ICh0aCA8PCAoMzItMSkpKSBeICgodGwgPj4+IDgpIHwgKHRoIDw8ICgzMi04KSkpIF4gKCh0bCA+Pj4gNykgfCAodGggPDwgKDMyLTcpKSk7XG5cbiAgICAgICAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgICAgICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgICAgICAgLy8gc2lnbWExXG4gICAgICAgICAgdGggPSB3aFsoaisxNCklMTZdO1xuICAgICAgICAgIHRsID0gd2xbKGorMTQpJTE2XTtcbiAgICAgICAgICBoID0gKCh0aCA+Pj4gMTkpIHwgKHRsIDw8ICgzMi0xOSkpKSBeICgodGwgPj4+ICg2MS0zMikpIHwgKHRoIDw8ICgzMi0oNjEtMzIpKSkpIF4gKHRoID4+PiA2KTtcbiAgICAgICAgICBsID0gKCh0bCA+Pj4gMTkpIHwgKHRoIDw8ICgzMi0xOSkpKSBeICgodGggPj4+ICg2MS0zMikpIHwgKHRsIDw8ICgzMi0oNjEtMzIpKSkpIF4gKCh0bCA+Pj4gNikgfCAodGggPDwgKDMyLTYpKSk7XG5cbiAgICAgICAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgICAgICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgICAgICAgYiArPSBhID4+PiAxNjtcbiAgICAgICAgICBjICs9IGIgPj4+IDE2O1xuICAgICAgICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICAgICAgICB3aFtqXSA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICAgICAgICB3bFtqXSA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcbiAgICAgICAgfVxuICAgICAgfVxuICAgIH1cblxuICAgIC8vIGFkZFxuICAgIGggPSBhaDA7XG4gICAgbCA9IGFsMDtcblxuICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgIGggPSBoaFswXTtcbiAgICBsID0gaGxbMF07XG5cbiAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgYiArPSBhID4+PiAxNjtcbiAgICBjICs9IGIgPj4+IDE2O1xuICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICBoaFswXSA9IGFoMCA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICBobFswXSA9IGFsMCA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgIGggPSBhaDE7XG4gICAgbCA9IGFsMTtcblxuICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgIGggPSBoaFsxXTtcbiAgICBsID0gaGxbMV07XG5cbiAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgYiArPSBhID4+PiAxNjtcbiAgICBjICs9IGIgPj4+IDE2O1xuICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICBoaFsxXSA9IGFoMSA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICBobFsxXSA9IGFsMSA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgIGggPSBhaDI7XG4gICAgbCA9IGFsMjtcblxuICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgIGggPSBoaFsyXTtcbiAgICBsID0gaGxbMl07XG5cbiAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgYiArPSBhID4+PiAxNjtcbiAgICBjICs9IGIgPj4+IDE2O1xuICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICBoaFsyXSA9IGFoMiA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICBobFsyXSA9IGFsMiA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgIGggPSBhaDM7XG4gICAgbCA9IGFsMztcblxuICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgIGggPSBoaFszXTtcbiAgICBsID0gaGxbM107XG5cbiAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgYiArPSBhID4+PiAxNjtcbiAgICBjICs9IGIgPj4+IDE2O1xuICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICBoaFszXSA9IGFoMyA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICBobFszXSA9IGFsMyA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgIGggPSBhaDQ7XG4gICAgbCA9IGFsNDtcblxuICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgIGggPSBoaFs0XTtcbiAgICBsID0gaGxbNF07XG5cbiAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgYiArPSBhID4+PiAxNjtcbiAgICBjICs9IGIgPj4+IDE2O1xuICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICBoaFs0XSA9IGFoNCA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICBobFs0XSA9IGFsNCA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgIGggPSBhaDU7XG4gICAgbCA9IGFsNTtcblxuICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgIGggPSBoaFs1XTtcbiAgICBsID0gaGxbNV07XG5cbiAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgYiArPSBhID4+PiAxNjtcbiAgICBjICs9IGIgPj4+IDE2O1xuICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICBoaFs1XSA9IGFoNSA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICBobFs1XSA9IGFsNSA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgIGggPSBhaDY7XG4gICAgbCA9IGFsNjtcblxuICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgIGggPSBoaFs2XTtcbiAgICBsID0gaGxbNl07XG5cbiAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgYiArPSBhID4+PiAxNjtcbiAgICBjICs9IGIgPj4+IDE2O1xuICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICBoaFs2XSA9IGFoNiA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICBobFs2XSA9IGFsNiA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgIGggPSBhaDc7XG4gICAgbCA9IGFsNztcblxuICAgIGEgPSBsICYgMHhmZmZmOyBiID0gbCA+Pj4gMTY7XG4gICAgYyA9IGggJiAweGZmZmY7IGQgPSBoID4+PiAxNjtcblxuICAgIGggPSBoaFs3XTtcbiAgICBsID0gaGxbN107XG5cbiAgICBhICs9IGwgJiAweGZmZmY7IGIgKz0gbCA+Pj4gMTY7XG4gICAgYyArPSBoICYgMHhmZmZmOyBkICs9IGggPj4+IDE2O1xuXG4gICAgYiArPSBhID4+PiAxNjtcbiAgICBjICs9IGIgPj4+IDE2O1xuICAgIGQgKz0gYyA+Pj4gMTY7XG5cbiAgICBoaFs3XSA9IGFoNyA9IChjICYgMHhmZmZmKSB8IChkIDw8IDE2KTtcbiAgICBobFs3XSA9IGFsNyA9IChhICYgMHhmZmZmKSB8IChiIDw8IDE2KTtcblxuICAgIHBvcyArPSAxMjg7XG4gICAgbiAtPSAxMjg7XG4gIH1cblxuICByZXR1cm4gbjtcbn1cblxuZnVuY3Rpb24gY3J5cHRvX2hhc2gob3V0LCBtLCBuKSB7XG4gIHZhciBoaCA9IG5ldyBJbnQzMkFycmF5KDgpLFxuICAgICAgaGwgPSBuZXcgSW50MzJBcnJheSg4KSxcbiAgICAgIHggPSBuZXcgVWludDhBcnJheSgyNTYpLFxuICAgICAgaSwgYiA9IG47XG5cbiAgaGhbMF0gPSAweDZhMDllNjY3O1xuICBoaFsxXSA9IDB4YmI2N2FlODU7XG4gIGhoWzJdID0gMHgzYzZlZjM3MjtcbiAgaGhbM10gPSAweGE1NGZmNTNhO1xuICBoaFs0XSA9IDB4NTEwZTUyN2Y7XG4gIGhoWzVdID0gMHg5YjA1Njg4YztcbiAgaGhbNl0gPSAweDFmODNkOWFiO1xuICBoaFs3XSA9IDB4NWJlMGNkMTk7XG5cbiAgaGxbMF0gPSAweGYzYmNjOTA4O1xuICBobFsxXSA9IDB4ODRjYWE3M2I7XG4gIGhsWzJdID0gMHhmZTk0ZjgyYjtcbiAgaGxbM10gPSAweDVmMWQzNmYxO1xuICBobFs0XSA9IDB4YWRlNjgyZDE7XG4gIGhsWzVdID0gMHgyYjNlNmMxZjtcbiAgaGxbNl0gPSAweGZiNDFiZDZiO1xuICBobFs3XSA9IDB4MTM3ZTIxNzk7XG5cbiAgY3J5cHRvX2hhc2hibG9ja3NfaGwoaGgsIGhsLCBtLCBuKTtcbiAgbiAlPSAxMjg7XG5cbiAgZm9yIChpID0gMDsgaSA8IG47IGkrKykgeFtpXSA9IG1bYi1uK2ldO1xuICB4W25dID0gMTI4O1xuXG4gIG4gPSAyNTYtMTI4KihuPDExMj8xOjApO1xuICB4W24tOV0gPSAwO1xuICB0czY0KHgsIG4tOCwgIChiIC8gMHgyMDAwMDAwMCkgfCAwLCBiIDw8IDMpO1xuICBjcnlwdG9faGFzaGJsb2Nrc19obChoaCwgaGwsIHgsIG4pO1xuXG4gIGZvciAoaSA9IDA7IGkgPCA4OyBpKyspIHRzNjQob3V0LCA4KmksIGhoW2ldLCBobFtpXSk7XG5cbiAgcmV0dXJuIDA7XG59XG5cbmZ1bmN0aW9uIGFkZChwLCBxKSB7XG4gIHZhciBhID0gZ2YoKSwgYiA9IGdmKCksIGMgPSBnZigpLFxuICAgICAgZCA9IGdmKCksIGUgPSBnZigpLCBmID0gZ2YoKSxcbiAgICAgIGcgPSBnZigpLCBoID0gZ2YoKSwgdCA9IGdmKCk7XG5cbiAgWihhLCBwWzFdLCBwWzBdKTtcbiAgWih0LCBxWzFdLCBxWzBdKTtcbiAgTShhLCBhLCB0KTtcbiAgQShiLCBwWzBdLCBwWzFdKTtcbiAgQSh0LCBxWzBdLCBxWzFdKTtcbiAgTShiLCBiLCB0KTtcbiAgTShjLCBwWzNdLCBxWzNdKTtcbiAgTShjLCBjLCBEMik7XG4gIE0oZCwgcFsyXSwgcVsyXSk7XG4gIEEoZCwgZCwgZCk7XG4gIFooZSwgYiwgYSk7XG4gIFooZiwgZCwgYyk7XG4gIEEoZywgZCwgYyk7XG4gIEEoaCwgYiwgYSk7XG5cbiAgTShwWzBdLCBlLCBmKTtcbiAgTShwWzFdLCBoLCBnKTtcbiAgTShwWzJdLCBnLCBmKTtcbiAgTShwWzNdLCBlLCBoKTtcbn1cblxuZnVuY3Rpb24gY3N3YXAocCwgcSwgYikge1xuICB2YXIgaTtcbiAgZm9yIChpID0gMDsgaSA8IDQ7IGkrKykge1xuICAgIHNlbDI1NTE5KHBbaV0sIHFbaV0sIGIpO1xuICB9XG59XG5cbmZ1bmN0aW9uIHBhY2sociwgcCkge1xuICB2YXIgdHggPSBnZigpLCB0eSA9IGdmKCksIHppID0gZ2YoKTtcbiAgaW52MjU1MTkoemksIHBbMl0pO1xuICBNKHR4LCBwWzBdLCB6aSk7XG4gIE0odHksIHBbMV0sIHppKTtcbiAgcGFjazI1NTE5KHIsIHR5KTtcbiAgclszMV0gXj0gcGFyMjU1MTkodHgpIDw8IDc7XG59XG5cbmZ1bmN0aW9uIHNjYWxhcm11bHQocCwgcSwgcykge1xuICB2YXIgYiwgaTtcbiAgc2V0MjU1MTkocFswXSwgZ2YwKTtcbiAgc2V0MjU1MTkocFsxXSwgZ2YxKTtcbiAgc2V0MjU1MTkocFsyXSwgZ2YxKTtcbiAgc2V0MjU1MTkocFszXSwgZ2YwKTtcbiAgZm9yIChpID0gMjU1OyBpID49IDA7IC0taSkge1xuICAgIGIgPSAoc1soaS84KXwwXSA+PiAoaSY3KSkgJiAxO1xuICAgIGNzd2FwKHAsIHEsIGIpO1xuICAgIGFkZChxLCBwKTtcbiAgICBhZGQocCwgcCk7XG4gICAgY3N3YXAocCwgcSwgYik7XG4gIH1cbn1cblxuZnVuY3Rpb24gc2NhbGFyYmFzZShwLCBzKSB7XG4gIHZhciBxID0gW2dmKCksIGdmKCksIGdmKCksIGdmKCldO1xuICBzZXQyNTUxOShxWzBdLCBYKTtcbiAgc2V0MjU1MTkocVsxXSwgWSk7XG4gIHNldDI1NTE5KHFbMl0sIGdmMSk7XG4gIE0ocVszXSwgWCwgWSk7XG4gIHNjYWxhcm11bHQocCwgcSwgcyk7XG59XG5cbmZ1bmN0aW9uIGNyeXB0b19zaWduX2tleXBhaXIocGssIHNrLCBzZWVkZWQpIHtcbiAgdmFyIGQgPSBuZXcgVWludDhBcnJheSg2NCk7XG4gIHZhciBwID0gW2dmKCksIGdmKCksIGdmKCksIGdmKCldO1xuICB2YXIgaTtcblxuICBpZiAoIXNlZWRlZCkgcmFuZG9tYnl0ZXMoc2ssIDMyKTtcbiAgY3J5cHRvX2hhc2goZCwgc2ssIDMyKTtcbiAgZFswXSAmPSAyNDg7XG4gIGRbMzFdICY9IDEyNztcbiAgZFszMV0gfD0gNjQ7XG5cbiAgc2NhbGFyYmFzZShwLCBkKTtcbiAgcGFjayhwaywgcCk7XG5cbiAgZm9yIChpID0gMDsgaSA8IDMyOyBpKyspIHNrW2krMzJdID0gcGtbaV07XG4gIHJldHVybiAwO1xufVxuXG52YXIgTCA9IG5ldyBGbG9hdDY0QXJyYXkoWzB4ZWQsIDB4ZDMsIDB4ZjUsIDB4NWMsIDB4MWEsIDB4NjMsIDB4MTIsIDB4NTgsIDB4ZDYsIDB4OWMsIDB4ZjcsIDB4YTIsIDB4ZGUsIDB4ZjksIDB4ZGUsIDB4MTQsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDAsIDB4MTBdKTtcblxuZnVuY3Rpb24gbW9kTChyLCB4KSB7XG4gIHZhciBjYXJyeSwgaSwgaiwgaztcbiAgZm9yIChpID0gNjM7IGkgPj0gMzI7IC0taSkge1xuICAgIGNhcnJ5ID0gMDtcbiAgICBmb3IgKGogPSBpIC0gMzIsIGsgPSBpIC0gMTI7IGogPCBrOyArK2opIHtcbiAgICAgIHhbal0gKz0gY2FycnkgLSAxNiAqIHhbaV0gKiBMW2ogLSAoaSAtIDMyKV07XG4gICAgICBjYXJyeSA9ICh4W2pdICsgMTI4KSA+PiA4O1xuICAgICAgeFtqXSAtPSBjYXJyeSAqIDI1NjtcbiAgICB9XG4gICAgeFtqXSArPSBjYXJyeTtcbiAgICB4W2ldID0gMDtcbiAgfVxuICBjYXJyeSA9IDA7XG4gIGZvciAoaiA9IDA7IGogPCAzMjsgaisrKSB7XG4gICAgeFtqXSArPSBjYXJyeSAtICh4WzMxXSA+PiA0KSAqIExbal07XG4gICAgY2FycnkgPSB4W2pdID4+IDg7XG4gICAgeFtqXSAmPSAyNTU7XG4gIH1cbiAgZm9yIChqID0gMDsgaiA8IDMyOyBqKyspIHhbal0gLT0gY2FycnkgKiBMW2pdO1xuICBmb3IgKGkgPSAwOyBpIDwgMzI7IGkrKykge1xuICAgIHhbaSsxXSArPSB4W2ldID4+IDg7XG4gICAgcltpXSA9IHhbaV0gJiAyNTU7XG4gIH1cbn1cblxuZnVuY3Rpb24gcmVkdWNlKHIpIHtcbiAgdmFyIHggPSBuZXcgRmxvYXQ2NEFycmF5KDY0KSwgaTtcbiAgZm9yIChpID0gMDsgaSA8IDY0OyBpKyspIHhbaV0gPSByW2ldO1xuICBmb3IgKGkgPSAwOyBpIDwgNjQ7IGkrKykgcltpXSA9IDA7XG4gIG1vZEwociwgeCk7XG59XG5cbi8vIE5vdGU6IGRpZmZlcmVuY2UgZnJvbSBDIC0gc21sZW4gcmV0dXJuZWQsIG5vdCBwYXNzZWQgYXMgYXJndW1lbnQuXG5mdW5jdGlvbiBjcnlwdG9fc2lnbihzbSwgbSwgbiwgc2spIHtcbiAgdmFyIGQgPSBuZXcgVWludDhBcnJheSg2NCksIGggPSBuZXcgVWludDhBcnJheSg2NCksIHIgPSBuZXcgVWludDhBcnJheSg2NCk7XG4gIHZhciBpLCBqLCB4ID0gbmV3IEZsb2F0NjRBcnJheSg2NCk7XG4gIHZhciBwID0gW2dmKCksIGdmKCksIGdmKCksIGdmKCldO1xuXG4gIGNyeXB0b19oYXNoKGQsIHNrLCAzMik7XG4gIGRbMF0gJj0gMjQ4O1xuICBkWzMxXSAmPSAxMjc7XG4gIGRbMzFdIHw9IDY0O1xuXG4gIHZhciBzbWxlbiA9IG4gKyA2NDtcbiAgZm9yIChpID0gMDsgaSA8IG47IGkrKykgc21bNjQgKyBpXSA9IG1baV07XG4gIGZvciAoaSA9IDA7IGkgPCAzMjsgaSsrKSBzbVszMiArIGldID0gZFszMiArIGldO1xuXG4gIGNyeXB0b19oYXNoKHIsIHNtLnN1YmFycmF5KDMyKSwgbiszMik7XG4gIHJlZHVjZShyKTtcbiAgc2NhbGFyYmFzZShwLCByKTtcbiAgcGFjayhzbSwgcCk7XG5cbiAgZm9yIChpID0gMzI7IGkgPCA2NDsgaSsrKSBzbVtpXSA9IHNrW2ldO1xuICBjcnlwdG9faGFzaChoLCBzbSwgbiArIDY0KTtcbiAgcmVkdWNlKGgpO1xuXG4gIGZvciAoaSA9IDA7IGkgPCA2NDsgaSsrKSB4W2ldID0gMDtcbiAgZm9yIChpID0gMDsgaSA8IDMyOyBpKyspIHhbaV0gPSByW2ldO1xuICBmb3IgKGkgPSAwOyBpIDwgMzI7IGkrKykge1xuICAgIGZvciAoaiA9IDA7IGogPCAzMjsgaisrKSB7XG4gICAgICB4W2kral0gKz0gaFtpXSAqIGRbal07XG4gICAgfVxuICB9XG5cbiAgbW9kTChzbS5zdWJhcnJheSgzMiksIHgpO1xuICByZXR1cm4gc21sZW47XG59XG5cbmZ1bmN0aW9uIHVucGFja25lZyhyLCBwKSB7XG4gIHZhciB0ID0gZ2YoKSwgY2hrID0gZ2YoKSwgbnVtID0gZ2YoKSxcbiAgICAgIGRlbiA9IGdmKCksIGRlbjIgPSBnZigpLCBkZW40ID0gZ2YoKSxcbiAgICAgIGRlbjYgPSBnZigpO1xuXG4gIHNldDI1NTE5KHJbMl0sIGdmMSk7XG4gIHVucGFjazI1NTE5KHJbMV0sIHApO1xuICBTKG51bSwgclsxXSk7XG4gIE0oZGVuLCBudW0sIEQpO1xuICBaKG51bSwgbnVtLCByWzJdKTtcbiAgQShkZW4sIHJbMl0sIGRlbik7XG5cbiAgUyhkZW4yLCBkZW4pO1xuICBTKGRlbjQsIGRlbjIpO1xuICBNKGRlbjYsIGRlbjQsIGRlbjIpO1xuICBNKHQsIGRlbjYsIG51bSk7XG4gIE0odCwgdCwgZGVuKTtcblxuICBwb3cyNTIzKHQsIHQpO1xuICBNKHQsIHQsIG51bSk7XG4gIE0odCwgdCwgZGVuKTtcbiAgTSh0LCB0LCBkZW4pO1xuICBNKHJbMF0sIHQsIGRlbik7XG5cbiAgUyhjaGssIHJbMF0pO1xuICBNKGNoaywgY2hrLCBkZW4pO1xuICBpZiAobmVxMjU1MTkoY2hrLCBudW0pKSBNKHJbMF0sIHJbMF0sIEkpO1xuXG4gIFMoY2hrLCByWzBdKTtcbiAgTShjaGssIGNoaywgZGVuKTtcbiAgaWYgKG5lcTI1NTE5KGNoaywgbnVtKSkgcmV0dXJuIC0xO1xuXG4gIGlmIChwYXIyNTUxOShyWzBdKSA9PT0gKHBbMzFdPj43KSkgWihyWzBdLCBnZjAsIHJbMF0pO1xuXG4gIE0oclszXSwgclswXSwgclsxXSk7XG4gIHJldHVybiAwO1xufVxuXG5mdW5jdGlvbiBjcnlwdG9fc2lnbl9vcGVuKG0sIHNtLCBuLCBwaykge1xuICB2YXIgaSwgbWxlbjtcbiAgdmFyIHQgPSBuZXcgVWludDhBcnJheSgzMiksIGggPSBuZXcgVWludDhBcnJheSg2NCk7XG4gIHZhciBwID0gW2dmKCksIGdmKCksIGdmKCksIGdmKCldLFxuICAgICAgcSA9IFtnZigpLCBnZigpLCBnZigpLCBnZigpXTtcblxuICBtbGVuID0gLTE7XG4gIGlmIChuIDwgNjQpIHJldHVybiAtMTtcblxuICBpZiAodW5wYWNrbmVnKHEsIHBrKSkgcmV0dXJuIC0xO1xuXG4gIGZvciAoaSA9IDA7IGkgPCBuOyBpKyspIG1baV0gPSBzbVtpXTtcbiAgZm9yIChpID0gMDsgaSA8IDMyOyBpKyspIG1baSszMl0gPSBwa1tpXTtcbiAgY3J5cHRvX2hhc2goaCwgbSwgbik7XG4gIHJlZHVjZShoKTtcbiAgc2NhbGFybXVsdChwLCBxLCBoKTtcblxuICBzY2FsYXJiYXNlKHEsIHNtLnN1YmFycmF5KDMyKSk7XG4gIGFkZChwLCBxKTtcbiAgcGFjayh0LCBwKTtcblxuICBuIC09IDY0O1xuICBpZiAoY3J5cHRvX3ZlcmlmeV8zMihzbSwgMCwgdCwgMCkpIHtcbiAgICBmb3IgKGkgPSAwOyBpIDwgbjsgaSsrKSBtW2ldID0gMDtcbiAgICByZXR1cm4gLTE7XG4gIH1cblxuICBmb3IgKGkgPSAwOyBpIDwgbjsgaSsrKSBtW2ldID0gc21baSArIDY0XTtcbiAgbWxlbiA9IG47XG4gIHJldHVybiBtbGVuO1xufVxuXG52YXIgY3J5cHRvX3NlY3JldGJveF9LRVlCWVRFUyA9IDMyLFxuICAgIGNyeXB0b19zZWNyZXRib3hfTk9OQ0VCWVRFUyA9IDI0LFxuICAgIGNyeXB0b19zZWNyZXRib3hfWkVST0JZVEVTID0gMzIsXG4gICAgY3J5cHRvX3NlY3JldGJveF9CT1haRVJPQllURVMgPSAxNixcbiAgICBjcnlwdG9fc2NhbGFybXVsdF9CWVRFUyA9IDMyLFxuICAgIGNyeXB0b19zY2FsYXJtdWx0X1NDQUxBUkJZVEVTID0gMzIsXG4gICAgY3J5cHRvX2JveF9QVUJMSUNLRVlCWVRFUyA9IDMyLFxuICAgIGNyeXB0b19ib3hfU0VDUkVUS0VZQllURVMgPSAzMixcbiAgICBjcnlwdG9fYm94X0JFRk9SRU5NQllURVMgPSAzMixcbiAgICBjcnlwdG9fYm94X05PTkNFQllURVMgPSBjcnlwdG9fc2VjcmV0Ym94X05PTkNFQllURVMsXG4gICAgY3J5cHRvX2JveF9aRVJPQllURVMgPSBjcnlwdG9fc2VjcmV0Ym94X1pFUk9CWVRFUyxcbiAgICBjcnlwdG9fYm94X0JPWFpFUk9CWVRFUyA9IGNyeXB0b19zZWNyZXRib3hfQk9YWkVST0JZVEVTLFxuICAgIGNyeXB0b19zaWduX0JZVEVTID0gNjQsXG4gICAgY3J5cHRvX3NpZ25fUFVCTElDS0VZQllURVMgPSAzMixcbiAgICBjcnlwdG9fc2lnbl9TRUNSRVRLRVlCWVRFUyA9IDY0LFxuICAgIGNyeXB0b19zaWduX1NFRURCWVRFUyA9IDMyLFxuICAgIGNyeXB0b19oYXNoX0JZVEVTID0gNjQ7XG5cbm5hY2wubG93bGV2ZWwgPSB7XG4gIGNyeXB0b19jb3JlX2hzYWxzYTIwOiBjcnlwdG9fY29yZV9oc2Fsc2EyMCxcbiAgY3J5cHRvX3N0cmVhbV94b3I6IGNyeXB0b19zdHJlYW1feG9yLFxuICBjcnlwdG9fc3RyZWFtOiBjcnlwdG9fc3RyZWFtLFxuICBjcnlwdG9fc3RyZWFtX3NhbHNhMjBfeG9yOiBjcnlwdG9fc3RyZWFtX3NhbHNhMjBfeG9yLFxuICBjcnlwdG9fc3RyZWFtX3NhbHNhMjA6IGNyeXB0b19zdHJlYW1fc2Fsc2EyMCxcbiAgY3J5cHRvX29uZXRpbWVhdXRoOiBjcnlwdG9fb25ldGltZWF1dGgsXG4gIGNyeXB0b19vbmV0aW1lYXV0aF92ZXJpZnk6IGNyeXB0b19vbmV0aW1lYXV0aF92ZXJpZnksXG4gIGNyeXB0b192ZXJpZnlfMTY6IGNyeXB0b192ZXJpZnlfMTYsXG4gIGNyeXB0b192ZXJpZnlfMzI6IGNyeXB0b192ZXJpZnlfMzIsXG4gIGNyeXB0b19zZWNyZXRib3g6IGNyeXB0b19zZWNyZXRib3gsXG4gIGNyeXB0b19zZWNyZXRib3hfb3BlbjogY3J5cHRvX3NlY3JldGJveF9vcGVuLFxuICBjcnlwdG9fc2NhbGFybXVsdDogY3J5cHRvX3NjYWxhcm11bHQsXG4gIGNyeXB0b19zY2FsYXJtdWx0X2Jhc2U6IGNyeXB0b19zY2FsYXJtdWx0X2Jhc2UsXG4gIGNyeXB0b19ib3hfYmVmb3Jlbm06IGNyeXB0b19ib3hfYmVmb3Jlbm0sXG4gIGNyeXB0b19ib3hfYWZ0ZXJubTogY3J5cHRvX2JveF9hZnRlcm5tLFxuICBjcnlwdG9fYm94OiBjcnlwdG9fYm94LFxuICBjcnlwdG9fYm94X29wZW46IGNyeXB0b19ib3hfb3BlbixcbiAgY3J5cHRvX2JveF9rZXlwYWlyOiBjcnlwdG9fYm94X2tleXBhaXIsXG4gIGNyeXB0b19oYXNoOiBjcnlwdG9faGFzaCxcbiAgY3J5cHRvX3NpZ246IGNyeXB0b19zaWduLFxuICBjcnlwdG9fc2lnbl9rZXlwYWlyOiBjcnlwdG9fc2lnbl9rZXlwYWlyLFxuICBjcnlwdG9fc2lnbl9vcGVuOiBjcnlwdG9fc2lnbl9vcGVuLFxuXG4gIGNyeXB0b19zZWNyZXRib3hfS0VZQllURVM6IGNyeXB0b19zZWNyZXRib3hfS0VZQllURVMsXG4gIGNyeXB0b19zZWNyZXRib3hfTk9OQ0VCWVRFUzogY3J5cHRvX3NlY3JldGJveF9OT05DRUJZVEVTLFxuICBjcnlwdG9fc2VjcmV0Ym94X1pFUk9CWVRFUzogY3J5cHRvX3NlY3JldGJveF9aRVJPQllURVMsXG4gIGNyeXB0b19zZWNyZXRib3hfQk9YWkVST0JZVEVTOiBjcnlwdG9fc2VjcmV0Ym94X0JPWFpFUk9CWVRFUyxcbiAgY3J5cHRvX3NjYWxhcm11bHRfQllURVM6IGNyeXB0b19zY2FsYXJtdWx0X0JZVEVTLFxuICBjcnlwdG9fc2NhbGFybXVsdF9TQ0FMQVJCWVRFUzogY3J5cHRvX3NjYWxhcm11bHRfU0NBTEFSQllURVMsXG4gIGNyeXB0b19ib3hfUFVCTElDS0VZQllURVM6IGNyeXB0b19ib3hfUFVCTElDS0VZQllURVMsXG4gIGNyeXB0b19ib3hfU0VDUkVUS0VZQllURVM6IGNyeXB0b19ib3hfU0VDUkVUS0VZQllURVMsXG4gIGNyeXB0b19ib3hfQkVGT1JFTk1CWVRFUzogY3J5cHRvX2JveF9CRUZPUkVOTUJZVEVTLFxuICBjcnlwdG9fYm94X05PTkNFQllURVM6IGNyeXB0b19ib3hfTk9OQ0VCWVRFUyxcbiAgY3J5cHRvX2JveF9aRVJPQllURVM6IGNyeXB0b19ib3hfWkVST0JZVEVTLFxuICBjcnlwdG9fYm94X0JPWFpFUk9CWVRFUzogY3J5cHRvX2JveF9CT1haRVJPQllURVMsXG4gIGNyeXB0b19zaWduX0JZVEVTOiBjcnlwdG9fc2lnbl9CWVRFUyxcbiAgY3J5cHRvX3NpZ25fUFVCTElDS0VZQllURVM6IGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTLFxuICBjcnlwdG9fc2lnbl9TRUNSRVRLRVlCWVRFUzogY3J5cHRvX3NpZ25fU0VDUkVUS0VZQllURVMsXG4gIGNyeXB0b19zaWduX1NFRURCWVRFUzogY3J5cHRvX3NpZ25fU0VFREJZVEVTLFxuICBjcnlwdG9faGFzaF9CWVRFUzogY3J5cHRvX2hhc2hfQllURVNcbn07XG5cbi8qIEhpZ2gtbGV2ZWwgQVBJICovXG5cbmZ1bmN0aW9uIGNoZWNrTGVuZ3RocyhrLCBuKSB7XG4gIGlmIChrLmxlbmd0aCAhPT0gY3J5cHRvX3NlY3JldGJveF9LRVlCWVRFUykgdGhyb3cgbmV3IEVycm9yKCdiYWQga2V5IHNpemUnKTtcbiAgaWYgKG4ubGVuZ3RoICE9PSBjcnlwdG9fc2VjcmV0Ym94X05PTkNFQllURVMpIHRocm93IG5ldyBFcnJvcignYmFkIG5vbmNlIHNpemUnKTtcbn1cblxuZnVuY3Rpb24gY2hlY2tCb3hMZW5ndGhzKHBrLCBzaykge1xuICBpZiAocGsubGVuZ3RoICE9PSBjcnlwdG9fYm94X1BVQkxJQ0tFWUJZVEVTKSB0aHJvdyBuZXcgRXJyb3IoJ2JhZCBwdWJsaWMga2V5IHNpemUnKTtcbiAgaWYgKHNrLmxlbmd0aCAhPT0gY3J5cHRvX2JveF9TRUNSRVRLRVlCWVRFUykgdGhyb3cgbmV3IEVycm9yKCdiYWQgc2VjcmV0IGtleSBzaXplJyk7XG59XG5cbmZ1bmN0aW9uIGNoZWNrQXJyYXlUeXBlcygpIHtcbiAgdmFyIHQsIGk7XG4gIGZvciAoaSA9IDA7IGkgPCBhcmd1bWVudHMubGVuZ3RoOyBpKyspIHtcbiAgICAgaWYgKCh0ID0gT2JqZWN0LnByb3RvdHlwZS50b1N0cmluZy5jYWxsKGFyZ3VtZW50c1tpXSkpICE9PSAnW29iamVjdCBVaW50OEFycmF5XScpXG4gICAgICAgdGhyb3cgbmV3IFR5cGVFcnJvcigndW5leHBlY3RlZCB0eXBlICcgKyB0ICsgJywgdXNlIFVpbnQ4QXJyYXknKTtcbiAgfVxufVxuXG5mdW5jdGlvbiBjbGVhbnVwKGFycikge1xuICBmb3IgKHZhciBpID0gMDsgaSA8IGFyci5sZW5ndGg7IGkrKykgYXJyW2ldID0gMDtcbn1cblxuLy8gVE9ETzogQ29tcGxldGVseSByZW1vdmUgdGhpcyBpbiB2MC4xNS5cbmlmICghbmFjbC51dGlsKSB7XG4gIG5hY2wudXRpbCA9IHt9O1xuICBuYWNsLnV0aWwuZGVjb2RlVVRGOCA9IG5hY2wudXRpbC5lbmNvZGVVVEY4ID0gbmFjbC51dGlsLmVuY29kZUJhc2U2NCA9IG5hY2wudXRpbC5kZWNvZGVCYXNlNjQgPSBmdW5jdGlvbigpIHtcbiAgICB0aHJvdyBuZXcgRXJyb3IoJ25hY2wudXRpbCBtb3ZlZCBpbnRvIHNlcGFyYXRlIHBhY2thZ2U6IGh0dHBzOi8vZ2l0aHViLmNvbS9kY2hlc3QvdHdlZXRuYWNsLXV0aWwtanMnKTtcbiAgfTtcbn1cblxubmFjbC5yYW5kb21CeXRlcyA9IGZ1bmN0aW9uKG4pIHtcbiAgdmFyIGIgPSBuZXcgVWludDhBcnJheShuKTtcbiAgcmFuZG9tYnl0ZXMoYiwgbik7XG4gIHJldHVybiBiO1xufTtcblxubmFjbC5zZWNyZXRib3ggPSBmdW5jdGlvbihtc2csIG5vbmNlLCBrZXkpIHtcbiAgY2hlY2tBcnJheVR5cGVzKG1zZywgbm9uY2UsIGtleSk7XG4gIGNoZWNrTGVuZ3RocyhrZXksIG5vbmNlKTtcbiAgdmFyIG0gPSBuZXcgVWludDhBcnJheShjcnlwdG9fc2VjcmV0Ym94X1pFUk9CWVRFUyArIG1zZy5sZW5ndGgpO1xuICB2YXIgYyA9IG5ldyBVaW50OEFycmF5KG0ubGVuZ3RoKTtcbiAgZm9yICh2YXIgaSA9IDA7IGkgPCBtc2cubGVuZ3RoOyBpKyspIG1baStjcnlwdG9fc2VjcmV0Ym94X1pFUk9CWVRFU10gPSBtc2dbaV07XG4gIGNyeXB0b19zZWNyZXRib3goYywgbSwgbS5sZW5ndGgsIG5vbmNlLCBrZXkpO1xuICByZXR1cm4gYy5zdWJhcnJheShjcnlwdG9fc2VjcmV0Ym94X0JPWFpFUk9CWVRFUyk7XG59O1xuXG5uYWNsLnNlY3JldGJveC5vcGVuID0gZnVuY3Rpb24oYm94LCBub25jZSwga2V5KSB7XG4gIGNoZWNrQXJyYXlUeXBlcyhib3gsIG5vbmNlLCBrZXkpO1xuICBjaGVja0xlbmd0aHMoa2V5LCBub25jZSk7XG4gIHZhciBjID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NlY3JldGJveF9CT1haRVJPQllURVMgKyBib3gubGVuZ3RoKTtcbiAgdmFyIG0gPSBuZXcgVWludDhBcnJheShjLmxlbmd0aCk7XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgYm94Lmxlbmd0aDsgaSsrKSBjW2krY3J5cHRvX3NlY3JldGJveF9CT1haRVJPQllURVNdID0gYm94W2ldO1xuICBpZiAoYy5sZW5ndGggPCAzMikgcmV0dXJuIGZhbHNlO1xuICBpZiAoY3J5cHRvX3NlY3JldGJveF9vcGVuKG0sIGMsIGMubGVuZ3RoLCBub25jZSwga2V5KSAhPT0gMCkgcmV0dXJuIGZhbHNlO1xuICByZXR1cm4gbS5zdWJhcnJheShjcnlwdG9fc2VjcmV0Ym94X1pFUk9CWVRFUyk7XG59O1xuXG5uYWNsLnNlY3JldGJveC5rZXlMZW5ndGggPSBjcnlwdG9fc2VjcmV0Ym94X0tFWUJZVEVTO1xubmFjbC5zZWNyZXRib3gubm9uY2VMZW5ndGggPSBjcnlwdG9fc2VjcmV0Ym94X05PTkNFQllURVM7XG5uYWNsLnNlY3JldGJveC5vdmVyaGVhZExlbmd0aCA9IGNyeXB0b19zZWNyZXRib3hfQk9YWkVST0JZVEVTO1xuXG5uYWNsLnNjYWxhck11bHQgPSBmdW5jdGlvbihuLCBwKSB7XG4gIGNoZWNrQXJyYXlUeXBlcyhuLCBwKTtcbiAgaWYgKG4ubGVuZ3RoICE9PSBjcnlwdG9fc2NhbGFybXVsdF9TQ0FMQVJCWVRFUykgdGhyb3cgbmV3IEVycm9yKCdiYWQgbiBzaXplJyk7XG4gIGlmIChwLmxlbmd0aCAhPT0gY3J5cHRvX3NjYWxhcm11bHRfQllURVMpIHRocm93IG5ldyBFcnJvcignYmFkIHAgc2l6ZScpO1xuICB2YXIgcSA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zY2FsYXJtdWx0X0JZVEVTKTtcbiAgY3J5cHRvX3NjYWxhcm11bHQocSwgbiwgcCk7XG4gIHJldHVybiBxO1xufTtcblxubmFjbC5zY2FsYXJNdWx0LmJhc2UgPSBmdW5jdGlvbihuKSB7XG4gIGNoZWNrQXJyYXlUeXBlcyhuKTtcbiAgaWYgKG4ubGVuZ3RoICE9PSBjcnlwdG9fc2NhbGFybXVsdF9TQ0FMQVJCWVRFUykgdGhyb3cgbmV3IEVycm9yKCdiYWQgbiBzaXplJyk7XG4gIHZhciBxID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NjYWxhcm11bHRfQllURVMpO1xuICBjcnlwdG9fc2NhbGFybXVsdF9iYXNlKHEsIG4pO1xuICByZXR1cm4gcTtcbn07XG5cbm5hY2wuc2NhbGFyTXVsdC5zY2FsYXJMZW5ndGggPSBjcnlwdG9fc2NhbGFybXVsdF9TQ0FMQVJCWVRFUztcbm5hY2wuc2NhbGFyTXVsdC5ncm91cEVsZW1lbnRMZW5ndGggPSBjcnlwdG9fc2NhbGFybXVsdF9CWVRFUztcblxubmFjbC5ib3ggPSBmdW5jdGlvbihtc2csIG5vbmNlLCBwdWJsaWNLZXksIHNlY3JldEtleSkge1xuICB2YXIgayA9IG5hY2wuYm94LmJlZm9yZShwdWJsaWNLZXksIHNlY3JldEtleSk7XG4gIHJldHVybiBuYWNsLnNlY3JldGJveChtc2csIG5vbmNlLCBrKTtcbn07XG5cbm5hY2wuYm94LmJlZm9yZSA9IGZ1bmN0aW9uKHB1YmxpY0tleSwgc2VjcmV0S2V5KSB7XG4gIGNoZWNrQXJyYXlUeXBlcyhwdWJsaWNLZXksIHNlY3JldEtleSk7XG4gIGNoZWNrQm94TGVuZ3RocyhwdWJsaWNLZXksIHNlY3JldEtleSk7XG4gIHZhciBrID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX2JveF9CRUZPUkVOTUJZVEVTKTtcbiAgY3J5cHRvX2JveF9iZWZvcmVubShrLCBwdWJsaWNLZXksIHNlY3JldEtleSk7XG4gIHJldHVybiBrO1xufTtcblxubmFjbC5ib3guYWZ0ZXIgPSBuYWNsLnNlY3JldGJveDtcblxubmFjbC5ib3gub3BlbiA9IGZ1bmN0aW9uKG1zZywgbm9uY2UsIHB1YmxpY0tleSwgc2VjcmV0S2V5KSB7XG4gIHZhciBrID0gbmFjbC5ib3guYmVmb3JlKHB1YmxpY0tleSwgc2VjcmV0S2V5KTtcbiAgcmV0dXJuIG5hY2wuc2VjcmV0Ym94Lm9wZW4obXNnLCBub25jZSwgayk7XG59O1xuXG5uYWNsLmJveC5vcGVuLmFmdGVyID0gbmFjbC5zZWNyZXRib3gub3BlbjtcblxubmFjbC5ib3gua2V5UGFpciA9IGZ1bmN0aW9uKCkge1xuICB2YXIgcGsgPSBuZXcgVWludDhBcnJheShjcnlwdG9fYm94X1BVQkxJQ0tFWUJZVEVTKTtcbiAgdmFyIHNrID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX2JveF9TRUNSRVRLRVlCWVRFUyk7XG4gIGNyeXB0b19ib3hfa2V5cGFpcihwaywgc2spO1xuICByZXR1cm4ge3B1YmxpY0tleTogcGssIHNlY3JldEtleTogc2t9O1xufTtcblxubmFjbC5ib3gua2V5UGFpci5mcm9tU2VjcmV0S2V5ID0gZnVuY3Rpb24oc2VjcmV0S2V5KSB7XG4gIGNoZWNrQXJyYXlUeXBlcyhzZWNyZXRLZXkpO1xuICBpZiAoc2VjcmV0S2V5Lmxlbmd0aCAhPT0gY3J5cHRvX2JveF9TRUNSRVRLRVlCWVRFUylcbiAgICB0aHJvdyBuZXcgRXJyb3IoJ2JhZCBzZWNyZXQga2V5IHNpemUnKTtcbiAgdmFyIHBrID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX2JveF9QVUJMSUNLRVlCWVRFUyk7XG4gIGNyeXB0b19zY2FsYXJtdWx0X2Jhc2UocGssIHNlY3JldEtleSk7XG4gIHJldHVybiB7cHVibGljS2V5OiBwaywgc2VjcmV0S2V5OiBuZXcgVWludDhBcnJheShzZWNyZXRLZXkpfTtcbn07XG5cbm5hY2wuYm94LnB1YmxpY0tleUxlbmd0aCA9IGNyeXB0b19ib3hfUFVCTElDS0VZQllURVM7XG5uYWNsLmJveC5zZWNyZXRLZXlMZW5ndGggPSBjcnlwdG9fYm94X1NFQ1JFVEtFWUJZVEVTO1xubmFjbC5ib3guc2hhcmVkS2V5TGVuZ3RoID0gY3J5cHRvX2JveF9CRUZPUkVOTUJZVEVTO1xubmFjbC5ib3gubm9uY2VMZW5ndGggPSBjcnlwdG9fYm94X05PTkNFQllURVM7XG5uYWNsLmJveC5vdmVyaGVhZExlbmd0aCA9IG5hY2wuc2VjcmV0Ym94Lm92ZXJoZWFkTGVuZ3RoO1xuXG5uYWNsLnNpZ24gPSBmdW5jdGlvbihtc2csIHNlY3JldEtleSkge1xuICBjaGVja0FycmF5VHlwZXMobXNnLCBzZWNyZXRLZXkpO1xuICBpZiAoc2VjcmV0S2V5Lmxlbmd0aCAhPT0gY3J5cHRvX3NpZ25fU0VDUkVUS0VZQllURVMpXG4gICAgdGhyb3cgbmV3IEVycm9yKCdiYWQgc2VjcmV0IGtleSBzaXplJyk7XG4gIHZhciBzaWduZWRNc2cgPSBuZXcgVWludDhBcnJheShjcnlwdG9fc2lnbl9CWVRFUyttc2cubGVuZ3RoKTtcbiAgY3J5cHRvX3NpZ24oc2lnbmVkTXNnLCBtc2csIG1zZy5sZW5ndGgsIHNlY3JldEtleSk7XG4gIHJldHVybiBzaWduZWRNc2c7XG59O1xuXG5uYWNsLnNpZ24ub3BlbiA9IGZ1bmN0aW9uKHNpZ25lZE1zZywgcHVibGljS2V5KSB7XG4gIGlmIChhcmd1bWVudHMubGVuZ3RoICE9PSAyKVxuICAgIHRocm93IG5ldyBFcnJvcignbmFjbC5zaWduLm9wZW4gYWNjZXB0cyAyIGFyZ3VtZW50czsgZGlkIHlvdSBtZWFuIHRvIHVzZSBuYWNsLnNpZ24uZGV0YWNoZWQudmVyaWZ5PycpO1xuICBjaGVja0FycmF5VHlwZXMoc2lnbmVkTXNnLCBwdWJsaWNLZXkpO1xuICBpZiAocHVibGljS2V5Lmxlbmd0aCAhPT0gY3J5cHRvX3NpZ25fUFVCTElDS0VZQllURVMpXG4gICAgdGhyb3cgbmV3IEVycm9yKCdiYWQgcHVibGljIGtleSBzaXplJyk7XG4gIHZhciB0bXAgPSBuZXcgVWludDhBcnJheShzaWduZWRNc2cubGVuZ3RoKTtcbiAgdmFyIG1sZW4gPSBjcnlwdG9fc2lnbl9vcGVuKHRtcCwgc2lnbmVkTXNnLCBzaWduZWRNc2cubGVuZ3RoLCBwdWJsaWNLZXkpO1xuICBpZiAobWxlbiA8IDApIHJldHVybiBudWxsO1xuICB2YXIgbSA9IG5ldyBVaW50OEFycmF5KG1sZW4pO1xuICBmb3IgKHZhciBpID0gMDsgaSA8IG0ubGVuZ3RoOyBpKyspIG1baV0gPSB0bXBbaV07XG4gIHJldHVybiBtO1xufTtcblxubmFjbC5zaWduLmRldGFjaGVkID0gZnVuY3Rpb24obXNnLCBzZWNyZXRLZXkpIHtcbiAgdmFyIHNpZ25lZE1zZyA9IG5hY2wuc2lnbihtc2csIHNlY3JldEtleSk7XG4gIHZhciBzaWcgPSBuZXcgVWludDhBcnJheShjcnlwdG9fc2lnbl9CWVRFUyk7XG4gIGZvciAodmFyIGkgPSAwOyBpIDwgc2lnLmxlbmd0aDsgaSsrKSBzaWdbaV0gPSBzaWduZWRNc2dbaV07XG4gIHJldHVybiBzaWc7XG59O1xuXG5uYWNsLnNpZ24uZGV0YWNoZWQudmVyaWZ5ID0gZnVuY3Rpb24obXNnLCBzaWcsIHB1YmxpY0tleSkge1xuICBjaGVja0FycmF5VHlwZXMobXNnLCBzaWcsIHB1YmxpY0tleSk7XG4gIGlmIChzaWcubGVuZ3RoICE9PSBjcnlwdG9fc2lnbl9CWVRFUylcbiAgICB0aHJvdyBuZXcgRXJyb3IoJ2JhZCBzaWduYXR1cmUgc2l6ZScpO1xuICBpZiAocHVibGljS2V5Lmxlbmd0aCAhPT0gY3J5cHRvX3NpZ25fUFVCTElDS0VZQllURVMpXG4gICAgdGhyb3cgbmV3IEVycm9yKCdiYWQgcHVibGljIGtleSBzaXplJyk7XG4gIHZhciBzbSA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zaWduX0JZVEVTICsgbXNnLmxlbmd0aCk7XG4gIHZhciBtID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NpZ25fQllURVMgKyBtc2cubGVuZ3RoKTtcbiAgdmFyIGk7XG4gIGZvciAoaSA9IDA7IGkgPCBjcnlwdG9fc2lnbl9CWVRFUzsgaSsrKSBzbVtpXSA9IHNpZ1tpXTtcbiAgZm9yIChpID0gMDsgaSA8IG1zZy5sZW5ndGg7IGkrKykgc21baStjcnlwdG9fc2lnbl9CWVRFU10gPSBtc2dbaV07XG4gIHJldHVybiAoY3J5cHRvX3NpZ25fb3BlbihtLCBzbSwgc20ubGVuZ3RoLCBwdWJsaWNLZXkpID49IDApO1xufTtcblxubmFjbC5zaWduLmtleVBhaXIgPSBmdW5jdGlvbigpIHtcbiAgdmFyIHBrID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NpZ25fUFVCTElDS0VZQllURVMpO1xuICB2YXIgc2sgPSBuZXcgVWludDhBcnJheShjcnlwdG9fc2lnbl9TRUNSRVRLRVlCWVRFUyk7XG4gIGNyeXB0b19zaWduX2tleXBhaXIocGssIHNrKTtcbiAgcmV0dXJuIHtwdWJsaWNLZXk6IHBrLCBzZWNyZXRLZXk6IHNrfTtcbn07XG5cbm5hY2wuc2lnbi5rZXlQYWlyLmZyb21TZWNyZXRLZXkgPSBmdW5jdGlvbihzZWNyZXRLZXkpIHtcbiAgY2hlY2tBcnJheVR5cGVzKHNlY3JldEtleSk7XG4gIGlmIChzZWNyZXRLZXkubGVuZ3RoICE9PSBjcnlwdG9fc2lnbl9TRUNSRVRLRVlCWVRFUylcbiAgICB0aHJvdyBuZXcgRXJyb3IoJ2JhZCBzZWNyZXQga2V5IHNpemUnKTtcbiAgdmFyIHBrID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NpZ25fUFVCTElDS0VZQllURVMpO1xuICBmb3IgKHZhciBpID0gMDsgaSA8IHBrLmxlbmd0aDsgaSsrKSBwa1tpXSA9IHNlY3JldEtleVszMitpXTtcbiAgcmV0dXJuIHtwdWJsaWNLZXk6IHBrLCBzZWNyZXRLZXk6IG5ldyBVaW50OEFycmF5KHNlY3JldEtleSl9O1xufTtcblxubmFjbC5zaWduLmtleVBhaXIuZnJvbVNlZWQgPSBmdW5jdGlvbihzZWVkKSB7XG4gIGNoZWNrQXJyYXlUeXBlcyhzZWVkKTtcbiAgaWYgKHNlZWQubGVuZ3RoICE9PSBjcnlwdG9fc2lnbl9TRUVEQllURVMpXG4gICAgdGhyb3cgbmV3IEVycm9yKCdiYWQgc2VlZCBzaXplJyk7XG4gIHZhciBwayA9IG5ldyBVaW50OEFycmF5KGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTKTtcbiAgdmFyIHNrID0gbmV3IFVpbnQ4QXJyYXkoY3J5cHRvX3NpZ25fU0VDUkVUS0VZQllURVMpO1xuICBmb3IgKHZhciBpID0gMDsgaSA8IDMyOyBpKyspIHNrW2ldID0gc2VlZFtpXTtcbiAgY3J5cHRvX3NpZ25fa2V5cGFpcihwaywgc2ssIHRydWUpO1xuICByZXR1cm4ge3B1YmxpY0tleTogcGssIHNlY3JldEtleTogc2t9O1xufTtcblxubmFjbC5zaWduLnB1YmxpY0tleUxlbmd0aCA9IGNyeXB0b19zaWduX1BVQkxJQ0tFWUJZVEVTO1xubmFjbC5zaWduLnNlY3JldEtleUxlbmd0aCA9IGNyeXB0b19zaWduX1NFQ1JFVEtFWUJZVEVTO1xubmFjbC5zaWduLnNlZWRMZW5ndGggPSBjcnlwdG9fc2lnbl9TRUVEQllURVM7XG5uYWNsLnNpZ24uc2lnbmF0dXJlTGVuZ3RoID0gY3J5cHRvX3NpZ25fQllURVM7XG5cbm5hY2wuaGFzaCA9IGZ1bmN0aW9uKG1zZykge1xuICBjaGVja0FycmF5VHlwZXMobXNnKTtcbiAgdmFyIGggPSBuZXcgVWludDhBcnJheShjcnlwdG9faGFzaF9CWVRFUyk7XG4gIGNyeXB0b19oYXNoKGgsIG1zZywgbXNnLmxlbmd0aCk7XG4gIHJldHVybiBoO1xufTtcblxubmFjbC5oYXNoLmhhc2hMZW5ndGggPSBjcnlwdG9faGFzaF9CWVRFUztcblxubmFjbC52ZXJpZnkgPSBmdW5jdGlvbih4LCB5KSB7XG4gIGNoZWNrQXJyYXlUeXBlcyh4LCB5KTtcbiAgLy8gWmVybyBsZW5ndGggYXJndW1lbnRzIGFyZSBjb25zaWRlcmVkIG5vdCBlcXVhbC5cbiAgaWYgKHgubGVuZ3RoID09PSAwIHx8IHkubGVuZ3RoID09PSAwKSByZXR1cm4gZmFsc2U7XG4gIGlmICh4Lmxlbmd0aCAhPT0geS5sZW5ndGgpIHJldHVybiBmYWxzZTtcbiAgcmV0dXJuICh2bih4LCAwLCB5LCAwLCB4Lmxlbmd0aCkgPT09IDApID8gdHJ1ZSA6IGZhbHNlO1xufTtcblxubmFjbC5zZXRQUk5HID0gZnVuY3Rpb24oZm4pIHtcbiAgcmFuZG9tYnl0ZXMgPSBmbjtcbn07XG5cbihmdW5jdGlvbigpIHtcbiAgLy8gSW5pdGlhbGl6ZSBQUk5HIGlmIGVudmlyb25tZW50IHByb3ZpZGVzIENTUFJORy5cbiAgLy8gSWYgbm90LCBtZXRob2RzIGNhbGxpbmcgcmFuZG9tYnl0ZXMgd2lsbCB0aHJvdy5cbiAgdmFyIGNyeXB0byA9IHR5cGVvZiBzZWxmICE9PSAndW5kZWZpbmVkJyA/IChzZWxmLmNyeXB0byB8fCBzZWxmLm1zQ3J5cHRvKSA6IG51bGw7XG4gIGlmIChjcnlwdG8gJiYgY3J5cHRvLmdldFJhbmRvbVZhbHVlcykge1xuICAgIC8vIEJyb3dzZXJzLlxuICAgIHZhciBRVU9UQSA9IDY1NTM2O1xuICAgIG5hY2wuc2V0UFJORyhmdW5jdGlvbih4LCBuKSB7XG4gICAgICB2YXIgaSwgdiA9IG5ldyBVaW50OEFycmF5KG4pO1xuICAgICAgZm9yIChpID0gMDsgaSA8IG47IGkgKz0gUVVPVEEpIHtcbiAgICAgICAgY3J5cHRvLmdldFJhbmRvbVZhbHVlcyh2LnN1YmFycmF5KGksIGkgKyBNYXRoLm1pbihuIC0gaSwgUVVPVEEpKSk7XG4gICAgICB9XG4gICAgICBmb3IgKGkgPSAwOyBpIDwgbjsgaSsrKSB4W2ldID0gdltpXTtcbiAgICAgIGNsZWFudXAodik7XG4gICAgfSk7XG4gIH0gZWxzZSBpZiAodHlwZW9mIHJlcXVpcmUgIT09ICd1bmRlZmluZWQnKSB7XG4gICAgLy8gTm9kZS5qcy5cbiAgICBjcnlwdG8gPSByZXF1aXJlKCdjcnlwdG8nKTtcbiAgICBpZiAoY3J5cHRvICYmIGNyeXB0by5yYW5kb21CeXRlcykge1xuICAgICAgbmFjbC5zZXRQUk5HKGZ1bmN0aW9uKHgsIG4pIHtcbiAgICAgICAgdmFyIGksIHYgPSBjcnlwdG8ucmFuZG9tQnl0ZXMobik7XG4gICAgICAgIGZvciAoaSA9IDA7IGkgPCBuOyBpKyspIHhbaV0gPSB2W2ldO1xuICAgICAgICBjbGVhbnVwKHYpO1xuICAgICAgfSk7XG4gICAgfVxuICB9XG59KSgpO1xuXG59KSh0eXBlb2YgbW9kdWxlICE9PSAndW5kZWZpbmVkJyAmJiBtb2R1bGUuZXhwb3J0cyA/IG1vZHVsZS5leHBvcnRzIDogKHNlbGYubmFjbCA9IHNlbGYubmFjbCB8fCB7fSkpO1xuIiwidmFyIEV4b251bSA9IHJlcXVpcmUoJy4vaW5kZXguanMnKTtcblxud2luZG93LkV4b251bSA9IHdpbmRvdy5FeG9udW0gfHwgRXhvbnVtO1xuIiwiXCJ1c2Ugc3RyaWN0XCI7XG5cbnZhciBzaGEgPSByZXF1aXJlKCdzaGEuanMnKTtcbnZhciBuYWNsID0gcmVxdWlyZSgndHdlZXRuYWNsJyk7XG52YXIgb2JqZWN0QXNzaWduID0gcmVxdWlyZSgnb2JqZWN0LWFzc2lnbicpO1xudmFyIGZzID0gcmVxdWlyZSgnZnMnKTtcbnZhciBiaWdJbnQgPSByZXF1aXJlKFwiYmlnLWludGVnZXJcIik7XG5cbnZhciBFeG9udW1DbGllbnQgPSAoZnVuY3Rpb24oKSB7XG5cbiAgICB2YXIgQ09OU1QgPSB7XG4gICAgICAgIE1JTl9JTlQ4OiAtMTI4LFxuICAgICAgICBNQVhfSU5UODogMTI3LFxuICAgICAgICBNSU5fSU5UMTY6IC0zMjc2OCxcbiAgICAgICAgTUFYX0lOVDE2OiAzMjc2NyxcbiAgICAgICAgTUlOX0lOVDMyOiAtMjE0NzQ4MzY0OCxcbiAgICAgICAgTUFYX0lOVDMyOiAyMTQ3NDgzNjQ3LFxuICAgICAgICBNSU5fSU5UNjQ6ICctOTIyMzM3MjAzNjg1NDc3NTgwOCcsXG4gICAgICAgIE1BWF9JTlQ2NDogJzkyMjMzNzIwMzY4NTQ3NzU4MDcnLFxuICAgICAgICBNQVhfVUlOVDg6IDI1NSxcbiAgICAgICAgTUFYX1VJTlQxNjogNjU1MzUsXG4gICAgICAgIE1BWF9VSU5UMzI6IDQyOTQ5NjcyOTUsXG4gICAgICAgIE1BWF9VSU5UNjQ6ICcxODQ0Njc0NDA3MzcwOTU1MTYxNScsXG4gICAgICAgIE1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIOiAzMixcbiAgICAgICAgU0lHTkFUVVJFX0xFTkdUSDogNjQsXG4gICAgICAgIE5FVFdPUktfSUQ6IDBcbiAgICB9O1xuICAgIHZhciBEQktleSA9IGNyZWF0ZU5ld1R5cGUoe1xuICAgICAgICBzaXplOiAzNCxcbiAgICAgICAgZmllbGRzOiB7XG4gICAgICAgICAgICB2YXJpYW50OiB7dHlwZTogVWludDgsIHNpemU6IDEsIGZyb206IDAsIHRvOiAxfSxcbiAgICAgICAgICAgIGtleToge3R5cGU6IEhhc2gsIHNpemU6IDMyLCBmcm9tOiAxLCB0bzogMzN9LFxuICAgICAgICAgICAgbGVuZ3RoOiB7dHlwZTogVWludDgsIHNpemU6IDEsIGZyb206IDMzLCB0bzogMzR9XG4gICAgICAgIH1cbiAgICB9KTtcbiAgICB2YXIgQnJhbmNoID0gY3JlYXRlTmV3VHlwZSh7XG4gICAgICAgIHNpemU6IDEzMixcbiAgICAgICAgZmllbGRzOiB7XG4gICAgICAgICAgICBsZWZ0X2hhc2g6IHt0eXBlOiBIYXNoLCBzaXplOiAzMiwgZnJvbTogMCwgdG86IDMyfSxcbiAgICAgICAgICAgIHJpZ2h0X2hhc2g6IHt0eXBlOiBIYXNoLCBzaXplOiAzMiwgZnJvbTogMzIsIHRvOiA2NH0sXG4gICAgICAgICAgICBsZWZ0X2tleToge3R5cGU6IERCS2V5LCBzaXplOiAzNCwgZnJvbTogNjQsIHRvOiA5OH0sXG4gICAgICAgICAgICByaWdodF9rZXk6IHt0eXBlOiBEQktleSwgc2l6ZTogMzQsIGZyb206IDk4LCB0bzogMTMyfVxuICAgICAgICB9XG4gICAgfSk7XG4gICAgdmFyIFJvb3RCcmFuY2ggPSBjcmVhdGVOZXdUeXBlKHtcbiAgICAgICAgc2l6ZTogNjYsXG4gICAgICAgIGZpZWxkczoge1xuICAgICAgICAgICAga2V5OiB7dHlwZTogREJLZXksIHNpemU6IDM0LCBmcm9tOiAwLCB0bzogMzR9LFxuICAgICAgICAgICAgaGFzaDoge3R5cGU6IEhhc2gsIHNpemU6IDMyLCBmcm9tOiAzNCwgdG86IDY2fVxuICAgICAgICB9XG4gICAgfSk7XG4gICAgdmFyIEJsb2NrID0gY3JlYXRlTmV3VHlwZSh7XG4gICAgICAgIHNpemU6IDExNixcbiAgICAgICAgZmllbGRzOiB7XG4gICAgICAgICAgICBoZWlnaHQ6IHt0eXBlOiBVaW50NjQsIHNpemU6IDgsIGZyb206IDAsIHRvOiA4fSxcbiAgICAgICAgICAgIHByb3Bvc2Vfcm91bmQ6IHt0eXBlOiBVaW50MzIsIHNpemU6IDQsIGZyb206IDgsIHRvOiAxMn0sXG4gICAgICAgICAgICB0aW1lOiB7dHlwZTogVGltZXNwZWMsIHNpemU6IDgsIGZyb206IDEyLCB0bzogMjB9LFxuICAgICAgICAgICAgcHJldl9oYXNoOiB7dHlwZTogSGFzaCwgc2l6ZTogMzIsIGZyb206IDIwLCB0bzogNTJ9LFxuICAgICAgICAgICAgdHhfaGFzaDoge3R5cGU6IEhhc2gsIHNpemU6IDMyLCBmcm9tOiA1MiwgdG86IDg0fSxcbiAgICAgICAgICAgIHN0YXRlX2hhc2g6IHt0eXBlOiBIYXNoLCBzaXplOiAzMiwgZnJvbTogODQsIHRvOiAxMTZ9XG4gICAgICAgIH1cbiAgICB9KTtcbiAgICB2YXIgTWVzc2FnZUhlYWQgPSBjcmVhdGVOZXdUeXBlKHtcbiAgICAgICAgc2l6ZTogMTAsXG4gICAgICAgIGZpZWxkczoge1xuICAgICAgICAgICAgbmV0d29ya19pZDoge3R5cGU6IFVpbnQ4LCBzaXplOiAxLCBmcm9tOiAwLCB0bzogMX0sXG4gICAgICAgICAgICB2ZXJzaW9uOiB7dHlwZTogVWludDgsIHNpemU6IDEsIGZyb206IDEsIHRvOiAyfSxcbiAgICAgICAgICAgIG1lc3NhZ2VfdHlwZToge3R5cGU6IFVpbnQxNiwgc2l6ZTogMiwgZnJvbTogMiwgdG86IDR9LFxuICAgICAgICAgICAgc2VydmljZV9pZDoge3R5cGU6IFVpbnQxNiwgc2l6ZTogMiwgZnJvbTogNCwgdG86IDZ9LFxuICAgICAgICAgICAgcGF5bG9hZDoge3R5cGU6IFVpbnQzMiwgc2l6ZTogNCwgZnJvbTogNiwgdG86IDEwfVxuICAgICAgICB9XG4gICAgfSk7XG4gICAgdmFyIFByZWNvbW1pdCA9IGNyZWF0ZU5ld01lc3NhZ2Uoe1xuICAgICAgICBzaXplOiA4NCxcbiAgICAgICAgc2VydmljZV9pZDogMCxcbiAgICAgICAgbWVzc2FnZV90eXBlOiA0LFxuICAgICAgICBmaWVsZHM6IHtcbiAgICAgICAgICAgIHZhbGlkYXRvcjoge3R5cGU6IFVpbnQzMiwgc2l6ZTogNCwgZnJvbTogMCwgdG86IDR9LFxuICAgICAgICAgICAgaGVpZ2h0OiB7dHlwZTogVWludDY0LCBzaXplOiA4LCBmcm9tOiA4LCB0bzogMTZ9LFxuICAgICAgICAgICAgcm91bmQ6IHt0eXBlOiBVaW50MzIsIHNpemU6IDQsIGZyb206IDE2LCB0bzogMjB9LFxuICAgICAgICAgICAgcHJvcG9zZV9oYXNoOiB7dHlwZTogSGFzaCwgc2l6ZTogMzIsIGZyb206IDIwLCB0bzogNTJ9LFxuICAgICAgICAgICAgYmxvY2tfaGFzaDoge3R5cGU6IEhhc2gsIHNpemU6IDMyLCBmcm9tOiA1MiwgdG86IDg0fVxuICAgICAgICB9XG4gICAgfSk7XG5cbiAgICAvKipcbiAgICAgKiBAY29uc3RydWN0b3JcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gdHlwZVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIE5ld1R5cGUodHlwZSkge1xuICAgICAgICB0aGlzLnNpemUgPSB0eXBlLnNpemU7XG4gICAgICAgIHRoaXMuZmllbGRzID0gdHlwZS5maWVsZHM7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gbWV0aG9kIHRvIHNlcmlhbGl6ZSBkYXRhIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnNcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gZGF0YVxuICAgICAqIEByZXR1cm5zIHtBcnJheX1cbiAgICAgKi9cbiAgICBOZXdUeXBlLnByb3RvdHlwZS5zZXJpYWxpemUgPSBmdW5jdGlvbihkYXRhKSB7XG4gICAgICAgIHJldHVybiBzZXJpYWxpemUoW10sIDAsIGRhdGEsIHRoaXMpO1xuICAgIH07XG5cbiAgICAvKipcbiAgICAgKiBDcmVhdGUgZWxlbWVudCBvZiBOZXdUeXBlIGNsYXNzXG4gICAgICogQHBhcmFtIHtPYmplY3R9IHR5cGVcbiAgICAgKiBAcmV0dXJucyB7TmV3VHlwZX1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBjcmVhdGVOZXdUeXBlKHR5cGUpIHtcbiAgICAgICAgcmV0dXJuIG5ldyBOZXdUeXBlKHR5cGUpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEBjb25zdHJ1Y3RvclxuICAgICAqIEBwYXJhbSB7T2JqZWN0fSB0eXBlXG4gICAgICovXG4gICAgZnVuY3Rpb24gTmV3TWVzc2FnZSh0eXBlKSB7XG4gICAgICAgIHRoaXMuc2l6ZSA9IHR5cGUuc2l6ZTtcbiAgICAgICAgdGhpcy5tZXNzYWdlX3R5cGUgPSB0eXBlLm1lc3NhZ2VfdHlwZTtcbiAgICAgICAgdGhpcy5zZXJ2aWNlX2lkID0gdHlwZS5zZXJ2aWNlX2lkO1xuICAgICAgICB0aGlzLnNpZ25hdHVyZSA9IHR5cGUuc2lnbmF0dXJlO1xuICAgICAgICB0aGlzLmZpZWxkcyA9IHR5cGUuZmllbGRzO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIG1ldGhvZCB0byBzZXJpYWxpemUgZGF0YSBpbnRvIGFycmF5IG9mIDgtYml0IGludGVnZXJzXG4gICAgICogQHBhcmFtIHtPYmplY3R9IGRhdGFcbiAgICAgKiBAcGFyYW0ge0Jvb2xlYW59IGN1dFNpZ25hdHVyZVxuICAgICAqIEByZXR1cm5zIHtBcnJheX1cbiAgICAgKi9cbiAgICBOZXdNZXNzYWdlLnByb3RvdHlwZS5zZXJpYWxpemUgPSBmdW5jdGlvbihkYXRhLCBjdXRTaWduYXR1cmUpIHtcbiAgICAgICAgdmFyIGJ1ZmZlciA9IE1lc3NhZ2VIZWFkLnNlcmlhbGl6ZSh7XG4gICAgICAgICAgICBuZXR3b3JrX2lkOiBDT05TVC5ORVRXT1JLX0lELFxuICAgICAgICAgICAgdmVyc2lvbjogMCxcbiAgICAgICAgICAgIG1lc3NhZ2VfdHlwZTogdGhpcy5tZXNzYWdlX3R5cGUsXG4gICAgICAgICAgICBzZXJ2aWNlX2lkOiB0aGlzLnNlcnZpY2VfaWRcbiAgICAgICAgfSk7XG5cbiAgICAgICAgLy8gc2VyaWFsaXplIGFuZCBhcHBlbmQgbWVzc2FnZSBib2R5XG4gICAgICAgIGJ1ZmZlciA9IHNlcmlhbGl6ZShidWZmZXIsIE1lc3NhZ2VIZWFkLnNpemUsIGRhdGEsIHRoaXMpO1xuICAgICAgICBpZiAodHlwZW9mIGJ1ZmZlciA9PT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIC8vIGNhbGN1bGF0ZSBwYXlsb2FkIGFuZCBpbnNlcnQgaXQgaW50byBidWZmZXJcbiAgICAgICAgVWludDMyKGJ1ZmZlci5sZW5ndGggKyBDT05TVC5TSUdOQVRVUkVfTEVOR1RILCBidWZmZXIsIE1lc3NhZ2VIZWFkLmZpZWxkcy5wYXlsb2FkLmZyb20sIE1lc3NhZ2VIZWFkLmZpZWxkcy5wYXlsb2FkLnRvKTtcblxuICAgICAgICBpZiAoY3V0U2lnbmF0dXJlICE9PSB0cnVlKSB7XG4gICAgICAgICAgICAvLyBhcHBlbmQgc2lnbmF0dXJlXG4gICAgICAgICAgICBEaWdlc3QodGhpcy5zaWduYXR1cmUsIGJ1ZmZlciwgYnVmZmVyLmxlbmd0aCwgYnVmZmVyLmxlbmd0aCArIENPTlNULlNJR05BVFVSRV9MRU5HVEgpO1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9O1xuXG4gICAgLyoqXG4gICAgICogQ3JlYXRlIGVsZW1lbnQgb2YgTmV3TWVzc2FnZSBjbGFzc1xuICAgICAqIEBwYXJhbSB7T2JqZWN0fSB0eXBlXG4gICAgICogQHJldHVybnMge05ld01lc3NhZ2V9XG4gICAgICovXG4gICAgZnVuY3Rpb24gY3JlYXRlTmV3TWVzc2FnZSh0eXBlKSB7XG4gICAgICAgIHJldHVybiBuZXcgTmV3TWVzc2FnZSh0eXBlKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge051bWJlciB8fCBTdHJpbmd9IHZhbHVlXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBJbnQ4KHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIGlmICghdmFsaWRhdGVJbnRlZ2VyKHZhbHVlLCBDT05TVC5NSU5fSU5UOCwgQ09OU1QuTUFYX0lOVDgsIGZyb20sIHRvLCAxKSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaWYgKHZhbHVlIDwgMCkge1xuICAgICAgICAgICAgdmFsdWUgPSBDT05TVC5NQVhfVUlOVDggKyB2YWx1ZSArIDFcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEludGVnZXJUb0J5dGVBcnJheSh2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0byk7XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge051bWJlciB8fCBTdHJpbmd9IHZhbHVlXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBJbnQxNih2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAoIXZhbGlkYXRlSW50ZWdlcih2YWx1ZSwgQ09OU1QuTUlOX0lOVDE2LCBDT05TVC5NQVhfSU5UMTYsIGZyb20sIHRvLCAyKSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaWYgKHZhbHVlIDwgMCkge1xuICAgICAgICAgICAgdmFsdWUgPSBDT05TVC5NQVhfVUlOVDE2ICsgdmFsdWUgKyAxO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0SW50ZWdlclRvQnl0ZUFycmF5KHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKTtcblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIGRhdGEgdHlwZVxuICAgICAqIEBwYXJhbSB7TnVtYmVyIHx8IFN0cmluZ30gdmFsdWVcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIEludDMyKHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIGlmICghdmFsaWRhdGVJbnRlZ2VyKHZhbHVlLCBDT05TVC5NSU5fSU5UMzIsIENPTlNULk1BWF9JTlQzMiwgZnJvbSwgdG8sIDQpKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICBpZiAodmFsdWUgPCAwKSB7XG4gICAgICAgICAgICB2YWx1ZSA9IENPTlNULk1BWF9VSU5UMzIgKyB2YWx1ZSArIDE7XG4gICAgICAgIH1cblxuICAgICAgICBpbnNlcnRJbnRlZ2VyVG9CeXRlQXJyYXkodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtOdW1iZXIgfHwgU3RyaW5nfSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gSW50NjQodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgdmFyIHZhbCA9IHZhbGlkYXRlQmlnSW50ZWdlcih2YWx1ZSwgQ09OU1QuTUlOX0lOVDY0LCBDT05TVC5NQVhfSU5UNjQsIGZyb20sIHRvLCA4KTtcblxuICAgICAgICBpZiAodmFsID09PSBmYWxzZSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9IGVsc2UgaWYgKCFiaWdJbnQuaXNJbnN0YW5jZSh2YWwpKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICBpZiAodmFsLmlzTmVnYXRpdmUoKSkge1xuICAgICAgICAgICAgdmFsID0gYmlnSW50KENPTlNULk1BWF9VSU5UNjQpLnBsdXMoMSkucGx1cyh2YWwpO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0QmlnSW50ZWdlclRvQnl0ZUFycmF5KHZhbCwgYnVmZmVyLCBmcm9tLCB0byk7XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge051bWJlciB8fCBTdHJpbmd9IHZhbHVlXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBVaW50OCh2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAoIXZhbGlkYXRlSW50ZWdlcih2YWx1ZSwgMCwgQ09OU1QuTUFYX1VJTlQ4LCBmcm9tLCB0bywgMSkpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEludGVnZXJUb0J5dGVBcnJheSh2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0byk7XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge051bWJlciB8fCBTdHJpbmd9IHZhbHVlXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBVaW50MTYodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgaWYgKCF2YWxpZGF0ZUludGVnZXIodmFsdWUsIDAsIENPTlNULk1BWF9VSU5UMTYsIGZyb20sIHRvLCAyKSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0SW50ZWdlclRvQnl0ZUFycmF5KHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKTtcblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIGRhdGEgdHlwZVxuICAgICAqIEBwYXJhbSB7TnVtYmVyIHx8IFN0cmluZ30gdmFsdWVcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIFVpbnQzMih2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAoIXZhbGlkYXRlSW50ZWdlcih2YWx1ZSwgMCwgQ09OU1QuTUFYX1VJTlQzMiwgZnJvbSwgdG8sIDQpKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICBpbnNlcnRJbnRlZ2VyVG9CeXRlQXJyYXkodmFsdWUsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtOdW1iZXIgfHwgU3RyaW5nfSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gVWludDY0KHZhbHVlLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIHZhciB2YWwgPSB2YWxpZGF0ZUJpZ0ludGVnZXIodmFsdWUsIDAsIENPTlNULk1BWF9VSU5UNjQsIGZyb20sIHRvLCA4KTtcblxuICAgICAgICBpZiAodmFsID09PSBmYWxzZSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9IGVsc2UgaWYgKCFiaWdJbnQuaXNJbnN0YW5jZSh2YWwpKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICBpbnNlcnRCaWdJbnRlZ2VyVG9CeXRlQXJyYXkodmFsLCBidWZmZXIsIGZyb20sIHRvKTtcblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEJ1aWx0LWluIGRhdGEgdHlwZVxuICAgICAqIEBwYXJhbSB7U3RyaW5nfSBzdHJpbmdcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIFN0cmluZyhzdHJpbmcsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgaWYgKHR5cGVvZiBzdHJpbmcgIT09ICdzdHJpbmcnKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBkYXRhIHR5cGUgaXMgcGFzc2VkIGFzIFN0cmluZy4gU3RyaW5nIGlzIHJlcXVpcmVkJyk7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH0gZWxzZSBpZiAoKHRvIC0gZnJvbSkgIT09IDgpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1N0cmluZyBzZWdtZW50IGlzIG9mIHdyb25nIGxlbmd0aC4gOCBieXRlcyBsb25nIGlzIHJlcXVpcmVkIHRvIHN0b3JlIHRyYW5zbWl0dGVkIHZhbHVlLicpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgdmFyIGJ1ZmZlckxlbmd0aCA9IGJ1ZmZlci5sZW5ndGg7XG4gICAgICAgIFVpbnQzMihidWZmZXJMZW5ndGgsIGJ1ZmZlciwgZnJvbSwgZnJvbSArIDQpOyAvLyBpbmRleCB3aGVyZSBzdHJpbmcgY29udGVudCBzdGFydHMgaW4gYnVmZmVyXG4gICAgICAgIGluc2VydFN0cmluZ1RvQnl0ZUFycmF5KHN0cmluZywgYnVmZmVyLCBidWZmZXJMZW5ndGgpOyAvLyBzdHJpbmcgY29udGVudFxuICAgICAgICBVaW50MzIoYnVmZmVyLmxlbmd0aCAtIGJ1ZmZlckxlbmd0aCwgYnVmZmVyLCBmcm9tICsgNCwgZnJvbSArIDgpOyAvLyBzdHJpbmcgbGVuZ3RoXG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gaGFzaFxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gSGFzaChoYXNoLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKGhhc2gpKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH0gZWxzZSBpZiAoKHRvIC0gZnJvbSkgIT09IDMyKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdIYXNoIHNlZ21lbnQgaXMgb2Ygd3JvbmcgbGVuZ3RoLiAzMiBieXRlcyBsb25nIGlzIHJlcXVpcmVkIHRvIHN0b3JlIHRyYW5zbWl0dGVkIHZhbHVlLicpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0SGV4YWRlY2ltYWxUb0FycmF5KGhhc2gsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IGRpZ2VzdFxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gRGlnZXN0KGRpZ2VzdCwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAoIXZhbGlkYXRlSGV4SGFzaChkaWdlc3QsIDY0KSkge1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9IGVsc2UgaWYgKCh0byAtIGZyb20pICE9PSA2NCkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignRGlnZXN0IHNlZ21lbnQgaXMgb2Ygd3JvbmcgbGVuZ3RoLiA2NCBieXRlcyBsb25nIGlzIHJlcXVpcmVkIHRvIHN0b3JlIHRyYW5zbWl0dGVkIHZhbHVlLicpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgaW5zZXJ0SGV4YWRlY2ltYWxUb0FycmF5KGRpZ2VzdCwgYnVmZmVyLCBmcm9tLCB0byk7XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBCdWlsdC1pbiBkYXRhIHR5cGVcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gcHVibGljS2V5XG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBQdWJsaWNLZXkocHVibGljS2V5LCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKHB1YmxpY0tleSkpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfSBlbHNlIGlmICgodG8gLSBmcm9tKSAhPT0gMzIpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1B1YmxpY0tleSBzZWdtZW50IGlzIG9mIHdyb25nIGxlbmd0aC4gMzIgYnl0ZXMgbG9uZyBpcyByZXF1aXJlZCB0byBzdG9yZSB0cmFuc21pdHRlZCB2YWx1ZS4nKTtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEhleGFkZWNpbWFsVG9BcnJheShwdWJsaWNLZXksIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtOdW1iZXIgfHwgU3RyaW5nfSBuYW5vc2Vjb25kc1xuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gVGltZXNwZWMobmFub3NlY29uZHMsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgdmFyIHZhbCA9IHZhbGlkYXRlQmlnSW50ZWdlcihuYW5vc2Vjb25kcywgMCwgQ09OU1QuTUFYX1VJTlQ2NCwgZnJvbSwgdG8sIDgpO1xuXG4gICAgICAgIGlmICh2YWwgPT09IGZhbHNlKSB7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH0gZWxzZSBpZiAoIWJpZ0ludC5pc0luc3RhbmNlKHZhbCkpIHtcbiAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgfVxuXG4gICAgICAgIGluc2VydEJpZ0ludGVnZXJUb0J5dGVBcnJheSh2YWwsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuXG4gICAgICAgIHJldHVybiBidWZmZXI7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQnVpbHQtaW4gZGF0YSB0eXBlXG4gICAgICogQHBhcmFtIHtib29sZWFufSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gQm9vbCh2YWx1ZSwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBpZiAodHlwZW9mIHZhbHVlICE9PSAnYm9vbGVhbicpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIGRhdGEgdHlwZSBpcyBwYXNzZWQgYXMgQm9vbGVhbi4gQm9vbGVhbiBpcyByZXF1aXJlZCcpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9IGVsc2UgaWYgKCh0byAtIGZyb20pICE9PSAxKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdCb29sIHNlZ21lbnQgaXMgb2Ygd3JvbmcgbGVuZ3RoLiAxIGJ5dGVzIGxvbmcgaXMgcmVxdWlyZWQgdG8gc3RvcmUgdHJhbnNtaXR0ZWQgdmFsdWUuJyk7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH1cblxuICAgICAgICBpbnNlcnRJbnRlZ2VyVG9CeXRlQXJyYXkodmFsdWUgPyAxIDogMCwgYnVmZmVyLCBmcm9tLCB0byk7XG5cbiAgICAgICAgcmV0dXJuIGJ1ZmZlcjtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDaGVjayBpbnRlZ2VyIG51bWJlciB2YWxpZGl0eVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBtaW5cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gbWF4XG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gbGVuZ3RoXG4gICAgICogQHJldHVybnMge0Jvb2xlYW59XG4gICAgICovXG4gICAgZnVuY3Rpb24gdmFsaWRhdGVJbnRlZ2VyKHZhbHVlLCBtaW4sIG1heCwgZnJvbSwgdG8sIGxlbmd0aCkge1xuICAgICAgICBpZiAodHlwZW9mIHZhbHVlICE9PSAnbnVtYmVyJykge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgZGF0YSB0eXBlIGlzIHBhc3NlZCBhcyBudW1iZXIuIFNob3VsZCBiZSBvZiB0eXBlIE51bWJlci4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfSBlbHNlIGlmICh2YWx1ZSA8IG1pbikge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignTnVtYmVyIHNob3VsZCBiZSBtb3JlIG9yIGVxdWFsIHRvICcgKyBtaW4gKyAnLicpO1xuICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICB9IGVsc2UgaWYgKHZhbHVlID4gbWF4KSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdOdW1iZXIgc2hvdWxkIGJlIGxlc3Mgb3IgZXF1YWwgdG8gJyArIG1heCArICcuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH0gZWxzZSBpZiAoKHRvIC0gZnJvbSkgIT09IGxlbmd0aCkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignTnVtYmVyIHNlZ21lbnQgaXMgb2Ygd3JvbmcgbGVuZ3RoLiAnICsgbGVuZ3RoICsgJyBieXRlcyBsb25nIGlzIHJlcXVpcmVkIHRvIHN0b3JlIHRyYW5zbWl0dGVkIHZhbHVlLicpO1xuICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIHRydWU7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ2hlY2sgYml0IGludGVnZXIgbnVtYmVyIHZhbGlkaXR5XG4gICAgICogQHBhcmFtIHtOdW1iZXIgfHwgU3RyaW5nfSB2YWx1ZVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBtaW5cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gbWF4XG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gbGVuZ3RoXG4gICAgICogQHJldHVybnMge2JpZ0ludCB8fCBCb29sZWFufVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIHZhbGlkYXRlQmlnSW50ZWdlcih2YWx1ZSwgbWluLCBtYXgsIGZyb20sIHRvLCBsZW5ndGgpIHtcbiAgICAgICAgdmFyIHZhbDtcblxuICAgICAgICBpZiAoISh0eXBlb2YgdmFsdWUgPT09ICdudW1iZXInIHx8IHR5cGVvZiB2YWx1ZSA9PT0gJ3N0cmluZycpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBkYXRhIHR5cGUgaXMgcGFzc2VkIGFzIG51bWJlci4gU2hvdWxkIGJlIG9mIHR5cGUgTnVtYmVyIG9yIFN0cmluZy4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfSBlbHNlIGlmICgodG8gLSBmcm9tKSAhPT0gbGVuZ3RoKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdOdW1iZXIgc2VnbWVudCBpcyBvZiB3cm9uZyBsZW5ndGguICcgKyBsZW5ndGggKyAnIGJ5dGVzIGxvbmcgaXMgcmVxdWlyZWQgdG8gc3RvcmUgdHJhbnNtaXR0ZWQgdmFsdWUuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH1cblxuICAgICAgICB0cnkge1xuICAgICAgICAgICAgdmFsID0gYmlnSW50KHZhbHVlKTtcblxuICAgICAgICAgICAgaWYgKHZhbC5sdChtaW4pKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignTnVtYmVyIHNob3VsZCBiZSBtb3JlIG9yIGVxdWFsIHRvICcgKyBtaW4gKyAnLicpO1xuICAgICAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgICAgIH0gZWxzZSBpZiAodmFsLmd0KG1heCkpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdOdW1iZXIgc2hvdWxkIGJlIGxlc3Mgb3IgZXF1YWwgdG8gJyArIG1heCArICcuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICByZXR1cm4gdmFsO1xuICAgICAgICB9IGNhdGNoIChlKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBkYXRhIHR5cGUgaXMgcGFzc2VkIGFzIG51bWJlci4gU2hvdWxkIGJlIG9mIHR5cGUgTnVtYmVyIG9yIFN0cmluZy4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfVxuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENoZWNrIGhhc2ggdmFsaWRpdHlcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gaGFzaFxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBieXRlc1xuICAgICAqIEByZXR1cm5zIHtCb29sZWFufVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIHZhbGlkYXRlSGV4SGFzaChoYXNoLCBieXRlcykge1xuICAgICAgICB2YXIgYnl0ZXMgPSBieXRlcyB8fCAzMjtcblxuICAgICAgICBpZiAodHlwZW9mIGhhc2ggIT09ICdzdHJpbmcnKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBkYXRhIHR5cGUgaXMgcGFzc2VkIGFzIGhleGFkZWNpbWFsIHN0cmluZy4gU3RyaW5nIGlzIHJlcXVpcmVkJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH0gZWxzZSBpZiAoaGFzaC5sZW5ndGggIT09IGJ5dGVzICogMikge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSGV4YWRlY2ltYWwgc3RyaW5nIGlzIG9mIHdyb25nIGxlbmd0aC4gJyArIGJ5dGVzICogMiArICcgY2hhciBzeW1ib2xzIGxvbmcgaXMgcmVxdWlyZWQuICcgKyBoYXNoLmxlbmd0aCArICcgaXMgcGFzc2VkLicpO1xuICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICB9XG5cbiAgICAgICAgZm9yICh2YXIgaSA9IDAsIGxlbiA9IGhhc2gubGVuZ3RoOyBpIDwgbGVuOyBpKyspIHtcbiAgICAgICAgICAgIGlmIChpc05hTihwYXJzZUludChoYXNoW2ldLCAxNikpKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBzeW1ib2wgaW4gaGV4YWRlY2ltYWwgc3RyaW5nLicpO1xuICAgICAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiB0cnVlO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENoZWNrIGFycmF5IG9mIDgtYml0IGludGVnZXJzIHZhbGlkaXR5XG4gICAgICogQHBhcmFtIHtBcnJheX0gYXJyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGJ5dGVzXG4gICAgICogQHJldHVybnMge0Jvb2xlYW59XG4gICAgICovXG4gICAgZnVuY3Rpb24gdmFsaWRhdGVCeXRlc0FycmF5KGFyciwgYnl0ZXMpIHtcbiAgICAgICAgaWYgKGJ5dGVzICYmIGFyci5sZW5ndGggIT09IGJ5dGVzKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdBcnJheSBvZiA4LWJpdCBpbnRlZ2VycyB2YWxpZGl0eSBpcyBvZiB3cm9uZyBsZW5ndGguICcgKyBieXRlcyAqIDIgKyAnIGNoYXIgc3ltYm9scyBsb25nIGlzIHJlcXVpcmVkLiAnICsgaGFzaC5sZW5ndGggKyAnIGlzIHBhc3NlZC4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfVxuXG4gICAgICAgIGZvciAodmFyIGkgPSAwLCBsZW4gPSBhcnIubGVuZ3RoOyBpIDwgbGVuOyBpKyspIHtcbiAgICAgICAgICAgIGlmICh0eXBlb2YgYXJyW2ldICE9PSAnbnVtYmVyJykge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIGRhdGEgdHlwZSBpcyBwYXNzZWQgYXMgYnl0ZS4gTnVtYmVyIGlzIHJlcXVpcmVkJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfSBlbHNlIGlmIChhcnJbaV0gPCAwIHx8IGFycltpXSA+IDI1NSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0J5dGUgc2hvdWxkIGJlIGluIFswLi4yNTVdIHJhbmdlLicpO1xuICAgICAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiB0cnVlO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENoZWNrIGJpbmFyeSBzdHJpbmcgdmFsaWRpdHlcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gc3RyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGJpdHNcbiAgICAgKiBAcmV0dXJucyB7Qm9vbGVhbn1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiB2YWxpZGF0ZUJpbmFyeVN0cmluZyhzdHIsIGJpdHMpIHtcbiAgICAgICAgaWYgKHR5cGVvZiBiaXRzICE9PSAndW5kZWZpbmVkJyAmJiBzdHIubGVuZ3RoICE9PSBiaXRzKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdCaW5hcnkgc3RyaW5nIGlzIG9mIHdyb25nIGxlbmd0aC4nKTtcbiAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICB9XG5cbiAgICAgICAgdmFyIGJpdDtcbiAgICAgICAgZm9yICh2YXIgaSA9IDA7IGkgPCBzdHIubGVuZ3RoOyBpKyspIHtcbiAgICAgICAgICAgIGJpdCA9IHBhcnNlSW50KHN0cltpXSk7XG4gICAgICAgICAgICBpZiAoaXNOYU4oYml0KSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIGJpdCBpcyBwYXNzZWQgaW4gYmluYXJ5IHN0cmluZy4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9IGVsc2UgaWYgKGJpdCA+IDEpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBiaXQgaXMgcGFzc2VkIGluIGJpbmFyeSBzdHJpbmcuIEJpdCBzaG91bGQgYmUgMCBvciAxLicpO1xuICAgICAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiB0cnVlO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIFNlcmlhbGl6ZSBkYXRhIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMgYW5kIGluc2VydCBpbnRvIGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBzaGlmdCAtIHRoZSBpbmRleCB0byBzdGFydCB3cml0ZSBpbnRvIGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7T2JqZWN0fSBkYXRhXG4gICAgICogQHBhcmFtIHR5cGUgLSBjYW4gYmUge05ld1R5cGV9IG9yIG9uZSBvZiBidWlsdC1pbiB0eXBlc1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIHNlcmlhbGl6ZShidWZmZXIsIHNoaWZ0LCBkYXRhLCB0eXBlKSB7XG4gICAgICAgIGZvciAodmFyIGkgPSAwLCBsZW4gPSB0eXBlLnNpemU7IGkgPCBsZW47IGkrKykge1xuICAgICAgICAgICAgYnVmZmVyW3NoaWZ0ICsgaV0gPSAwO1xuICAgICAgICB9XG5cbiAgICAgICAgZm9yICh2YXIgZmllbGROYW1lIGluIGRhdGEpIHtcbiAgICAgICAgICAgIGlmICghZGF0YS5oYXNPd25Qcm9wZXJ0eShmaWVsZE5hbWUpKSB7XG4gICAgICAgICAgICAgICAgY29udGludWU7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIHZhciBmaWVsZFR5cGUgPSB0eXBlLmZpZWxkc1tmaWVsZE5hbWVdO1xuXG4gICAgICAgICAgICBpZiAodHlwZW9mIGZpZWxkVHlwZSA9PT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKGZpZWxkTmFtZSArICcgZmllbGQgd2FzIG5vdCBmb3VuZCBpbiBjb25maWd1cmF0aW9uIG9mIHR5cGUuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICB2YXIgZmllbGREYXRhID0gZGF0YVtmaWVsZE5hbWVdO1xuICAgICAgICAgICAgdmFyIGZyb20gPSBzaGlmdCArIGZpZWxkVHlwZS5mcm9tO1xuXG4gICAgICAgICAgICBpZiAoZmllbGRUeXBlLnR5cGUgaW5zdGFuY2VvZiBOZXdUeXBlKSB7XG4gICAgICAgICAgICAgICAgdmFyIGlzRml4ZWQgPSAoZnVuY3Rpb24gY2hlY2tJZklzRml4ZWQoZmllbGRzKSB7XG4gICAgICAgICAgICAgICAgICAgIGZvciAodmFyIGZpZWxkTmFtZSBpbiBmaWVsZHMpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGlmICghZmllbGRzLmhhc093blByb3BlcnR5KGZpZWxkTmFtZSkpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBjb250aW51ZTtcbiAgICAgICAgICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKGZpZWxkc1tmaWVsZE5hbWVdLnR5cGUgaW5zdGFuY2VvZiBOZXdUeXBlKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgY2hlY2tJZklzRml4ZWQoZmllbGRzW2ZpZWxkTmFtZV0udHlwZS5maWVsZHMpO1xuICAgICAgICAgICAgICAgICAgICAgICAgfSBlbHNlIGlmIChmaWVsZHNbZmllbGROYW1lXS50eXBlID09PSBTdHJpbmcpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHRydWU7XG4gICAgICAgICAgICAgICAgfSkoZmllbGRUeXBlLnR5cGUuZmllbGRzKTtcblxuICAgICAgICAgICAgICAgIGlmIChpc0ZpeGVkID09PSB0cnVlKSB7XG4gICAgICAgICAgICAgICAgICAgIHNlcmlhbGl6ZShidWZmZXIsIGZyb20sIGZpZWxkRGF0YSwgZmllbGRUeXBlLnR5cGUpO1xuICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgIHZhciBlbmQgPSBidWZmZXIubGVuZ3RoO1xuICAgICAgICAgICAgICAgICAgICBVaW50MzIoZW5kLCBidWZmZXIsIGZyb20sIGZyb20gKyA0KTtcbiAgICAgICAgICAgICAgICAgICAgc2VyaWFsaXplKGJ1ZmZlciwgZW5kLCBmaWVsZERhdGEsIGZpZWxkVHlwZS50eXBlKTtcbiAgICAgICAgICAgICAgICAgICAgVWludDMyKGJ1ZmZlci5sZW5ndGggLSBlbmQsIGJ1ZmZlciwgZnJvbSArIDQsIGZyb20gKyA4KTtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIGJ1ZmZlciA9IGZpZWxkVHlwZS50eXBlKGZpZWxkRGF0YSwgYnVmZmVyLCBmcm9tLCBzaGlmdCArIGZpZWxkVHlwZS50byk7XG4gICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBidWZmZXIgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9XG4gICAgICAgIH1cblxuICAgICAgICByZXR1cm4gYnVmZmVyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENvbnZlcnQgaGV4YWRlY2ltYWwgaW50byBhcnJheSBvZiA4LWJpdCBpbnRlZ2VycyBhbmQgaW5zZXJ0IGludG8gYnVmZmVyXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IHN0clxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gaW5zZXJ0SGV4YWRlY2ltYWxUb0FycmF5KHN0ciwgYnVmZmVyLCBmcm9tLCB0bykge1xuICAgICAgICBmb3IgKHZhciBpID0gMCwgbGVuID0gc3RyLmxlbmd0aDsgaSA8IGxlbjsgaSArPSAyKSB7XG4gICAgICAgICAgICBidWZmZXJbZnJvbV0gPSBwYXJzZUludChzdHIuc3Vic3RyKGksIDIpLCAxNik7XG4gICAgICAgICAgICBmcm9tKys7XG5cbiAgICAgICAgICAgIGlmIChmcm9tID4gdG8pIHtcbiAgICAgICAgICAgICAgICBicmVhaztcbiAgICAgICAgICAgIH1cbiAgICAgICAgfVxuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENvbnZlcnQgaW50ZWdlciBpbnRvIGFycmF5IG9mIDgtYml0IGludGVnZXJzIGFuZCBpbnNlcnQgaW50byBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gbnVtYmVyXG4gICAgICogQHBhcmFtIHtBcnJheX0gYnVmZmVyXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGZyb21cbiAgICAgKiBAcGFyYW0ge051bWJlcn0gdG9cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBpbnNlcnRJbnRlZ2VyVG9CeXRlQXJyYXkobnVtYmVyLCBidWZmZXIsIGZyb20sIHRvKSB7XG4gICAgICAgIHZhciBzdHIgPSBudW1iZXIudG9TdHJpbmcoMTYpO1xuXG4gICAgICAgIGluc2VydE51bWJlckluSGV4VG9CeXRlQXJyYXkoc3RyLCBidWZmZXIsIGZyb20sIHRvKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDb252ZXJ0IGJpZyBpbnRlZ2VyIGludG8gYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMgYW5kIGluc2VydCBpbnRvIGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7YmlnSW50fSBudW1iZXJcbiAgICAgKiBAcGFyYW0ge0FycmF5fSBidWZmZXJcbiAgICAgKiBAcGFyYW0ge051bWJlcn0gZnJvbVxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSB0b1xuICAgICAqL1xuICAgIGZ1bmN0aW9uIGluc2VydEJpZ0ludGVnZXJUb0J5dGVBcnJheShudW1iZXIsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgdmFyIHN0ciA9IG51bWJlci50b1N0cmluZygxNik7XG5cbiAgICAgICAgaW5zZXJ0TnVtYmVySW5IZXhUb0J5dGVBcnJheShzdHIsIGJ1ZmZlciwgZnJvbSwgdG8pO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEluc2VydCBudW1iZXIgaW4gaGV4IGludG8gYnVmZmVyXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IG51bWJlclxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IHRvXG4gICAgICovXG4gICAgZnVuY3Rpb24gaW5zZXJ0TnVtYmVySW5IZXhUb0J5dGVBcnJheShudW1iZXIsIGJ1ZmZlciwgZnJvbSwgdG8pIHtcbiAgICAgICAgLy8gc3RvcmUgTnVtYmVyIGFzIGxpdHRsZS1lbmRpYW5cbiAgICAgICAgaWYgKG51bWJlci5sZW5ndGggPCAzKSB7XG4gICAgICAgICAgICBidWZmZXJbZnJvbV0gPSBwYXJzZUludChudW1iZXIsIDE2KTtcbiAgICAgICAgICAgIHJldHVybiB0cnVlO1xuICAgICAgICB9XG5cbiAgICAgICAgZm9yICh2YXIgaSA9IG51bWJlci5sZW5ndGg7IGkgPiAwOyBpIC09IDIpIHtcbiAgICAgICAgICAgIGlmIChpID4gMSkge1xuICAgICAgICAgICAgICAgIGJ1ZmZlcltmcm9tXSA9IHBhcnNlSW50KG51bWJlci5zdWJzdHIoaSAtIDIsIDIpLCAxNik7XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIGJ1ZmZlcltmcm9tXSA9IHBhcnNlSW50KG51bWJlci5zdWJzdHIoMCwgMSksIDE2KTtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgZnJvbSsrO1xuXG4gICAgICAgICAgICBpZiAoZnJvbSA+PSB0bykge1xuICAgICAgICAgICAgICAgIGJyZWFrO1xuICAgICAgICAgICAgfVxuICAgICAgICB9XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ29udmVydCBTdHJpbmcgaW50byBhcnJheSBvZiA4LWJpdCBpbnRlZ2VycyBhbmQgaW5zZXJ0IGludG8gYnVmZmVyXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IHN0clxuICAgICAqIEBwYXJhbSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEBwYXJhbSB7TnVtYmVyfSBmcm9tXG4gICAgICovXG4gICAgZnVuY3Rpb24gaW5zZXJ0U3RyaW5nVG9CeXRlQXJyYXkoc3RyLCBidWZmZXIsIGZyb20pIHtcbiAgICAgICAgZm9yICh2YXIgaSA9IDA7IGkgPCBzdHIubGVuZ3RoOyBpKyspIHtcbiAgICAgICAgICAgIHZhciBjID0gc3RyLmNoYXJDb2RlQXQoaSk7XG5cbiAgICAgICAgICAgIGlmIChjIDwgMTI4KSB7XG4gICAgICAgICAgICAgICAgYnVmZmVyW2Zyb20rK10gPSBjO1xuICAgICAgICAgICAgfSBlbHNlIGlmIChjIDwgMjA0OCkge1xuICAgICAgICAgICAgICAgIGJ1ZmZlcltmcm9tKytdID0gKGMgPj4gNikgfCAxOTI7XG4gICAgICAgICAgICAgICAgYnVmZmVyW2Zyb20rK10gPSAoYyAmIDYzKSB8IDEyODtcbiAgICAgICAgICAgIH0gZWxzZSBpZiAoKChjICYgMHhGQzAwKSA9PSAweEQ4MDApICYmIChpICsgMSkgPCBzdHIubGVuZ3RoICYmICgoc3RyLmNoYXJDb2RlQXQoaSArIDEpICYgMHhGQzAwKSA9PSAweERDMDApKSB7XG4gICAgICAgICAgICAgICAgLy8gc3Vycm9nYXRlIHBhaXJcbiAgICAgICAgICAgICAgICBjID0gMHgxMDAwMCArICgoYyAmIDB4MDNGRikgPDwgMTApICsgKHN0ci5jaGFyQ29kZUF0KCsraSkgJiAweDAzRkYpO1xuICAgICAgICAgICAgICAgIGJ1ZmZlcltmcm9tKytdID0gKGMgPj4gMTgpIHwgMjQwO1xuICAgICAgICAgICAgICAgIGJ1ZmZlcltmcm9tKytdID0gKChjID4+IDEyKSAmIDYzKSB8IDEyODtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbSsrXSA9ICgoYyA+PiA2KSAmIDYzKSB8IDEyODtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbSsrXSA9IChjICYgNjMpIHwgMTI4O1xuICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbSsrXSA9IChjID4+IDEyKSB8IDIyNDtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbSsrXSA9ICgoYyA+PiA2KSAmIDYzKSB8IDEyODtcbiAgICAgICAgICAgICAgICBidWZmZXJbZnJvbSsrXSA9IChjICYgNjMpIHwgMTI4O1xuICAgICAgICAgICAgfVxuICAgICAgICB9XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ29udmVydCBoZXhhZGVjaW1hbCBzdHJpbmcgaW50byBhcnJheSBvZiA4LWJpdCBpbnRlZ2Vyc1xuICAgICAqIEBwYXJhbSB7U3RyaW5nfSBzdHJcbiAgICAgKiAgaGV4YWRlY2ltYWxcbiAgICAgKiBAcmV0dXJucyB7VWludDhBcnJheX1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBoZXhhZGVjaW1hbFRvVWludDhBcnJheShzdHIpIHtcbiAgICAgICAgaWYgKHR5cGVvZiBzdHIgIT09ICdzdHJpbmcnKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBkYXRhIHR5cGUgb2YgaGV4YWRlY2ltYWwgc3RyaW5nJyk7XG4gICAgICAgICAgICByZXR1cm4gbmV3IFVpbnQ4QXJyYXkoW10pO1xuICAgICAgICB9XG5cbiAgICAgICAgdmFyIGFycmF5ID0gW107XG4gICAgICAgIGZvciAodmFyIGkgPSAwLCBsZW4gPSBzdHIubGVuZ3RoOyBpIDwgbGVuOyBpICs9IDIpIHtcbiAgICAgICAgICAgIGFycmF5LnB1c2gocGFyc2VJbnQoc3RyLnN1YnN0cihpLCAyKSwgMTYpKTtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBuZXcgVWludDhBcnJheShhcnJheSk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ29udmVydCBoZXhhZGVjaW1hbCBzdHJpbmcgaW50byBhcnJheSBvZiA4LWJpdCBpbnRlZ2Vyc1xuICAgICAqIEBwYXJhbSB7U3RyaW5nfSBzdHJcbiAgICAgKiAgaGV4YWRlY2ltYWxcbiAgICAgKiBAcmV0dXJucyB7U3RyaW5nfVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIGhleGFkZWNpbWFsVG9CaW5hcnlTdHJpbmcoc3RyKSB7XG4gICAgICAgIHZhciBiaW5hcnlTdHIgPSAnJztcbiAgICAgICAgZm9yICh2YXIgaSA9IDAsIGxlbiA9IHN0ci5sZW5ndGg7IGkgPCBsZW47IGkgKz0gMikge1xuICAgICAgICAgICAgYmluYXJ5U3RyICs9IHBhcnNlSW50KHN0ci5zdWJzdHIoaSwgMiksIDE2KS50b1N0cmluZygyKTtcbiAgICAgICAgfVxuICAgICAgICByZXR1cm4gYmluYXJ5U3RyO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENvbnZlcnQgYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMgaW50byBoZXhhZGVjaW1hbCBzdHJpbmdcbiAgICAgKiBAcGFyYW0ge1VpbnQ4QXJyYXl9IHVpbnQ4YXJyXG4gICAgICogQHJldHVybnMge1N0cmluZ31cbiAgICAgKi9cbiAgICBmdW5jdGlvbiB1aW50OEFycmF5VG9IZXhhZGVjaW1hbCh1aW50OGFycikge1xuICAgICAgICBpZiAoISh1aW50OGFyciBpbnN0YW5jZW9mIFVpbnQ4QXJyYXkpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyBkYXRhIHR5cGUgb2YgYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMnKTtcbiAgICAgICAgICAgIHJldHVybiAnJztcbiAgICAgICAgfVxuXG4gICAgICAgIHZhciBzdHIgPSAnJztcbiAgICAgICAgZm9yICh2YXIgaSA9IDAsIGxlbiA9IHVpbnQ4YXJyLmxlbmd0aDsgaSA8IGxlbjsgaSsrKSB7XG4gICAgICAgICAgICB2YXIgaGV4ID0gKHVpbnQ4YXJyW2ldKS50b1N0cmluZygxNik7XG4gICAgICAgICAgICBoZXggPSAoaGV4Lmxlbmd0aCA9PT0gMSkgPyAnMCcgKyBoZXggOiBoZXg7XG4gICAgICAgICAgICBzdHIgKz0gaGV4O1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIHN0ci50b0xvd2VyQ2FzZSgpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIENvbnZlcnQgYmluYXJ5IHN0cmluZyBpbnRvIGFycmF5IG9mIDgtYml0IGludGVnZXJzXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IGJpbmFyeVN0clxuICAgICAqIEByZXR1cm5zIHtVaW50OEFycmF5fVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIGJpbmFyeVN0cmluZ1RvVWludDhBcnJheShiaW5hcnlTdHIpIHtcbiAgICAgICAgdmFyIGFycmF5ID0gW107XG4gICAgICAgIGZvciAodmFyIGkgPSAwLCBsZW4gPSBiaW5hcnlTdHIubGVuZ3RoOyBpIDwgbGVuOyBpICs9IDgpIHtcbiAgICAgICAgICAgIGFycmF5LnB1c2gocGFyc2VJbnQoYmluYXJ5U3RyLnN1YnN0cihpLCA4KSwgMikpO1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIG5ldyBVaW50OEFycmF5KGFycmF5KTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDb252ZXJ0IGJpbmFyeSBzdHJpbmcgaW50byBoZXhhZGVjaW1hbCBzdHJpbmdcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gYmluYXJ5U3RyXG4gICAgICogQHJldHVybnMge3N0cmluZ31cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBiaW5hcnlTdHJpbmdUb0hleGFkZWNpbWFsKGJpbmFyeVN0cikge1xuICAgICAgICB2YXIgc3RyID0gJyc7XG4gICAgICAgIGZvciAodmFyIGkgPSAwLCBsZW4gPSBiaW5hcnlTdHIubGVuZ3RoOyBpIDwgbGVuOyBpICs9IDgpIHtcbiAgICAgICAgICAgIHZhciBoZXggPSAocGFyc2VJbnQoYmluYXJ5U3RyLnN1YnN0cihpLCA4KSwgMikpLnRvU3RyaW5nKDE2KTtcbiAgICAgICAgICAgIGhleCA9IChoZXgubGVuZ3RoID09PSAxKSA/ICcwJyArIGhleCA6IGhleDtcbiAgICAgICAgICAgIHN0ciArPSBoZXg7XG4gICAgICAgIH1cblxuICAgICAgICByZXR1cm4gc3RyLnRvTG93ZXJDYXNlKCk7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogQ2hlY2sgaWYgZWxlbWVudCBpcyBvZiB0eXBlIHtPYmplY3R9XG4gICAgICogQHBhcmFtIG9ialxuICAgICAqIEByZXR1cm5zIHtib29sZWFufVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIGlzT2JqZWN0KG9iaikge1xuICAgICAgICByZXR1cm4gKHR5cGVvZiBvYmogPT09ICdvYmplY3QnICYmICFBcnJheS5pc0FycmF5KG9iaikgJiYgb2JqICE9PSBudWxsICYmICEob2JqIGluc3RhbmNlb2YgRGF0ZSkpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEdldCBsZW5ndGggb2Ygb2JqZWN0XG4gICAgICogQHBhcmFtIHtPYmplY3R9IG9ialxuICAgICAqIEByZXR1cm5zIHtOdW1iZXJ9XG4gICAgICovXG4gICAgZnVuY3Rpb24gZ2V0T2JqZWN0TGVuZ3RoKG9iaikge1xuICAgICAgICB2YXIgbCA9IDA7XG4gICAgICAgIGZvciAodmFyIHByb3AgaW4gb2JqKSB7XG4gICAgICAgICAgICBpZiAob2JqLmhhc093blByb3BlcnR5KHByb3ApKSB7XG4gICAgICAgICAgICAgICAgbCsrO1xuICAgICAgICAgICAgfVxuICAgICAgICB9XG4gICAgICAgIHJldHVybiBsO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEdldCBTSEEyNTYgaGFzaFxuICAgICAqIE1ldGhvZCB3aXRoIG92ZXJsb2FkaW5nLiBBY2NlcHQgdHdvIGNvbWJpbmF0aW9ucyBvZiBhcmd1bWVudHM6XG4gICAgICogMSkge09iamVjdH0gZGF0YSwge05ld1R5cGV9IHR5cGVcbiAgICAgKiAyKSB7QXJyYXl9IGJ1ZmZlclxuICAgICAqIEByZXR1cm4ge1N0cmluZ31cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBoYXNoKGRhdGEsIHR5cGUpIHtcbiAgICAgICAgdmFyIGJ1ZmZlcjtcbiAgICAgICAgaWYgKHR5cGUgaW5zdGFuY2VvZiBOZXdUeXBlKSB7XG4gICAgICAgICAgICBpZiAoaXNPYmplY3QoZGF0YSkpIHtcbiAgICAgICAgICAgICAgICBidWZmZXIgPSB0eXBlLnNlcmlhbGl6ZShkYXRhKTtcbiAgICAgICAgICAgICAgICBpZiAodHlwZW9mIGJ1ZmZlciA9PT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBkYXRhIHBhcmFtZXRlci4gSW5zdGFuY2Ugb2YgTmV3VHlwZSBpcyBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgdHlwZSBvZiBkYXRhLiBTaG91bGQgYmUgb2JqZWN0LicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIGlmICh0eXBlIGluc3RhbmNlb2YgTmV3TWVzc2FnZSkge1xuICAgICAgICAgICAgaWYgKGlzT2JqZWN0KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgYnVmZmVyID0gdHlwZS5zZXJpYWxpemUoZGF0YSk7XG4gICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBidWZmZXIgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgZGF0YSBwYXJhbWV0ZXIuIEluc3RhbmNlIG9mIE5ld01lc3NhZ2UgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIHR5cGUgb2YgZGF0YS4gU2hvdWxkIGJlIG9iamVjdC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9XG4gICAgICAgIH0gZWxzZSBpZiAoZGF0YSBpbnN0YW5jZW9mIFVpbnQ4QXJyYXkpIHtcbiAgICAgICAgICAgIGJ1ZmZlciA9IGRhdGE7XG4gICAgICAgIH0gZWxzZSBpZiAoQXJyYXkuaXNBcnJheShkYXRhKSkge1xuICAgICAgICAgICAgYnVmZmVyID0gbmV3IFVpbnQ4QXJyYXkoZGF0YSk7XG4gICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIHNoYSgnc2hhMjU2JykudXBkYXRlKGJ1ZmZlciwgJ3V0ZjgnKS5kaWdlc3QoJ2hleCcpO1xuICAgIH1cblxuICAgIC8qKlxuICAgICAqIEdldCBFRDI1NTE5IHNpZ25hdHVyZVxuICAgICAqIE1ldGhvZCB3aXRoIG92ZXJsb2FkaW5nLiBBY2NlcHQgdHdvIGNvbWJpbmF0aW9ucyBvZiBhcmd1bWVudHM6XG4gICAgICogMSkge09iamVjdH0gZGF0YSwge05ld1R5cGUgfHwgTmV3TWVzc2FnZX0gdHlwZSwge1N0cmluZ30gc2VjcmV0S2V5XG4gICAgICogMikge0FycmF5fSBidWZmZXIsIHtTdHJpbmd9IHNlY3JldEtleVxuICAgICAqIEByZXR1cm4ge1N0cmluZ31cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBzaWduKGRhdGEsIHR5cGUsIHNlY3JldEtleSkge1xuICAgICAgICB2YXIgYnVmZmVyO1xuICAgICAgICB2YXIgc2VjcmV0S2V5ID0gc2VjcmV0S2V5O1xuICAgICAgICB2YXIgc2lnbmF0dXJlO1xuXG4gICAgICAgIGlmICh0eXBlb2Ygc2VjcmV0S2V5ICE9PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgaWYgKHR5cGUgaW5zdGFuY2VvZiBOZXdUeXBlKSB7XG4gICAgICAgICAgICAgICAgYnVmZmVyID0gdHlwZS5zZXJpYWxpemUoZGF0YSk7XG4gICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBidWZmZXIgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgZGF0YSBwYXJhbWV0ZXIuIEluc3RhbmNlIG9mIE5ld1R5cGUgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2UgaWYgKHR5cGUgaW5zdGFuY2VvZiBOZXdNZXNzYWdlKSB7XG4gICAgICAgICAgICAgICAgYnVmZmVyID0gdHlwZS5zZXJpYWxpemUoZGF0YSwgdHJ1ZSk7XG4gICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBidWZmZXIgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgZGF0YSBwYXJhbWV0ZXIuIEluc3RhbmNlIG9mIE5ld01lc3NhZ2UgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdHlwZSBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgfVxuICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgaWYgKCF2YWxpZGF0ZUJ5dGVzQXJyYXkoZGF0YSkpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGRhdGEgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgYnVmZmVyID0gZGF0YTtcbiAgICAgICAgICAgIHNlY3JldEtleSA9IHR5cGU7XG4gICAgICAgIH1cblxuICAgICAgICBpZiAoIXZhbGlkYXRlSGV4SGFzaChzZWNyZXRLZXksIDY0KSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBzZWNyZXRLZXkgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgYnVmZmVyID0gbmV3IFVpbnQ4QXJyYXkoYnVmZmVyKTtcbiAgICAgICAgc2VjcmV0S2V5ID0gaGV4YWRlY2ltYWxUb1VpbnQ4QXJyYXkoc2VjcmV0S2V5KTtcbiAgICAgICAgc2lnbmF0dXJlID0gbmFjbC5zaWduLmRldGFjaGVkKGJ1ZmZlciwgc2VjcmV0S2V5KTtcblxuICAgICAgICByZXR1cm4gdWludDhBcnJheVRvSGV4YWRlY2ltYWwoc2lnbmF0dXJlKTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBWZXJpZmllcyBFRDI1NTE5IHNpZ25hdHVyZVxuICAgICAqIE1ldGhvZCB3aXRoIG92ZXJsb2FkaW5nLiBBY2NlcHQgdHdvIGNvbWJpbmF0aW9ucyBvZiBhcmd1bWVudHM6XG4gICAgICogMSkge09iamVjdH0gZGF0YSwge05ld1R5cGUgfHwgTmV3TWVzc2FnZX0gdHlwZSwge1N0cmluZ30gc2lnbmF0dXJlLCB7U3RyaW5nfSBwdWJsaWNLZXlcbiAgICAgKiAyKSB7QXJyYXl9IGJ1ZmZlciwge1N0cmluZ30gc2lnbmF0dXJlLCB7U3RyaW5nfSBwdWJsaWNLZXlcbiAgICAgKiBAcmV0dXJuIHtCb29sZWFufVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIHZlcmlmeVNpZ25hdHVyZShkYXRhLCB0eXBlLCBzaWduYXR1cmUsIHB1YmxpY0tleSkge1xuICAgICAgICB2YXIgYnVmZmVyO1xuICAgICAgICB2YXIgc2lnbmF0dXJlID0gc2lnbmF0dXJlO1xuICAgICAgICB2YXIgcHVibGljS2V5ID0gcHVibGljS2V5O1xuXG4gICAgICAgIGlmICh0eXBlb2YgcHVibGljS2V5ICE9PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgaWYgKHR5cGUgaW5zdGFuY2VvZiBOZXdUeXBlKSB7XG4gICAgICAgICAgICAgICAgYnVmZmVyID0gdHlwZS5zZXJpYWxpemUoZGF0YSk7XG4gICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBidWZmZXIgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgZGF0YSBwYXJhbWV0ZXIuIEluc3RhbmNlIG9mIE5ld1R5cGUgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2UgaWYgKHR5cGUgaW5zdGFuY2VvZiBOZXdNZXNzYWdlKSB7XG4gICAgICAgICAgICAgICAgYnVmZmVyID0gdHlwZS5zZXJpYWxpemUoZGF0YSwgdHJ1ZSk7XG4gICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBidWZmZXIgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgZGF0YSBwYXJhbWV0ZXIuIEluc3RhbmNlIG9mIE5ld01lc3NhZ2UgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdHlwZSBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgfVxuICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgaWYgKCF2YWxpZGF0ZUJ5dGVzQXJyYXkoZGF0YSkpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGRhdGEgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgYnVmZmVyID0gZGF0YTtcbiAgICAgICAgICAgIHB1YmxpY0tleSA9IHNpZ25hdHVyZTtcbiAgICAgICAgICAgIHNpZ25hdHVyZSA9IHR5cGU7XG4gICAgICAgIH1cblxuICAgICAgICBpZiAoIXZhbGlkYXRlSGV4SGFzaChwdWJsaWNLZXkpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHB1YmxpY0tleSBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICByZXR1cm47XG4gICAgICAgIH0gZWxzZSBpZiAoIXZhbGlkYXRlSGV4SGFzaChzaWduYXR1cmUsIDY0KSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBzaWduYXR1cmUgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICB9XG5cbiAgICAgICAgYnVmZmVyID0gbmV3IFVpbnQ4QXJyYXkoYnVmZmVyKTtcbiAgICAgICAgc2lnbmF0dXJlID0gaGV4YWRlY2ltYWxUb1VpbnQ4QXJyYXkoc2lnbmF0dXJlKTtcbiAgICAgICAgcHVibGljS2V5ID0gaGV4YWRlY2ltYWxUb1VpbnQ4QXJyYXkocHVibGljS2V5KTtcblxuICAgICAgICByZXR1cm4gbmFjbC5zaWduLmRldGFjaGVkLnZlcmlmeShidWZmZXIsIHNpZ25hdHVyZSwgcHVibGljS2V5KTtcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDaGVjayBNZXJrbGUgdHJlZSBwcm9vZiBhbmQgcmV0dXJuIGFycmF5IG9mIGVsZW1lbnRzXG4gICAgICogQHBhcmFtIHtTdHJpbmd9IHJvb3RIYXNoXG4gICAgICogQHBhcmFtIHtOdW1iZXJ9IGNvdW50XG4gICAgICogQHBhcmFtIHtPYmplY3R9IHByb29mTm9kZVxuICAgICAqIEBwYXJhbSB7QXJyYXl9IHJhbmdlXG4gICAgICogQHBhcmFtIHtOZXdUeXBlfSB0eXBlIChvcHRpb25hbClcbiAgICAgKiBAcmV0dXJuIHtBcnJheX1cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBtZXJrbGVQcm9vZihyb290SGFzaCwgY291bnQsIHByb29mTm9kZSwgcmFuZ2UsIHR5cGUpIHtcbiAgICAgICAgLyoqXG4gICAgICAgICAqIENhbGN1bGF0ZSBoZWlnaHQgb2YgbWVya2xlIHRyZWVcbiAgICAgICAgICogQHBhcmFtIHtiaWdJbnR9IGNvdW50XG4gICAgICAgICAqIEByZXR1cm4ge051bWJlcn1cbiAgICAgICAgICovXG4gICAgICAgIGZ1bmN0aW9uIGNhbGNIZWlnaHQoY291bnQpIHtcbiAgICAgICAgICAgIHZhciBpID0gMDtcbiAgICAgICAgICAgIHdoaWxlIChiaWdJbnQoMikucG93KGkpLmx0KGNvdW50KSkge1xuICAgICAgICAgICAgICAgIGkrKztcbiAgICAgICAgICAgIH1cbiAgICAgICAgICAgIHJldHVybiBpO1xuICAgICAgICB9XG5cbiAgICAgICAgLyoqXG4gICAgICAgICAqIEdldCB2YWx1ZSBmcm9tIG5vZGUsIGluc2VydCBpbnRvIGVsZW1lbnRzIGFycmF5IGFuZCByZXR1cm4gaXRzIGhhc2hcbiAgICAgICAgICogQHBhcmFtIGRhdGFcbiAgICAgICAgICogQHBhcmFtIHtOdW1iZXJ9IGRlcHRoXG4gICAgICAgICAqIEBwYXJhbSB7TnVtYmVyfSBpbmRleFxuICAgICAgICAgKiBAcmV0dXJucyB7U3RyaW5nfVxuICAgICAgICAgKi9cbiAgICAgICAgZnVuY3Rpb24gZ2V0SGFzaChkYXRhLCBkZXB0aCwgaW5kZXgpIHtcbiAgICAgICAgICAgIHZhciBlbGVtZW50O1xuICAgICAgICAgICAgdmFyIGVsZW1lbnRzSGFzaDtcblxuICAgICAgICAgICAgaWYgKChkZXB0aCArIDEpICE9PSBoZWlnaHQpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdWYWx1ZSBub2RlIGlzIG9uIHdyb25nIGhlaWdodCBpbiB0cmVlLicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH0gZWxzZSBpZiAoaW5kZXggPCBzdGFydCB8fCBlbmQubHQoaW5kZXgpKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgaW5kZXggb2YgdmFsdWUgbm9kZS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9IGVsc2UgaWYgKChzdGFydCArIGVsZW1lbnRzLmxlbmd0aCkgIT09IGluZGV4KSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignVmFsdWUgbm9kZSBpcyBvbiB3cm9uZyBwb3NpdGlvbiBpbiB0cmVlLicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgaWYgKHR5cGVvZiBkYXRhID09PSAnc3RyaW5nJykge1xuICAgICAgICAgICAgICAgIGlmICh2YWxpZGF0ZUhleEhhc2goZGF0YSkpIHtcbiAgICAgICAgICAgICAgICAgICAgZWxlbWVudCA9IGRhdGE7XG4gICAgICAgICAgICAgICAgICAgIGVsZW1lbnRzSGFzaCA9IGhhc2goaGV4YWRlY2ltYWxUb1VpbnQ4QXJyYXkoZWxlbWVudCkpO1xuICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgaGV4YWRlY2ltYWwgc3RyaW5nIGlzIHBhc3NlZCBhcyB2YWx1ZSBpbiB0cmVlLicpO1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIGlmIChBcnJheS5pc0FycmF5KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgaWYgKHZhbGlkYXRlQnl0ZXNBcnJheShkYXRhKSkge1xuICAgICAgICAgICAgICAgICAgICBlbGVtZW50ID0gZGF0YS5zbGljZSgwKTsgLy8gY2xvbmUgYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnNcbiAgICAgICAgICAgICAgICAgICAgZWxlbWVudHNIYXNoID0gaGFzaChlbGVtZW50KTtcbiAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGFycmF5IG9mIDgtYml0IGludGVnZXJzIGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2UgaWYgKGlzT2JqZWN0KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgaWYgKE5ld1R5cGUucHJvdG90eXBlLmlzUHJvdG90eXBlT2YodHlwZSkpIHtcbiAgICAgICAgICAgICAgICAgICAgZWxlbWVudCA9IG9iamVjdEFzc2lnbihkYXRhKTsgLy8gZGVlcCBjb3B5XG4gICAgICAgICAgICAgICAgICAgIGVsZW1lbnRzSGFzaCA9IGhhc2goZWxlbWVudCwgdHlwZSk7XG4gICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB0eXBlIG9mIHR5cGUgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIG9mIGRhdGEgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgaWYgKHR5cGVvZiBlbGVtZW50c0hhc2ggPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICBlbGVtZW50cy5wdXNoKGVsZW1lbnQpO1xuICAgICAgICAgICAgcmV0dXJuIGVsZW1lbnRzSGFzaDtcbiAgICAgICAgfVxuXG4gICAgICAgIC8qKlxuICAgICAgICAgKiBSZWN1cnNpdmUgdHJlZSB0cmF2ZXJzYWwgZnVuY3Rpb25cbiAgICAgICAgICogQHBhcmFtIHtPYmplY3R9IG5vZGVcbiAgICAgICAgICogQHBhcmFtIHtOdW1iZXJ9IGRlcHRoXG4gICAgICAgICAqIEBwYXJhbSB7TnVtYmVyfSBpbmRleFxuICAgICAgICAgKiBAcmV0dXJucyB7U3RyaW5nfVxuICAgICAgICAgKi9cbiAgICAgICAgZnVuY3Rpb24gcmVjdXJzaXZlKG5vZGUsIGRlcHRoLCBpbmRleCkge1xuICAgICAgICAgICAgdmFyIGhhc2hMZWZ0O1xuICAgICAgICAgICAgdmFyIGhhc2hSaWdodDtcbiAgICAgICAgICAgIHZhciBzdW1taW5nQnVmZmVyO1xuXG4gICAgICAgICAgICBpZiAodHlwZW9mIG5vZGUubGVmdCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICBpZiAodHlwZW9mIG5vZGUubGVmdCA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgICAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2gobm9kZS5sZWZ0KSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgaGFzaExlZnQgPSBub2RlLmxlZnQ7XG4gICAgICAgICAgICAgICAgfSBlbHNlIGlmIChpc09iamVjdChub2RlLmxlZnQpKSB7XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2Ygbm9kZS5sZWZ0LnZhbCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGhhc2hMZWZ0ID0gZ2V0SGFzaChub2RlLmxlZnQudmFsLCBkZXB0aCwgaW5kZXggKiAyKTtcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGhhc2hMZWZ0ID0gcmVjdXJzaXZlKG5vZGUubGVmdCwgZGVwdGggKyAxLCBpbmRleCAqIDIpO1xuICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgaGFzaExlZnQgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdHlwZSBvZiBsZWZ0IG5vZGUuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignTGVmdCBub2RlIGlzIG1pc3NlZC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgaWYgKGRlcHRoID09PSAwKSB7XG4gICAgICAgICAgICAgICAgcm9vdEJyYW5jaCA9ICdyaWdodCc7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmICh0eXBlb2Ygbm9kZS5yaWdodCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICBpZiAodHlwZW9mIG5vZGUucmlnaHQgPT09ICdzdHJpbmcnKSB7XG4gICAgICAgICAgICAgICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKG5vZGUucmlnaHQpKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgICAgICBoYXNoUmlnaHQgPSBub2RlLnJpZ2h0O1xuICAgICAgICAgICAgICAgIH0gZWxzZSBpZiAoaXNPYmplY3Qobm9kZS5yaWdodCkpIHtcbiAgICAgICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBub2RlLnJpZ2h0LnZhbCAhPT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGhhc2hSaWdodCA9IGdldEhhc2gobm9kZS5yaWdodC52YWwsIGRlcHRoLCBpbmRleCAqIDIgKyAxKTtcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGhhc2hSaWdodCA9IHJlY3Vyc2l2ZShub2RlLnJpZ2h0LCBkZXB0aCArIDEsIGluZGV4ICogMiArIDEpO1xuICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgaGFzaFJpZ2h0ID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2YgcmlnaHQgbm9kZS4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgc3VtbWluZ0J1ZmZlciA9IG5ldyBVaW50OEFycmF5KDY0KTtcbiAgICAgICAgICAgICAgICBzdW1taW5nQnVmZmVyLnNldChoZXhhZGVjaW1hbFRvVWludDhBcnJheShoYXNoTGVmdCkpO1xuICAgICAgICAgICAgICAgIHN1bW1pbmdCdWZmZXIuc2V0KGhleGFkZWNpbWFsVG9VaW50OEFycmF5KGhhc2hSaWdodCksIDMyKTtcbiAgICAgICAgICAgIH0gZWxzZSBpZiAoZGVwdGggPT09IDAgfHwgcm9vdEJyYW5jaCA9PT0gJ2xlZnQnKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignUmlnaHQgbGVhZiBpcyBtaXNzZWQgaW4gbGVmdCBicmFuY2ggb2YgdHJlZS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgc3VtbWluZ0J1ZmZlciA9IGhleGFkZWNpbWFsVG9VaW50OEFycmF5KGhhc2hMZWZ0KTtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgcmV0dXJuIGhhc2goc3VtbWluZ0J1ZmZlcik7XG4gICAgICAgIH1cblxuICAgICAgICB2YXIgZWxlbWVudHMgPSBbXTtcbiAgICAgICAgdmFyIHJvb3RCcmFuY2ggPSAnbGVmdCc7XG5cbiAgICAgICAgLy8gdmFsaWRhdGUgcm9vdEhhc2hcbiAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2gocm9vdEhhc2gpKSB7XG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9XG5cbiAgICAgICAgLy8gdmFsaWRhdGUgY291bnRcbiAgICAgICAgaWYgKCEodHlwZW9mIGNvdW50ID09PSAnbnVtYmVyJyB8fCB0eXBlb2YgY291bnQgPT09ICdzdHJpbmcnKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB2YWx1ZSBpcyBwYXNzZWQgYXMgY291bnQgcGFyYW1ldGVyLiBOdW1iZXIgb3Igc3RyaW5nIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICB0cnkge1xuICAgICAgICAgICAgY291bnQgPSBiaWdJbnQoY291bnQpO1xuICAgICAgICB9IGNhdGNoIChlKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIGlzIHBhc3NlZCBhcyBjb3VudCBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9XG4gICAgICAgIGlmIChjb3VudC5sdCgwKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBjb3VudCBwYXJhbWV0ZXIuIENvdW50IGNhblxcJ3QgYmUgYmVsb3cgemVyby4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH1cblxuICAgICAgICAvLyB2YWxpZGF0ZSByYW5nZVxuICAgICAgICBpZiAoIUFycmF5LmlzQXJyYXkocmFuZ2UpIHx8IHJhbmdlLmxlbmd0aCAhPT0gMikge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB0eXBlIG9mIHJhbmdlIHBhcmFtZXRlci4gQXJyYXkgb2YgdHdvIGVsZW1lbnRzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfSBlbHNlIGlmICghKHR5cGVvZiByYW5nZVswXSA9PT0gJ251bWJlcicgfHwgdHlwZW9mIHJhbmdlWzBdID09PSAnc3RyaW5nJykpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdmFsdWUgaXMgcGFzc2VkIGFzIHN0YXJ0IG9mIHJhbmdlIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH0gZWxzZSBpZiAoISh0eXBlb2YgcmFuZ2VbMV0gPT09ICdudW1iZXInIHx8IHR5cGVvZiByYW5nZVsxXSA9PT0gJ3N0cmluZycpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIGlzIHBhc3NlZCBhcyBlbmQgb2YgcmFuZ2UgcGFyYW1ldGVyLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICB0cnkge1xuICAgICAgICAgICAgdmFyIHJhbmdlU3RhcnQgPSBiaWdJbnQocmFuZ2VbMF0pO1xuICAgICAgICB9IGNhdGNoIChlKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIGlzIHBhc3NlZCBhcyBzdGFydCBvZiByYW5nZSBwYXJhbWV0ZXIuIE51bWJlciBvciBzdHJpbmcgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9XG4gICAgICAgIHRyeSB7XG4gICAgICAgICAgICB2YXIgcmFuZ2VFbmQgPSBiaWdJbnQocmFuZ2VbMV0pO1xuICAgICAgICB9IGNhdGNoIChlKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHZhbHVlIGlzIHBhc3NlZCBhcyBlbmQgb2YgcmFuZ2UgcGFyYW1ldGVyLiBOdW1iZXIgb3Igc3RyaW5nIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICBpZiAocmFuZ2VTdGFydC5ndChyYW5nZUVuZCkpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgcmFuZ2UgcGFyYW1ldGVyLiBTdGFydCBpbmRleCBjYW5cXCd0IGJlIG91dCBvZiByYW5nZS4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH0gZWxzZSBpZiAocmFuZ2VTdGFydC5sdCgwKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCByYW5nZSBwYXJhbWV0ZXIuIFN0YXJ0IGluZGV4IGNhblxcJ3QgYmUgYmVsb3cgemVyby4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH0gZWxzZSBpZiAocmFuZ2VFbmQubHQoMCkpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgcmFuZ2UgcGFyYW1ldGVyLiBFbmQgaW5kZXggY2FuXFwndCBiZSBiZWxvdyB6ZXJvLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfSBlbHNlIGlmIChyYW5nZVN0YXJ0Lmd0KGNvdW50Lm1pbnVzKDEpKSkge1xuICAgICAgICAgICAgcmV0dXJuIFtdO1xuICAgICAgICB9XG5cbiAgICAgICAgLy8gdmFsaWRhdGUgcHJvb2ZOb2RlXG4gICAgICAgIGlmICghaXNPYmplY3QocHJvb2ZOb2RlKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB0eXBlIG9mIHByb29mTm9kZSBwYXJhbWV0ZXIuIE9iamVjdCBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgIH1cblxuICAgICAgICB2YXIgaGVpZ2h0ID0gY2FsY0hlaWdodChjb3VudCk7XG4gICAgICAgIHZhciBzdGFydCA9IHJhbmdlU3RhcnQ7XG4gICAgICAgIHZhciBlbmQgPSByYW5nZUVuZC5sdChjb3VudCkgPyByYW5nZUVuZCA6IGNvdW50Lm1pbnVzKDEpO1xuICAgICAgICB2YXIgYWN0dWFsSGFzaCA9IHJlY3Vyc2l2ZShwcm9vZk5vZGUsIDAsIDApO1xuXG4gICAgICAgIGlmICh0eXBlb2YgYWN0dWFsSGFzaCA9PT0gJ3VuZGVmaW5lZCcpIHsgLy8gdHJlZSBpcyBpbnZhbGlkXG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9IGVsc2UgaWYgKHJvb3RIYXNoLnRvTG93ZXJDYXNlKCkgIT09IGFjdHVhbEhhc2gpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ3Jvb3RIYXNoIHBhcmFtZXRlciBpcyBub3QgZXF1YWwgdG8gYWN0dWFsIGhhc2guJyk7XG4gICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICB9IGVsc2UgaWYgKGJpZ0ludChlbGVtZW50cy5sZW5ndGgpLm5lcShlbmQuZXEoc3RhcnQpID8gMSA6IGVuZC5taW51cyhzdGFydCkucGx1cygxKSkpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0FjdHVhbCBlbGVtZW50cyBpbiB0cmVlIGFtb3VudCBpcyBub3QgZXF1YWwgdG8gcmVxdWVzdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuXG4gICAgICAgIHJldHVybiBlbGVtZW50cztcbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBDaGVjayBNZXJrbGUgUGF0cmljaWEgdHJlZSBwcm9vZiBhbmQgcmV0dXJuIGVsZW1lbnRcbiAgICAgKiBAcGFyYW0ge1N0cmluZ30gcm9vdEhhc2hcbiAgICAgKiBAcGFyYW0ge09iamVjdH0gcHJvb2ZOb2RlXG4gICAgICogQHBhcmFtIGtleVxuICAgICAqIEBwYXJhbSB7TmV3VHlwZX0gdHlwZSAob3B0aW9uYWwpXG4gICAgICogQHJldHVybiB7T2JqZWN0fVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIG1lcmtsZVBhdHJpY2lhUHJvb2Yocm9vdEhhc2gsIHByb29mTm9kZSwga2V5LCB0eXBlKSB7XG4gICAgICAgIC8qKlxuICAgICAgICAgKiBHZXQgdmFsdWUgZnJvbSBub2RlXG4gICAgICAgICAqIEBwYXJhbSBkYXRhXG4gICAgICAgICAqIEByZXR1cm5zIHtTdHJpbmcgfHwgQXJyYXkgfHwgT2JqZWN0fVxuICAgICAgICAgKi9cbiAgICAgICAgZnVuY3Rpb24gZ2V0SGFzaChkYXRhKSB7XG4gICAgICAgICAgICB2YXIgZWxlbWVudHNIYXNoO1xuXG4gICAgICAgICAgICBpZiAodHlwZW9mIGRhdGEgPT09ICdzdHJpbmcnKSB7XG4gICAgICAgICAgICAgICAgaWYgKHZhbGlkYXRlSGV4SGFzaChkYXRhKSkge1xuICAgICAgICAgICAgICAgICAgICBlbGVtZW50ID0gZGF0YTtcbiAgICAgICAgICAgICAgICAgICAgZWxlbWVudHNIYXNoID0gaGFzaChoZXhhZGVjaW1hbFRvVWludDhBcnJheShlbGVtZW50KSk7XG4gICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBoZXhhZGVjaW1hbCBzdHJpbmcgaXMgcGFzc2VkIGFzIHZhbHVlIGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2UgaWYgKEFycmF5LmlzQXJyYXkoZGF0YSkpIHtcbiAgICAgICAgICAgICAgICBpZiAodmFsaWRhdGVCeXRlc0FycmF5KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgICAgIGVsZW1lbnQgPSBkYXRhLnNsaWNlKDApOyAvLyBjbG9uZSBhcnJheSBvZiA4LWJpdCBpbnRlZ2Vyc1xuICAgICAgICAgICAgICAgICAgICBlbGVtZW50c0hhc2ggPSBoYXNoKGVsZW1lbnQpO1xuICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICB9IGVsc2UgaWYgKGlzT2JqZWN0KGRhdGEpKSB7XG4gICAgICAgICAgICAgICAgZWxlbWVudCA9IG9iamVjdEFzc2lnbihkYXRhKTsgLy8gZGVlcCBjb3B5XG4gICAgICAgICAgICAgICAgZWxlbWVudHNIYXNoID0gaGFzaChlbGVtZW50LCB0eXBlKTtcbiAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCB2YWx1ZSBub2RlIGluIHRyZWUuIE9iamVjdCBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm47XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmICh0eXBlb2YgZWxlbWVudHNIYXNoID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgIHJldHVybjtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgcmV0dXJuIGVsZW1lbnRzSGFzaDtcbiAgICAgICAgfVxuXG4gICAgICAgIC8qKlxuICAgICAgICAgKiBDaGVjayBlaXRoZXIgc3VmZml4IGlzIGEgcGFydCBvZiBzZWFyY2gga2V5XG4gICAgICAgICAqIEBwYXJhbSB7U3RyaW5nfSBwcmVmaXhcbiAgICAgICAgICogQHBhcmFtIHtTdHJpbmd9IHN1ZmZpeFxuICAgICAgICAgKiBAcmV0dXJucyB7Qm9vbGVhbn1cbiAgICAgICAgICovXG4gICAgICAgIGZ1bmN0aW9uIGlzUGFydE9mU2VhcmNoS2V5KHByZWZpeCwgc3VmZml4KSB7XG4gICAgICAgICAgICB2YXIgZGlmZiA9IGtleUJpbmFyeS5zdWJzdHIocHJlZml4Lmxlbmd0aCk7XG4gICAgICAgICAgICByZXR1cm4gZGlmZlswXSA9PT0gc3VmZml4WzBdO1xuICAgICAgICB9XG5cbiAgICAgICAgLyoqXG4gICAgICAgICAqIFJlY3Vyc2l2ZSB0cmVlIHRyYXZlcnNhbCBmdW5jdGlvblxuICAgICAgICAgKiBAcGFyYW0ge09iamVjdH0gbm9kZVxuICAgICAgICAgKiBAcGFyYW0ge1N0cmluZ30ga2V5UHJlZml4XG4gICAgICAgICAqIEByZXR1cm5zIHtTdHJpbmd9XG4gICAgICAgICAqL1xuICAgICAgICBmdW5jdGlvbiByZWN1cnNpdmUobm9kZSwga2V5UHJlZml4KSB7XG4gICAgICAgICAgICBpZiAoZ2V0T2JqZWN0TGVuZ3RoKG5vZGUpICE9PSAyKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBudW1iZXIgb2YgY2hpbGRyZW4gaW4gdGhlIHRyZWUgbm9kZS4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgdmFyIGxldmVsRGF0YSA9IHt9O1xuXG4gICAgICAgICAgICBmb3IgKHZhciBrZXlTdWZmaXggaW4gbm9kZSkge1xuICAgICAgICAgICAgICAgIGlmICghbm9kZS5oYXNPd25Qcm9wZXJ0eShrZXlTdWZmaXgpKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnRpbnVlO1xuICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgIC8vIHZhbGlkYXRlIGtleVxuICAgICAgICAgICAgICAgIGlmICghdmFsaWRhdGVCaW5hcnlTdHJpbmcoa2V5U3VmZml4KSkge1xuICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICB2YXIgYnJhbmNoVmFsdWVIYXNoO1xuICAgICAgICAgICAgICAgIHZhciBmdWxsS2V5ID0ga2V5UHJlZml4ICsga2V5U3VmZml4O1xuICAgICAgICAgICAgICAgIHZhciBub2RlVmFsdWUgPSBub2RlW2tleVN1ZmZpeF07XG4gICAgICAgICAgICAgICAgdmFyIGJyYW5jaFR5cGU7XG4gICAgICAgICAgICAgICAgdmFyIGJyYW5jaEtleTtcbiAgICAgICAgICAgICAgICB2YXIgYnJhbmNoS2V5SGFzaDtcblxuICAgICAgICAgICAgICAgIGlmIChmdWxsS2V5Lmxlbmd0aCA9PT0gQ09OU1QuTUVSS0xFX1BBVFJJQ0lBX0tFWV9MRU5HVEggKiA4KSB7XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2Ygbm9kZVZhbHVlID09PSAnc3RyaW5nJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2gobm9kZVZhbHVlKSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgICAgICAgICAgYnJhbmNoVmFsdWVIYXNoID0gbm9kZVZhbHVlO1xuICAgICAgICAgICAgICAgICAgICAgICAgYnJhbmNoVHlwZSA9ICdoYXNoJztcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIGlmIChpc09iamVjdChub2RlVmFsdWUpKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBpZiAodHlwZW9mIG5vZGVWYWx1ZS52YWwgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignTGVhZiB0cmVlIGNvbnRhaW5zIGludmFsaWQgZGF0YS4nKTtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgICAgIH0gZWxzZSBpZiAodHlwZW9mIGVsZW1lbnQgIT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignVHJlZSBjYW4gbm90IGNvbnRhaW5zIG1vcmUgdGhhbiBvbmUgbm9kZSB3aXRoIHZhbHVlLicpO1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgICAgICAgICBicmFuY2hWYWx1ZUhhc2ggPSBnZXRIYXNoKG5vZGVWYWx1ZS52YWwpO1xuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBicmFuY2hWYWx1ZUhhc2ggPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgICAgICAgICBicmFuY2hUeXBlID0gJ3ZhbHVlJztcbiAgICAgICAgICAgICAgICAgICAgfSAgZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2Ygbm9kZSBpbiB0cmVlIGxlYWYuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgICAgIGJyYW5jaEtleUhhc2ggPSBiaW5hcnlTdHJpbmdUb0hleGFkZWNpbWFsKGZ1bGxLZXkpO1xuICAgICAgICAgICAgICAgICAgICBicmFuY2hLZXkgPSB7XG4gICAgICAgICAgICAgICAgICAgICAgICB2YXJpYW50OiAxLFxuICAgICAgICAgICAgICAgICAgICAgICAga2V5OiBicmFuY2hLZXlIYXNoLFxuICAgICAgICAgICAgICAgICAgICAgICAgbGVuZ3RoOiAwXG4gICAgICAgICAgICAgICAgICAgIH07XG4gICAgICAgICAgICAgICAgfSBlbHNlIGlmIChmdWxsS2V5Lmxlbmd0aCA8IENPTlNULk1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIICogOCkgeyAvLyBub2RlIGlzIGJyYW5jaFxuICAgICAgICAgICAgICAgICAgICBpZiAodHlwZW9mIG5vZGVWYWx1ZSA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKG5vZGVWYWx1ZSkpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgICAgIGJyYW5jaFZhbHVlSGFzaCA9IG5vZGVWYWx1ZTtcbiAgICAgICAgICAgICAgICAgICAgICAgIGJyYW5jaFR5cGUgPSAnaGFzaCc7XG4gICAgICAgICAgICAgICAgICAgIH0gZWxzZSBpZiAoaXNPYmplY3Qobm9kZVZhbHVlKSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKHR5cGVvZiBub2RlVmFsdWUudmFsICE9PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ05vZGUgd2l0aCB2YWx1ZSBpcyBhdCBub24tbGVhZiBwb3NpdGlvbiBpbiB0cmVlLicpO1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgICAgICAgICBicmFuY2hWYWx1ZUhhc2ggPSByZWN1cnNpdmUobm9kZVZhbHVlLCBmdWxsS2V5KTtcbiAgICAgICAgICAgICAgICAgICAgICAgIGlmIChicmFuY2hWYWx1ZUhhc2ggPT09IG51bGwpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgICAgIGJyYW5jaFR5cGUgPSAnYnJhbmNoJztcbiAgICAgICAgICAgICAgICAgICAgfSAgZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2Ygbm9kZSBpbiB0cmVlLicpO1xuICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgICAgICB2YXIgYmluYXJ5S2V5TGVuZ3RoID0gZnVsbEtleS5sZW5ndGg7XG4gICAgICAgICAgICAgICAgICAgIHZhciBiaW5hcnlLZXkgPSBmdWxsS2V5O1xuXG4gICAgICAgICAgICAgICAgICAgIGZvciAodmFyIGogPSAwOyBqIDwgKENPTlNULk1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIICogOCAtIGZ1bGxLZXkubGVuZ3RoKTsgaisrKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBiaW5hcnlLZXkgKz0gJzAnO1xuICAgICAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICAgICAgYnJhbmNoS2V5SGFzaCA9IGJpbmFyeVN0cmluZ1RvSGV4YWRlY2ltYWwoYmluYXJ5S2V5KTtcbiAgICAgICAgICAgICAgICAgICAgYnJhbmNoS2V5ID0ge1xuICAgICAgICAgICAgICAgICAgICAgICAgdmFyaWFudDogMCxcbiAgICAgICAgICAgICAgICAgICAgICAgIGtleTogYnJhbmNoS2V5SGFzaCxcbiAgICAgICAgICAgICAgICAgICAgICAgIGxlbmd0aDogYmluYXJ5S2V5TGVuZ3RoXG4gICAgICAgICAgICAgICAgICAgIH07XG4gICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBsZW5ndGggb2Yga2V5IGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgIHJldHVybiBudWxsO1xuICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgIGlmIChrZXlTdWZmaXhbMF0gPT09ICcwJykgeyAvLyAnMCcgYXQgdGhlIGJlZ2lubmluZyBtZWFucyBsZWZ0IGJyYW5jaC9sZWFmXG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgbGV2ZWxEYXRhLmxlZnQgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBsZXZlbERhdGEubGVmdCA9IHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBoYXNoOiBicmFuY2hWYWx1ZUhhc2gsXG4gICAgICAgICAgICAgICAgICAgICAgICAgICAga2V5OiBicmFuY2hLZXksXG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgdHlwZTogYnJhbmNoVHlwZSxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBzdWZmaXg6IGtleVN1ZmZpeCxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBzaXplOiBmdWxsS2V5Lmxlbmd0aFxuICAgICAgICAgICAgICAgICAgICAgICAgfTtcbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0xlZnQgbm9kZSBpcyBkdXBsaWNhdGVkIGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgIH0gZWxzZSB7IC8vICcxJyBhdCB0aGUgYmVnaW5uaW5nIG1lYW5zIHJpZ2h0IGJyYW5jaC9sZWFmXG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgbGV2ZWxEYXRhLnJpZ2h0ID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgbGV2ZWxEYXRhLnJpZ2h0ID0ge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIGhhc2g6IGJyYW5jaFZhbHVlSGFzaCxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBrZXk6IGJyYW5jaEtleSxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICB0eXBlOiBicmFuY2hUeXBlLFxuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHN1ZmZpeDoga2V5U3VmZml4LFxuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHNpemU6IGZ1bGxLZXkubGVuZ3RoXG4gICAgICAgICAgICAgICAgICAgICAgICB9O1xuICAgICAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignUmlnaHQgbm9kZSBpcyBkdXBsaWNhdGVkIGluIHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gbnVsbDtcbiAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgaWYgKChsZXZlbERhdGEubGVmdC50eXBlID09PSAnaGFzaCcpICYmIChsZXZlbERhdGEucmlnaHQudHlwZSA9PT0gJ2hhc2gnKSAmJiAoZnVsbEtleS5sZW5ndGggPCBDT05TVC5NRVJLTEVfUEFUUklDSUFfS0VZX0xFTkdUSCAqIDgpKSB7XG4gICAgICAgICAgICAgICAgaWYgKGlzUGFydE9mU2VhcmNoS2V5KGtleVByZWZpeCwgbGV2ZWxEYXRhLmxlZnQuc3VmZml4KSkge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdUcmVlIGlzIGludmFsaWQuIExlZnQga2V5IGlzIGEgcGFydCBvZiBzZWFyY2gga2V5IGJ1dCBpdHMgYnJhbmNoIGlzIG5vdCBleHBhbmRlZC4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgfSBlbHNlIGlmIChpc1BhcnRPZlNlYXJjaEtleShrZXlQcmVmaXgsIGxldmVsRGF0YS5yaWdodC5zdWZmaXgpKSB7XG4gICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1RyZWUgaXMgaW52YWxpZC4gUmlnaHQga2V5IGlzIGEgcGFydCBvZiBzZWFyY2gga2V5IGJ1dCBpdHMgYnJhbmNoIGlzIG5vdCBleHBhbmRlZC4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICByZXR1cm4gaGFzaCh7XG4gICAgICAgICAgICAgICAgbGVmdF9oYXNoOiBsZXZlbERhdGEubGVmdC5oYXNoLFxuICAgICAgICAgICAgICAgIHJpZ2h0X2hhc2g6IGxldmVsRGF0YS5yaWdodC5oYXNoLFxuICAgICAgICAgICAgICAgIGxlZnRfa2V5OiBsZXZlbERhdGEubGVmdC5rZXksXG4gICAgICAgICAgICAgICAgcmlnaHRfa2V5OiBsZXZlbERhdGEucmlnaHQua2V5XG4gICAgICAgICAgICB9LCBCcmFuY2gpO1xuICAgICAgICB9XG5cbiAgICAgICAgdmFyIGVsZW1lbnQ7XG5cbiAgICAgICAgLy8gdmFsaWRhdGUgcm9vdEhhc2ggcGFyYW1ldGVyXG4gICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKHJvb3RIYXNoKSkge1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICB2YXIgcm9vdEhhc2ggPSByb290SGFzaC50b0xvd2VyQ2FzZSgpO1xuXG4gICAgICAgIC8vIHZhbGlkYXRlIHByb29mTm9kZSBwYXJhbWV0ZXJcbiAgICAgICAgaWYgKCFpc09iamVjdChwcm9vZk5vZGUpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2YgcHJvb2ZOb2RlIHBhcmFtZXRlci4gT2JqZWN0IGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuXG4gICAgICAgIC8vIHZhbGlkYXRlIGtleSBwYXJhbWV0ZXJcbiAgICAgICAgdmFyIGtleSA9IGtleTtcbiAgICAgICAgaWYgKEFycmF5LmlzQXJyYXkoa2V5KSkge1xuICAgICAgICAgICAgaWYgKHZhbGlkYXRlQnl0ZXNBcnJheShrZXksIENPTlNULk1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIKSkge1xuICAgICAgICAgICAgICAgIGtleSA9IHVpbnQ4QXJyYXlUb0hleGFkZWNpbWFsKGtleSk7XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgICAgICB9XG4gICAgICAgIH0gZWxzZSBpZiAodHlwZW9mIGtleSA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKGtleSwgQ09OU1QuTUVSS0xFX1BBVFJJQ0lBX0tFWV9MRU5HVEgpKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgdHlwZSBvZiBrZXkgcGFyYW1ldGVyLiBBYXJyYXkgb2YgOC1iaXQgaW50ZWdlcnMgb3IgaGV4YWRlY2ltYWwgc3RyaW5nIGlzIGV4cGVjdGVkLicpO1xuICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgfVxuICAgICAgICB2YXIga2V5QmluYXJ5ID0gaGV4YWRlY2ltYWxUb0JpbmFyeVN0cmluZyhrZXkpO1xuXG4gICAgICAgIHZhciBwcm9vZk5vZGVSb290TnVtYmVyT2ZOb2RlcyA9IGdldE9iamVjdExlbmd0aChwcm9vZk5vZGUpO1xuICAgICAgICBpZiAocHJvb2ZOb2RlUm9vdE51bWJlck9mTm9kZXMgPT09IDApIHtcbiAgICAgICAgICAgIGlmIChyb290SGFzaCA9PT0gKG5ldyBVaW50OEFycmF5KENPTlNULk1FUktMRV9QQVRSSUNJQV9LRVlfTEVOR1RIICogMikpLmpvaW4oJycpKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7XG4gICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ0ludmFsaWQgcm9vdEhhc2ggcGFyYW1ldGVyIG9mIGVtcHR5IHRyZWUuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIGlmIChwcm9vZk5vZGVSb290TnVtYmVyT2ZOb2RlcyA9PT0gMSkge1xuICAgICAgICAgICAgZm9yICh2YXIgaSBpbiBwcm9vZk5vZGUpIHtcbiAgICAgICAgICAgICAgICBpZiAoIXByb29mTm9kZS5oYXNPd25Qcm9wZXJ0eShpKSkge1xuICAgICAgICAgICAgICAgICAgICBjb250aW51ZTtcbiAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICBpZiAoIXZhbGlkYXRlQmluYXJ5U3RyaW5nKGksIDI1NikpIHtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgICAgICB2YXIgZGF0YSA9IHByb29mTm9kZVtpXTtcbiAgICAgICAgICAgICAgICB2YXIgbm9kZUtleUJ1ZmZlciA9IGJpbmFyeVN0cmluZ1RvVWludDhBcnJheShpKTtcbiAgICAgICAgICAgICAgICB2YXIgbm9kZUtleSA9IHVpbnQ4QXJyYXlUb0hleGFkZWNpbWFsKG5vZGVLZXlCdWZmZXIpO1xuICAgICAgICAgICAgICAgIHZhciBub2RlSGFzaDtcblxuICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgZGF0YSA9PT0gJ3N0cmluZycpIHtcbiAgICAgICAgICAgICAgICAgICAgaWYgKCF2YWxpZGF0ZUhleEhhc2goZGF0YSkpIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgICAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgICAgICAgICBub2RlSGFzaCA9IGhhc2goe1xuICAgICAgICAgICAgICAgICAgICAgICAga2V5OiB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgdmFyaWFudDogMSxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBrZXk6IG5vZGVLZXksXG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgbGVuZ3RoOiAwXG4gICAgICAgICAgICAgICAgICAgICAgICB9LFxuICAgICAgICAgICAgICAgICAgICAgICAgaGFzaDogZGF0YVxuICAgICAgICAgICAgICAgICAgICB9LCBSb290QnJhbmNoKTtcblxuICAgICAgICAgICAgICAgICAgICBpZiAocm9vdEhhc2ggPT09IG5vZGVIYXNoKSB7XG4gICAgICAgICAgICAgICAgICAgICAgICBpZiAoa2V5ICE9PSBub2RlS2V5KSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7IC8vIG5vIGVsZW1lbnQgd2l0aCBkYXRhIGluIHRyZWVcbiAgICAgICAgICAgICAgICAgICAgICAgIH0gZWxzZSB7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignSW52YWxpZCBrZXkgd2l0aCBoYXNoIGlzIGluIHRoZSByb290IG9mIHByb29mTm9kZSBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ3Jvb3RIYXNoIHBhcmFtZXRlciBpcyBub3QgZXF1YWwgdG8gYWN0dWFsIGhhc2guJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICAgICAgICAgICAgICB9XG4gICAgICAgICAgICAgICAgfSBlbHNlIGlmIChpc09iamVjdChkYXRhKSkge1xuICAgICAgICAgICAgICAgICAgICB2YXIgZWxlbWVudHNIYXNoID0gZ2V0SGFzaChkYXRhLnZhbCk7XG4gICAgICAgICAgICAgICAgICAgIGlmICh0eXBlb2YgZWxlbWVudHNIYXNoID09PSAndW5kZWZpbmVkJykge1xuICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICAgICAgICAgIG5vZGVIYXNoID0gaGFzaCh7XG4gICAgICAgICAgICAgICAgICAgICAgICBrZXk6IHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICB2YXJpYW50OiAxLFxuICAgICAgICAgICAgICAgICAgICAgICAgICAgIGtleTogbm9kZUtleSxcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBsZW5ndGg6IDBcbiAgICAgICAgICAgICAgICAgICAgICAgIH0sXG4gICAgICAgICAgICAgICAgICAgICAgICBoYXNoOiBlbGVtZW50c0hhc2hcbiAgICAgICAgICAgICAgICAgICAgfSwgUm9vdEJyYW5jaCk7XG5cbiAgICAgICAgICAgICAgICAgICAgaWYgKHJvb3RIYXNoID09PSBub2RlSGFzaCkge1xuICAgICAgICAgICAgICAgICAgICAgICAgaWYgKGtleSA9PT0gbm9kZUtleSkge1xuICAgICAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiBlbGVtZW50O1xuICAgICAgICAgICAgICAgICAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIGtleSB3aXRoIHZhbHVlIGlzIGluIHRoZSByb290IG9mIHByb29mTm9kZSBwYXJhbWV0ZXIuJyk7XG4gICAgICAgICAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZFxuICAgICAgICAgICAgICAgICAgICAgICAgfVxuICAgICAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcigncm9vdEhhc2ggcGFyYW1ldGVyIGlzIG5vdCBlcXVhbCB0byBhY3R1YWwgaGFzaC4nKTtcbiAgICAgICAgICAgICAgICAgICAgICAgIHJldHVybiB1bmRlZmluZWQ7XG4gICAgICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgICAgICB9IGVsc2Uge1xuICAgICAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdJbnZhbGlkIHR5cGUgb2YgdmFsdWUgaW4gdGhlIHJvb3Qgb2YgcHJvb2ZOb2RlIHBhcmFtZXRlci4nKTtcbiAgICAgICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIH1cbiAgICAgICAgfSBlbHNlIHtcbiAgICAgICAgICAgIHZhciBhY3R1YWxIYXNoID0gcmVjdXJzaXZlKHByb29mTm9kZSwgJycpO1xuXG4gICAgICAgICAgICBpZiAoYWN0dWFsSGFzaCA9PT0gbnVsbCkgeyAvLyB0cmVlIGlzIGludmFsaWRcbiAgICAgICAgICAgICAgICByZXR1cm4gdW5kZWZpbmVkO1xuICAgICAgICAgICAgfSBlbHNlIGlmIChyb290SGFzaCAhPT0gYWN0dWFsSGFzaCkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ3Jvb3RIYXNoIHBhcmFtZXRlciBpcyBub3QgZXF1YWwgdG8gYWN0dWFsIGhhc2guJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIHVuZGVmaW5lZDtcbiAgICAgICAgICAgIH0gZWxzZSBpZiAodHlwZW9mIGVsZW1lbnQgPT09ICd1bmRlZmluZWQnKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIG51bGw7IC8vIG5vIGVsZW1lbnQgd2l0aCBkYXRhIGluIHRyZWVcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgcmV0dXJuIGVsZW1lbnQ7XG4gICAgICAgIH1cbiAgICB9XG5cbiAgICAvKipcbiAgICAgKiBWZXJpZmllcyBibG9ja1xuICAgICAqIEBwYXJhbSB7T2JqZWN0fSBkYXRhXG4gICAgICogQHBhcmFtIHtBcnJheX0gdmFsaWRhdG9yc1xuICAgICAqIEByZXR1cm4ge0Jvb2xlYW59XG4gICAgICovXG4gICAgZnVuY3Rpb24gdmVyaWZ5QmxvY2soZGF0YSwgdmFsaWRhdG9ycykge1xuICAgICAgICBpZiAoIWlzT2JqZWN0KGRhdGEpKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyB0eXBlIG9mIGRhdGEgcGFyYW1ldGVyLiBPYmplY3QgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH0gZWxzZSBpZiAoIWlzT2JqZWN0KGRhdGEuYmxvY2spKSB7XG4gICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyB0eXBlIG9mIGJsb2NrIGZpZWxkIGluIGRhdGEgcGFyYW1ldGVyLiBPYmplY3QgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgIH0gZWxzZSBpZiAoIUFycmF5LmlzQXJyYXkoZGF0YS5wcmVjb21taXRzKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgdHlwZSBvZiBwcmVjb21taXRzIGZpZWxkIGluIGRhdGEgcGFyYW1ldGVyLiBBcnJheSBpcyBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfSBlbHNlIGlmICghQXJyYXkuaXNBcnJheSh2YWxpZGF0b3JzKSkge1xuICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgdHlwZSBvZiB2YWxpZGF0b3JzcGFyYW1ldGVyLiBBcnJheSBpcyBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgfVxuXG4gICAgICAgIGZvciAodmFyIGkgaW4gdmFsaWRhdG9ycykge1xuICAgICAgICAgICAgaWYgKCF2YWxpZGF0b3JzLmhhc093blByb3BlcnR5KGkpKSB7XG4gICAgICAgICAgICAgICAgY29udGludWU7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmICghdmFsaWRhdGVIZXhIYXNoKHZhbGlkYXRvcnNbaV0pKSB7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfVxuICAgICAgICB9XG5cbiAgICAgICAgdmFyIHZhbGlkYXRvcnNUb3RhbE51bWJlciA9IHZhbGlkYXRvcnMubGVuZ3RoO1xuICAgICAgICB2YXIgdW5pcXVlVmFsaWRhdG9ycyA9IFtdO1xuXG4gICAgICAgIHZhciByb3VuZDtcbiAgICAgICAgdmFyIGJsb2NrSGFzaCA9IGhhc2goZGF0YS5ibG9jaywgQmxvY2spO1xuXG4gICAgICAgIGZvciAodmFyIGkgaW4gZGF0YS5wcmVjb21taXRzKSB7XG4gICAgICAgICAgICBpZiAoIWRhdGEucHJlY29tbWl0cy5oYXNPd25Qcm9wZXJ0eShpKSkge1xuICAgICAgICAgICAgICAgIGNvbnRpbnVlO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICB2YXIgcHJlY29tbWl0ID0gZGF0YS5wcmVjb21taXRzW2ldO1xuXG4gICAgICAgICAgICBpZiAoIWlzT2JqZWN0KHByZWNvbW1pdC5ib2R5KSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIHR5cGUgb2YgcHJlY29tbWl0cyBib2R5LiBPYmplY3QgaXMgZXhwZWN0ZWQuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfSBlbHNlIGlmICghdmFsaWRhdGVIZXhIYXNoKHByZWNvbW1pdC5zaWduYXR1cmUsIDY0KSkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIHR5cGUgb2YgcHJlY29tbWl0cyBzaWduYXR1cmUuIEhleGFkZWNpbWFsIG9mIDY0IGxlbmd0aCBpcyBleHBlY3RlZC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmIChwcmVjb21taXQuYm9keS52YWxpZGF0b3IgPj0gdmFsaWRhdG9yc1RvdGFsTnVtYmVyKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgaW5kZXggcGFzc2VkLiBWYWxpZGF0b3IgZG9lcyBub3QgZXhpc3QuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICBpZiAodW5pcXVlVmFsaWRhdG9ycy5pbmRleE9mKHByZWNvbW1pdC5ib2R5LnZhbGlkYXRvcikgPT09IC0xKSB7XG4gICAgICAgICAgICAgICAgdW5pcXVlVmFsaWRhdG9ycy5wdXNoKHByZWNvbW1pdC5ib2R5LnZhbGlkYXRvcik7XG4gICAgICAgICAgICB9XG5cbiAgICAgICAgICAgIGlmIChwcmVjb21taXQuYm9keS5oZWlnaHQgIT09IGRhdGEuYmxvY2suaGVpZ2h0KSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3JvbmcgaGVpZ2h0IG9mIGJsb2NrIGluIHByZWNvbW1pdC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9IGVsc2UgaWYgKHByZWNvbW1pdC5ib2R5LmJsb2NrX2hhc2ggIT09IGJsb2NrSGFzaCkge1xuICAgICAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ1dyb25nIGhhc2ggb2YgYmxvY2sgaW4gcHJlY29tbWl0LicpO1xuICAgICAgICAgICAgICAgIHJldHVybiBmYWxzZTtcbiAgICAgICAgICAgIH1cblxuICAgICAgICAgICAgaWYgKHR5cGVvZiByb3VuZCA9PT0gJ3VuZGVmaW5lZCcpIHtcbiAgICAgICAgICAgICAgICByb3VuZCA9IHByZWNvbW1pdC5ib2R5LnJvdW5kO1xuICAgICAgICAgICAgfSBlbHNlIGlmIChwcmVjb21taXQuYm9keS5yb3VuZCAhPT0gcm91bmQpIHtcbiAgICAgICAgICAgICAgICBjb25zb2xlLmVycm9yKCdXcm9uZyByb3VuZCBpbiBwcmVjb21taXQuJyk7XG4gICAgICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICAgICAgfVxuXG4gICAgICAgICAgICB2YXIgcHVibGljS2V5ID0gdmFsaWRhdG9yc1twcmVjb21taXQuYm9keS52YWxpZGF0b3JdO1xuXG4gICAgICAgICAgICBpZiAoIXZlcmlmeVNpZ25hdHVyZShwcmVjb21taXQuYm9keSwgUHJlY29tbWl0LCBwcmVjb21taXQuc2lnbmF0dXJlLCBwdWJsaWNLZXkpKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5lcnJvcignV3Jvbmcgc2lnbmF0dXJlIG9mIHByZWNvbW1pdC4nKTtcbiAgICAgICAgICAgICAgICByZXR1cm4gZmFsc2U7XG4gICAgICAgICAgICB9XG4gICAgICAgIH1cblxuICAgICAgICBpZiAodW5pcXVlVmFsaWRhdG9ycy5sZW5ndGggPD0gdmFsaWRhdG9yc1RvdGFsTnVtYmVyICogMiAvIDMpIHtcbiAgICAgICAgICAgIGNvbnNvbGUuZXJyb3IoJ05vdCBlbm91Z2ggcHJlY29tbWl0cyBmcm9tIHVuaXF1ZSB2YWxpZGF0b3JzLicpO1xuICAgICAgICAgICAgcmV0dXJuIGZhbHNlO1xuICAgICAgICB9XG5cbiAgICAgICAgcmV0dXJuIHRydWU7XG4gICAgfVxuXG4gICAgLyoqXG4gICAgICogR2VuZXJhdGUgcmFuZG9tIHBhaXIgb2YgcHVibGljS2V5IGFuZCBzZWNyZXRLZXlcbiAgICAgKiBAcmV0dXJuIHtPYmplY3R9XG4gICAgICogIHB1YmxpY0tleSB7U3RyaW5nfVxuICAgICAqICBzZWNyZXRLZXkge1N0cmluZ31cbiAgICAgKi9cbiAgICBmdW5jdGlvbiBrZXlQYWlyKCkge1xuICAgICAgICB2YXIgcGFpciA9IG5hY2wuc2lnbi5rZXlQYWlyKCk7XG4gICAgICAgIHZhciBwdWJsaWNLZXkgPSB1aW50OEFycmF5VG9IZXhhZGVjaW1hbChwYWlyLnB1YmxpY0tleSk7XG4gICAgICAgIHZhciBzZWNyZXRLZXkgPSB1aW50OEFycmF5VG9IZXhhZGVjaW1hbChwYWlyLnNlY3JldEtleSk7XG5cbiAgICAgICAgcmV0dXJuIHtcbiAgICAgICAgICAgIHB1YmxpY0tleTogcHVibGljS2V5LFxuICAgICAgICAgICAgc2VjcmV0S2V5OiBzZWNyZXRLZXlcbiAgICAgICAgfVxuICAgIH1cblxuICAgIC8qKiBHZW5lcmF0ZSByYW5kb20gbnVtYmVyIG9mIHR5cGUgb2YgVWludDY0XG4gICAgICogQHJldHVybiB7U3RyaW5nfVxuICAgICAqL1xuICAgIGZ1bmN0aW9uIHJhbmRvbVVpbnQ2NCgpIHtcbiAgICAgICAgcmV0dXJuIGJpZ0ludC5yYW5kQmV0d2VlbigwLCBDT05TVC5NQVhfVUlOVDY0KTtcbiAgICB9XG5cbiAgICByZXR1cm4ge1xuXG4gICAgICAgIC8vIEFQSSBtZXRob2RzXG4gICAgICAgIG5ld1R5cGU6IGNyZWF0ZU5ld1R5cGUsXG4gICAgICAgIG5ld01lc3NhZ2U6IGNyZWF0ZU5ld01lc3NhZ2UsXG4gICAgICAgIGhhc2g6IGhhc2gsXG4gICAgICAgIHNpZ246IHNpZ24sXG4gICAgICAgIHZlcmlmeVNpZ25hdHVyZTogdmVyaWZ5U2lnbmF0dXJlLFxuICAgICAgICBtZXJrbGVQcm9vZjogbWVya2xlUHJvb2YsXG4gICAgICAgIG1lcmtsZVBhdHJpY2lhUHJvb2Y6IG1lcmtsZVBhdHJpY2lhUHJvb2YsXG4gICAgICAgIHZlcmlmeUJsb2NrOiB2ZXJpZnlCbG9jayxcbiAgICAgICAga2V5UGFpcjoga2V5UGFpcixcbiAgICAgICAgcmFuZG9tVWludDY0OiByYW5kb21VaW50NjQsXG5cbiAgICAgICAgLy8gYnVpbHQtaW4gdHlwZXNcbiAgICAgICAgSW50ODogSW50OCxcbiAgICAgICAgSW50MTY6IEludDE2LFxuICAgICAgICBJbnQzMjogSW50MzIsXG4gICAgICAgIEludDY0OiBJbnQ2NCxcbiAgICAgICAgVWludDg6IFVpbnQ4LFxuICAgICAgICBVaW50MTY6IFVpbnQxNixcbiAgICAgICAgVWludDMyOiBVaW50MzIsXG4gICAgICAgIFVpbnQ2NDogVWludDY0LFxuICAgICAgICBTdHJpbmc6IFN0cmluZyxcbiAgICAgICAgSGFzaDogSGFzaCxcbiAgICAgICAgRGlnZXN0OiBEaWdlc3QsXG4gICAgICAgIFB1YmxpY0tleTogUHVibGljS2V5LFxuICAgICAgICBUaW1lc3BlYzogVGltZXNwZWMsXG4gICAgICAgIEJvb2w6IEJvb2xcblxuICAgIH07XG5cbn0pKCk7XG5cbm1vZHVsZS5leHBvcnRzID0gRXhvbnVtQ2xpZW50O1xuIl19
