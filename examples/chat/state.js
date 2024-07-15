// @ts-check
import {Signaller} from "./lib/signaller.js";

/** @typedef {{peer: string, app_uuid: string}} App */
/** @typedef {{peer: string, uuid: string, data: unknown}} AppAnnouncement */
/** @typedef {{message: string, from: App, to: App}} Message */

export const local_peer_id = new Signaller(/** @type {null | string} */(null));
export const local_app_uuid = new Signaller(/** @type {null | string} */(null));
export const selected_announcement = new Signaller(/** @type {null | App} */(null));
export const peers = new Signaller(new Set());
export const messages = new Signaller(/** @type {Message[]} */([]));
export const announcements = new Signaller(
    /** @type {import("../../lib/client.ts").AppAnnouncement[]} */
    ([])
);

/**
 * @param {App} from 
 * @param {App} to
 * @param {string} message
 */
export function appendMessage(from, to, message) {
    messages.value.push({from, to, message});
    messages.signal();
}

