// @ts-check
import {Signaller, watch} from "./signaller.js";
import {delegate} from "./events.js";

/** @typedef {{peer: string, uuid: string}} Invite */
/** @typedef {{peer: string, app_instance_uuid: string}} AppInstance */
/** @typedef {{message: string, from: AppInstance, to: AppInstance}} Message */

// State
const local_peer_id = new Signaller(/** @type {null | string} */(null));
const local_app_uuid = new Signaller(/** @type {null | string} */(null));
const selected_invite = new Signaller(/** @type {null | AppInstance} */(null));
const peers = new Signaller(new Set());
const messages = new Signaller(/** @type {Message[]} */([]));
const invites = new Signaller(/** @type {Invite[]} */([]));

async function updateLocalPeerId() {
    local_peer_id.value = await fetch("/_api/v1/local_peer_id").then(
        res => res.text()
    );
}

async function updateLocalAppInstance() {
    const data = await fetch("/_api/v1/application_instance").then(
        res => res.json()
    );
    local_app_uuid.value = data.uuid;
}

/** @param {{peer: string, uuid: string}} invite */
function renderInvite(invite) {
    const li = document.createElement('li');
    li.textContent = invite.peer;
    li.dataset.invite = JSON.stringify(invite);
    return li;
}

/** @param {{peer: string, uuid: string}[]} invites */
async function updatePeers() {
   const res = await fetch("/_api/v1/peers");
   const data = new Set(await res.json());
   // Send invites to newly discovered peers
   const new_peers = data.difference(peers.value);
   for (const peer of new_peers) {
       await fetch("/_api/v1/message_invite", {
           method: 'POST',
           body: JSON.stringify({peer}),
       });
   }
   peers.value = data;
}

async function updateInvites() {
   const res = await fetch("/_api/v1/message_invites");
   const data = /** @type {{peer: string, uuid: string}[]} */(await res.json());
   // Only list invites for peers in current discovered list
   const new_invites = data.filter(x => peers.value.has(x.peer));
   // Update state only if invites have changed
   if (JSON.stringify(new_invites) !== JSON.stringify(invites.value)) {
       invites.value = new_invites;
   }
}

/**
 * @param {AppInstance} from 
 * @param {AppInstance} to
 * @param {string} message
 */
function appendMessage(from, to, message) {
    messages.value.push({from, to, message});
    messages.signal();
}

async function getMessages() {
    while (true) {
        const res = await fetch("/_api/v1/message_read");
        const data = await res.json();
        if (data && local_peer_id.value && local_app_uuid.value) {
            const from = {
                peer: data.peer,
                app_instance_uuid: data.uuid,
            };
            const to = {
                peer: local_peer_id.value,
                app_instance_uuid: local_app_uuid.value,
            };
            appendMessage(from, to, data.message);
            await fetch("/_api/v1/message_next", {method: "POST"});
        } else {
            // Check again in 1 second
            setTimeout(getMessages, 1000);
            return;
        }
    }
}

const form = document.getElementById('send-message-form');
const input = form.querySelector('input[type=text]');

form.addEventListener('submit', /** @param {SubmitEvent} ev */async ev => {
    ev.preventDefault();
    if (selected_invite.value && local_peer_id.value && local_app_uuid.value) {
        const message = input.value;
        await fetch("/_api/v1/message_send", {
            method: "POST",
            body: JSON.stringify({
                peer: selected_invite.value.peer,
                app_instance_uuid: selected_invite.value.app_instance_uuid,
                message,
            })
        });
        input.value = "";
        const from = {
            peer: local_peer_id.value,
            app_instance_uuid: local_app_uuid.value,
        };
        const to = {
            peer: selected_invite.value.peer,
            app_instance_uuid: selected_invite.value.app_instance_uuid,
        };
        appendMessage(from, to, message);
    }
});

delegate(document.body, "click", "#peers li", /** @this {HTMLLIElement} */function () {
    selected_invite.value = JSON.parse(this.dataset.invite);
});

function renderLocalPeerId() {
    const el = document.getElementById('local-peer-id');
    el.textContent = `You: ${local_peer_id.value}`;
}

function renderSelectedInvite() {
    const json = JSON.stringify(selected_invite.value);
    for (const li of document.querySelectorAll("#peers li")) {
        if (li.dataset.invite === json) {
            li.classList.add("active");
        } else {
            li.classList.remove("active");
        }
    }
    if (selected_invite.value) {
        form.style.display = 'flex';
        input.focus();
    } else {
        form.style.display = 'none';
    }
}

function renderMessages() {
    const el = document.getElementById('message-history');
    let txt = "";
    if (selected_invite.value) {
        for (const msg of messages.value) {
            const match_from = (
                msg.from.peer === selected_invite.value.peer &&
                msg.from.app_instance_uuid === selected_invite.value.app_instance_uuid
            );
            const match_to = (
                msg.to.peer === selected_invite.value.peer &&
                msg.to.app_instance_uuid === selected_invite.value.app_instance_uuid
            );
            if (match_from || match_to) {
                const from = msg.from.peer === local_peer_id.value ? 'You' : msg.from.peer;
                txt += `<${from}>: ${msg.message}\n`;
            }
        }
    }
    el.textContent = txt;
}

function renderInvites() {
    const el = document.getElementById('peers');
    el.innerHTML = '';
    if (invites.value.length === 0) {
        const span = document.createElement('span');
        span.textContent = "No peers discovered yet";
        el.appendChild(span);
    } else {
        const ul = document.createElement('ul');
        for (const invite of invites.value) {
            ul.appendChild(renderInvite(invite));
        }
        el.appendChild(ul);
    }
}

// Update UI when state changes
watch([selected_invite], renderSelectedInvite);
watch([local_peer_id], renderLocalPeerId);
watch([local_peer_id, selected_invite, messages], renderMessages);
watch([invites], renderInvites);

// Initialize example app
renderInvites();
await updateLocalPeerId();
await updateLocalAppInstance();
await updatePeers();
await updateInvites();

// Start polling for messages
getMessages();

// Poll server for new peers and invites
setInterval(updatePeers, 2000);
setInterval(updateInvites, 2000);
