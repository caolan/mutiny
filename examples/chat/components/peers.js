import {watch} from "../lib/signaller.js";
import {delegate} from "../lib/events.js";
import {announcements, selected_announcement} from "../state.js";

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
            watch([announcements], () => this.updatePeers()),
            watch([selected_announcement], () => this.updateSelected()),
            delegate(this.peers, "click", "#peers li", function () {
                selected_announcement.value = JSON.parse(this.dataset.announcement);
            }),
        ];
        this.updatePeers();
    }

    disconnectedCallback() {
        for (const destroy of this.cleanup) destroy();
    }

    updatePeers() {
        this.peers.innerHTML = '';
        if (announcements.value.length === 0) {
            const span = document.createElement('span');
            span.textContent = "No peers discovered yet";
            this.peers.appendChild(span);
        } else {
            const ul = document.createElement('ul');
            for (const announcement  of announcements.value) {
                const li = document.createElement('li');
                li.textContent = announcement.peer;
                li.dataset.announcement = JSON.stringify(announcement);
                ul.appendChild(li);
            }
            this.peers.appendChild(ul);
        }
        this.updateSelected();
    }

    updateSelected() {
        const json = JSON.stringify(selected_announcement.value);
        for (const li of this.peers.querySelectorAll("li")) {
            if (li.dataset.announcement === json) {
                li.classList.add("active");
            } else {
                li.classList.remove("active");
            }
        }
    }
}

customElements.define("chat-peers", ChatPeers);
