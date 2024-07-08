/**
 * @file Notify subscribers when a value is updated.
 * @author Caolan McMahon <caolan@caolan.uk>
 */

// @ts-check

/** 
 * Represents a value that may change multiple times. Callbacks may
 * be added and to be called after the next change.
 * 
 * @template T
 */
export class Signaller {
    /** @type {Array<function(Signaller<T>): unknown>} */
    #waiting = [];

    /** @type T */
    #value;

    /** @param {T} initial */
    constructor(initial) {
        this.#value = initial;
    }

    /** @return {T} */
    get value() {
        return this.#value;
    }

    /**
     * If the new value is not strictly equal to the current value, the 
     * current value is updated and a change is signalled.
     * 
     * @param {T} value
     */
    set value(value) {
        if (this.#value !== value) {
            this.#value = value;
            this.signal();
        }
    }

    /**
     * Takes all callbacks from the waiting list (clearing it) and calls 
     * each callback synchronously in the order they were added.
     * 
     * You do not need to call this function directly unless you have
     * mutated the current value and wish to signal the change to any 
     * watchers manually.
     */
    signal() {
        if (this.#waiting.length) {
            const calling = this.#waiting;
            this.#waiting = [];
            for (const callback of calling) {
                callback(this);
            }
        }
    }

    /**
     * Appends a callback to the waiting list that will be called on 
     * the next set().
     * 
     * @param {function(Signaller<T>): unknown} callback - Function
     * called with this Signaller instance as its only argument. Return 
     * value is ignored.
     */
    addCallback(callback) {
        this.#waiting.push(callback);
    }

    /**
     * Removes the first matching callback from the waiting list (if 
     * any). Does nothing if callback has not been registered and will not
     * throw.
     * 
     * @param {function(Signaller<T>): unknown} callback - A function that may
     * have been previously provided to addCallback().
     */
    removeCallback(callback) {
        for (let i = 0, len = this.#waiting.length; i < len; i++) {
            if (this.#waiting[i] === callback) {
                this.#waiting.splice(i, 1);
                return;
            }
        }
    }
}

/**
 * Watches an Array of Signallers for changes. The handler will always be
 * called asynchronously following a set() of the Signaller. Multiple
 * signals before the next handler call completes will not queue 
 * multiple handler calls. This means not every intermediate value of a 
 * Signaller will necessarily be seen by the handler.
 * 
 * @template T 
 * @template {Signaller<T>} S 
 * @param {S[]} signallers - The Signallers to watch for changes.
 * @param {function (S[]): unknown} handler
 * A function with one argument: an array of Signallers that called
 * set() since the last call. It is guaranteed to be called
 * asynchronously after an underlying Signaller calls set(). Unless the
 * handler previously returned a Promise that is still completing, it will
 * always be called before yielding to the main event loop (otherwise it
 * will first wait for the last call's Promise to complete).
 * 
 * @returns {function (): void} A function to stop watching for changes.
 * After calling this function, the handler is guaranteed to not be called
 * by again even if a signal is queued.
 */
export function watch(signallers,  handler) {
    let stop = false;
    let sleeping = true;
    /** @type {S[]} */
    let changed = [];
    /** @type {function (Signaller<T>): void} */
    const callback = s => {
        changed.push(/** @type {S} */(s));
        if (sleeping) {
            sleeping = false;
            queueMicrotask(wake);
        }
    };
    const wake = async () => {
        while (!stop && changed.length) {
            for (const s of changed) {
                s.addCallback(callback);
            }
            const c = changed;
            changed = [];
            await handler(c);
        }
        sleeping = true;
    };
    for (const s of signallers) {
        s.addCallback(callback);
    }
    return () => {
        stop = true;
        for (const s of signallers) {
            s.removeCallback(callback);
        }
    };
}
