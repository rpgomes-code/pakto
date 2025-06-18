/**
 * Process polyfill for browser environments
 * Provides Node.js-compatible process object
 */
(function() {
    'use strict';

    var processPolyfill = {
        env: {
            NODE_ENV: 'production'
        },

        platform: (function() {
            if (navigator.platform.indexOf('Win') !== -1) return 'win32';
            if (navigator.platform.indexOf('Mac') !== -1) return 'darwin';
            if (navigator.platform.indexOf('Linux') !== -1) return 'linux';
            return 'unknown';
        })(),

        arch: 'x64', // Assume x64 for browsers

        version: 'v16.0.0', // Fake Node.js version

        versions: {
            node: '16.0.0',
            v8: '9.0.0'
        },

        argv: ['node', 'script.js'],

        cwd: function() {
            return '/';
        },

        nextTick: function(callback) {
            setTimeout(callback, 0);
        },

        exit: function(code) {
            console.warn('process.exit() called with code:', code);
        },

        stderr: {
            write: function(data) {
                console.error(data);
            }
        },

        stdout: {
            write: function(data) {
                console.log(data);
            }
        },

        stdin: {
            on: function() {
                console.warn('stdin not supported in browser');
            }
        }
    };

    // Export polyfill
    window.processPolyfill = processPolyfill;

})();