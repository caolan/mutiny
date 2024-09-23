export default function eventStream<T>(iter: AsyncIterableIterator<T>, map: (event: T) => [string, string]) {
    let stop = false;
    const body = new ReadableStream({
        start(controller) {
            (async () => {
                const encoder = new TextEncoder();
                try {
                    for await (const event of iter) {
                        if (stop) break;
                        const [name, data] = map(event);
                        controller.enqueue(
                            encoder.encode(
                                `event: ${name}\r\ndata: ${data}\r\n\r\n`
                            )
                        );
                    }
                } catch (err) {
                    // Report error and just end the stream instead of
                    // stopping the whole server process.
                    console.error(`Event stream error: ${err.message}`);
                }
            })();
        },
        cancel() {
            stop = true;
        }
    });
    return new Response(body, {
        headers: {
            'Content-Type': 'text/event-stream',
        }
    });
}
