// State
let local_peer_id = null;
let peers = new Set();
let prev_invites = "";
let selected_invite = null;
const messages = [];


function delegate(node, event_name, selector, listener, options) {
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

async function updateLocalPeerId() {
    local_peer_id = await fetch("/_api/v1/local_peer_id").then(res => res.text());
    document.getElementById('local-peer-id').textContent = 'You: ' + local_peer_id;
    renderMessages();
}

function renderInvite(invite) {
    const li = document.createElement('li');
    li.textContent = invite.peer;
    li.dataset.invite = JSON.stringify(invite);
    return li;
}

function renderInvites(invites) {
    if (invites.length === 0) {
        const span = document.createElement('span');
        span.textContent = "No peers discovered yet";
        return span;
    } else {
        const ul = document.createElement('ul');
        for (const invite of invites) {
            ul.appendChild(renderInvite(invite));
        }
        return ul;
    }
}

async function updatePeers() {
   const res = await fetch("/_api/v1/peers");
   const data = new Set(await res.json());
   // Send invites to newly discovered peers
   const new_peers = data.difference(peers);
   for (const peer of new_peers) {
       await fetch("/_api/v1/message_invite", {
           method: 'POST',
           body: JSON.stringify({peer}),
       });
   }
   peers = data;
}

async function updateInvites() {
   const res = await fetch("/_api/v1/message_invites");
   const data = await res.json()
   // Only list invites for peers in current discovered list
   const invites = data.filter(x => peers.has(x.peer));
   const json = JSON.stringify(invites);
   // Update UI if invites have changed
   if (json !== prev_invites) {
       prev_invites = json;
       console.log(invites);
       const el = document.getElementById('peers');
       el.innerHTML = '';
       el.appendChild(renderInvites(invites));
   }
}

function renderMessages() {
    const el = document.getElementById('message-history');
    let txt = "";
    if (selected_invite) {
        for (const msg of messages) {
            if (msg.from === selected_invite.peer || msg.to === selected_invite.peer) {
                const from = msg.from === local_peer_id ? 'You' : msg.from;
                txt += `<${from}>: ${msg.message}\n`;
            }
        }
    }
    el.textContent = txt;
}

async function getMessages() {
    while (true) {
        const res = await fetch("/_api/v1/message_read");
        const data = await res.json();
        if (data.message) {
            messages.push({
                from: data.message.peer,
                to: local_peer_id,
                message: data.message.message,
            });
            await fetch("/_api/v1/message_next", {method: "POST"});
        } else {
            // Check again in 1 second
            setTimeout(getMessages, 1000);
            return;
        }
    }
}

function appendMessage(from, to, message) {
    messages.push({from, to, message});
    renderMessages();
}

const form = document.getElementById('send-message-form');
const input = form.querySelector('input[type=text]');

form.addEventListener('submit', async ev => {
    ev.preventDefault();
    if (selected_invite) {
        const message = input.value;
        await fetch("/_api/v1/message_send", {
            method: "POST",
            body: JSON.stringify({
                peer: selected_invite.peer,
                app_instance_uuid: selected_invite.app_instance_uuid,
                message,
            })
        });
        input.value = "";
        appendMessage(local_peer_id, selected_invite.peer, message);
    }
});

delegate(document.body, "click", "#peers li", function () {
    for (const li of document.querySelectorAll("#peers li")) {
        li.classList.remove("active");
    }
    this.classList.add("active");
    selected_invite = JSON.parse(this.dataset.invite);
    form.style.display = 'flex';
    input.focus();
});


// Initialize example app
await updateLocalPeerId();
await updatePeers();
await updateInvites();

// Start polling for messages
getMessages();

// Poll server for new peers
// setInterval(updatePeers, 2000);
// setInterval(updateInvites, 2000);
