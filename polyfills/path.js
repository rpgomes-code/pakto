/**
 * Path polyfill for browser environments
 * Provides Node.js-compatible path functionality
 */
(function() {
    'use strict';

    var isWindows = navigator.platform.indexOf('Win') === 0;

    function normalizeArray(parts, allowAboveRoot) {
        var up = 0;
        for (var i = parts.length - 1; i >= 0; i--) {
            var last = parts[i];
            if (last === '.') {
                parts.splice(i, 1);
            } else if (last === '..') {
                parts.splice(i, 1);
                up++;
            } else if (up) {
                parts.splice(i, 1);
                up--;
            }
        }

        if (allowAboveRoot) {
            for (; up--; up) {
                parts.unshift('..');
            }
        }

        return parts;
    }

    var pathPolyfill = {
        sep: isWindows ? '\\' : '/',
        delimiter: isWindows ? ';' : ':',
        posix: null,
        win32: null,

        normalize: function(path) {
            var isAbsolute = this.isAbsolute(path);
            var trailingSlash = path && path[path.length - 1] === '/';

            path = normalizeArray(path.split('/').filter(function(p) {
                return !!p;
            }), !isAbsolute).join('/');

            if (!path && !isAbsolute) {
                path = '.';
            }
            if (path && trailingSlash) {
                path += '/';
            }

            return (isAbsolute ? '/' : '') + path;
        },

        isAbsolute: function(path) {
            if (isWindows) {
                return path.length > 1 && path[1] === ':';
            }
            return path.charAt(0) === '/';
        },

        join: function() {
            var paths = Array.prototype.slice.call(arguments);
            return this.normalize(paths.filter(function(p) {
                if (typeof p !== 'string') {
                    throw new TypeError('Arguments to path.join must be strings');
                }
                return p;
            }).join('/'));
        },

        resolve: function() {
            var resolvedPath = '';
            var resolvedAbsolute = false;

            for (var i = arguments.length - 1; i >= -1 && !resolvedAbsolute; i--) {
                var path = (i >= 0) ? arguments[i] : '/'; // Use root as fallback

                if (typeof path !== 'string') {
                    throw new TypeError('Arguments to path.resolve must be strings');
                }

                if (!path) {
                    continue;
                }

                resolvedPath = path + '/' + resolvedPath;
                resolvedAbsolute = this.isAbsolute(path);
            }

            resolvedPath = normalizeArray(resolvedPath.split('/').filter(function(p) {
                return !!p;
            }), !resolvedAbsolute).join('/');

            return ((resolvedAbsolute ? '/' : '') + resolvedPath) || '.';
        },

        relative: function(from, to) {
            from = this.resolve(from).substr(1);
            to = this.resolve(to).substr(1);

            function trim(arr) {
                var start = 0;
                for (; start < arr.length; start++) {
                    if (arr[start] !== '') break;
                }

                var end = arr.length - 1;
                for (; end >= 0; end--) {
                    if (arr[end] !== '') break;
                }

                if (start > end) return [];
                return arr.slice(start, end - start + 1);
            }

            var fromParts = trim(from.split('/'));
            var toParts = trim(to.split('/'));

            var length = Math.min(fromParts.length, toParts.length);
            var samePartsLength = length;
            for (var i = 0; i < length; i++) {
                if (fromParts[i] !== toParts[i]) {
                    samePartsLength = i;
                    break;
                }
            }

            var outputParts = [];
            for (var i = samePartsLength; i < fromParts.length; i++) {
                outputParts.push('..');
            }

            outputParts = outputParts.concat(toParts.slice(samePartsLength));

            return outputParts.join('/');
        },

        dirname: function(path) {
            var result = this.splitPath(path);
            var root = result[0];
            var dir = result[1];

            if (!root && !dir) {
                return '.';
            }

            if (dir) {
                dir = dir.substr(0, dir.length - 1);
            }

            return root + dir;
        },

        basename: function(path, ext) {
            var f = this.splitPath(path)[2];
            if (ext && f.substr(-1 * ext.length) === ext) {
                f = f.substr(0, f.length - ext.length);
            }
            return f;
        },

        extname: function(path) {
            return this.splitPath(path)[3];
        },

        parse: function(path) {
            var allParts = this.splitPath(path);
            return {
                root: allParts[0],
                dir: allParts[0] + allParts[1].slice(0, -1),
                base: allParts[2],
                ext: allParts[3],
                name: allParts[2].slice(0, allParts[2].length - allParts[3].length)
            };
        },

        format: function(pathObject) {
            if (typeof pathObject !== 'object' || pathObject === null) {
                throw new TypeError('Parameter "pathObject" must be an object');
            }

            var dir = pathObject.dir || pathObject.root;
            var base = pathObject.base || ((pathObject.name || '') + (pathObject.ext || ''));

            if (!dir) {
                return base;
            }

            if (dir === pathObject.root) {
                return dir + base;
            }

            return dir + this.sep + base;
        },

        splitPath: function(filename) {
            var splitPathRe = /^(\/?|)([\s\S]*?)((?:\.{1,2}|[^\/]+?|)(\.[^.\/]*|))(?:[\/]*)$/;
            return splitPathRe.exec(filename).slice(1);
        }
    };

    // Create posix and win32 objects
    pathPolyfill.posix = Object.assign({}, pathPolyfill, {
        sep: '/',
        delimiter: ':'
    });

    pathPolyfill.win32 = Object.assign({}, pathPolyfill, {
        sep: '\\',
        delimiter: ';'
    });

    // Expose to global scope
    window.pathPolyfill = pathPolyfill;

})();