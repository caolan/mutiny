import {watch} from "../lib/signaller.js";
import {delegate} from "../lib/events.js";
import {peers, announcements, selected_announcement} from "../state.js";

export default class ChatPeers extends HTMLElement {
    constructor() {
        super();
    }

    connectedCallback() {
        this.cleanup = [
            watch([peers, announcements], () => this.updatePeers()),
            watch([selected_announcement], () => this.updateSelected()),
            delegate(this, "click", "li", function () {
                selected_announcement.value = JSON.parse(this.dataset.announcement);
            }),
            delegate(this, "click", "button", ev => {
                ev.preventDefault();
                this.dial();
            }),
        ];
        this.updatePeers();
    }

    async dial() {
        const address = prompt("Dial address (e.g. /ip4/127.0.0.1/tcp/33985):");
        if (address) {
            await fetch("/_api/v1/dial", {
                method: 'POST',
                body: JSON.stringify({address}),
            });
        }
    }

    disconnectedCallback() {
        for (const destroy of this.cleanup) destroy();
    }

    updatePeers() {
        this.innerHTML = '';
        if (announcements.value.length === 0) {
            const span = document.createElement('span');
            span.textContent = "No peers discovered yet";
            this.appendChild(span);
        } else {
            const ul = document.createElement('ul');
            for (const announcement of announcements.value) {
                // Only show app announcements from known peers
                if (peers.value.has(announcement.peer)) {
                    const li = document.createElement('li');
                    li.textContent = announcement.data.nick || announcement.peer;
                    li.dataset.announcement = JSON.stringify(announcement);
                    ul.appendChild(li);
                }
            }
            this.appendChild(ul);
        }
        const btn = document.createElement('button');
        btn.textContent = 'Add peer';
        this.appendChild(btn);
        this.updateSelected();
    }

    updateSelected() {
        const json = JSON.stringify(selected_announcement.value);
        for (const li of this.querySelectorAll("li")) {
            if (li.dataset.announcement === json) {
                li.classList.add("active");
            } else {
                li.classList.remove("active");
            }
        }
    }
}

customElements.define("chat-peers", ChatPeers);
