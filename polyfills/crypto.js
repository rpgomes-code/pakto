/**
 * Crypto polyfill for browser environments
 * Provides Node.js-compatible crypto functionality using Web Crypto API
 */
(function() {
    'use strict';

    var crypto = window.crypto || window.msCrypto;

    if (!crypto || !crypto.subtle) {
        throw new Error('Web Crypto API not supported in this environment');
    }

    function createHash(algorithm) {
        var algo = algorithm.toLowerCase().replace('-', '');
        var webCryptoAlgo;

        switch (algo) {
            case 'sha1':
                webCryptoAlgo = 'SHA-1';
                break;
            case 'sha256':
                webCryptoAlgo = 'SHA-256';
                break;
            case 'sha384':
                webCryptoAlgo = 'SHA-384';
                break;
            case 'sha512':
                webCryptoAlgo = 'SHA-512';
                break;
            default:
                throw new Error('Unsupported hash algorithm: ' + algorithm);
        }

        var hasher = {
            _data: new Uint8Array(0),

            update: function(data) {
                if (typeof data === 'string') {
                    data = new TextEncoder().encode(data);
                }

                var combined = new Uint8Array(this._data.length + data.length);
                combined.set(this._data);
                combined.set(data, this._data.length);
                this._data = combined;

                return this;
            },

            digest: function(encoding) {
                var self = this;

                return crypto.subtle.digest(webCryptoAlgo, this._data)
                    .then(function(hash) {
                        var hashArray = new Uint8Array(hash);

                        if (encoding === 'hex') {
                            return Array.from(hashArray)
                                .map(function(b) { return b.toString(16).padStart(2, '0'); })
                                .join('');
                        } else if (encoding === 'base64') {
                            return btoa(String.fromCharCode.apply(null, hashArray));
                        } else {
                            return hashArray;
                        }
                    });
            }
        };

        return hasher;
    }

    function randomBytes(size) {
        var bytes = new Uint8Array(size);
        crypto.getRandomValues(bytes);
        return bytes;
    }

    function pbkdf2(password, salt, iterations, keylen, digest, callback) {
        if (typeof digest === 'function') {
            callback = digest;
            digest = 'sha1';
        }

        var algo = 'PBKDF2';
        var hash = digest.toUpperCase().replace(/^SHA/, 'SHA-');

        if (typeof password === 'string') {
            password = new TextEncoder().encode(password);
        }
        if (typeof salt === 'string') {
            salt = new TextEncoder().encode(salt);
        }

        crypto.subtle.importKey(
            'raw',
            password,
            { name: algo },
            false,
            ['deriveBits']
        ).then(function(key) {
            return crypto.subtle.deriveBits(
                {
                    name: algo,
                    salt: salt,
                    iterations: iterations,
                    hash: hash
                },
                key,
                keylen * 8
            );
        }).then(function(bits) {
            var result = new Uint8Array(bits);
            if (callback) callback(null, result);
            return result;
        }).catch(function(err) {
            if (callback) callback(err);
            throw err;
        });
    }

    // Export polyfill
    window.cryptoPolyfill = {
        createHash: createHash,
        randomBytes: randomBytes,
        pbkdf2: pbkdf2
    };

})();