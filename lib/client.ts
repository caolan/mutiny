// A client library to talk to the mutiny daemon via unix socket

import { resolve } from "https://deno.land/std@0.224.0/path/mod.ts";
import { writeAll } from "https://deno.land/std@0.224.0/io/mod.ts";
import * as msgpack from "https://deno.land/std@0.224.0/msgpack/mod.ts";
import { readFullBuffer } from "./streams.ts";
import { assert } from "./assert.ts";

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
    return ifDefined(userRuntimePath(), dir => resolve(dir, "mutiny"));
}

export function defaultSocketPath(): string | undefined {
    return ifDefined(appRuntimePath(), dir => resolve(dir, "mutinyd.socket"));
}

type ConnectOptions = {
    socket_path?: string,
};

export async function connect({socket_path}: ConnectOptions): Promise<MutinyClient> {
    const path = socket_path ?? defaultSocketPath(); 
    if (!path) {
        throw new Error("Could not determine mutinyd.socket path");
    }
    console.log(`Connecting to ${path}`);
    const conn = await Deno.connect({
        transport: 'unix',
        path,
    });
    return new MutinyClient(conn);
}

type MutinyRequest = {Ping: null} | {LocalPeerId: null};
type MutinyResponse = {Pong: null} | {LocalPeerId: string};

export class MutinyClient {
    constructor(
        private conn: Deno.UnixConn,
    ) {}

    private async request(request: MutinyRequest): Promise<MutinyResponse> {
        // reused for both request and response
        let length_buf = new ArrayBuffer(4);

        // send request
        const encoded = msgpack.encode(request);
        new DataView(length_buf).setUint32(0, encoded.byteLength, false);
        await writeAll(this.conn, new Uint8Array(length_buf, 0));
        await writeAll(this.conn, encoded);

        // read response
        const reader = this.conn.readable.getReader({mode: "byob"});
        length_buf = await readFullBuffer(reader, length_buf);
        const response_len = new DataView(length_buf).getUint32(0, false);
        const response_buf = await readFullBuffer(
            reader,
            new ArrayBuffer(response_len),
        );
        reader.releaseLock();
        return msgpack.decode(
            new Uint8Array(response_buf)
        ) as MutinyResponse;
    }

    async ping(): Promise<undefined> {
        await this.request({Ping: null});
    }

    async localPeerId(): Promise<string> {
        const response = await this.request({LocalPeerId: null});
        assert('LocalPeerId' in response);
        return response.LocalPeerId;
    }
}
