// A client library to talk to the mutiny daemon via unix socket

import { resolve } from "@std/path";
import { writeAll } from "@std/io";
import * as msgpack from "@std/msgpack";
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

interface JsonObject { [name: string]: JsonValue }
interface JsonArray extends Array<JsonValue> { }
type JsonValue = (null | boolean | number | string | JsonObject | JsonArray);

export type Message = {
    type: "Message",
    id: number,
    peer: string,
    uuid: string,
    message: Uint8Array,
};

export type MessageJson = {
    type: "Message",
    id: number,
    peer: string,
    uuid: string,
    message: string,
};

export type AppAnnouncement = {
    type: "AppAnnouncement",
    peer: string, 
    app_uuid: string,
    data: JsonValue,
};

export type MutinyRequest = {
    id: number,
    body: MutinyRequestBody,
};
export type MutinyRequestBody = {type: "LocalPeerId"}
    | {type: "Peers"}
    | {type: "AppAnnouncements"}
    | {type: "GetLastPort", app_uuid: string}
    | {type: "SetLastPort", app_uuid: string, port: number}
    | {type: "AppInstanceUuid", label: string}
    | {type: "CreateAppInstance", label: string}
    | {type: "Announce", peer: string, app_uuid: string, data: JsonValue}
    | {type: "AppAnnouncements"}
    | {
        type: "SendMessage", 
        peer: string,
        app_uuid: string,
        from_app_uuid: string,
        message: Uint8Array,
    }
    | {type: "InboxMessages", app_uuid: string}
    | {type: "DeleteInboxMessage", app_uuid: string, message_id: number}
    | {type: "SubscribePeerEvents"}
    | {type: "SubscribeAnnounceEvents"}
    | {type: "SubscribeInboxEvents", app_uuid: string}
    ;

export type MutinyResponse = {
    request_id: number,
    body: MutinyResponseBody,
};

export type PeerEvent = {type: "PeerDiscovered", peer_id: string}
    | {type: "PeerExpired", peer_id: string};

export type MutinyResponseBody = {type: "Success"} 
    | {type: "Error", message: string}
    | {type: "LocalPeerId", peer_id: string}
    | {type: "Peers", peers: string[]}
    | {type: "AppInstanceUuid", uuid: string | null}
    | {type: "GetLastPort", port: number | null}
    | {type: "CreateAppInstance", uuid: string}
    | {type: "Message", message: Message}
    | {type: "InboxMessages", messages: Message[]}
    | {type: "AppAnnouncements",  announcements: AppAnnouncement[]}
    | PeerEvent
    ;

function sleep(ms: number): Promise<void> {
    return new Promise(resolve => {
        setTimeout(resolve, ms);
    });
}

export class MutinyClient {
    private next_request_id = 1;
    private sending_requests = false;
    private dispatching_responses = false;
    private queued: MutinyRequest[] = [];
    private waiting: Map<number, {
        resolve: (response: MutinyResponseBody) => void,
        reject: (err: Error) => void,
    }> = new Map();
    private conn?: Deno.UnixConn;
    private socket_path: string;
    private connection_promise?: Promise<Deno.UnixConn>;

    constructor({socket_path}: ConnectOptions) {
        const path = socket_path ?? defaultSocketPath();
        if (!path) {
            throw new Error("Could not determine mutinyd.socket path");
        }
        this.socket_path = path;
    }

    private async connect() {
        if (!this.connection_promise) {
            this.connection_promise = (async () => {
                while (true) {
                    try {
                        this.conn = await Deno.connect({
                            transport: 'unix',
                            path: this.socket_path,
                        });
                        break;
                    } catch (err) {
                        console.error(`Error connecting to ${this.socket_path}: ${err.message}`);
                        // Deno.errors.NotFound
                        // Deno.errors.ConnectionRefused
                        // ...
                        await sleep(1000);
                    }
                }
                console.log(`Connected to ${this.socket_path}`);
                this.connection_promise = undefined;
                return this.conn;
            })();
        }
        return await this.connection_promise;
    }

    private async getConnection(): Promise<Deno.UnixConn> {
        return this.conn ?? await this.connect();
    }

    private async sendRequests() {
        const conn = await this.getConnection();
        while (this.queued.length > 0) {
            const requests = this.queued;
            this.queued = [];
            for (const request of requests) {
                // send request
                console.log("Sending", request);
                const length_buf = new ArrayBuffer(4);
                const encoded = msgpack.encode(request);
                new DataView(length_buf).setUint32(0, encoded.byteLength, false);
                await writeAll(conn, new Uint8Array(length_buf, 0));
                await writeAll(conn, encoded);
            }
        }
        this.sending_requests = false;
    }

    private async dispatchResponses() {
        while (this.waiting.size > 0) {
            const conn = await this.getConnection();
            // read response
            let length_buf = new ArrayBuffer(4);
            let response_buf;
            try {
                const reader = conn.readable.getReader({mode: "byob"});
                length_buf = await readFullBuffer(reader, length_buf);
                const response_len = new DataView(length_buf).getUint32(0, false);
                response_buf = await readFullBuffer(
                    reader,
                    new ArrayBuffer(response_len),
                );
                reader.releaseLock();
            } catch (err) {
                // Failed to read full response.
                // This is unrecoverable, so reject all waiting requests
                // and disconnect.
                console.error(`Mutinyd client error: ${err.message}`);
                if (this.conn) {
                    try {
                        this.conn.close();
                    } catch (_) {
                        // Do nothing.
                    }
                }
                this.conn = undefined;
                const waiting = this.waiting;
                this.waiting = new Map();
                for (const req of waiting.values()) {
                    req.reject(err);
                }
                continue;
            }
            const response = msgpack.decode(
                new Uint8Array(response_buf)
            ) as MutinyResponse;
            console.log("Received", response);
            const req = this.waiting.get(response.request_id);
            if (req) {
                // Synchronous delete allows while loop to exit
                // when all requests have been responded to - in
                // the case of requests with multiple responses,
                // it is necessary to register the next promise
                // and call dispatchResponses again.
                this.waiting.delete(response.request_id);
                if (response.body.type === 'Error') {
                    req.reject(new Error(response.body.message))
                } else {
                    req.resolve(response.body);
                }
            }
        }
        this.dispatching_responses = false;
    }

    private queueRequest(request: MutinyRequest): Promise<MutinyResponseBody> {
        // Create promise and register with request id
        const promise = new Promise((resolve, reject) => {
            this.waiting.set(request.id, {resolve, reject});
        }) as Promise<MutinyResponseBody>;

        // Queue request
        this.queued.push(request);

        // Start sending queued requests (if not already)
        this.queueSendingRequests();
        // Start dispatching responses (if not already)
        this.queueDispatchResponses();
        return promise;
    }

    queueDispatchResponses() {
        if (!this.dispatching_responses) {
            this.dispatching_responses = true;
            queueMicrotask(() => this.dispatchResponses());
        }
    }

    queueSendingRequests() {
        if (!this.sending_requests) {
            this.sending_requests = true;
            queueMicrotask(() => this.sendRequests());
        }
    }

    private async requestOne(body: MutinyRequestBody): Promise<MutinyResponseBody> {
        const request = {id: this.next_request_id++, body};
        const response = await this.queueRequest(request);
        // No more responses expected so no need to register new promise
        return response;
    }

    async localPeerId(): Promise<string> {
        const response = await this.requestOne({type: "LocalPeerId"});
        assert(response.type === 'LocalPeerId');
        return response.peer_id;
    }

    async peers(): Promise<string[]> {
        const response = await this.requestOne({type: "Peers"});
        assert(response.type === 'Peers');
        return response.peers;
    }

    private _subscribe<R>(request: MutinyRequest): AsyncIterableIterator<R> {
        const waiting = this.waiting;
        let promise: Promise<MutinyResponseBody> = this.queueRequest(request);
        return {
            [Symbol.asyncIterator]() {
                return this;
            },
            return(value?: PeerEvent) {
                // Remove waiting promise
                waiting.delete(request.id);
                return Promise.resolve({value, done: true});
            },
            next: async () => {
                const value = await promise;
                // Register next promise
                promise = new Promise((resolve, reject) => {
                    waiting.set(request.id, {resolve, reject});
                }) as Promise<MutinyResponseBody>;
                // Start dispatching responses (if not already)
                this.queueDispatchResponses();
                return {value: value as R};
            }
        };
    }

    peerEvents(): AsyncIterableIterator<PeerEvent> {
        const body: MutinyRequestBody = {type: "SubscribePeerEvents"};
        const request = {id: this.next_request_id++, body};
        return this._subscribe(request);
    }

    announceEvents(): AsyncIterableIterator<AppAnnouncement> {
        const body: MutinyRequestBody = {type: "SubscribeAnnounceEvents"};
        const request = {id: this.next_request_id++, body};
        return this._subscribe(request);
    }

    inboxEvents(app_uuid: string): AsyncIterableIterator<Message> {
        const body: MutinyRequestBody = {type: "SubscribeInboxEvents", app_uuid};
        const request = {id: this.next_request_id++, body};
        return this._subscribe(request);
    }

    async appInstanceUuid(label: string): Promise<string | null> {
        const response = await this.requestOne({type: "AppInstanceUuid", label});
        assert(response.type === 'AppInstanceUuid');
        return response.uuid;
    }

    async getLastPort(app_uuid: string): Promise<number | null> {
        const response = await this.requestOne({type: "GetLastPort", app_uuid});
        assert(response.type === 'GetLastPort');
        return response.port;
    }

    async setLastPort(app_uuid: string, port: number): Promise<void> {
        const response = await this.requestOne({type: "SetLastPort", app_uuid, port});
        assert(response.type === 'Success');
    }

    async createAppInstance(label: string): Promise<string> {
        const response = await this.requestOne({type: "CreateAppInstance", label});
        assert(response.type === 'CreateAppInstance');
        return response.uuid;
    }

    async announce(peer: string, app_uuid: string, data: JsonValue): Promise<void> {
        const response = await this.requestOne({type: "Announce", peer, app_uuid, data});
        assert(response.type === 'Success');
    }

    async announcements(): Promise<AppAnnouncement[]> {
        const response = await this.requestOne({type: "AppAnnouncements"});
        assert(response.type === 'AppAnnouncements');
        return response.announcements;
    }

    async sendMessage(
        peer: string,
        app_uuid: string,
        from_app_uuid: string,
        message: Uint8Array
    ): Promise<void> {
        const response = await this.requestOne({
            type: "SendMessage", 
            peer, 
            app_uuid,
            from_app_uuid,
            message,
        });
        assert(response.type === 'Success');
        return;
    }

    async inboxMessages(app_uuid: string): Promise<Message[]> {
        const response = await this.requestOne({type: "InboxMessages", app_uuid});
        assert(response.type === 'InboxMessages');
        return response.messages;
    }

    async deleteInboxMessage(app_uuid: string, message_id: number): Promise<void> {
        const response = await this.requestOne({type: "DeleteInboxMessage", app_uuid, message_id});
        assert(response.type === 'Success');
        return;
    }
}
