import {watch} from "../lib/signaller.js";
import {delegate} from "../lib/events.js";
import {invites, selected_invite} from "../state.js";

export default class ChatPeers extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        this.shadow = this.attachShadow({mode: "open"});
        this.shadow.innerHTML = `
            <link rel="stylesheet" href="style.css">
            <div id="peers"></div>
        `;
        this.peers = this.shadow.getElementById('peers');
        this.cleanup = [
            watch([invites], () => this.updatePeers()),
            watch([selected_invite], () => this.updateSelected()),
            delegate(this.peers, "click", "#peers li", function () {
                selected_invite.value = JSON.parse(this.dataset.invite);
            }),
        ];
        this.updatePeers();
    }

    disconnectedCallback() {
        for (const destroy of this.cleanup) destroy();
    }

    updatePeers() {
        this.peers.innerHTML = '';
        if (invites.value.length === 0) {
            const span = document.createElement('span');
            span.textContent = "No peers discovered yet";
            this.peers.appendChild(span);
        } else {
            const ul = document.createElement('ul');
            for (const invite of invites.value) {
                const li = document.createElement('li');
                li.textContent = invite.peer;
                li.dataset.invite = JSON.stringify(invite);
                ul.appendChild(li);
            }
            this.peers.appendChild(ul);
        }
        this.updateSelected();
    }

    updateSelected() {
        const json = JSON.stringify(selected_invite.value);
        for (const li of this.peers.querySelectorAll("li")) {
            if (li.dataset.invite === json) {
                li.classList.add("active");
            } else {
                li.classList.remove("active");
            }
        }
    }
}

customElements.define("chat-peers", ChatPeers);
