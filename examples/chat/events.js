export function delegate(node, event_name, selector, listener, options) {
    const fn = (event) => {
        let target = event.target;
        while (target) {
            if (target.matches(selector)) {
                return listener.call(target, event);
            }
            if (target === node) break;
            target = target.parentNode;
        }
    };
    node.addEventListener(event_name, fn, options);
    // Return undelegate function
    return () => node.removeEventListener(event_name, fn);
}
