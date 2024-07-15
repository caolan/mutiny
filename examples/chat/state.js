// @ts-check
import {Signaller} from "./lib/signaller.js";

/** @typedef {{peer: string, uuid: string}} Invite */
/** @typedef {{peer: string, app_uuid: string}} AppInstance */
/** @typedef {{message: string, from: AppInstance, to: AppInstance}} Message */

export const local_peer_id = new Signaller(/** @type {null | string} */(null));
export const local_app_uuid = new Signaller(/** @type {null | string} */(null));
export const selected_invite = new Signaller(/** @type {null | AppInstance} */(null));
export const peers = new Signaller(new Set());
export const messages = new Signaller(/** @type {Message[]} */([]));
export const invites = new Signaller(/** @type {Invite[]} */([]));

/**
 * @param {AppInstance} from 
 * @param {AppInstance} to
 * @param {string} message
 */
export function appendMessage(from, to, message) {
    messages.value.push({from, to, message});
    messages.signal();
}

