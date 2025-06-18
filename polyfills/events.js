/**
 * EventEmitter polyfill for browser environments
 * Provides Node.js-compatible EventEmitter functionality
 */
(function() {
    'use strict';

    function EventEmitter() {
        this._events = {};
        this._maxListeners = 10;
    }

    EventEmitter.prototype.on = function(event, listener) {
        if (!this._events[event]) {
            this._events[event] = [];
        }

        this._events[event].push(listener);

        if (this._events[event].length > this._maxListeners) {
            console.warn('Possible EventEmitter memory leak detected. ' +
                this._events[event].length + ' listeners added. ' +
                'Use emitter.setMaxListeners() to increase limit.');
        }

        return this;
    };

    EventEmitter.prototype.addListener = EventEmitter.prototype.on;

    EventEmitter.prototype.once = function(event, listener) {
        var self = this;

        function onceWrapper() {
            self.removeListener(event, onceWrapper);
            listener.apply(this, arguments);
        }

        this.on(event, onceWrapper);
        return this;
    };

    EventEmitter.prototype.removeListener = function(event, listener) {
        if (!this._events[event]) {
            return this;
        }

        var index = this._events[event].indexOf(listener);
        if (index !== -1) {
            this._events[event].splice(index, 1);
        }

        if (this._events[event].length === 0) {
            delete this._events[event];
        }

        return this;
    };

    EventEmitter.prototype.off = EventEmitter.prototype.removeListener;

    EventEmitter.prototype.removeAllListeners = function(event) {
        if (event) {
            delete this._events[event];
        } else {
            this._events = {};
        }
        return this;
    };

    EventEmitter.prototype.emit = function(event) {
        if (!this._events[event]) {
            return false;
        }

        var listeners = this._events[event].slice();
        var args = Array.prototype.slice.call(arguments, 1);

        for (var i = 0; i < listeners.length; i++) {
            try {
                listeners[i].apply(this, args);
            } catch (err) {
                console.error('EventEmitter error:', err);
            }
        }

        return true;
    };

    EventEmitter.prototype.listeners = function(event) {
        return this._events[event] ? this._events[event].slice() : [];
    };

    EventEmitter.prototype.listenerCount = function(event) {
        return this._events[event] ? this._events[event].length : 0;
    };

    EventEmitter.prototype.setMaxListeners = function(n) {
        this._maxListeners = n;
        return this;
    };

    EventEmitter.prototype.getMaxListeners = function() {
        return this._maxListeners;
    };

    EventEmitter.prototype.eventNames = function() {
        return Object.keys(this._events);
    };

    EventEmitter.prototype.prependListener = function(event, listener) {
        if (!this._events[event]) {
            this._events[event] = [];
        }

        this._events[event].unshift(listener);
        return this;
    };

    EventEmitter.prototype.prependOnceListener = function(event, listener) {
        var self = this;

        function onceWrapper() {
            self.removeListener(event, onceWrapper);
            listener.apply(this, arguments);
        }

        this.prependListener(event, onceWrapper);
        return this;
    };

    // Export polyfill
    window.EventEmitterPolyfill = EventEmitter;

})();