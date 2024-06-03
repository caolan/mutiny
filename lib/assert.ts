// Throws an error with given message if test argument is false.
export function assert(test: boolean, msg?: string): asserts test {
    if (!test) {
        throw new Error('Assertion error' + (msg ? `: ${msg}`: ''));
    }
}

// Will cause a compile-time typescript error if the provided argument
// is not of the 'never' type. This is useful for checking that switch
// cases are exhaustive. If this is unexpected called at runtime a
// runtime error will be thrown.
export function assertNever(value: never) {
    throw new Error('Unexpected value: ' + value);
}
