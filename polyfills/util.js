/**
 * Util polyfill for browser environments
 * Provides Node.js-compatible util functionality
 */
(function() {
    'use strict';

    var toString = Object.prototype.toString;
    var hasOwnProperty = Object.prototype.hasOwnProperty;

    function isArray(obj) {
        return Array.isArray ? Array.isArray(obj) : toString.call(obj) === '[object Array]';
    }

    function isDate(obj) {
        return toString.call(obj) === '[object Date]';
    }

    function isRegExp(obj) {
        return toString.call(obj) === '[object RegExp]';
    }

    function isError(obj) {
        return toString.call(obj) === '[object Error]' || obj instanceof Error;
    }

    function isFunction(obj) {
        return typeof obj === 'function';
    }

    function isObject(obj) {
        return typeof obj === 'object' && obj !== null;
    }

    function isPrimitive(obj) {
        return obj === null ||
            typeof obj === 'boolean' ||
            typeof obj === 'number' ||
            typeof obj === 'string' ||
            typeof obj === 'symbol' ||
            typeof obj === 'undefined';
    }

    function isString(obj) {
        return typeof obj === 'string';
    }

    function isNumber(obj) {
        return typeof obj === 'number';
    }

    function isBoolean(obj) {
        return typeof obj === 'boolean';
    }

    function isNull(obj) {
        return obj === null;
    }

    function isUndefined(obj) {
        return obj === undefined;
    }

    function isNullOrUndefined(obj) {
        return obj == null;
    }

    function isBuffer(obj) {
        return obj && typeof obj === 'object' &&
            typeof obj.constructor === 'function' &&
            typeof obj.constructor.isBuffer === 'function' &&
            obj.constructor.isBuffer(obj);
    }

    function inspect(obj, opts) {
        opts = opts || {};
        var depth = typeof opts.depth !== 'undefined' ? opts.depth : 2;
        var colors = opts.colors || false;
        var showHidden = opts.showHidden || false;

        function formatValue(value, recurseTimes) {
            // Handle null and undefined
            if (value === null) return 'null';
            if (value === undefined) return 'undefined';

            // Handle primitives
            if (isPrimitive(value)) {
                if (isString(value)) {
                    return "'" + value.replace(/'/g, "\\'") + "'";
                }
                return String(value);
            }

            // Handle recursion depth
            if (recurseTimes < 0) {
                if (isRegExp(value)) {
                    return value.toString();
                } else {
                    return '[Object]';
                }
            }

            // Handle arrays
            if (isArray(value)) {
                var output = [];
                for (var i = 0; i < value.length; i++) {
                    if (hasOwnProperty.call(value, i)) {
                        output.push(formatValue(value[i], recurseTimes - 1));
                    } else {
                        output.push('');
                    }
                }
                return '[ ' + output.join(', ') + ' ]';
            }

            // Handle functions
            if (isFunction(value)) {
                var name = value.name ? ': ' + value.name : '';
                return '[Function' + name + ']';
            }

            // Handle dates
            if (isDate(value)) {
                return value.toISOString();
            }

            // Handle regex
            if (isRegExp(value)) {
                return value.toString();
            }

            // Handle errors
            if (isError(value)) {
                return '[' + value.toString() + ']';
            }

            // Handle objects
            var keys = Object.keys(value);
            if (!showHidden) {
                keys = keys.filter(function(key) {
                    return key[0] !== '_';
                });
            }

            if (keys.length === 0) {
                if (isFunction(value)) {
                    var name = value.name ? ': ' + value.name : '';
                    return '[Function' + name + ']';
                }
                return '{}';
            }

            var output = [];
            for (var i = 0; i < keys.length; i++) {
                var key = keys[i];
                var str = key + ': ' + formatValue(value[key], recurseTimes - 1);
                output.push(str);
            }

            return '{ ' + output.join(', ') + ' }';
        }

        return formatValue(obj, depth);
    }

    function format(f) {
        var i = 1;
        var args = arguments;
        var str = String(f).replace(/(%?)(%([sdj%]))/g, function(x, escapeChar, format, specifier) {
            if (escapeChar) return x;
            if (i >= args.length) return x;

            switch (specifier) {
                case 's': return String(args[i++]);
                case 'd': return Number(args[i++]);
                case 'j':
                    try {
                        return JSON.stringify(args[i++]);
                    } catch (_) {
                        return '[Circular]';
                    }
                case '%': return '%';
                default:
                    return x;
            }
        });

        for (var arg = args[i]; i < args.length; arg = args[++i]) {
            if (isNull(arg) || (!isObject(arg) && !isFunction(arg))) {
                str += ' ' + arg;
            } else {
                str += ' ' + inspect(arg);
            }
        }

        return str;
    }

    function deprecate(fn, msg) {
        if (typeof process !== 'undefined' && process.noDeprecation === true) {
            return fn;
        }

        var warned = false;
        function deprecated() {
            if (!warned) {
                console.warn(msg);
                warned = true;
            }
            return fn.apply(this, arguments);
        }

        return deprecated;
    }

    function inherits(ctor, superCtor) {
        ctor.super_ = superCtor;
        ctor.prototype = Object.create(superCtor.prototype, {
            constructor: {
                value: ctor,
                enumerable: false,
                writable: true,
                configurable: true
            }
        });
    }

    function promisify(original) {
        if (typeof original !== 'function') {
            throw new TypeError('The "original" argument must be of type Function');
        }

        function fn() {
            var args = Array.prototype.slice.call(arguments);
            var self = this;

            return new Promise(function(resolve, reject) {
                args.push(function(err, value) {
                    if (err) {
                        reject(err);
                    } else {
                        resolve(value);
                    }
                });

                try {
                    original.apply(self, args);
                } catch (err) {
                    reject(err);
                }
            });
        }

        Object.setPrototypeOf(fn, Object.getPrototypeOf(original));
        Object.defineProperty(fn, 'name', { value: original.name });
        return fn;
    }

    function callbackify(original) {
        if (typeof original !== 'function') {
            throw new TypeError('The "original" argument must be of type Function');
        }

        function callbackified() {
            var args = Array.prototype.slice.call(arguments);
            var maybeCb = args.pop();

            if (typeof maybeCb !== 'function') {
                throw new TypeError('The last argument must be of type Function');
            }

            var self = this;
            var ret;

            try {
                ret = original.apply(self, args);
            } catch (err) {
                return setTimeout(maybeCb, 0, err);
            }

            if (ret && typeof ret.then === 'function') {
                ret.then(
                    function(value) { setTimeout(maybeCb, 0, null, value); },
                    function(err) { setTimeout(maybeCb, 0, err); }
                );
            } else {
                setTimeout(maybeCb, 0, null, ret);
            }
        }

        Object.setPrototypeOf(callbackified, Object.getPrototypeOf(original));
        Object.defineProperty(callbackified, 'name', { value: original.name });
        return callbackified;
    }

    var utilPolyfill = {
        // Type checking functions
        isArray: isArray,
        isDate: isDate,
        isRegExp: isRegExp,
        isError: isError,
        isFunction: isFunction,
        isObject: isObject,
        isPrimitive: isPrimitive,
        isString: isString,
        isNumber: isNumber,
        isBoolean: isBoolean,
        isNull: isNull,
        isUndefined: isUndefined,
        isNullOrUndefined: isNullOrUndefined,
        isBuffer: isBuffer,

        // Formatting functions
        inspect: inspect,
        format: format,

        // Utility functions
        deprecate: deprecate,
        inherits: inherits,
        promisify: promisify,
        callbackify: callbackify,

        // TextEncoder/TextDecoder for compatibility
        TextEncoder: typeof TextEncoder !== 'undefined' ? TextEncoder : function() {
            this.encode = function(str) {
                var result = new Uint8Array(str.length);
                for (var i = 0; i < str.length; i++) {
                    result[i] = str.charCodeAt(i);
                }
                return result;
            };
        },

        TextDecoder: typeof TextDecoder !== 'undefined' ? TextDecoder : function() {
            this.decode = function(buffer) {
                return String.fromCharCode.apply(null, new Uint8Array(buffer));
            };
        }
    };

    // Export polyfill
    window.utilPolyfill = utilPolyfill;

})();