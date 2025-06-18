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

        execPath: '/usr/bin/node',

        cwd: function() {
            return '/';
        },

        chdir: function(directory) {
            console.warn('process.chdir() is not supported in browser environment');
        },

        nextTick: function(callback) {
            if (typeof callback !== 'function') {
                throw new TypeError('callback must be a function');
            }

            var args = Array.prototype.slice.call(arguments, 1);
            setTimeout(function() {
                callback.apply(null, args);
            }, 0);
        },

        exit: function(code) {
            console.warn('process.exit() called with code:', code);
        },

        kill: function(pid, signal) {
            console.warn('process.kill() is not supported in browser environment');
        },

        pid: 1,
        ppid: 0,

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
            },
            pause: function() {},
            resume: function() {},
            setEncoding: function() {}
        },

        hrtime: function(time) {
            var now = performance.now() * 1e-3;
            var seconds = Math.floor(now);
            var nanoseconds = Math.floor((now % 1) * 1e9);

            if (time) {
                seconds = seconds - time[0];
                nanoseconds = nanoseconds - time[1];
                if (nanoseconds < 0) {
                    seconds--;
                    nanoseconds += 1e9;
                }
            }

            return [seconds, nanoseconds];
        },

        uptime: function() {
            return performance.now() / 1000;
        },

        memoryUsage: function() {
            return {
                rss: 0,
                heapTotal: 0,
                heapUsed: 0,
                external: 0
            };
        },

        cpuUsage: function() {
            return {
                user: 0,
                system: 0
            };
        },

        binding: function() {
            throw new Error('process.binding is not supported');
        },

        umask: function() {
            return 0;
        }
    };

    // Add EventEmitter-like functionality
    processPolyfill._events = {};
    processPolyfill.on = function(event, listener) {
        if (!this._events[event]) {
            this._events[event] = [];
        }
        this._events[event].push(listener);
        return this;
    };

    processPolyfill.emit = function(event) {
        if (!this._events[event]) {
            return false;
        }

        var listeners = this._events[event].slice();
        var args = Array.prototype.slice.call(arguments, 1);

        for (var i = 0; i < listeners.length; i++) {
            try {
                listeners[i].apply(this, args);
            } catch (err) {
                console.error('Process event error:', err);
            }
        }

        return true;
    };

    processPolyfill.removeListener = function(event, listener) {
        if (!this._events[event]) {
            return this;
        }

        var index = this._events[event].indexOf(listener);
        if (index !== -1) {
            this._events[event].splice(index, 1);
        }

        return this;
    };

    // Export polyfill
    window.processPolyfill = processPolyfill;

})();