import { join } from "@std/path/join";
import { assertEquals } from "@std/assert";
import { MutinyClient } from "../src/client.ts";
import { Server }from "../src/server.ts";

const BASE_URL = "http://localhost:8000";
const ROOT = join(import.meta.dirname as string, "www");

const APP = {
    uuid: "1234567890",
    label: "Example app",
};

function makeServer(client: unknown) {
    return new Server(client as MutinyClient, APP, ROOT);
}

Deno.test("Get app instance information", async () => {
    const server = makeServer({});
    const request = new Request(`${BASE_URL}/_api/v1/application`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, APP);
});

Deno.test("Get local peer id", async () => {
    const server = makeServer({
        localPeerId() {
            return Promise.resolve("123abc");
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/local_peer_id`);
    const response = await server.handleRequest(request);
    const data = await response.text();
    assertEquals(data, "123abc");
});

Deno.test("Get peers list", async () => {
    const peers = ["peer1", "peer2", "peer3"];
    const server = makeServer({
        peers() {
            return Promise.resolve(peers);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/peers`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, peers);
});

Deno.test("Send app announcement", async () => {
    const calls: [string, string, unknown][] = [];
    const server = makeServer({
        announce(peer: string, uuid: string, data: unknown) {
            calls.push([peer, uuid, data]);
            return Promise.resolve(undefined);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/announcements/outbox`, {
        method: "POST",
        body: JSON.stringify({
            peer: "peer2", 
            app_uuid: "app2",
            data: "example data",
        }),
    });
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, {success: true});
    assertEquals(calls, [["peer2", APP.uuid, "example data"]]);
});

Deno.test("List app announcements", async () => {
    const announcements = [
        {
            peer: "peer1",
            app_uuid: "app1",
            data: {
                id: "example.app.one",
                version: "1.1.1",
            }
        },
        {
            peer: "peer2",
            app_uuid: "app2",
            data: {
                id: "example.app.two",
                version: "2.2.2",
            }
        },
    ];
    const server = makeServer({
        announcements() {
            return Promise.resolve(announcements);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/announcements/inbox`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, announcements);
});

Deno.test("Read inbox (with message)", async () => {
    const message = {
        peer: "peer2",
        uuid: "app2",
        message: new TextEncoder().encode("hello"),
    };
    const server = makeServer({
        inboxMessages(uuid: string) {
            assertEquals(uuid, APP.uuid);
            return Promise.resolve([message]);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/messages/inbox`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, [{
        peer: "peer2",
        uuid: "app2",
        message: "hello",
    }]);
});

Deno.test("Read inbox (with no message)", async () => {
    const server = makeServer({
        inboxMessages(uuid: string) {
            assertEquals(uuid, APP.uuid);
            return Promise.resolve([]);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/messages/inbox`);
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, []);
});

Deno.test("Delete inbox message", async () => {
    const calls: [string, number][] = [];
    const server = makeServer({
        deleteInboxMessage(uuid: string, message_id: number) {
            calls.push([uuid, message_id]);
            return Promise.resolve(null);
        },
    });
    const request = new Request(`${BASE_URL}/_api/v1/messages/inbox`, {
        method: "DELETE",
        body: JSON.stringify({
            message_id: 123,
        })
    });
    const response = await server.handleRequest(request);
    const data = await response.json();
    assertEquals(data, {success: true});
    assertEquals(calls, [[APP.uuid, 123]]);
});

Deno.test("Unknown API path", async () => {
    const server = makeServer({});
    const request = new Request(`${BASE_URL}/_api/v1/not_found`);
    const response = await server.handleRequest(request);
    assertEquals(response.status, 404);
});

Deno.test("Get static file from app root directory", async () => {
    const server = makeServer({});
    const request = new Request(`${BASE_URL}/example.txt`);
    const response = await server.handleRequest(request);
    assertEquals(response.status, 200);
    const data = await response.text();
    assertEquals(data, "Hello, world!\n");
});
