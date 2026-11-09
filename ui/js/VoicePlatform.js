.pragma library

// Explicit Qt adapters for the browser primitives used by the native controller.
// No global constructor or prototype is modified.
function create() {
    function Signal() {
        this.aborted = false
        this.reason = undefined
        this.listeners = []
    }
    Signal.prototype.addEventListener = function(type, callback, options) {
        if (type !== "abort" || !callback) return
        var capture = options === true || !!(options && options.capture)
        if (this.listeners.some(function(item) { return item.active && item.callback === callback && item.capture === capture })) return
        this.listeners.push({callback:callback, capture:capture, once:!!(options && options.once), active:true})
    }
    Signal.prototype.removeEventListener = function(type, callback, options) {
        if (type !== "abort") return
        var capture = options === true || !!(options && options.capture)
        this.listeners.forEach(function(item) { if (item.callback === callback && item.capture === capture) item.active = false })
    }
    Signal.prototype.throwIfAborted = function() { if (this.aborted) throw this.reason }
    function Controller() { this.signal = new Signal() }
    Controller.prototype.abort = function(reason) {
        var signal = this.signal
        if (signal.aborted) return
        signal.aborted = true
        if (reason === undefined) { reason = new Error("This operation was aborted"); reason.name = "AbortError"; reason.code = 20 }
        signal.reason = reason
        var event = {type:"abort", target:signal, currentTarget:signal}
        signal.listeners.slice().forEach(function(item) {
            if (!item.active) return
            if (item.once) item.active = false
            try {
                if (typeof item.callback === "function") item.callback.call(signal, event)
                else if (typeof item.callback.handleEvent === "function") item.callback.handleEvent(event)
            } catch (_) {
                // DOM abort dispatch does not throw a listener's exception back
                // through abort(). Controller callbacks receive no audio here.
            }
        })
    }
    var constructors = []
    function fieldAccepts(rule, value) {
        if (rule.kind === "string") return typeof value === "string"
        if (rule.kind === "literals") return rule.values.indexOf(value) >= 0
        if (rule.kind === "optional") return value === undefined || fieldAccepts(rule.inner, value)
        return rule.kind === "defect"
    }
    var Schema = {
        String: {kind:"string"},
        Literals: function(values) { return {kind:"literals", values:values.slice()} },
        Defect: function() { return {kind:"defect"} },
        optional: function(rule) { return {kind:"optional", inner:rule} },
        TaggedError: function() {
            return function(tag, fields) {
                class Tagged extends Error {
                    constructor(values) {
                        if (!values || !Object.keys(fields).every(function(key) { return fieldAccepts(fields[key],values[key]) })) throw new Error("Schema validation failed")
                        super(values.message)
                        this.name = tag
                        this._tag = tag
                        Object.keys(fields).forEach(function(key) { if (Object.prototype.hasOwnProperty.call(values,key)) this[key] = values[key] }, this)
                    }
                }
                var brand = "~effect/Schema/Class/" + tag
                Object.defineProperty(Tagged.prototype,brand,{get:function() { return brand }})
                constructors.push({type:Tagged, tag:tag, fields:fields, brand:brand})
                return Tagged
            }
        },
        is: function(type) {
            var entry = constructors.find(function(item) { return item.type === type || type.prototype instanceof item.type })
            if (!entry) throw new Error("Unsupported voice schema dependency")
            return function(value) {
                if (!value || (typeof value !== "object" && typeof value !== "function")) return false
                // Effect's class type predicate admits instances or the HMR
                // class marker; it does not structurally decode a plain record.
                return value instanceof type || entry.brand in value
            }
        }
    }
    // Realm-local Array.flatMap for admitted ordinary metadata arrays. This
    // does not install a global method, honor custom species or claim generic
    // array-like/proxy semantics. Each mapper result is flattened immediately.
    function flatMap(receiver, mapper) {
        if (!Array.isArray(receiver) || Object.getPrototypeOf(receiver) !== Array.prototype ||
                Object.prototype.hasOwnProperty.call(receiver, "constructor"))
            throw new TypeError("Ordinary metadata array required")
        if (typeof mapper !== "function") throw new TypeError("Mapper required")
        var length = receiver.length, output = [], target = 0
        function append(value) {
            if (target >= 9007199254740991) throw new TypeError("Metadata array limit")
            Object.defineProperty(output, String(target++), {value:value, writable:true, enumerable:true, configurable:true})
        }
        for (var index = 0; index < length; index++) {
            if (!(index in receiver)) continue
            var mapped = mapper(receiver[index], index, receiver)
            if (Array.isArray(mapped)) {
                var nestedLength = mapped.length
                for (var nested = 0; nested < nestedLength; nested++)
                    if (nested in mapped) append(mapped[nested])
            } else append(mapped)
        }
        return output
    }
    return {Schema:Schema, AbortController:Controller, flatMap:flatMap}
}
