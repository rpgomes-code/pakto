/**
 * Buffer polyfill for browser environments
 * Provides Node.js-compatible Buffer functionality
 */
(function() {
    'use strict';

    function BufferPolyfill(data, encoding) {
        if (data === undefined) {
            return new Uint8Array(0);
        }

        if (typeof data === 'number') {
            return new Uint8Array(data);
        }

        if (typeof data === 'string') {
            encoding = encoding || 'utf8';

            switch (encoding.toLowerCase()) {
                case 'utf8':
                case 'utf-8':
                    return new TextEncoder().encode(data);
                case 'hex':
                    return hexToUint8Array(data);
                case 'base64':
                    return base64ToUint8Array(data);
                default:
                    throw new Error('Unsupported encoding: ' + encoding);
            }
        }

        if (data instanceof ArrayBuffer) {
            return new Uint8Array(data);
        }

        if (data instanceof Uint8Array) {
            return data;
        }

        if (Array.isArray(data)) {
            return new Uint8Array(data);
        }

        throw new Error('Invalid data type for Buffer');
    }

    function hexToUint8Array(hex) {
        var result = new Uint8Array(hex.length / 2);
        for (var i = 0; i < result.length; i++) {
            result[i] = parseInt(hex.substr(i * 2, 2), 16);
        }
        return result;
    }

    function base64ToUint8Array(base64) {
        var binary = atob(base64);
        var result = new Uint8Array(binary.length);
        for (var i = 0; i < binary.length; i++) {
            result[i] = binary.charCodeAt(i);
        }
        return result;
    }

    BufferPolyfill.from = function(data, encoding) {
        return new BufferPolyfill(data, encoding);
    };

    BufferPolyfill.alloc = function(size, fill) {
        var buffer = new Uint8Array(size);
        if (fill !== undefined) {
            buffer.fill(fill);
        }
        return buffer;
    };

    BufferPolyfill.allocUnsafe = function(size) {
        return new Uint8Array(size);
    };

    BufferPolyfill.isBuffer = function(obj) {
        return obj instanceof Uint8Array;
    };

    // Add methods to Uint8Array prototype
    if (!Uint8Array.prototype.toString) {
        Uint8Array.prototype.toString = function(encoding) {
            encoding = encoding || 'utf8';

            switch (encoding.toLowerCase()) {
                case 'utf8':
                case 'utf-8':
                    return new TextDecoder().decode(this);
                case 'hex':
                    return Array.from(this)
                        .map(function(b) { return b.toString(16).padStart(2, '0'); })
                        .join('');
                case 'base64':
                    return btoa(String.fromCharCode.apply(null, this));
                default:
                    throw new Error('Unsupported encoding: ' + encoding);
            }
        };
    }

    // Export polyfill
    window.BufferPolyfill = BufferPolyfill;

})();