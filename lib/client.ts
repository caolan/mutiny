// A client library to talk to the FE daemon via unix socket

import { resolve } from "https://deno.land/std@0.224.0/path/mod.ts";
import { writeAll } from "https://deno.land/std@0.224.0/io/mod.ts";
import { TextLineStream } from "https://deno.land/std@0.224.0/streams/mod.ts";

function ifDefined<T, R>(value: undefined | T, f: (value: T) => R): R | undefined {
    return (value === undefined) ? undefined : f(value);
}

function userRuntimePath(): string | undefined {
    if (Deno.build.os === "darwin") {
        // Just pick something sensible
        return ifDefined(
            Deno.env.get("HOME"), 
            home => resolve(home, "Library/Caches/TemporaryItems")
        );
    }
    // Assume following freedesktop.org specification (Linux etc)
    return Deno.env.get("XDG_RUNTIME_DIR");
}

function appRuntimePath(): string | undefined {
    return ifDefined(userRuntimePath(), dir => resolve(dir, "fe"));
}

export function defaultSocketPath(): string | undefined {
    return ifDefined(appRuntimePath(), dir => resolve(dir, "fed.socket"));
}

type ConnectOptions = {
    socket_path?: string,
};

export async function connect({socket_path}: ConnectOptions): Promise<FEClient> {
    const path = socket_path ?? defaultSocketPath(); 
    if (!path) {
        throw new Error("Could not determine fed.socket path");
    }
    console.log(`Connecting to ${path}`);
    const conn = await Deno.connect({
        transport: 'unix',
        path,
    });
    return new FEClient(conn);
}

export class FEClient {
    constructor(
        private conn: Deno.UnixConn,
    ) {}


    async ping(): Promise<string> {
        await writeAll(this.conn, new TextEncoder().encode("ping\n"));
        const lines = this.conn.readable
            .pipeThrough(new TextDecoderStream())
            .pipeThrough(new TextLineStream());
        for await (const line of lines) {
            return line;
        }
        throw new Error('No response');
    }
}
