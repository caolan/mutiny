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
    const local_peer_id = await fetch("/_api/v1/local_peer_id").then(res => res.text());
    document.getElementById('local-peer-id').textContent = 'You: ' + local_peer_id;
}

function renderPeer(peer) {
    const li = document.createElement('li');
    li.textContent = peer;
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
            ul.appendChild(renderPeer(invite.peer));
        }
        return ul;
    }
}

let prev_raw_invites = "";

async function updateInvites() {
   const res = await fetch("/_api/v1/message_invites");
   const raw = await res.text()
   // Update UI if invites have changed
   if (raw !== prev_raw_invites) {
       prev_raw_invites = raw;
       const invites = JSON.parse(raw);
       console.log(invites);
       const el = document.getElementById('peers');
       el.innerHTML = '';
       el.appendChild(renderInvites(invites));
   }
}

let prev_peers = new Set();

async function updatePeers() {
   const res = await fetch("/_api/v1/peers");
   const peers = new Set(await res.json());
   // Send invites to newly discovered peers
   const new_peers = peers.difference(prev_peers);
   for (const peer of new_peers) {
       await fetch("/_api/v1/message_invite", {
           method: 'POST',
           body: JSON.stringify({peer}),
       });
   }
   prev_peers = peers;
}

delegate(document.body, "click", "#peers li", function () {
    for (const li of document.querySelectorAll("#peers li")) {
        li.classList.remove("active");
    }
    this.classList.add("active");
});

// Initialize example app
updateLocalPeerId();
updatePeers();
updateInvites();

// Poll server for new peers
// setInterval(updatePeers, 2000);
// setInterval(updateInvites, 2000);
